//! Sector-grid world walker for the editor preview.
//!
//! Brush scenes render through the BSP CSG surfaces; a grid scene renders
//! the faces `psxed_project::grid::faces` derives from the document. Each
//! visible sector is expanded to polygons, lit per corner from the scene's
//! point lights and the room ambient, and submitted through the same
//! polygon path (`emit_uv_polygon`) the brushes use, so clipping, depth
//! slots, texture windows and sidedness behave identically.
//!
//! Cells are culled by their bounding box before any face is built, so a
//! close view of a large level only pays for what is on screen.

use super::*;

use psxed_project::grid::faces::{cell_faces, GridFace};
use psxed_project::grid::light::bake_face_corners;
use psxed_project::grid::{GridRoom, GridSector, GridWorld, SECTOR_UNITS};

/// Editor-space bounds of one sector, from its lowest to highest geometry.
fn cell_bounds(
    world: &GridWorld,
    room: &GridRoom,
    x: usize,
    z: usize,
    sector: &GridSector,
) -> Option<PreviewBrushBounds> {
    let (low, high) = sector.height_range()?;
    let [cx, cz] = room.world_cell(x, z);
    let scale = world.scale;
    let y0 = scale.to_editor(room.y_offset + low);
    let y1 = scale.to_editor(room.y_offset + high);
    Some(PreviewBrushBounds {
        min: [
            f64::from(scale.to_editor(cx * SECTOR_UNITS)),
            f64::from(y0),
            f64::from(scale.to_editor(cz * SECTOR_UNITS)),
        ],
        max: [
            f64::from(scale.to_editor((cx + 1) * SECTOR_UNITS)),
            f64::from(y1),
            f64::from(scale.to_editor((cz + 1) * SECTOR_UNITS)),
        ],
    })
}

/// Widest span the preview camera's fixed-point rotate handles safely; a
/// world past it is clipped by the same guard the brush path uses.
const PREVIEW_COORD_LIMIT: i32 = 500_000;

fn emit_grid_face(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    face: &GridFace,
    ambient: [u8; 3],
    lights: &[psxed_project::brush_light::BrushPointLight],
    scratch: &mut PreviewScratch,
) {
    if face.points.len() > PREVIEW_FACE_VERTEX_CAP
        || face
            .points
            .iter()
            .any(|p| p.iter().any(|c| c.abs() > PREVIEW_COORD_LIMIT))
    {
        return;
    }
    let shade = face_shade(
        project,
        face.material,
        brush_fallback_color(face.cell[0].unsigned_abs() as usize),
        textures,
    );
    let sidedness = if face.double_sided {
        psxed_project::MaterialFaceSidedness::Both
    } else {
        psxed_project::MaterialFaceSidedness::Front
    };
    let shade = shade.with_sidedness(sidedness);
    let tint = match shade {
        FaceShade::Flat { rgb, .. } => [rgb.0, rgb.1, rgb.2],
        FaceShade::Textured { tint, .. } => [tint.0, tint.1, tint.2],
    };
    let colors: Vec<(u8, u8, u8)> = bake_face_corners(face, tint, ambient, lights)
        .into_iter()
        .map(|c| (c[0], c[1], c[2]))
        .collect();
    let vertices: Vec<[f64; 3]> = face.points.iter().map(|p| p.map(f64::from)).collect();
    let mut texels = face.texels.clone();
    if let FaceShade::Textured { slot, .. } = shade {
        psxed_project::brush::rebase_texel_uvs(
            &mut texels,
            [
                f64::from(slot.texture_width.max(8)),
                f64::from(slot.texture_height.max(8)),
            ],
        );
    }
    emit_uv_polygon(scratch, camera, shade, &vertices, &texels, Some(&colors));
}

/// Draw the active scene's grid world, if it has one.
pub(super) fn walk_grid(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    scratch: &mut PreviewScratch,
) {
    let Some(world) = project.active_scene().world.grid() else {
        return;
    };
    let lights = collect_bsp_preview_bake_lights(project);
    let mut faces = Vec::new();
    for (room_index, room) in world.rooms.iter().enumerate() {
        for (x, z, sector) in room.iter() {
            if scratch.geometry_full() {
                return;
            }
            let Some(bounds) = cell_bounds(world, room, x, z, sector) else {
                continue;
            };
            if !preview_brush_bounds_visible(camera, bounds) {
                continue;
            }
            faces.clear();
            cell_faces(world, room_index, x, z, &mut faces);
            for face in &faces {
                emit_grid_face(
                    project,
                    textures,
                    camera,
                    face,
                    room.ambient,
                    &lights,
                    scratch,
                );
            }
        }
    }
}

/// Editor-space bounding box of the whole grid world, for the depth range.
pub(super) fn grid_world_bounds(project: &ProjectDocument) -> Option<([f64; 3], [f64; 3])> {
    let bounds = project.active_scene().world.grid()?.editor_bounds()?;
    Some((bounds.min.map(f64::from), bounds.max.map(f64::from)))
}
