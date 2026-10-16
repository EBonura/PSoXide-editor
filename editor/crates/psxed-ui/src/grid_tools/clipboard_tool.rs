//! Copy, the paste ghost and delete, shared by every tool.
//!
//! Ctrl+C copies the selected faces. Ctrl+V starts a ghost that follows the
//! pointer over the grid; H and V mirror it, R turns it a quarter, a click
//! drops it (Shift keeps the ghost for stamping again) and Esc lets go.

use super::view::{cell_at_editor, editor_y};
use super::*;

use psxed_project::grid::edit::{clear_cells, delete_faces, GridClipboard, GridElement, Transform};

/// Ctrl+C.
pub(super) fn copy(ws: &mut EditorWorkspace) {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let faces = ws.grid.selection.faces(world);
    match GridClipboard::capture(world, &faces) {
        Some(clip) => {
            ws.status = format!(
                "Copied {} faces ({}x{} sectors)",
                faces.len(),
                clip.size()[0],
                clip.size()[1]
            );
            ws.grid.clipboard = Some(clip);
        }
        None => ws.status = "Select faces to copy".to_string(),
    }
}

/// Ctrl+V.
pub(super) fn begin_paste(ws: &mut EditorWorkspace) {
    let Some(clip) = ws.grid.clipboard.clone() else {
        ws.status = "Nothing copied".to_string();
        return;
    };
    ws.grid.gesture = Gesture::Idle;
    ws.grid.paste = Some(PasteGhost { clip, at: None });
    ws.status = "Paste: click to place, H/V flip, R turn, Esc cancel".to_string();
}

/// Mirror or turn the ghost.
pub(super) fn transform_ghost(ws: &mut EditorWorkspace, t: Transform) {
    if let Some(ghost) = ws.grid.paste.as_mut() {
        ghost.clip.transform(t);
    }
}

/// Smallest corner of the clip for a pointer: its centre sits on the sector
/// under the pointer.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let room = ws.grid_target_room();
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
    let cell = match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            Some(cell_at_editor(world.scale, x, z))
        }
        GridView::Persp(rect) => ws
            .pick_face_3d(rect, ptr.pos)
            .map(|h| h.point)
            .or_else(|| {
                let (origin, dir) = ws.camera_ray_for_pointer(rect, ptr.pos)?;
                view::ray_plane_y(origin, dir, editor_y(world, y_offset, 0))
            })
            .map(|p| cell_at_editor(world.scale, p[0], p[2])),
    };
    let size = ws.grid.paste.as_ref().map_or([1, 1], |g| g.clip.size());
    if let Some(ghost) = ws.grid.paste.as_mut() {
        ghost.at = cell.map(|c| [c[0] - size[0] / 2, c[1] - size[1] / 2]);
    }
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    hover(ws, ptr);
    let Some(ghost) = ws.grid.paste.clone() else {
        return;
    };
    let Some(at) = ghost.at else { return };
    let room = ws.grid_target_room();
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let outcome = ghost.clip.paste(world, room, at);
    if let Some(refusal) = outcome.refused {
        ws.status = format!("Cannot paste there: {refusal:?}");
        return;
    }
    ws.history.record(before);
    ws.status = format!("Pasted {} faces", outcome.faces.len());
    ws.grid
        .selection
        .set(outcome.faces.into_iter().map(GridElement::Face));
    if !ptr.shift() {
        ws.grid.paste = None;
    }
    ws.after_grid_selection_changed();
    ws.grid_changed();
}

/// Delete or Backspace: remove the selected faces (a selected sector is
/// cleared entirely). Returns whether the selection held anything.
pub(super) fn delete_selection(ws: &mut EditorWorkspace) -> bool {
    if ws.grid.selection.is_empty() {
        return false;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    let cells: Vec<(usize, [i32; 2])> = ws
        .grid
        .selection
        .items()
        .iter()
        .filter_map(|e| match e {
            GridElement::Cell { room, cell } => Some((*room, *cell)),
            _ => None,
        })
        .collect();
    let faces = ws.grid.selection.faces(world);
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let mut removed = delete_faces(world, &faces);
    removed += usize::from(clear_cells(world, &cells) > 0);
    ws.grid.selection.clear();
    if removed > 0 {
        ws.history.record(before);
        ws.status = format!("Deleted {} faces", faces.len());
        ws.grid_changed();
    }
    true
}
