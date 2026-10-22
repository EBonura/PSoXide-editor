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

use super::grid_cache::{
    BoxSource, GridGeometry, LazyBox, PreparedFace, PreparedPiece, PreparedRoom, RoomsToDraw,
};
use psxed_project::grid::faces::{cell_faces_with, GridFace};
use psxed_project::grid::lighting::{BakeRig, Gather, GiField, LightScene};
use psxed_project::grid::mesh_faces::{mesh_faces_with, placed as placed_mesh};
use psxed_project::grid::plane_faces::plane_faces_with;
use psxed_project::grid::uv::ProjectUvDefaults;
use psxed_project::grid::uv_split;
use psxed_project::grid::{GridRoom, GridWorld, SECTOR_UNITS};
use psxed_ui::GridPreviewView;

/// Editor-space bounds of one sector, from its lowest to highest geometry.
fn cell_bounds(
    world: &GridWorld,
    room_index: usize,
    room: &GridRoom,
    x: usize,
    z: usize,
) -> Option<PreviewCellBounds> {
    let [cx, cz] = room.world_cell(x, z);
    // Every leaf that touches the sector: a sector, finer cells, or the part
    // of a bigger cell over it.
    let mut range: Option<(i32, i32)> = None;
    for (_, leaf) in room.leaves.in_sector([cx, cz]) {
        if let Some((lo, hi)) = leaf.height_range() {
            range = Some(range.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    let (mut low, mut high) = range?;
    // Derived walls hosted here can reach past the sector's own surfaces
    // (a riser runs down to the neighbour's floor).
    if room.wall_mode == psxed_project::grid::WallMode::Derived {
        let derived = if room.leaves.is_sectors_only() {
            psxed_project::grid::derive::derived_range(room, [cx, cz])
        } else {
            psxed_project::grid::leaf_derive::derived_range_in(world, room_index, [cx, cz])
        };
        if let Some((lo, hi)) = derived {
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

/// Prepare one face (already cut at the tile boundaries the way the cook cuts
/// it) for emission: the shade, the baked corner colours and the texels
/// rebased into the material's texture, none of which depend on the camera.
fn prepare_piece(
    project: &ProjectDocument,
    textures: &EditorTextures,
    face: &GridFace,
    parent: &GridFace,
    ambient: [u8; 3],
    rig: &BakeRig<'_>,
    out: &mut Vec<PreparedPiece>,
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
    let colors: Vec<(u8, u8, u8)> = rig
        .corners(face, Some(parent), tint, ambient)
        .into_iter()
        .map(|c| (c[0], c[1], c[2]))
        .collect();
    let verts: Vec<[f64; 3]> = face.points.iter().map(|p| p.map(f64::from)).collect();
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
    out.push(PreparedPiece {
        shade,
        verts,
        uvs: texels,
        colors,
    });
}

/// Prepare `face`, cut at the texture tile boundaries the way the cook cuts
/// it, so the preview shows what a Play build draws.
fn prepare_face(
    project: &ProjectDocument,
    textures: &EditorTextures,
    scale: psxed_project::grid::GridScale,
    face: &GridFace,
    ambient: [u8; 3],
    rig: &BakeRig<'_>,
) -> PreparedFace {
    let mut pieces = Vec::new();
    // The preview draws through the same texture window a Play build uses,
    // so it cuts at the same four-tile pieces.
    if uv_split::wants_split(face, true) {
        for piece in uv_split::split(face, scale, true) {
            prepare_piece(
                project,
                textures,
                &piece,
                face,
                ambient,
                rig,
                &mut pieces,
            );
        }
    } else {
        prepare_piece(
            project,
            textures,
            face,
            face,
            ambient,
            rig,
            &mut pieces,
        );
    }
    PreparedFace {
        // The cutaway keeps or drops a face by its lowest corner, whole.
        low_y: face.points.iter().map(|p| p[1]).min().unwrap_or(i32::MAX),
        pieces,
    }
}

/// Fill in the faces of a box the camera has just shown for the first time.
fn prepare_box(
    project: &ProjectDocument,
    textures: &EditorTextures,
    world: &GridWorld,
    defaults: &ProjectUvDefaults,
    room_index: usize,
    source: BoxSource,
    rig: &BakeRig<'_>,
) -> Vec<PreparedFace> {
    let room = &world.rooms[room_index];
    let mut faces = Vec::new();
    match source {
        BoxSource::Cell(x, z) => {
            cell_faces_with(world, defaults, room_index, x, z, &mut faces);
        }
        BoxSource::Plane(index) => {
            plane_faces_with(world, defaults, room_index, index, &mut faces);
        }
        BoxSource::Mesh(index) => {
            mesh_faces_with(world, defaults, room_index, index, &mut faces);
        }
    }
    faces
        .iter()
        .map(|face| prepare_face(project, textures, world.scale, face, room.ambient, rig))
        .collect()
}

/// Run `f` with the rig a Play build bakes with: the world's collision for
/// shadows, the point lights, the facing shade and, for a world that gathers
/// its light, the solved field. A world whose collision cannot be built is
/// shown unshadowed.
fn with_bake_rig<R>(
    world: &GridWorld,
    scene: Option<&LightScene>,
    lights: &[psxed_project::grid::light::GridLight],
    field: Option<&GiField>,
    f: impl FnOnce(&BakeRig<'_>) -> R,
) -> R {
    let gather = world
        .global_light
        .enabled
        .then_some(Gather { field });
    match scene {
        // The scene parsed when it was built, so a rig over it cannot fail.
        Some(scene) => scene
            .with_rig(lights, world.shading, gather, f)
            .expect("a built scene parses"),
        None => {
            let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale)
                .or_else(|_| psxed_project::grid::cook::RuntimeUnits::new(Default::default()))
                .expect("the default scale is valid");
            f(&BakeRig {
                units,
                lights,
                shade: world.shading,
                occluded: &|_, _| false,
                gather,
            })
        }
    }
}

/// The boxes of a room the camera can pick from: every cell that has
/// geometry, every enabled plane, every placed mesh, each with the bounds the
/// view culls it by. Their faces come later, on first sight.
pub(super) fn prepare_room(world: &GridWorld, room_index: usize) -> PreparedRoom {
    let room = &world.rooms[room_index];
    let mut prepared = PreparedRoom::default();
    for [x, z] in room.face_cells() {
        if let Some(bounds) = cell_bounds(world, room_index, room, x, z) {
            prepared
                .cells
                .push(LazyBox::new(bounds, BoxSource::Cell(x, z)));
        }
    }
    for (index, plane) in room.planes.iter().enumerate() {
        if plane.enabled {
            prepared.planes.push(LazyBox::new(
                plane_bounds(world, room, plane),
                BoxSource::Plane(index),
            ));
        }
    }
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
        prepared
            .meshes
            .push(LazyBox::new(bounds, BoxSource::Mesh(index)));
    }
    prepared
}

/// Draw the active scene's grid world, if it has one.
///
/// Everything about a face that does not depend on the camera is prepared
/// once and kept (see [`super::grid_cache`]); a frame culls the boxes
/// against the camera, drops what the cutaway hides and emits what is left.
pub(super) fn walk_grid(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    view: &GridPreviewView,
    geometry: &mut GridGeometry,
    scratch: &mut PreviewScratch,
) {
    let Some(world) = project.active_scene().world.grid() else {
        return;
    };
    let defaults = ProjectUvDefaults(project);
    geometry.set_ghost(world, view.paint.as_ref());
    let RoomsToDraw {
        rooms,
        lights,
        scene,
        field,
        mut ghost,
    } = geometry.rooms_lights_ghost(world);
    with_bake_rig(world, scene, lights, field, |rig| {
    for (room_index, slot) in rooms.iter_mut().enumerate() {
        // Rooms the author hid in the Rooms panel are not drawn.
        if view.hides_room(room_index) {
            continue;
        }
        // The room a paint preview lands in is drawn from the world that has
        // the paint written, instead of from the document's.
        let (world, prepared) = match ghost.as_deref_mut().filter(|g| g.room == room_index) {
            Some(ghost) => (&ghost.world, &mut ghost.prepared),
            None => (
                world,
                slot.prepared
                    .get_or_insert_with(|| prepare_room(world, room_index)),
            ),
        };
        for boxes in [
            &mut prepared.cells,
            &mut prepared.planes,
            &mut prepared.meshes,
        ] {
            for lazy in boxes.iter_mut() {
                // The storey cutaway: nothing at or above its plane is drawn.
                if !preview_cell_bounds_visible(camera, lazy.bounds)
                    || (lazy.is_cell() && view.cuts(lazy.bounds.min[1]))
                {
                    continue;
                }
                if scratch.geometry_full() {
                    // A visible box that no longer fits: count it, keep
                    // counting so the report says how much is missing.
                    scratch.dropped = scratch.dropped.saturating_add(1);
                    continue;
                }
                let faces = lazy.faces.get_or_insert_with(|| {
                    prepare_box(
                        project,
                        textures,
                        world,
                        &defaults,
                        room_index,
                        lazy.source,
                        rig,
                    )
                });
                for face in faces
                    .iter()
                    .filter(|face| view.cutaway.is_none_or(|cut| face.low_y < cut))
                {
                    for piece in &face.pieces {
                        emit_uv_polygon(
                            scratch,
                            camera,
                            piece.shade,
                            &piece.verts,
                            &piece.uvs,
                            Some(&piece.colors),
                        );
                    }
                }
            }
        }
    }
    });
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
