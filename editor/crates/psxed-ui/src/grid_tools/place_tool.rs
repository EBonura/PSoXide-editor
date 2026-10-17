//! The Place tool: drop the chosen node (light, spawn, actor) on the grid.
//!
//! The position snaps to the centre of the sector under the pointer and to
//! the floor height there, so a placed actor stands on the floor, however
//! the floor slopes or which room it is in. Alt snaps to the fine lattice
//! (64 units) and Shift+Alt places freely.

use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::SECTOR_UNITS;

/// Where a node would land for the pointer, in editor units.
pub(super) fn spot(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<[f32; 3]> {
    let world = ws.project.active_scene().world.grid()?;
    let point = match ptr.view {
        GridView::Persp(rect) => ws
            .pick_face_3d(rect, ptr.pos)
            .map(|h| h.point)
            .or_else(|| {
                let (origin, dir) = ws.camera_ray_for_pointer(rect, ptr.pos)?;
                view::ray_plane_y(origin, dir, 0.0)
            })?,
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            [x, f32::MAX, z]
        }
    };
    let centre = if ptr.free() {
        [point[0], point[2]]
    } else if ptr.alt() {
        // The fine lattice: 64 grid units.
        let fine = |v: f32| {
            let grid = world.scale.to_grid(v.round() as i32);
            world.scale.to_editor(psxed_project::grid::units::snap_to(
                grid,
                psxed_project::grid::FINE_UNITS,
            )) as f32
        };
        [fine(point[0]), fine(point[2])]
    } else {
        let cell = cell_at_editor(world.scale, point[0], point[2]);
        let half = SECTOR_UNITS / 2;
        [
            world.scale.to_editor(cell[0] * SECTOR_UNITS + half) as f32,
            world.scale.to_editor(cell[1] * SECTOR_UNITS + half) as f32,
        ]
    };
    // The floor under the point, not above what the pointer touched.
    let limit = if point[1] == f32::MAX {
        i32::MAX / 2
    } else {
        point[1].round() as i32 + 96
    };
    let floor = world.floor_height_at(centre[0].round() as i32, centre[1].round() as i32, limit);
    let y = floor.map_or(if point[1] == f32::MAX { 0.0 } else { point[1] }, |f| {
        f as f32
    });
    Some([centre[0], y, centre[1]])
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = match spot(ws, ptr) {
        Some(position) => GridHover::Spot { position },
        None => GridHover::None,
    };
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(position) = spot(ws, ptr) else {
        ws.status = "Place over the floor".to_string();
        return;
    };
    if ws.place_at_world_hit([
        position[0],
        position[1] + ws.place_height_offset(),
        position[2],
    ]) {
        ws.return_to_select_after_place();
        ws.set_grid_tool(GridTool::Select);
    }
}
