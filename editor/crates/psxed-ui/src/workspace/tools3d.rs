//! 3D-viewport tool dispatch.
//!
//! Each tool is a stateless dispatch object; all in-flight state stays on
//! the workspace (`Interaction`, selection, previews), so tool switches can
//! never orphan private state. egui stays outside: `draw_viewport_3d_body`
//! translates the frame's response into `ToolFrame3d` + event calls, which
//! keeps every tool drivable headlessly through the ViewportHarness.

use super::*;

/// Pre-resolved pointer context for one frame of 3D-viewport input. All
/// fields are `Copy` snapshots so tools take `&mut EditorWorkspace` freely.
#[derive(Clone, Copy)]
pub(crate) struct ToolFrame3d {
    /// Viewport screen rectangle.
    pub rect: egui::Rect,
    /// Pointer position while a press/drag interaction is active.
    pub pointer_interact: Option<egui::Pos2>,
    /// Pointer hover position (may exist without an interaction).
    pub pointer_hover: Option<egui::Pos2>,
    /// Keyboard modifiers this frame.
    pub modifiers: egui::Modifiers,
    /// Topmost pick under the pointer (gizmo > entity).
    pub pointer_target: Option<Viewport3dPointerTarget>,
}

/// One 3D-viewport tool. Default bodies are no-ops so a tool implements
/// only the events it consumes.
pub(crate) trait ViewportTool3d {
    /// Primary button drag started.
    fn primary_pressed(&self, _ws: &mut EditorWorkspace, _frame: &ToolFrame3d) {}
    /// Primary button held and moving.
    fn primary_dragged(&self, _ws: &mut EditorWorkspace, _frame: &ToolFrame3d) {}
    /// Primary button drag finished.
    fn primary_released(&self, _ws: &mut EditorWorkspace, _frame: &ToolFrame3d) {}
    /// Primary click (press and release without a drag).
    fn primary_clicked(&self, _ws: &mut EditorWorkspace, _frame: &ToolFrame3d) {}
}

/// Node and entity selection and the node gizmo: the 3D tool of
/// a scene that has no grid world, and the base of every grid tool when the
/// pointer is over a node.
pub(crate) struct SelectTool;

impl ViewportTool3d for SelectTool {
    fn primary_pressed(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        let Some(pointer) = frame.pointer_interact else {
            return;
        };
        match frame.pointer_target {
            Some(Viewport3dPointerTarget::NodeGizmo(handle)) => {
                ws.begin_node_gizmo_handle_drag(handle, frame.rect, pointer);
            }
            Some(Viewport3dPointerTarget::Entity(_)) => {}
            None => {}
        }
    }

    fn primary_dragged(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        if let Interaction::NodeGizmo(_) = ws.interaction {
            if let Some(p) = frame.pointer_interact {
                ws.update_node_gizmo_drag(frame.rect, p, frame.modifiers.shift);
            }
        }
    }

    fn primary_released(&self, ws: &mut EditorWorkspace, _frame: &ToolFrame3d) {
        if let Interaction::NodeGizmo(_) = ws.interaction {
            ws.end_node_gizmo_drag();
        }
    }

    fn primary_clicked(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        match frame.pointer_target {
            Some(Viewport3dPointerTarget::Entity(hit)) => {
                ws.select_node_with_modifiers(hit.node, frame.modifiers);
            }
            // An additive click that resolves to nothing adds nothing. It
            // must not wipe the multi-selection the user is assembling, so
            // only a plain click on empty space clears.
            None if frame.modifiers.shift || frame.modifiers.command || frame.modifiers.ctrl => {}
            None => ws.clear_all_selections(),
            Some(Viewport3dPointerTarget::NodeGizmo(_)) => {}
        }
    }
}

/// The active tool's dispatch object for a scene without a grid world.
pub(crate) fn tool_impl_3d(_tool: ViewTool) -> &'static dyn ViewportTool3d {
    &SelectTool
}
