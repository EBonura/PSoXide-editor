//! The Floor and Ceiling tools: drag a rectangle of sectors.
//!
//! The rectangle follows the pointer over the plane at the surface's
//! height; holding Shift locks it and turns pointer travel into height, in
//! clicks (finer with Alt). Releasing creates the surface on every sector
//! that has none, growing the room as needed. Ctrl or Cmd deletes instead.
//! The last height used is kept for the next rectangle.

use super::view::{cell_at_editor, editor_y};
use super::*;

use psxed_project::grid::edit::{create_surfaces, delete_surfaces, CellRect, RectRefusal};
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

/// The sector under the pointer on the plane at `height` of `room`.
fn cell_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    y_offset: i32,
    height: i32,
) -> Option<[i32; 2]> {
    let world = ws.project.active_scene().world.grid()?;
    let y = editor_y(world, y_offset, height);
    let p = ws.grid_plane_point(ptr, y)?;
    Some(cell_at_editor(world.scale, p[0], p[2]))
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: SurfaceKind) {
    let (room, y_offset) = room_offset(ws);
    let height = default_height(ws, kind);
    ws.grid.hover = match cell_under(ws, ptr, y_offset, height) {
        Some(cell) => GridHover::Cell { room, cell, height },
        None => GridHover::None,
    };
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: SurfaceKind) {
    let (room, y_offset) = room_offset(ws);
    let height = default_height(ws, kind);
    let Some(cell) = cell_under(ws, &ptr.at_press(), y_offset, height) else {
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
            let c = g.current;
            let x = w.scale.to_editor(c[0] * psxed_project::grid::SECTOR_UNITS) as f32;
            let z = w.scale.to_editor(c[1] * psxed_project::grid::SECTOR_UNITS) as f32;
            [x, editor_y(w, y_offset, base), z]
        });
        let ppu = anchor_world.map_or(0.1, |p| ws.grid_pixels_per_unit(ptr, p));
        let step = if ptr.alt() { FINE_UNITS } else { CLICK_UNITS };
        let raw = ((origin_y - ptr.pos.y) / ppu).round() as i32;
        g.height = snap_to(base + raw, step);
    } else {
        g.adjust = None;
        if let Some(cell) = cell_under(ws, ptr, y_offset, g.height) {
            g.current = cell;
        }
    }
    ws.grid.gesture = Gesture::Rect(g);
}

/// Create or delete over the finished rectangle. Returns the status line.
pub(super) fn commit(ws: &mut EditorWorkspace, g: RectGesture) {
    let rect = CellRect::from_corners(g.anchor, g.current);
    apply(ws, g.kind, g.room, rect, g.height, g.erase);
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
    let Some(cell) = cell_under(ws, ptr, y_offset, height) else {
        return;
    };
    apply(
        ws,
        kind,
        room,
        CellRect::from_corners(cell, cell),
        height,
        ptr.command(),
    );
}

fn apply(
    ws: &mut EditorWorkspace,
    kind: SurfaceKind,
    room: usize,
    rect: CellRect,
    height: i32,
    erase: bool,
) {
    let material = ws.grid.material;
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let mut room = room;
    if world.rooms.is_empty() {
        if erase {
            ws.status = format!("No {} to delete there", kind.label());
            return;
        }
        let mut first = GridRoom::new("Room 0", 1, 1);
        // The first room starts where the first rectangle does.
        first.origin = rect.min;
        world.rooms.push(first);
        room = 0;
    }
    let outcome = if erase {
        delete_surfaces(world, room, kind, rect)
    } else {
        create_surfaces(world, room, kind, rect, height, material)
    };
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
    ws.status = match (erase, outcome.skipped) {
        (true, _) => format!("Deleted {} {}s", outcome.changed, kind.label()),
        (false, 0) => format!("Created {} {}s", outcome.changed, kind.label()),
        (false, n) => format!(
            "Created {} {}s ({n} sectors already had one)",
            outcome.changed,
            kind.label()
        ),
    };
    ws.history.record(before);
    ws.grid_changed();
}
