//! Frame clear and sky cyclorama backdrop submission.

use super::*;

/// Stamp a GP0(02h) fill-rectangle into `scratch.clear_packet` and
/// link it into the back-most OT slot so it runs first when DMA
/// walks the chain -- which is the same pattern PS1 software uses to
/// "clear" the framebuffer at the start of every frame, since the
/// HwRenderer (faithfully) preserves VRAM across frames the way real
/// hardware does.
pub(crate) fn push_clear(scratch: &mut PreviewScratch, color: [u8; 3]) {
    // PSX VRAM coords; the editor's HwRenderer renders the same
    // 320×240 sub-rect that the runtime frame-buffer would land in.
    let color_word =
        0x0200_0000_u32 | color[0] as u32 | ((color[1] as u32) << 8) | ((color[2] as u32) << 16);
    let xy_word = 0u32; // top-left at (0, 0)
    let wh_word = ((240u32) << 16) | 320u32; // pack_xy(320, 240)
                                             // word[0] is rewritten by `OtFrame::add_raw` with the
                                             // chain tag -- leave it at 0 here.
    scratch.clear_packet[1] = color_word;
    scratch.clear_packet[2] = xy_word;
    scratch.clear_packet[3] = wh_word;
    let ptr: *mut u32 = scratch.clear_packet.as_mut_ptr();
    unsafe {
        scratch
            .ot
            .resume_frame()
            .add_raw(PREVIEW_CLEAR_SLOT, ptr, 3);
    }
}

pub(crate) fn push_cyclorama(
    scratch: &mut PreviewScratch,
    sky: psxed_project::ResolvedSkySettings,
    camera: psx_engine::WorldCamera,
) {
    if !sky.enabled {
        return;
    }
    for quad in psxed_project::generate_sky_cyclorama(sky).into_iter().rev() {
        let Some(projected) = preview_project_cyclorama_quad(quad.direction_q12, camera) else {
            continue;
        };
        if preview_cyclorama_quad_outside_screen(projected) {
            continue;
        }
        push_sky_quad_corners(scratch, projected, quad.rgb);
    }
}

pub(crate) fn push_projected_scene_sky(
    scratch: &mut PreviewScratch,
    mode: SkyMode,
    view_rotation: Mat3I16,
    material_tick: u32,
    textures: &EditorTextures,
) {
    let Some(slot) = textures.scene_sky_slot().filter(|slot| slot.mode == mode) else {
        return;
    };
    // Only the layered sky scrolls with the clock; the cube is still.
    if mode == SkyMode::QuakeLayered {
        scratch.clock_used = true;
    }
    let word_capacity = match mode {
        SkyMode::QuakeLayered => psx_engine::sky::VIEW_RAY_SKY_PACKET_WORDS,
        SkyMode::Cube => psx_engine::sky::VIEW_RAY_CUBE_SKY_PACKET_WORDS,
        SkyMode::Off | SkyMode::Panorama => return,
    };
    let stream = {
        let mut arena = PrimitivePacketArena::new(&mut scratch.projected_sky_packets);
        let Some(mut reservation) = arena.reserve_packet_words(word_capacity) else {
            return;
        };
        let first = reservation.words_mut().as_mut_ptr();
        let submitted = unsafe {
            match mode {
                SkyMode::QuakeLayered => psx_engine::sky::submit_view_ray_layered_sky_to_slot(
                    slot.tpage_word,
                    slot.clut_word,
                    [0, 0],
                    [128, 128],
                    view_rotation,
                    [SCREEN_W as i16, SCREEN_H as i16],
                    [SCREEN_CX as i16, SCREEN_CY as i16],
                    PROJ_H as i16,
                    material_tick,
                    PREVIEW_SKY_SLOT as u16,
                    first,
                ),
                SkyMode::Cube => psx_engine::sky::submit_view_ray_cube_sky_to_slot(
                    slot.tpage_word,
                    slot.clut_word,
                    view_rotation,
                    [SCREEN_W as i16, SCREEN_H as i16],
                    [SCREEN_CX as i16, SCREEN_CY as i16],
                    PROJ_H as i16,
                    PREVIEW_SKY_SLOT as u16,
                    first,
                ),
                SkyMode::Off | SkyMode::Panorama => unreachable!(),
            }
        };
        let words = unsafe { submitted.next_packet.offset_from(first) }.max(0) as usize;
        let Some(stream) = reservation.commit(words, submitted.packets as usize) else {
            return;
        };
        stream
    };
    // SAFETY: the preview table and every packet it links live in `scratch`
    // for the whole preview frame; the host walks them with `iter_packets`,
    // never by DMA. `stream` is a committed range of that scratch.
    unsafe {
        let mut ot = psx_engine::OtFrame::resume(&mut scratch.ot);
        ot.add_committed_tagged_packet_stream_unchecked(stream);
    }
}

pub(crate) fn preview_project_cyclorama_quad(
    dirs: [[i16; 3]; 4],
    camera: psx_engine::WorldCamera,
) -> Option<[(i16, i16); 4]> {
    Some([
        preview_project_cyclorama_direction(dirs[0], camera)?,
        preview_project_cyclorama_direction(dirs[1], camera)?,
        preview_project_cyclorama_direction(dirs[2], camera)?,
        preview_project_cyclorama_direction(dirs[3], camera)?,
    ])
}

pub(crate) fn preview_project_cyclorama_direction(
    dir: [i16; 3],
    camera: psx_engine::WorldCamera,
) -> Option<(i16, i16)> {
    let x = i32::from(dir[0]);
    let y = i32::from(dir[1]);
    let z = i32::from(dir[2]);
    let sin_yaw = camera.sin_yaw.raw();
    let cos_yaw = camera.cos_yaw.raw();
    let sin_pitch = camera.sin_pitch.raw();
    let cos_pitch = camera.cos_pitch.raw();
    let x1 = mul_q12_i32(x, cos_yaw) - mul_q12_i32(z, sin_yaw);
    let z1 = -mul_q12_i32(x, sin_yaw) - mul_q12_i32(z, cos_yaw);
    let y2 = mul_q12_i32(y, cos_pitch) - mul_q12_i32(z1, sin_pitch);
    let z2 = mul_q12_i32(y, sin_pitch) + mul_q12_i32(z1, cos_pitch);
    if z2 <= NEAR_Z {
        return None;
    }
    let sx = SCREEN_CX + (x1 * PROJ_H) / z2;
    let sy = SCREEN_CY - (y2 * PROJ_H) / z2;
    Some((sx.clamp(-512, 831) as i16, sy.clamp(-256, 495) as i16))
}

pub(crate) fn preview_cyclorama_quad_outside_screen(points: [(i16, i16); 4]) -> bool {
    let min_x = points.iter().map(|p| p.0).min().unwrap_or(0);
    let max_x = points.iter().map(|p| p.0).max().unwrap_or(0);
    let min_y = points.iter().map(|p| p.1).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.1).max().unwrap_or(0);
    max_x < 0 || min_x >= SCREEN_W as i16 || max_y < 0 || min_y >= SCREEN_H as i16
}

pub(crate) fn push_sky_quad_corners(
    scratch: &mut PreviewScratch,
    points: [(i16, i16); 4],
    colors: [[u8; 3]; 4],
) {
    if scratch.sky_used >= scratch.sky_quads.len() {
        return;
    }
    let quad = QuadGouraud::new(
        points,
        [
            (colors[0][0], colors[0][1], colors[0][2]),
            (colors[1][0], colors[1][1], colors[1][2]),
            (colors[2][0], colors[2][1], colors[2][2]),
            (colors[3][0], colors[3][1], colors[3][2]),
        ],
    );
    let idx = scratch.sky_used;
    scratch.sky_quads[idx] = quad;
    scratch.sky_used += 1;
    let ptr: *mut QuadGouraud = &mut scratch.sky_quads[idx];
    unsafe {
        scratch
            .ot
            .resume_frame()
            .add_raw(PREVIEW_SKY_SLOT, ptr.cast::<u32>(), QuadGouraud::WORDS);
    }
}
