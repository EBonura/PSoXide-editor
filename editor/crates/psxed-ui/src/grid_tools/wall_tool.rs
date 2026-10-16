//! The Wall tool: drag a run of walls along sector edges.
//!
//! The wall goes on the edge nearest the pointer (or the edge chosen with R,
//! diagonals included) of the sector under it, and fills the gap there as
//! `psxed_project::grid::edit::next_wall_heights` works it out. A drag locks
//! to the edge's axis (or 45 degrees for a diagonal) and places one wall per
//! sector. Each wall faces the side the camera is on. Ctrl removes the wall
//! nearest the pointer height instead.

use super::view::{cell_at_editor, editor_y};
use super::*;

use psxed_project::grid::edit::{
    facing_toward, next_wall_heights, place_wall, remove_wall, wall_run_cells, WallRefusal,
    WallSpec,
};
use psxed_project::grid::limits::MAX_ROOM_SIDE;
use psxed_project::grid::sector::GridSector;
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::{GridWorld, SECTOR_UNITS};

/// A wall spot the pointer resolved to.
pub(super) struct WallTarget {
    pub room: usize,
    pub cell: [i32; 2],
    pub edge: WallEdge,
    /// Room-relative height under the pointer, 3D only.
    pub pick_y: Option<i32>,
}

/// Nearest cardinal edge to a point inside `cell`, from its position in
/// editor units.
fn nearest_edge(world: &GridWorld, cell: [i32; 2], x: f32, z: f32) -> WallEdge {
    let size = world.scale.sector_editor_units() as f32;
    let x0 = world.scale.to_editor(cell[0] * SECTOR_UNITS) as f32;
    let z0 = world.scale.to_editor(cell[1] * SECTOR_UNITS) as f32;
    let (fx, fz) = (
        ((x - x0) / size).clamp(0.0, 1.0),
        ((z - z0) / size).clamp(0.0, 1.0),
    );
    [
        (fz, WallEdge::North),
        (1.0 - fx, WallEdge::East),
        (1.0 - fz, WallEdge::South),
        (fx, WallEdge::West),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map_or(WallEdge::North, |(_, edge)| edge)
}

pub(super) fn target_at(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<WallTarget> {
    let world = ws.project.active_scene().world.grid()?;
    let (point, hit_room, from_wall) = match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            ([x, 0.0, z], None, false)
        }
        GridView::Persp(rect) => {
            let hit = ws.pick_face_3d(rect, ptr.pos);
            let (origin, dir) = ws.camera_ray_for_pointer(rect, ptr.pos)?;
            let target_room = ws.grid_target_room();
            let y_offset = world.rooms.get(target_room).map_or(0, |r| r.y_offset);
            match hit {
                Some(h) => (
                    h.point,
                    Some(h.face.room),
                    matches!(
                        h.face.target,
                        psxed_project::grid::edit::FaceTarget::Wall { .. }
                    ),
                ),
                None => (
                    view::ray_plane_y(origin, dir, editor_y(world, y_offset, 0))?,
                    None,
                    false,
                ),
            }
        }
    };
    let mut point = point;
    if from_wall {
        // A hit on a wall sits exactly on the edge line; step towards the
        // camera so the sector on the viewer's side owns the new wall.
        let camera = ws.viewport_3d_camera().position_f32();
        let (dx, dz) = (camera[0] - point[0], camera[2] - point[2]);
        let length = (dx * dx + dz * dz).sqrt().max(1e-3);
        point[0] += dx / length * 3.0;
        point[2] += dz / length * 3.0;
    }
    let cell = cell_at_editor(world.scale, point[0], point[2]);
    let room = ws
        .grid_active_room
        .or(hit_room)
        .unwrap_or(ws.grid.last_room);
    let room = room.min(world.rooms.len().saturating_sub(1));
    let edge = match ws.grid.wall_dir {
        WallDirection::Auto => nearest_edge(world, cell, point[0], point[2]),
        WallDirection::Edge(e) => e,
    };
    let pick_y = (!ptr.is_top()).then(|| {
        let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
        world.scale.to_grid(point[1].round() as i32) - y_offset
    });
    Some(WallTarget {
        room,
        cell,
        edge,
        pick_y,
    })
}

fn heights_for(world: &GridWorld, t: &WallTarget) -> Option<[i32; 4]> {
    let room = world.rooms.get(t.room)?;
    let sector = room
        .local(t.cell)
        .and_then(|[x, z]| room.sector(x, z))
        .cloned()
        .unwrap_or_else(GridSector::default);
    next_wall_heights(
        &sector,
        t.edge,
        psxed_project::grid::edit::walls::room_height_bounds(room),
        t.pick_y,
    )
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = match target_at(ws, ptr) {
        Some(t) => {
            let heights = ws
                .project
                .active_scene()
                .world
                .grid()
                .and_then(|w| heights_for(w, &t));
            GridHover::Wall {
                room: t.room,
                cell: t.cell,
                edge: t.edge,
                heights,
            }
        }
        None => GridHover::None,
    };
}

fn viewer_for(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<[i64; 2]> {
    if ptr.is_top() {
        return None;
    }
    let world = ws.project.active_scene().world.grid()?;
    let camera = ws.viewport_3d_camera().position_f32();
    Some([
        i64::from(world.scale.to_grid(camera[0].round() as i32)),
        i64::from(world.scale.to_grid(camera[2].round() as i32)),
    ])
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(t) = target_at(ws, &ptr.at_press()) else {
        return;
    };
    ws.grid.gesture = Gesture::Walls(WallGesture {
        room: t.room,
        edge: t.edge,
        anchor: t.cell,
        current: t.cell,
        pick_y: t.pick_y,
        erase: ptr.command(),
        viewer: viewer_for(ws, ptr),
    });
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Walls(mut g) = ws.grid.gesture else {
        return;
    };
    g.erase = ptr.command();
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    // Follow the pointer over the plane through the gap being filled, so the
    // cell does not jump between the wall and the floor behind it.
    let y_offset = world.rooms.get(g.room).map_or(0, |r| r.y_offset);
    let plane = editor_y(world, y_offset, g.pick_y.unwrap_or(0));
    if let Some(p) = ws.grid_plane_point(ptr, plane) {
        g.current = cell_at_editor(world.scale, p[0], p[2]);
    }
    // Keep the run to a size the room cap allows.
    let cap = MAX_ROOM_SIDE as i32;
    g.current = [
        g.current[0].clamp(g.anchor[0] - cap, g.anchor[0] + cap),
        g.current[1].clamp(g.anchor[1] - cap, g.anchor[1] + cap),
    ];
    ws.grid.gesture = Gesture::Walls(g);
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    let gesture = std::mem::take(&mut ws.grid.gesture);
    if let Gesture::Walls(g) = gesture {
        commit(ws, &g);
    }
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(t) = target_at(ws, ptr) else { return };
    let g = WallGesture {
        room: t.room,
        edge: t.edge,
        anchor: t.cell,
        current: t.cell,
        pick_y: t.pick_y,
        erase: ptr.command(),
        viewer: viewer_for(ws, ptr),
    };
    commit(ws, &g);
}

pub(super) fn commit(ws: &mut EditorWorkspace, g: &WallGesture) {
    let before = ws.project.clone();
    let material = ws.grid_new_material(g.room, true);
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    if g.room >= world.rooms.len() {
        ws.status = "Make a floor first: walls need a room".to_string();
        return;
    }
    let cells = wall_run_cells(g.anchor, g.current, g.edge);
    let (mut done, mut covered, mut too_large) = (0usize, 0usize, false);
    for cell in cells {
        if g.erase {
            if remove_wall(world, g.room, cell, g.edge, g.pick_y) {
                done += 1;
            }
            continue;
        }
        let facing = g
            .viewer
            .map_or(WallFacing::Front, |v| facing_toward(cell, g.edge, v));
        let spec = WallSpec { facing, material };
        match place_wall(world, g.room, cell, g.edge, g.pick_y, spec) {
            Ok(_) => done += 1,
            Err(WallRefusal::Covered) => covered += 1,
            Err(WallRefusal::RoomTooLarge) => too_large = true,
            Err(WallRefusal::NoSuchRoom) => {}
        }
    }
    if done == 0 {
        ws.status = if too_large {
            format!("Room would pass {MAX_ROOM_SIDE} sectors: make a second room")
        } else if g.erase {
            "No wall there to remove".to_string()
        } else {
            "That edge is fully covered".to_string()
        };
        return;
    }
    ws.status = match (g.erase, covered) {
        (true, _) => format!("Removed {done} walls"),
        (false, 0) => format!("Placed {done} {} walls", g.edge.label()),
        (false, n) => format!(
            "Placed {done} {} walls ({n} edges already covered)",
            g.edge.label()
        ),
    };
    ws.grid.last_room = g.room;
    ws.history.record(before);
    ws.grid_changed();
}
