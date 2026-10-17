//! 2D-viewport tool dispatch.
//!
//! The 2D twin of [`super::tools3d::ViewportTool3d`]. `draw_viewport` turns the
//! frame's response into a [`ToolFrame2d`] and calls the active tool's event
//! methods in a fixed order (hover, press, drag, release, click, frame end).
//! Tools are stateless dispatch objects; in-flight state lives on the
//! workspace, so the same event stream can be fed from egui or from a test.
//!
//! A scene with a grid world gets the grid tools from `crate::grid_tools`;
//! a scene without one gets [`NodeTool2d`] (see [`tool_impl_2d`]).

use super::*;

/// Pre-resolved pointer context for one frame of 2D-viewport input.
#[derive(Clone, Copy)]
pub(crate) struct ToolFrame2d<'a> {
    /// Viewport screen rectangle.
    pub rect: egui::Rect,
    /// Screen/world mapping of the active orthographic view.
    pub transform: ViewportTransform,
    /// Whether the view is the Top view (the plan, where nodes live).
    pub top_view: bool,
    /// Pointer in view-plane world units (hover, else interact).
    pub pointer_world: Option<[f32; 2]>,
    /// Pointer while a press or drag is active.
    pub pointer_interact: Option<egui::Pos2>,
    /// Where the primary button went down, if it is down.
    pub press_origin: Option<egui::Pos2>,
    /// Keyboard modifiers this frame.
    pub modifiers: egui::Modifiers,
    /// Scene nodes under the pointer, from the scene overlay pass.
    pub hits: &'a [ViewportHit],
    /// Pointer is over the viewport.
    pub hovered: bool,
    /// A drag-and-drop payload is in flight; tools stand down.
    pub dnd_active: bool,
    /// Primary button drag started this frame.
    pub drag_started: bool,
    /// Primary button is dragging.
    pub dragged: bool,
    /// Primary button drag ended this frame.
    pub drag_stopped: bool,
    /// Primary click (no drag) this frame.
    pub clicked: bool,
}

/// One 2D-viewport tool. Default bodies are no-ops so a tool implements only
/// the events it consumes.
pub(crate) trait ViewportTool2d {
    /// Pointer hover, cursor and keyboard handling; runs every frame.
    fn hover(&self, _ws: &mut EditorWorkspace, _ui: &egui::Ui, _frame: &ToolFrame2d) {}
    /// Primary button drag started.
    fn primary_pressed(&self, _ws: &mut EditorWorkspace, _ui: &egui::Ui, _frame: &ToolFrame2d) {}
    /// Primary button held and moving.
    fn primary_dragged(&self, _ws: &mut EditorWorkspace, _ui: &egui::Ui, _frame: &ToolFrame2d) {}
    /// Primary button drag finished.
    fn primary_released(&self, _ws: &mut EditorWorkspace, _ui: &egui::Ui, _frame: &ToolFrame2d) {}
    /// Primary click (press and release without a drag).
    fn primary_clicked(&self, _ws: &mut EditorWorkspace, _ui: &egui::Ui, _frame: &ToolFrame2d) {}
    /// After everything else this frame.
    fn frame_end(&self, _ws: &mut EditorWorkspace, _frame: &ToolFrame2d) {}
}

/// The tool that handles 2D input for the current project and tool.
pub(crate) fn tool_impl_2d(ws: &EditorWorkspace) -> &'static dyn ViewportTool2d {
    if ws.is_grid_world() {
        crate::grid_tools::grid_tool_2d()
    } else {
        &NodeTool2d
    }
}

/// Node selection and dragging in a scene that has no grid world.
pub(crate) struct NodeTool2d;

impl ViewportTool2d for NodeTool2d {
    fn primary_dragged(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active {
            return;
        }
        if f.top_view {
            ws.drag_selected_node(ui.input(|input| input.pointer.delta()));
        }
    }

    fn primary_clicked(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active {
            return;
        }
        if let Some(pos) = f.pointer_interact {
            let world = f.transform.screen_to_world(pos);
            let modifiers = ui.input(|input| input.modifiers);
            ws.handle_viewport_click(world, f.hits, modifiers);
        }
    }

    fn frame_end(&self, ws: &mut EditorWorkspace, f: &ToolFrame2d) {
        if !f.dragged {
            ws.end_node_drag_2d();
        }
    }
}
