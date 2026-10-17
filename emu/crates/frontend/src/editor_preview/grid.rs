//! Sector-grid world walker for the editor preview.
//!
//! A grid scene renders the faces `psxed_project::grid::faces` derives from
//! the document. Each
//! visible sector is expanded to polygons, lit per corner from the scene's
//! point lights and the room ambient, and submitted through the same
//! polygon path (`emit_uv_polygon`), so clipping, depth slots, texture
//! windows and sidedness are shared with every other preview surface.
//!
//! Cells are culled by their bounding box before any face is built, so a
//! close view of a large level only pays for what is on screen.

use super::*;

use psxed_project::grid::faces::{cell_faces_with, GridFace};
use psxed_project::grid::light::{bake_face_corners, FacingShade};
use psxed_project::grid::mesh_faces::{mesh_faces_with, placed as placed_mesh};
use psxed_project::grid::plane_faces::plane_faces_with;
use psxed_project::grid::uv::ProjectUvDefaults;
use psxed_project::grid::uv_split;
use psxed_project::grid::{GridRoom, GridSector, GridWorld, SECTOR_UNITS};

/// Editor-space bounds of one sector, from its lowest to highest geometry.
fn cell_bounds(
    world: &GridWorld,
    room: &GridRoom,
    x: usize,
    z: usize,
    sector: &GridSector,
) -> Option<PreviewCellBounds> {
    let [cx, cz] = room.world_cell(x, z);
    let (mut low, mut high) = sector.height_range()?;
    // Derived walls hosted here can reach past the sector's own surfaces
    // (a riser runs down to the neighbour's floor).
    if room.wall_mode == psxed_project::grid::WallMode::Derived {
        if let Some((lo, hi)) = psxed_project::grid::derive::derived_range(room, [cx, cz]) {
            low = low.min(lo);
            high = high.max(hi);
        }
    }
    let scale = world.scale;
    let y0 = scale.to_editor(room.y_offset + low);
    let y1 = scale.to_editor(room.y_offset + high);
    Some(PreviewCellBounds {
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
/// world past it is clipped by the same guard the runtime uses.
const PREVIEW_COORD_LIMIT: i32 = 500_000;

fn emit_grid_face(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    face: &GridFace,
    ambient: [u8; 3],
    lights: &[psxed_project::grid::light::GridLight],
    facing: FacingShade,
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
        fallback_face_color(face.cell[0].unsigned_abs() as usize),
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
    let colors: Vec<(u8, u8, u8)> = bake_face_corners(face, tint, ambient, lights, facing)
        .into_iter()
        .map(|c| (c[0], c[1], c[2]))
        .collect();
    let vertices: Vec<[f64; 3]> = face.points.iter().map(|p| p.map(f64::from)).collect();
    let mut texels = face.texels.clone();
    if let FaceShade::Textured { slot, .. } = shade {
        psxed_project::face_uv::rebase_texel_uvs(
            &mut texels,
            [
                f64::from(slot.texture_width.max(8)),
                f64::from(slot.texture_height.max(8)),
            ],
        );
    }
    emit_uv_polygon(scratch, camera, shade, &vertices, &texels, Some(&colors));
}

/// Emit `face`, cut at the texture tile boundaries the way the cook cuts
/// it, so the preview shows what a Play build draws.
#[allow(clippy::too_many_arguments)]
fn emit_cut_face(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    scale: psxed_project::grid::GridScale,
    face: &GridFace,
    ambient: [u8; 3],
    lights: &[psxed_project::grid::light::GridLight],
    facing: FacingShade,
    scratch: &mut PreviewScratch,
) {
    // The preview draws through the same texture window a Play build uses,
    // so it cuts at the same four-tile pieces.
    if uv_split::wants_split(face, true) {
        for piece in uv_split::split(face, scale, true) {
            emit_grid_face(
                project, textures, camera, &piece, ambient, lights, facing, scratch,
            );
        }
    } else {
        emit_grid_face(project, textures, camera, face, ambient, lights, facing, scratch);
    }
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
    let lights = collect_grid_bake_lights(project);
    let defaults = ProjectUvDefaults(project);
    // Rooms the author hid in the Rooms panel; a view setting the workspace
    // publishes, since the preview only sees the project.
    let hidden = psxed_ui::grid_preview_hidden_rooms();
    let mut faces = Vec::new();
    for (room_index, room) in world.rooms.iter().enumerate() {
        if hidden.contains(&room_index) {
            continue;
        }
        for (x, z, sector) in room.iter() {
            let Some(bounds) = cell_bounds(world, room, x, z, sector) else {
                continue;
            };
            if !preview_cell_bounds_visible(camera, bounds) {
                continue;
            }
            if scratch.geometry_full() {
                // A visible sector that no longer fits: count it, keep
                // counting so the report says how much is missing.
                scratch.dropped = scratch.dropped.saturating_add(1);
                continue;
            }
            faces.clear();
            cell_faces_with(world, &defaults, room_index, x, z, &mut faces);
            for face in &faces {
                emit_cut_face(
                    project,
                    textures,
                    camera,
                    world.scale,
                    face,
                    room.ambient,
                    &lights,
                    world.shading,
                    scratch,
                );
            }
        }
        // Free planes, culled by their own boxes.
        for (index, plane) in room.planes.iter().enumerate() {
            if !plane.enabled
                || !preview_cell_bounds_visible(camera, plane_bounds(world, room, plane))
            {
                continue;
            }
            if scratch.geometry_full() {
                scratch.dropped = scratch.dropped.saturating_add(1);
                continue;
            }
            faces.clear();
            plane_faces_with(world, &defaults, room_index, index, &mut faces);
            for face in &faces {
                emit_cut_face(
                    project,
                    textures,
                    camera,
                    world.scale,
                    face,
                    room.ambient,
                    &lights,
                    world.shading,
                    scratch,
                );
            }
        }
        // Placed static meshes, culled by their own boxes, drawn with the
        // faces (and so the bake) the cook uses.
        for index in 0..room.meshes.len() {
            let Some(placed) = placed_mesh(world, room_index, index) else {
                continue;
            };
            let Some((lo, hi)) = placed.mesh.bounds(&placed.built) else {
                continue;
            };
            let scale = world.scale;
            let y = |v: i32| f64::from(scale.to_editor(room.y_offset + v));
            let bounds = PreviewCellBounds {
                min: [
                    f64::from(scale.to_editor(lo[0])),
                    y(lo[1]),
                    f64::from(scale.to_editor(lo[2])),
                ],
                max: [
                    f64::from(scale.to_editor(hi[0])),
                    y(hi[1]),
                    f64::from(scale.to_editor(hi[2])),
                ],
            };
            if !preview_cell_bounds_visible(camera, bounds) {
                continue;
            }
            if scratch.geometry_full() {
                scratch.dropped = scratch.dropped.saturating_add(1);
                continue;
            }
            faces.clear();
            mesh_faces_with(world, &defaults, room_index, index, &mut faces);
            for face in &faces {
                emit_cut_face(
                    project,
                    textures,
                    camera,
                    world.scale,
                    face,
                    room.ambient,
                    &lights,
                    world.shading,
                    scratch,
                );
            }
        }
    }
}

/// Editor-space box around a free plane, for the same cull the cells get.
fn plane_bounds(
    world: &GridWorld,
    room: &GridRoom,
    plane: &psxed_project::grid::GridPlane,
) -> PreviewCellBounds {
    let (lo, hi) = plane.bounds();
    let scale = world.scale;
    let y = |v: i32| f64::from(scale.to_editor(room.y_offset + v));
    PreviewCellBounds {
        min: [
            f64::from(scale.to_editor(lo[0])),
            y(lo[1]),
            f64::from(scale.to_editor(lo[2])),
        ],
        max: [
            f64::from(scale.to_editor(hi[0])),
            y(hi[1]),
            f64::from(scale.to_editor(hi[2])),
        ],
    }
}

/// Editor-space bounding box of the whole grid world, for the depth range.
pub(super) fn grid_world_bounds(project: &ProjectDocument) -> Option<([f64; 3], [f64; 3])> {
    let bounds = project.active_scene().world.grid()?.editor_bounds()?;
    Some((bounds.min.map(f64::from), bounds.max.map(f64::from)))
}
