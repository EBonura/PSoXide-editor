//! The Wall tool: drag a run of walls along sector edges.
//!
//! The wall goes on the edge nearest the pointer (or the edge chosen with R,
//! diagonals included) of the sector under it, and fills the gap there as
//! `psxed_project::grid::edit::next_wall_heights` works it out. A drag locks
//! to the edge's axis (or 45 degrees for a diagonal) and places one wall per
//! sector. Each wall faces the side the camera is on. Ctrl removes the wall
//! nearest the pointer height instead.

use super::cell_size::{cell_at, cell_units};
use super::view::editor_y;
use super::*;

use psxed_project::grid::edit::{
    facing_toward, leaf_wall_gap, place_wall, place_wall_leaf, remove_wall, remove_wall_leaf,
    split_to_fit, wall_gap, wall_run_cells, ClickRect, WallRefusal, WallSpec,
};
use psxed_project::grid::leaf::edge_clicks;
use psxed_project::grid::limits::MAX_ROOM_SIDE;
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::{GridRoom, GridWorld, LeafKey};

/// A wall spot the pointer resolved to.
pub(super) struct WallTarget {
    pub room: usize,
    /// The cell under the pointer, in cells of `level`.
    pub cell: [i32; 2],
    /// The cell level: 2 sectors, 3 halves, 4 quarters.
    pub level: u8,
    pub edge: WallEdge,
    /// Room-relative height under the pointer, 3D only.
    pub pick_y: Option<i32>,
}

/// Nearest cardinal edge to a point inside `cell` of `level`, from its
/// position in editor units.
fn nearest_edge(world: &GridWorld, cell: [i32; 2], level: u8, x: f32, z: f32) -> WallEdge {
    let unit = cell_units(level);
    let size = world.scale.to_editor(unit) as f32;
    let x0 = world.scale.to_editor(cell[0] * unit) as f32;
    let z0 = world.scale.to_editor(cell[1] * unit) as f32;
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
    let level = ws.grid.cell_size.level();
    let cell = cell_at(world.scale, point[0], point[2], level);
    let room = ws
        .grid_active_room
        .or(hit_room)
        .unwrap_or(ws.grid.last_room);
    let room = room.min(world.rooms.len().saturating_sub(1));
    let edge = match ws.grid.wall_dir {
        WallDirection::Auto => nearest_edge(world, cell, level, point[0], point[2]),
        WallDirection::Edge(e) => e,
    };
    let pick_y = if ptr.is_top() {
        // The storey cursor picks the gap in a column of several floors.
        let several = world.rooms.get(room).is_some_and(|r| {
            r.local(cell)
                .and_then(|[x, z]| r.sector(x, z))
                .is_some_and(|s| !s.is_single_span())
        });
        several.then(|| ws.aim_height(room)).flatten()
    } else {
        let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
        Some(world.scale.to_grid(point[1].round() as i32) - y_offset)
    };
    Some(WallTarget {
        room,
        cell,
        level,
        edge,
        pick_y,
    })
}

/// The leaf of `room` that covers `cell` of `level` (its first click).
fn leaf_of_cell(room: &GridRoom, cell: [i32; 2], level: u8) -> Option<LeafKey> {
    let e = edge_clicks(level);
    room.leaf_at([cell[0] * e, cell[1] * e]).map(|(key, _)| key)
}

fn heights_for(world: &GridWorld, t: &WallTarget) -> Option<[i32; 4]> {
    let room = world.rooms.get(t.room)?;
    if t.level == 2 {
        return wall_gap(room, t.cell, t.edge, t.pick_y);
    }
    let key = leaf_of_cell(room, t.cell, t.level)?;
    leaf_wall_gap(room, key, t.edge, t.pick_y)
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
    let level = ws.grid.cell_size.level();
    if let Some(p) = ws.grid_plane_point(ptr, plane) {
        g.current = cell_at(world.scale, p[0], p[2], level);
    }
    // Keep the run to a size the room cap allows, in cells of this level.
    let cap = MAX_ROOM_SIDE as i32 * (cell_units(2) / cell_units(level));
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

/// Leaves of `room` that lie on the cardinal `edge` of `cell`, a cell of
/// `level`.
fn leaves_on_edge(room: &GridRoom, cell: [i32; 2], level: u8, edge: WallEdge) -> Vec<LeafKey> {
    let e = edge_clicks(level);
    let (x0, z0) = (cell[0] * e, cell[1] * e);
    room.leaves()
        .map(|(key, _)| key)
        .filter(|k| {
            let [x1, z1] = k.end_click();
            k.x >= x0
                && k.z >= z0
                && x1 <= x0 + e
                && z1 <= z0 + e
                && match edge {
                    WallEdge::North => k.z == z0,
                    WallEdge::South => z1 == z0 + e,
                    WallEdge::West => k.x == x0,
                    WallEdge::East => x1 == x0 + e,
                    _ => false,
                }
        })
        .collect()
}

/// A run of walls on cells smaller than a sector. The leaves are split to fit
/// the run first (the library's split to fit, as the MCP's `size` does), so
/// every wall is one cell long; then each leaf on the run's edge gets its
/// wall, or loses the one nearest the pointer when erasing.
/// Returns (walls made or removed, edges already covered, room too large).
fn run_on_cells(
    world: &mut GridWorld,
    g: &WallGesture,
    cells: &[[i32; 2]],
    level: u8,
    material: Option<psxed_project::ResourceId>,
) -> (usize, usize, bool) {
    let lo = [
        cells.iter().map(|c| c[0]).min().unwrap_or(0),
        cells.iter().map(|c| c[1]).min().unwrap_or(0),
    ];
    let hi = [
        cells.iter().map(|c| c[0]).max().unwrap_or(0),
        cells.iter().map(|c| c[1]).max().unwrap_or(0),
    ];
    let Some(room) = world.rooms.get_mut(g.room) else {
        return (0, 0, false);
    };
    split_to_fit(room, ClickRect::of_cells(level, lo, hi), level);
    let mut keys: Vec<LeafKey> = Vec::new();
    for cell in cells {
        for key in leaves_on_edge(room, *cell, level, g.edge) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    let (mut done, mut covered, mut too_large) = (0usize, 0usize, false);
    for key in keys {
        if g.erase {
            if remove_wall_leaf(world, g.room, key, g.edge, g.pick_y) {
                done += 1;
            }
            continue;
        }
        let facing = g
            .viewer
            .map_or(WallFacing::Front, |v| facing_toward(key.cell(), g.edge, v));
        let spec = WallSpec { facing, material };
        match place_wall_leaf(world, g.room, key, g.edge, g.pick_y, spec) {
            Ok(_) => done += 1,
            Err(WallRefusal::RoomTooLarge) => too_large = true,
            Err(WallRefusal::NoSuchRoom) => {}
            Err(WallRefusal::Covered | WallRefusal::BigCell) => covered += 1,
        }
    }
    (done, covered, too_large)
}

pub(super) fn commit(ws: &mut EditorWorkspace, g: &WallGesture) {
    let level = ws.grid.cell_size.level();
    if level != 2 && g.edge.is_diagonal() {
        ws.status = "Diagonal walls are sector-sized: pick Sector for them".to_string();
        return;
    }
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
    if level != 2 {
        (done, covered, too_large) = run_on_cells(world, g, &cells, level, material);
    } else {
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
                Err(WallRefusal::BigCell) => covered += 1,
            }
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
