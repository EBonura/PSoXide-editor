//! 2D-viewport tool dispatch (the Top, Front and Side views).
//!
//! The 2D twin of [`super::tools::ViewportTool3d`]. `draw_viewport` turns the
//! frame's response into a [`ToolFrame2d`] and calls the active tool's event
//! methods in a fixed order (hover, press, drag, release, click, frame end).
//! Tools are stateless dispatch objects; in-flight state lives on the
//! workspace, so the same event stream can be fed from egui or from a test.
//!
//! [`BrushTool2d`] holds the behaviour that used to be an inline `if` ladder
//! in `draw_viewport`, unchanged, for brush projects. Grid projects get the
//! grid tools from `crate::grid_tools` instead (see [`tool_impl_2d`]).

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
    /// The click was a double click.
    pub double_clicked: bool,
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
        &BrushTool2d
    }
}

/// Selection, brush drawing and brush element editing in a brush world.
pub(crate) struct BrushTool2d;

impl BrushTool2d {
    fn brush_edit_active(ws: &EditorWorkspace) -> bool {
        matches!(ws.active_tool, ViewTool::Brush)
            || (matches!(ws.active_tool, ViewTool::Select) && ws.selected_brush.is_some())
    }

    fn brush_gesture_in_flight(ws: &EditorWorkspace) -> bool {
        ws.brush_move.is_some() || ws.brush_vertex_drag.is_some() || ws.brush_extrude.is_some()
    }
}

impl ViewportTool2d for BrushTool2d {
    fn hover(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active || !Self::brush_edit_active(ws) {
            return;
        }
        ws.selection.hovered_brush_handle = None;
        if f.hovered {
            if let Some(world) = f.pointer_world {
                let tolerance = 8.0 / ws.viewport_zoom.max(f32::EPSILON);
                ws.selection.hovered_brush_handle =
                    ws.pick_brush_elements_2d(world, tolerance).first().copied();
            }
        }
        if f.hovered && matches!(ws.active_tool, ViewTool::Brush) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if f.hovered
            && f.pointer_world.is_some_and(|world| {
                ws.brush_edit_mode != BrushEditMode::Move
                    || ws.pick_brush_face_for_move_at_2d(world).is_some()
            })
        {
            ui.ctx().set_cursor_icon(match ws.brush_edit_mode {
                BrushEditMode::Move => egui::CursorIcon::Grab,
                BrushEditMode::Face => egui::CursorIcon::ResizeHorizontal,
                BrushEditMode::Edge | BrushEditMode::Vertex | BrushEditMode::Clip => {
                    egui::CursorIcon::Crosshair
                }
            });
        }
        if f.hovered
            && ws
                .brush_drag
                .is_some_and(|drag| drag.stage == BrushCreateStage::Height)
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if f.hovered {
            ws.brush_tool_keyboard(ui);
        }
    }

    fn primary_pressed(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active {
            return;
        }
        if Self::brush_edit_active(ws) {
            let additive_select = matches!(ws.active_tool, ViewTool::Select)
                && (f.modifiers.shift || f.modifiers.command || f.modifiers.ctrl);
            if !additive_select {
                if let Some(pos) = f.press_origin.or(f.pointer_interact) {
                    if !ws.begin_brush_height_drag(pos.y) {
                        let world = f.transform.screen_to_world(pos);
                        let tolerance = 8.0 / ws.viewport_zoom;
                        if matches!(ws.active_tool, ViewTool::Brush) {
                            ws.begin_brush_drag_2d(world);
                        } else if ws.brush_edit_mode == BrushEditMode::Move {
                            ws.begin_brush_move_2d(world);
                        } else {
                            match ws.brush_edit_mode {
                                BrushEditMode::Move => unreachable!(),
                                // Clip is click-driven; a drag starts nothing.
                                BrushEditMode::Clip => false,
                                BrushEditMode::Face => ws.begin_brush_resize_2d(world, tolerance),
                                BrushEditMode::Edge => {
                                    ws.begin_brush_edge_drag_2d(world, tolerance)
                                }
                                BrushEditMode::Vertex => {
                                    ws.begin_brush_vertex_drag_2d(world, tolerance)
                                }
                            };
                        }
                    }
                }
            }
        }
        // Marquee select: a Select-tool drag that no brush gesture claimed.
        let bsp_brush_marquee = !ws.project.active_scene().brushes.is_empty();
        if matches!(ws.active_tool, ViewTool::Select)
            && !Self::brush_gesture_in_flight(ws)
            && bsp_brush_marquee
        {
            if let Some(start) = f.press_origin.or(f.pointer_interact) {
                let modifiers = ui.input(|input| input.modifiers);
                ws.begin_viewport_box_select(start, modifiers);
            }
        }
    }

    fn primary_dragged(&self, ws: &mut EditorWorkspace, _ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active {
            return;
        }
        if Self::brush_edit_active(ws) {
            if let Some(pos) = f.pointer_interact {
                if ws.brush_drag.is_some_and(|drag| {
                    drag.stage == BrushCreateStage::Height && drag.height_dragging
                }) {
                    ws.update_brush_height_drag(pos.y);
                } else {
                    let world = f.transform.screen_to_world(pos);
                    if ws.brush_move.is_some() {
                        ws.update_brush_move_2d(world);
                    } else if ws.brush_vertex_drag.is_some() {
                        ws.update_brush_vertex_drag_2d(world);
                    } else if ws.brush_extrude.is_some() {
                        ws.update_brush_resize_2d(world);
                    } else {
                        ws.update_brush_drag_2d(world);
                    }
                }
            }
        }
        if matches!(ws.active_tool, ViewTool::Select) {
            if let Some(current) = f.pointer_interact {
                ws.update_viewport_box_select(current, f.transform);
            }
        }
        if f.top_view
            && ws.interaction.box_select_2d().is_none()
            && !Self::brush_gesture_in_flight(ws)
            && ws.brush_drag.is_none()
        {
            ws.drag_selected_node(ui_pointer_delta(_ui));
        }
    }

    fn primary_released(&self, ws: &mut EditorWorkspace, _ui: &egui::Ui, f: &ToolFrame2d) {
        if !f.dnd_active && Self::brush_edit_active(ws) {
            ws.commit_brush_gesture_2d();
        }
    }

    fn primary_clicked(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if f.dnd_active {
            return;
        }
        if ws
            .brush_drag
            .is_some_and(|drag| drag.stage == BrushCreateStage::Height)
        {
            ws.commit_brush_drag();
        } else if matches!(ws.active_tool, ViewTool::Brush) {
            // Draw owns the pointer exclusively. Selection and sub-element
            // editing belong to Select mode.
        } else if let Some(pos) = f.pointer_interact {
            let world = f.transform.screen_to_world(pos);
            let modifiers = ui.input(|input| input.modifiers);
            let group_double_click =
                f.double_clicked && ws.handle_group_double_click_2d(world, f.hits);
            if !group_double_click {
                ws.handle_viewport_click(world, f.hits, modifiers);
            }
        }
    }

    fn frame_end(&self, ws: &mut EditorWorkspace, f: &ToolFrame2d) {
        if !f.dragged {
            ws.end_node_drag_2d();
        }
    }
}

fn ui_pointer_delta(ui: &egui::Ui) -> Vec2 {
    ui.input(|input| input.pointer.delta())
}
