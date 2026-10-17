//! Routing pointer events from both views to the grid tools.
//!
//! [`GridTool2d`] and [`GridTool3d`] are the adapters the viewports call (see
//! `workspace/tools2d.rs` and `workspace/viewport.rs`). Both build a
//! [`GridPointer`] and send it to [`dispatch`], which hands it to the tool
//! the strip has selected. A paste ghost, when there is one, takes every
//! event first. The Select tool lets nodes (lights, spawns, actors) keep
//! their own gizmos and clicks: a press on one is handed to the node tools.

use super::*;

use crate::workspace::tools2d::{ToolFrame2d, ViewportTool2d};
use crate::workspace::tools3d::{SelectTool, ToolFrame3d, ViewportTool3d};

use super::{
    bands_tool, clipboard_tool, face_tools, height_tool, place_tool, plane_tool, rect_tool,
    select_tool, verb_tool, wall_tool,
};

/// A pointer event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ev {
    Hover,
    Press,
    Drag,
    Release,
    Click,
}

/// Send one event to the active tool.
pub(crate) fn dispatch(ws: &mut EditorWorkspace, ev: Ev, mut ptr: GridPointer) {
    ptr.cycle = ws.cycle_for(ev, &ptr);
    if ws.grid.paste.is_some() {
        match ev {
            Ev::Hover | Ev::Drag => clipboard_tool::hover(ws, &ptr),
            Ev::Click => clipboard_tool::click(ws, &ptr),
            Ev::Press | Ev::Release => {}
        }
        return;
    }
    if ws.grid.plane_paste.is_some() {
        match ev {
            Ev::Hover | Ev::Drag => plane_tool::paste_hover(ws, &ptr),
            Ev::Click => plane_tool::paste_click(ws, &ptr),
            Ev::Press | Ev::Release => {}
        }
        return;
    }
    let taken = match ev {
        Ev::Hover => plane_tool::hover(ws, &ptr),
        Ev::Press => plane_tool::press(ws, &ptr),
        Ev::Drag => plane_tool::drag(ws, &ptr),
        Ev::Release => plane_tool::release(ws, &ptr),
        Ev::Click => plane_tool::click(ws, &ptr),
    };
    if taken {
        return;
    }
    match (ws.grid.tool, ev) {
        (GridTool::Plane, _) => {}
        (GridTool::Select, Ev::Hover) => select_tool::hover(ws, &ptr),
        (GridTool::Select, Ev::Press) => select_tool::press(ws, &ptr),
        (GridTool::Select, Ev::Drag) => select_tool::drag(ws, &ptr),
        (GridTool::Select, Ev::Release) => select_tool::release(ws, &ptr),
        (GridTool::Select, Ev::Click) => select_tool::click(ws, &ptr),

        (GridTool::Floor | GridTool::Ceiling, Ev::Hover) => {
            rect_tool::hover(ws, &ptr, surface_of(ws.grid.tool))
        }
        (GridTool::Floor | GridTool::Ceiling, Ev::Press) => {
            rect_tool::press(ws, &ptr, surface_of(ws.grid.tool))
        }
        (GridTool::Floor | GridTool::Ceiling, Ev::Drag) => rect_tool::drag(ws, &ptr),
        (GridTool::Floor | GridTool::Ceiling, Ev::Release) => rect_tool::release(ws),
        (GridTool::Floor | GridTool::Ceiling, Ev::Click) => {
            rect_tool::click(ws, &ptr, surface_of(ws.grid.tool))
        }

        (GridTool::Wall, Ev::Hover) => wall_tool::hover(ws, &ptr),
        (GridTool::Wall, Ev::Press) => wall_tool::press(ws, &ptr),
        (GridTool::Wall, Ev::Drag) => wall_tool::drag(ws, &ptr),
        (GridTool::Wall, Ev::Release) => wall_tool::release(ws),
        (GridTool::Wall, Ev::Click) => wall_tool::click(ws, &ptr),

        (GridTool::Height, Ev::Hover) => {
            ws.grid.hover = match height_tool::element_at(ws, &ptr) {
                Some(element) => GridHover::Elements(vec![element]),
                None => GridHover::None,
            };
        }
        (GridTool::Height, Ev::Press) => {
            height_tool::begin(ws, &ptr);
        }
        (GridTool::Height, Ev::Drag) => height_tool::drag(ws, &ptr),
        (GridTool::Height, Ev::Release) => height_tool::release(ws),
        (GridTool::Height, Ev::Click) => select_tool::click(ws, &ptr),

        (GridTool::Paint, Ev::Hover) | (GridTool::Split, Ev::Hover) => face_tools::hover(ws, &ptr),
        (GridTool::Paint, Ev::Press) => face_tools::paint_press(ws, &ptr),
        (GridTool::Paint, Ev::Drag) => face_tools::paint_drag(ws, &ptr),
        (GridTool::Paint, Ev::Release) => face_tools::paint_release(ws),
        (GridTool::Paint, Ev::Click) => face_tools::paint_click(ws, &ptr),
        (GridTool::Split, Ev::Click) => face_tools::split_click(ws, &ptr),
        (GridTool::Split, _) => {}

        (GridTool::Place, Ev::Hover) => place_tool::hover(ws, &ptr),
        (GridTool::Place, Ev::Click) => place_tool::click(ws, &ptr),
        (GridTool::Place, _) => {}

        (GridTool::Bands, Ev::Hover) => bands_tool::hover(ws, &ptr),
        (GridTool::Bands, Ev::Press) => bands_tool::press(ws, &ptr),
        (GridTool::Bands, Ev::Drag) => bands_tool::drag(ws, &ptr),
        (GridTool::Bands, Ev::Release) => bands_tool::release(ws),
        (GridTool::Bands, Ev::Click) => bands_tool::click(ws, &ptr),

        (GridTool::Raise | GridTool::Ramp | GridTool::Stairs, Ev::Hover) => {
            verb_tool::hover(ws, &ptr)
        }
        (GridTool::Raise | GridTool::Ramp | GridTool::Stairs, Ev::Press) => {
            verb_tool::press(ws, &ptr)
        }
        (GridTool::Raise | GridTool::Ramp | GridTool::Stairs, Ev::Drag) => {
            verb_tool::drag(ws, &ptr)
        }
        (GridTool::Raise | GridTool::Ramp | GridTool::Stairs, Ev::Release) => {
            verb_tool::release(ws)
        }
        (GridTool::Raise | GridTool::Ramp | GridTool::Stairs, Ev::Click) => {
            verb_tool::click(ws, &ptr)
        }
    }
}

fn surface_of(tool: GridTool) -> SurfaceKind {
    if tool == GridTool::Ceiling {
        SurfaceKind::Ceiling
    } else {
        SurfaceKind::Floor
    }
}

/// The 2D adapter.
pub(crate) struct GridTool2d;

static GRID_TOOL_2D: GridTool2d = GridTool2d;
static GRID_TOOL_3D: GridTool3d = GridTool3d;

/// The 2D tool for grid worlds.
pub(crate) fn grid_tool_2d() -> &'static dyn ViewportTool2d {
    &GRID_TOOL_2D
}

/// The 3D tool for grid worlds.
pub(crate) fn grid_tool_3d() -> &'static dyn ViewportTool3d {
    &GRID_TOOL_3D
}

impl GridTool2d {
    fn pointer(f: &ToolFrame2d, pos: Pos2, press: Option<Pos2>) -> GridPointer {
        GridPointer {
            view: GridView::Top(f.transform),
            pos,
            press,
            modifiers: f.modifiers,
            cycle: 0,
        }
    }

    /// Only the Top view shows the plan; Front and Side are read-only here.
    fn editable(f: &ToolFrame2d) -> bool {
        f.top_view && !f.dnd_active
    }
}

impl ViewportTool2d for GridTool2d {
    fn hover(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if !Self::editable(f) {
            return;
        }
        if f.hovered {
            ui.ctx().set_cursor_icon(ws.grid_cursor());
        }
        if matches!(ws.grid.gesture, Gesture::Idle | Gesture::Node) && f.hovered {
            if let Some(pos) = f
                .pointer_interact
                .or_else(|| f.pointer_world.map(|w| f.transform.world_to_screen(w)))
            {
                ws.grid.hover_pinned = false;
                dispatch(ws, Ev::Hover, Self::pointer(f, pos, None));
            }
        } else if !f.hovered && !ws.grid.hover_pinned {
            ws.grid.hover = GridHover::None;
        }
    }

    fn primary_pressed(&self, ws: &mut EditorWorkspace, _ui: &egui::Ui, f: &ToolFrame2d) {
        if !Self::editable(f) {
            return;
        }
        let Some(pos) = f.press_origin.or(f.pointer_interact) else {
            return;
        };
        if ws.grid.tool == GridTool::Select && ws.grid.paste.is_none() {
            let world = f.transform.screen_to_world(pos);
            let on_selected_node = f
                .hits
                .iter()
                .any(|h| h.id == ws.selection.selected_node && h.contains(world));
            if on_selected_node {
                ws.grid.gesture = Gesture::Node;
                return;
            }
        }
        dispatch(ws, Ev::Press, Self::pointer(f, pos, Some(pos)));
    }

    fn primary_dragged(&self, ws: &mut EditorWorkspace, ui: &egui::Ui, f: &ToolFrame2d) {
        if !Self::editable(f) {
            return;
        }
        if matches!(ws.grid.gesture, Gesture::Node) {
            ws.drag_selected_node(ui.input(|i| i.pointer.delta()));
            return;
        }
        if let Some(pos) = f.pointer_interact {
            dispatch(ws, Ev::Drag, Self::pointer(f, pos, None));
        }
    }

    fn primary_released(&self, ws: &mut EditorWorkspace, _ui: &egui::Ui, f: &ToolFrame2d) {
        if !Self::editable(f) {
            return;
        }
        if matches!(ws.grid.gesture, Gesture::Node) {
            ws.grid.gesture = Gesture::Idle;
            return;
        }
        let pos = f.pointer_interact.unwrap_or(f.rect.center());
        dispatch(ws, Ev::Release, Self::pointer(f, pos, None));
    }

    fn primary_clicked(&self, ws: &mut EditorWorkspace, _ui: &egui::Ui, f: &ToolFrame2d) {
        if !Self::editable(f) {
            return;
        }
        let Some(pos) = f.pointer_interact else {
            return;
        };
        let world = f.transform.screen_to_world(pos);
        let node_under = f.hits.iter().any(|h| h.contains(world));
        if ws.grid.tool == GridTool::Select && ws.grid.paste.is_none() && node_under {
            // A node under the pointer: the ordinary node click.
            ws.grid.selection.clear();
            ws.handle_viewport_click(world, f.hits, f.modifiers);
            return;
        }
        dispatch(ws, Ev::Click, Self::pointer(f, pos, None));
    }

    fn frame_end(&self, ws: &mut EditorWorkspace, f: &ToolFrame2d) {
        if !f.dragged {
            ws.end_node_drag_2d();
        }
    }
}

/// The 3D adapter.
pub(crate) struct GridTool3d;

impl GridTool3d {
    fn pointer(
        ws: &EditorWorkspace,
        f: &ToolFrame3d,
        pos: Pos2,
        press: Option<Pos2>,
    ) -> GridPointer {
        let _ = ws;
        GridPointer {
            view: GridView::Persp(f.rect),
            pos,
            press,
            modifiers: f.modifiers,
            cycle: 0,
        }
    }

    fn node_target(ws: &EditorWorkspace, f: &ToolFrame3d) -> bool {
        ws.grid.tool == GridTool::Select
            && ws.grid.paste.is_none()
            && matches!(
                f.pointer_target,
                Some(Viewport3dPointerTarget::NodeGizmo(_) | Viewport3dPointerTarget::Entity(_))
            )
    }
}

impl ViewportTool3d for GridTool3d {
    fn primary_pressed(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        let Some(pos) = frame.pointer_interact else {
            return;
        };
        if Self::node_target(ws, frame) {
            ws.grid.gesture = Gesture::Node;
            SelectTool.primary_pressed(ws, frame);
            return;
        }
        dispatch(ws, Ev::Press, Self::pointer(ws, frame, pos, Some(pos)));
    }

    fn primary_dragged(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        if matches!(ws.grid.gesture, Gesture::Node) {
            SelectTool.primary_dragged(ws, frame);
            return;
        }
        if let Some(pos) = frame.pointer_interact {
            dispatch(ws, Ev::Drag, Self::pointer(ws, frame, pos, None));
        }
    }

    fn primary_released(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        if matches!(ws.grid.gesture, Gesture::Node) {
            SelectTool.primary_released(ws, frame);
            ws.grid.gesture = Gesture::Idle;
            return;
        }
        let pos = frame.pointer_interact.unwrap_or(frame.rect.center());
        dispatch(ws, Ev::Release, Self::pointer(ws, frame, pos, None));
    }

    fn primary_clicked(&self, ws: &mut EditorWorkspace, frame: &ToolFrame3d) {
        let Some(pos) = frame.pointer_interact else {
            return;
        };
        if Self::node_target(ws, frame) {
            ws.grid.selection.clear();
            SelectTool.primary_clicked(ws, frame);
            return;
        }
        dispatch(ws, Ev::Click, Self::pointer(ws, frame, pos, None));
    }
}

/// Hover in the 3D view, called every frame the pointer is over it.
pub(crate) fn hover_3d(
    ws: &mut EditorWorkspace,
    ctx: &egui::Context,
    rect: Rect,
    pos: Option<Pos2>,
    modifiers: egui::Modifiers,
) {
    let Some(pos) = pos else {
        if !ws.grid.hover_pinned {
            ws.grid.hover = GridHover::None;
        }
        return;
    };
    ws.grid.hover_pinned = false;
    if !matches!(ws.grid.gesture, Gesture::Idle) {
        return;
    }
    let ptr = GridPointer {
        view: GridView::Persp(rect),
        pos,
        press: None,
        modifiers,
        cycle: 0,
    };
    dispatch(ws, Ev::Hover, ptr);
    ctx.set_cursor_icon(ws.grid_cursor());
}
