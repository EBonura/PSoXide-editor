//! Raising and lowering vertices, edges and faces.
//!
//! Used by the Height tool (every drag) and by Shift-drag in the Select
//! tool. The pointer's vertical travel is turned into grid units at the
//! rate that keeps the grabbed point under the pointer, then snapped to
//! clicks (finer with Alt). The corners that move are the selection's, plus
//! every corner coincident with them while linking is on (L toggles it).

use super::pick::PickLevel;
use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::{CornerRef, FaceRef, GridElement, HeightDrag};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS, SECTOR_UNITS};

/// Pixels from a cell corner within which the Top view grabs the vertex.
const TOP_VERTEX_PIXELS: f32 = 10.0;
/// Pixels from a cell edge within which the Top view grabs the edge.
const TOP_EDGE_PIXELS: f32 = 5.0;

/// Corner number of a sector-local offset.
fn corner_number(offset: [i32; 2]) -> u8 {
    match offset {
        [0, 0] => 0,
        [1, 0] => 1,
        [1, 1] => 2,
        _ => 3,
    }
}

/// What the Height tool grabs in the Top view: a floor vertex near a cell
/// corner, a floor edge near a cell edge, else the floor (or ceiling) of
/// the cell.
pub(super) fn top_element(
    ws: &EditorWorkspace,
    transform: ViewportTransform,
    pos: Pos2,
) -> Option<GridElement> {
    let world = ws.project.active_scene().world.grid()?;
    let scale = world.scale;
    let size = scale.sector_editor_units() as f32;
    let [wx, wz] = transform.screen_to_world(pos);
    let cell = cell_at_editor(scale, wx, wz);
    let room = ws.top_room_at(cell)?;
    let surface_of = |cell: [i32; 2]| {
        let r = &world.rooms[room];
        let sector = r.local(cell).and_then(|[x, z]| r.sector(x, z))?;
        if sector.floor().is_some() {
            Some(FaceRef::floor(room, cell))
        } else if sector.ceiling().is_some() {
            Some(FaceRef::ceiling(room, cell))
        } else {
            None
        }
    };
    // Nearest lattice vertex.
    let (gx, gz) = ((wx / size).round() as i32, (wz / size).round() as i32);
    let vertex_screen =
        transform.world_to_screen(view::editor_xz(scale, gx * SECTOR_UNITS, gz * SECTOR_UNITS));
    if vertex_screen.distance(pos) <= TOP_VERTEX_PIXELS {
        for (dx, dz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
            let c = [gx + dx, gz + dz];
            if let Some(face) = surface_of(c) {
                let offset = [gx - c[0], gz - c[1]];
                return Some(GridElement::Vertex(CornerRef {
                    face,
                    layer: 0,
                    corner: corner_number(offset),
                }));
            }
        }
    }
    // Nearest cell edge of the cell under the pointer.
    let face = surface_of(cell)?;
    let (x0, z0) = (
        scale.to_editor(cell[0] * SECTOR_UNITS) as f32,
        scale.to_editor(cell[1] * SECTOR_UNITS) as f32,
    );
    let zoom_px = |d: f32| (d * transform.screen_radius(1.0)).abs();
    let (fx, fz) = (wx - x0, wz - z0);
    let candidates = [
        (zoom_px(fz), 0u8),
        (zoom_px(size - fx), 1),
        (zoom_px(size - fz), 2),
        (zoom_px(fx), 3),
    ];
    if let Some((d, edge)) = candidates.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
        if d <= TOP_EDGE_PIXELS {
            return Some(GridElement::Edge(psxed_project::grid::edit::EdgeRef {
                face,
                layer: 0,
                edge,
            }));
        }
    }
    Some(GridElement::Face(face))
}

/// What a press at `ptr` grabs for a height edit.
pub(super) fn element_at(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<GridElement> {
    match ptr.view {
        GridView::Top(transform) => top_element(ws, transform, ptr.pos),
        GridView::Persp(rect) => ws
            .pick_element_3d(rect, ptr.pos, PickLevel::Vertex)
            .map(|(e, _)| e),
    }
}

/// Start a height drag on what is under the pointer (or the selection when
/// that is part of it).
pub(super) fn begin(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    let press = ptr.at_press();
    // Alt on a face (not a vertex or an edge) grabs one triangle.
    if press.alt() && super::triangle_tool::begin_drag(ws, ptr) {
        return true;
    }
    let Some(element) = element_at(ws, &press) else {
        return false;
    };
    if !ws.grid.selection.contains(&element) {
        ws.grid.selection.replace(element);
    }
    ws.after_grid_selection_changed();
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    let corners = ws.grid.selection.corners(world);
    let drag = HeightDrag::begin(world, &corners, ws.grid.link);
    if drag.is_empty() {
        return false;
    }
    // Rate: pixels per grid unit at the grabbed point.
    let grab = corners
        .first()
        .and_then(|c| c.grid_pos(world))
        .map(|g| {
            [
                world.scale.to_editor(g[0]) as f32,
                world.scale.to_editor(g[1]) as f32,
                world.scale.to_editor(g[2]) as f32,
            ]
        })
        .unwrap_or([0.0; 3]);
    let ppu = ws.grid_pixels_per_unit(&press, grab);
    ws.grid.gesture = Gesture::Height(HeightGesture {
        drag,
        press_y: press.pos.y,
        pixels_per_unit: ppu,
        delta: 0,
        before: Some(Box::new(ws.project.clone())),
        triangle: None,
    });
    true
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Height(mut g) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    // A triangle drag keeps Alt for the grab; Shift is its fine step.
    let fine = if g.triangle.is_some() {
        ptr.shift()
    } else {
        ptr.alt()
    };
    let step = if fine { FINE_UNITS } else { CLICK_UNITS };
    let raw = ((g.press_y - ptr.pos.y) / g.pixels_per_unit.max(1e-4)).round() as i32;
    let delta = snap_to(raw, step);
    if delta != g.delta {
        nudge_gesture(ws, &mut g, delta);
    }
    ws.grid.gesture = Gesture::Height(g);
}

/// Put the gesture's corners `delta` grid units from where they began,
/// recording the undo step at the first change.
pub(super) fn nudge_gesture(ws: &mut EditorWorkspace, g: &mut HeightGesture, delta: i32) {
    if let Some(before) = g.before.take() {
        ws.history.record(*before);
    }
    if let Some(world) = ws.project.active_scene_mut().world.grid_mut() {
        if let Some(triangle) = &g.triangle {
            triangle.prepare(world, delta);
        }
        g.drag.apply(world, delta);
    }
    g.delta = delta;
    ws.grid_changed();
    let clicks = delta as f32 / CLICK_UNITS as f32;
    ws.status = match &g.triangle {
        Some(triangle) => format!(
            "{} {:+} units ({clicks:+.2} clicks): a diagonal wall shows between the halves",
            triangle.label(),
            delta
        ),
        None => format!(
            "Height {:+} units ({clicks:+.2} clicks), {} corners{}",
            delta,
            g.drag.len(),
            if ws.grid.link { "" } else { ", unlinked" }
        ),
    };
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    if matches!(ws.grid.gesture, Gesture::Height(_)) {
        ws.grid.gesture = Gesture::Idle;
    }
}

/// Raise or lower the whole selection by `delta` grid units (the keyboard).
pub(super) fn nudge_selection(ws: &mut EditorWorkspace, delta: i32) {
    if super::plane_tool::nudge_selected(ws, delta) {
        return;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let corners = ws.grid.selection.corners(world);
    if corners.is_empty() {
        ws.status = "Select something to raise or lower".to_string();
        return;
    }
    let drag = HeightDrag::begin(world, &corners, ws.grid.link);
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    if drag.apply(world, delta) {
        ws.history.record(before);
        ws.status = format!("Moved {} corners by {delta:+}", drag.len());
        ws.grid_changed();
    }
}
