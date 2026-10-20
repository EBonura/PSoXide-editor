//! The Floor and Ceiling tools: drag a rectangle of sectors.
//!
//! The rectangle follows the pointer over the plane at the surface's
//! height; holding Shift locks it and turns pointer travel into height, in
//! clicks (finer with Alt). Releasing creates the surface on every sector
//! that has none, growing the room as needed. Ctrl or Cmd deletes instead.
//! The last height used is kept for the next rectangle.

use super::cell_size::{cell_at, cell_origin_units};
use super::view::editor_y;
use super::*;

use psxed_project::grid::edit::focus::Focus;
use psxed_project::grid::edit::{
    create_surfaces, create_surfaces_cells, delete_surfaces, delete_surfaces_cells,
    join_fresh_floors, CellRect, JoinOutcome, RectRefusal,
};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{GridRoom, CLICK_UNITS, FINE_UNITS};

/// The room and its vertical offset the next rectangle will land in.
fn room_offset(ws: &EditorWorkspace) -> (usize, i32) {
    let room = ws.grid_target_room();
    let y = ws
        .project
        .active_scene()
        .world
        .grid()
        .and_then(|w| w.rooms.get(room))
        .map_or(0, |r| r.y_offset);
    (room, y)
}

fn default_height(ws: &EditorWorkspace, kind: SurfaceKind) -> i32 {
    match kind {
        SurfaceKind::Floor => ws.grid.floor_height,
        SurfaceKind::Ceiling => ws.grid.ceiling_height,
    }
}

/// The cell of `level` under the pointer on the plane at `height` of `room`.
fn cell_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    y_offset: i32,
    height: i32,
    level: u8,
) -> Option<[i32; 2]> {
    let world = ws.project.active_scene().world.grid()?;
    let y = editor_y(world, y_offset, height);
    let p = ws.grid_plane_point(ptr, y)?;
    Some(cell_at(world.scale, p[0], p[2], level))
}

/// Room-relative height of the `kind` surface under a 3D pointer, when the
/// pointer is over one in the target room.
fn height_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    kind: SurfaceKind,
    room: usize,
) -> Option<i32> {
    let GridView::Persp(rect) = ptr.view else {
        return None;
    };
    let hit = ws.pick_face_3d(rect, ptr.pos)?;
    let on_kind = matches!(
        hit.face.target,
        psxed_project::grid::edit::FaceTarget::Surface { kind: k, .. } if k == kind
    );
    if hit.face.room != room || !on_kind {
        return None;
    }
    let world = ws.project.active_scene().world.grid()?;
    let heights = hit.face.surface(world)?.heights;
    Some(heights.iter().sum::<i32>() / 4)
}

/// The height the next rectangle is made at: pointing at an existing surface
/// of the same kind in the 3D view continues at its level, so extending a
/// platform or a ceiling stays on it; otherwise the tool's own height. The
/// second value says whether the surface under the pointer was taken.
fn aimed_height(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    kind: SurfaceKind,
    room: usize,
) -> (i32, bool) {
    match height_under(ws, ptr, kind, room) {
        Some(height) => (height, true),
        None => (default_height(ws, kind), false),
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: SurfaceKind) {
    let (room, y_offset) = room_offset(ws);
    // The ghost shows where a press would land; hovering changes no setting.
    let (height, _) = aimed_height(ws, ptr, kind, room);
    let level = ws.grid.cell_size.level();
    ws.grid.hover = match cell_under(ws, ptr, y_offset, height, level) {
        Some(cell) => GridHover::Cell { room, cell, height },
        None => GridHover::None,
    };
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: SurfaceKind) {
    let (room, y_offset) = room_offset(ws);
    let ptr_at_press = ptr.at_press();
    let (height, adopted) = aimed_height(ws, &ptr_at_press, kind, room);
    if adopted {
        // Taking the level of the surface under the pointer is a change to
        // the tool's height, so it happens on the press, not on a hover.
        match kind {
            SurfaceKind::Floor => ws.grid.floor_height = height,
            SurfaceKind::Ceiling => ws.grid.ceiling_height = height,
        }
    }
    let level = ws.grid.cell_size.level();
    let Some(cell) = cell_under(ws, &ptr.at_press(), y_offset, height, level) else {
        return;
    };
    ws.grid.gesture = Gesture::Rect(RectGesture {
        kind,
        room,
        anchor: cell,
        current: cell,
        height,
        adjust: None,
        erase: ptr.command(),
        level,
    });
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Rect(mut g) = ws.grid.gesture else {
        return;
    };
    let (_, y_offset) = {
        let world = ws.project.active_scene().world.grid();
        (
            g.room,
            world
                .and_then(|w| w.rooms.get(g.room))
                .map_or(0, |r| r.y_offset),
        )
    };
    g.erase = ptr.command();
    if ptr.shift() {
        // Height mode: the rectangle stays put, pointer travel is height.
        let (origin_y, base) = *g.adjust.get_or_insert((ptr.pos.y, g.height));
        let anchor_world = ws.project.active_scene().world.grid().map(|w| {
            let [ux, uz] = cell_origin_units(g.level, g.current);
            let x = w.scale.to_editor(ux) as f32;
            let z = w.scale.to_editor(uz) as f32;
            [x, editor_y(w, y_offset, base), z]
        });
        let ppu = anchor_world.map_or(0.1, |p| ws.grid_pixels_per_unit(ptr, p));
        let step = if ptr.alt() { FINE_UNITS } else { CLICK_UNITS };
        let raw = ((origin_y - ptr.pos.y) / ppu).round() as i32;
        g.height = snap_to(base + raw, step);
    } else {
        g.adjust = None;
        if let Some(cell) = cell_under(ws, ptr, y_offset, g.height, g.level) {
            g.current = cell;
        }
    }
    ws.grid.gesture = Gesture::Rect(g);
}

/// Create or delete over the finished rectangle. Returns the status line.
pub(super) fn commit(ws: &mut EditorWorkspace, g: RectGesture) {
    let rect = CellRect::from_corners(g.anchor, g.current);
    apply(ws, g.kind, g.room, rect, g.height, g.erase, g.level);
    if !g.erase {
        match g.kind {
            SurfaceKind::Floor => ws.grid.floor_height = g.height,
            SurfaceKind::Ceiling => ws.grid.ceiling_height = g.height,
        }
    }
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    let gesture = std::mem::take(&mut ws.grid.gesture);
    if let Gesture::Rect(g) = gesture {
        commit(ws, g);
    }
}

/// A click without a drag: one sector.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: SurfaceKind) {
    let (room, y_offset) = room_offset(ws);
    let height = default_height(ws, kind);
    let level = ws.grid.cell_size.level();
    let Some(cell) = cell_under(ws, ptr, y_offset, height, level) else {
        return;
    };
    apply(
        ws,
        kind,
        room,
        CellRect::from_corners(cell, cell),
        height,
        ptr.command(),
        level,
    );
}

/// The cells of `rect` that hold no floor in `room`, before an edit.
fn fresh_floor_cells(project: &ProjectDocument, room: usize, rect: CellRect) -> Vec<[i32; 2]> {
    let Some(room) = project
        .active_scene()
        .world
        .grid()
        .and_then(|w| w.rooms.get(room))
    else {
        return Vec::new();
    };
    rect.cells()
        .filter(|&cell| {
            !room
                .local(cell)
                .and_then(|[x, z]| room.sector(x, z))
                .is_some_and(|s| s.ground_floor().is_some())
        })
        .collect()
}

/// What joining the new floor to its neighbours did, for the status line:
/// the walls opened in its own room, and the other rooms it touches.
fn join_status(ws: &EditorWorkspace, join: &JoinOutcome) -> String {
    let mut text = String::new();
    if join.opened_edges > 0 {
        let n = join.opened_edges;
        text += &format!(", opened {n} wall{}", if n == 1 { "" } else { "s" });
    }
    if join.kept_edges > 0 {
        let n = join.kept_edges;
        text += &format!(
            ", kept {n} wall{} where the floors differ in height",
            if n == 1 { "" } else { "s" }
        );
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return text;
    };
    for &index in &join.touched_rooms {
        let Some(other) = world.rooms.get(index) else {
            continue;
        };
        text += &if other.wall_mode == psxed_project::grid::WallMode::Derived {
            format!(
                "; touches {}: its walls are derived, its wall on that edge still closes it",
                other.name
            )
        } else {
            format!(
                "; touches {}: its authored walls stay, remove them to connect",
                other.name
            )
        };
    }
    text
}

/// Apply a rectangle of cells of `level` (2 sectors, 3 halves, 4 quarters):
/// the sector tools for whole sectors, the library's cell functions below
/// that, as the MCP's `size: half|quarter` does.
#[allow(clippy::too_many_arguments)]
fn apply(
    ws: &mut EditorWorkspace,
    kind: SurfaceKind,
    room: usize,
    rect: CellRect,
    height: i32,
    erase: bool,
    level: u8,
) {
    let material = ws.grid_new_material(room, false);
    // The span of each column the cursor names; cells below a sector have no
    // spans to aim at.
    let focus = if level == 2 {
        match ws.capture_focus(room, rect.cells()) {
            Ok(focus) => focus,
            Err(why) => {
                ws.status = why;
                return;
            }
        }
    } else {
        Focus::none(room)
    };
    let before = ws.project.clone();
    // The sectors that have no floor yet: where a new floor joins the room.
    let fresh = if level == 2 && !erase && kind == SurfaceKind::Floor {
        fresh_floor_cells(&before, room, rect)
    } else {
        Vec::new()
    };
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let mut room = room;
    if world.rooms.is_empty() {
        if erase {
            ws.status = format!("No {} to delete there", kind.label());
            return;
        }
        let mut first = GridRoom::new_derived("Room 0", 1, 1);
        // The first room starts where the first rectangle does, in sectors.
        let per_sector = 2_i32.pow(u32::from(level) - 2).max(1);
        first.origin = [
            rect.min[0].div_euclid(per_sector),
            rect.min[1].div_euclid(per_sector),
        ];
        world.rooms.push(first);
        room = 0;
    }
    let aimed = match (level, erase) {
        (2, true) => focus.run(world, |w| delete_surfaces(w, room, kind, rect)),
        (2, false) => focus.run(world, |w| {
            create_surfaces(w, room, kind, rect, height, material)
        }),
        (_, true) => focus.run(world, |w| {
            delete_surfaces_cells(w, room, kind, level, rect.min, rect.max)
        }),
        (_, false) => focus.run(world, |w| {
            create_surfaces_cells(w, room, kind, level, rect.min, rect.max, height, material)
        }),
    };
    let aimed = match aimed {
        Ok(aimed) => aimed,
        Err(refusal) => {
            ws.status = ws.describe_refusal(refusal);
            return;
        }
    };
    let outcome = aimed.value;
    if let Some(refusal) = outcome.refused {
        ws.status = match refusal {
            RectRefusal::RoomTooLarge { needed } => format!(
                "Room would be {needed} sectors wide, the cap is {}: make a second room",
                psxed_project::grid::limits::MAX_ROOM_SIDE
            ),
            RectRefusal::NoSuchRoom => "No such room".to_string(),
        };
        return;
    }
    if outcome.changed == 0 {
        ws.status = if erase {
            format!("No {} to delete there", kind.label())
        } else {
            format!("A {} is already there", kind.label())
        };
        return;
    }
    ws.grid.last_room = room;
    let join = match ws.project.active_scene_mut().world.grid_mut() {
        Some(world) if !fresh.is_empty() && room < world.rooms.len() => {
            join_fresh_floors(world, room, &fresh)
        }
        _ => JoinOutcome::default(),
    };
    ws.status = match (erase, outcome.skipped) {
        (true, _) => format!("Deleted {} {}s", outcome.changed, kind.label()),
        (false, 0) => format!("Created {} {}s", outcome.changed, kind.label()),
        (false, n) => format!(
            "Created {} {}s ({n} sectors already had one)",
            outcome.changed,
            kind.label()
        ),
    };
    if !erase {
        // Say where it went: a rectangle far from the room still joins it.
        let name = ws
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
            .map(|r| r.name.clone());
        if let Some(name) = name {
            ws.status += &format!(" in {name}");
        }
    }
    ws.status += &join_status(ws, &join);
    if aimed.slivers > 0 {
        ws.status += &format!(
            ", {} span(s) too thin to stand in filled solid",
            aimed.slivers
        );
    }
    ws.history.record(before);
    ws.grid_changed();
    // The cursor follows a floor made: the next verb acts on its storey.
    if kind == SurfaceKind::Floor && !erase {
        ws.follow_edit(room, Some(height));
    }
}
