//! The Select tool.
//!
//! * Click picks the finest element under the pointer: vertex, then edge,
//!   then face in 3D; a wall line or a whole sector in the Top view.
//! * Shift-click extends from the last pick along a range (the rectangle of
//!   surfaces, the connected run of walls, the line of vertices, the
//!   rectangle of sectors). Ctrl or Cmd toggles. Clicking nothing clears.
//! * Dragging on nothing draws a marquee; Ctrl or Cmd adds to what is
//!   selected.
//! * Shift-drag on an element raises and lowers it (see `height_tool`).

use super::pick::PickLevel;
use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::{
    all_faces_of_room, cells_in_rect, faces_of_cell, range_corners, range_surfaces, range_walls,
    CellRect, FaceRef, FaceTarget, GridElement,
};
use psxed_project::grid::GridWorld;

impl EditorWorkspace {
    /// Bookkeeping after the grid selection changed: node selection stands
    /// down, the last room follows, the palette finds the face's material.
    pub(crate) fn after_grid_selection_changed(&mut self) {
        let Some(primary) = self.grid.selection.primary().copied() else {
            return;
        };
        let scroll_to = self.project.active_scene().world.grid().and_then(|world| {
            let face = self.grid.selection.faces(world).first().copied()?;
            psxed_project::grid::edit::material_of(world, face, None)
        });
        self.grid.last_room = primary.room();
        self.clear_node_selection_state();
        self.clear_brush_selection();
        if scroll_to.is_some() {
            self.grid.palette_scroll_to = scroll_to;
        }
    }
}

fn all_surface_cells(selection: &[GridElement], kind_of: FaceTarget) -> Vec<[i32; 2]> {
    selection
        .iter()
        .filter_map(|e| match e {
            GridElement::Face(f) if f.target == kind_of => Some(f.cell),
            _ => None,
        })
        .collect()
}

/// The elements a Shift-click at `picked` adds, given what is selected.
fn extend_range(
    world: &GridWorld,
    selected: &[GridElement],
    picked: GridElement,
) -> Vec<GridElement> {
    let Some(anchor) = selected.last().copied() else {
        return vec![picked];
    };
    match (anchor, picked) {
        (GridElement::Face(a), GridElement::Face(b)) if a.room == b.room => {
            match (a.target, b.target) {
                (FaceTarget::Surface(ka), FaceTarget::Surface(kb)) if ka == kb => {
                    let mut cells = all_surface_cells(selected, a.target);
                    cells.push(a.cell);
                    range_surfaces(world, a.room, ka, &cells, b.cell)
                        .into_iter()
                        .map(GridElement::Face)
                        .collect()
                }
                (FaceTarget::Wall { .. }, FaceTarget::Wall { .. }) => range_walls(world, a, b)
                    .into_iter()
                    .map(GridElement::Face)
                    .collect(),
                _ => vec![picked],
            }
        }
        (GridElement::Vertex(a), GridElement::Vertex(b)) => range_corners(world, a, b)
            .into_iter()
            .map(GridElement::Vertex)
            .collect(),
        (GridElement::Cell { room: ra, cell: ca }, GridElement::Cell { room: rb, cell: cb })
            if ra == rb =>
        {
            cells_in_rect(world, ra, CellRect::from_corners(ca, cb))
        }
        _ => vec![picked],
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = match ws.grid_pick_for_select(ptr, PickLevel::Vertex) {
        Some(elements) => GridHover::Elements(elements),
        None => GridHover::None,
    };
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if ptr.shift() && height_tool::begin(ws, ptr) {
        return;
    }
    ws.grid.gesture = Gesture::Marquee {
        start: ptr.press.unwrap_or(ptr.pos),
        current: ptr.pos,
        additive: ptr.command() || ptr.shift(),
    };
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    match &mut ws.grid.gesture {
        Gesture::Marquee { current, .. } => *current = ptr.pos,
        Gesture::Height(_) => height_tool::drag(ws, ptr),
        _ => {}
    }
}

pub(super) fn release(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Marquee {
            start,
            current,
            additive,
        } => finish_marquee(ws, ptr, start, current, additive),
        Gesture::Height(g) => {
            ws.grid.gesture = Gesture::Height(g);
            height_tool::release(ws);
        }
        other => ws.grid.gesture = other,
    }
}

/// Select what a marquee from `start` to `end` encloses.
fn finish_marquee(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    start: Pos2,
    end: Pos2,
    additive: bool,
) {
    let rect = Rect::from_two_pos(start, end);
    let elements = marquee_elements(ws, ptr, rect);
    if !additive {
        ws.grid.selection.clear();
    }
    let count = elements.len();
    ws.grid.selection.extend(elements);
    ws.after_grid_selection_changed();
    ws.status = format!("{count} selected, {} in all", ws.grid.selection.len());
}

/// Elements enclosed by a screen rectangle.
pub(super) fn marquee_elements(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    rect: Rect,
) -> Vec<GridElement> {
    match ptr.view {
        GridView::Top(transform) => {
            let Some(world) = ws.project.active_scene().world.grid() else {
                return Vec::new();
            };
            let [ax, az] = transform.screen_to_world(rect.min);
            let [bx, bz] = transform.screen_to_world(rect.max);
            let a = cell_at_editor(world.scale, ax.min(bx), az.min(bz));
            let b = cell_at_editor(world.scale, ax.max(bx), az.max(bz));
            let cells = CellRect::from_corners(a, b);
            let mut out = Vec::new();
            for cell in cells.cells() {
                let Some(room) = ws.top_room_at(cell) else {
                    continue;
                };
                if !faces_of_cell(world, room, cell).is_empty() {
                    out.push(GridElement::Cell { room, cell });
                }
            }
            out
        }
        GridView::Persp(view) => {
            let cam = pick::Cam::new(ws.viewport_3d_camera(), view);
            let eye = ws.viewport_3d_camera().position_f32();
            ws.with_pick_cache(|cache| {
                let mut seen = std::collections::BTreeSet::new();
                for pf in &cache.faces {
                    if ws.grid_hidden_rooms.contains(&pf.face.room) {
                        continue;
                    }
                    let n = pf.points.len() as f32;
                    let centre = pf.points.iter().fold([0.0; 3], |acc, p| {
                        [acc[0] + p[0] / n, acc[1] + p[1] / n, acc[2] + p[2] / n]
                    });
                    let to_eye = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
                    let facing = pf.normal[0] * to_eye[0]
                        + pf.normal[1] * to_eye[1]
                        + pf.normal[2] * to_eye[2];
                    if facing <= 0.0 && !pf.double_sided {
                        continue;
                    }
                    if cam.project(centre).is_some_and(|(p, _)| rect.contains(p)) {
                        seen.insert(pf.face);
                    }
                }
                seen.into_iter().map(GridElement::Face).collect()
            })
            .unwrap_or_default()
        }
    }
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let picked = ws.grid_pick_for_select(ptr, PickLevel::Vertex);
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let Some(picked) = picked else {
        if !ptr.shift() && !ptr.command() {
            ws.grid.selection.clear();
        }
        return;
    };
    if ptr.command() {
        for element in picked {
            ws.grid.selection.toggle(element);
        }
    } else if ptr.shift() {
        let selected: Vec<GridElement> = ws.grid.selection.items().to_vec();
        let first = picked[0];
        let mut added = extend_range(world, &selected, first);
        added.extend(picked.iter().skip(1).copied());
        ws.grid.selection.extend(added);
        // The click becomes the new anchor.
        ws.grid.selection.add(first);
    } else {
        ws.grid.selection.set(picked);
    }
    ws.after_grid_selection_changed();
    ws.status = format!("{} selected", ws.grid.selection.len());
}

/// Select everything in the target room.
pub(super) fn select_all(ws: &mut EditorWorkspace) {
    let room = ws.grid_target_room();
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let faces: Vec<FaceRef> = all_faces_of_room(world, room);
    let count = faces.len();
    ws.grid
        .selection
        .set(faces.into_iter().map(GridElement::Face));
    ws.after_grid_selection_changed();
    ws.status = format!("Selected all {count} faces of room {room}");
}
