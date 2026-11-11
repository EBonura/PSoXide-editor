//! The context menu of an element: a right click that did not move.
//!
//! Right-drag stays the camera. egui opens the menu only for a click that
//! stayed put, so an orbit never opens it.

use super::target::{Hit, Target};
use super::wall_handle::WallHandle;
use super::*;

/// What the menu was opened over.
#[derive(Clone, Debug)]
pub(in crate::grid_tools) struct MenuTarget {
    hit: Hit,
    /// The spot on the ground the right click was over, editor units.
    spot: Option<[f32; 3]>,
}

/// Call every frame the viewport is drawn with the Edit tool: records what a
/// right click lands on and shows the menu.
pub(in crate::grid_tools) fn attach(
    ws: &mut EditorWorkspace,
    response: &egui::Response,
    view: GridView,
) {
    if ws.grid.tool != GridTool::Edit {
        return;
    }
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos().or(response.hover_pos()) {
            let ptr = GridPointer {
                view,
                pos,
                press: None,
                modifiers: egui::Modifiers::NONE,
                cycle: 0,
            };
            let hit = target::at(ws, &ptr);
            let spot = if hit.point != [0.0; 3] {
                Some(hit.point)
            } else {
                ws.grid_plane_point(&ptr, 0.0)
            };
            ws.grid.edit.menu = (hit.target != Target::Nothing).then_some(MenuTarget { hit, spot });
        }
    }
    if ws.grid.edit.menu.is_none() {
        return;
    }
    response.context_menu(|ui| {
        let Some(menu) = ws.grid.edit.menu.clone() else {
            ui.close_menu();
            return;
        };
        show(ws, ui, &menu.hit, menu.spot);
    });
}

fn show(ws: &mut EditorWorkspace, ui: &mut egui::Ui, hit: &Hit, spot: Option<[f32; 3]>) {
    if let Some(at) = spot {
        if ui.button("Play from here").clicked() {
            ws.play_from_here(at, 0.0);
            ui.close_menu();
        }
    }
    let is_wall = hit.target.wall().is_some();
    let label = if is_wall { "Select run" } else { "Select room" };
    if ui.button(label).clicked() {
        pick_select::select_group(ws, hit);
        ui.close_menu();
    }
    if ui.button("Select").clicked() {
        ws.grid.selection.set(hit.target.elements());
        ws.after_grid_selection_changed();
        ui.close_menu();
    }
    if ui.button("Frame").clicked() {
        ws.grid.selection.set(hit.target.elements());
        ws.after_grid_selection_changed();
        ws.frame_viewport();
        ui.close_menu();
    }
    if ui.button("Delete").clicked() {
        ws.grid.selection.set(hit.target.elements());
        clipboard_tool::delete_selection(ws);
        ui.close_menu();
    }
    if let Some(face) = hit.target.wall() {
        if ui.button("Reset wall to automatic").clicked() {
            reset_wall(ws, face);
            ui.close_menu();
        }
    }
}

/// Put a wall back to what the cells give it: one undo step.
pub(crate) fn reset_wall(ws: &mut EditorWorkspace, face: FaceRef) {
    let before = ws.project.clone();
    let handle = WallHandle::of(face);
    let result = match (handle, ws.project.active_scene_mut().world.grid_mut()) {
        (Some(h), Some(world)) => h.reset_to_automatic(world),
        _ => Err("No wall".to_string()),
    };
    match result {
        Ok(()) if ws.project != before => {
            ws.history.record(before);
            ws.grid_changed();
            ws.status = "Wall is automatic again".to_string();
        }
        Ok(()) => ws.status = "That wall is already automatic".to_string(),
        Err(reason) => ws.status = reason,
    }
}
