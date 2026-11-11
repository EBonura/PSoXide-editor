//! Phase 1 of the UX redesign, driven as state x input sequences.

use super::grid_rig::*;
use crate::grid_tools::GridTool;

const TOOLS: [GridTool; 6] = [
    GridTool::Height,
    GridTool::Paint,
    GridTool::Floor,
    GridTool::Wall,
    GridTool::Extrude,
    GridTool::Select,
];

#[test]
fn the_selection_survives_every_tool_switch() {
    let mut rig = GridRig::top("ux-select-survives", flat_project(4, 4, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    let picked = rig.ws.grid.selection.len();
    assert_eq!(picked, 1);
    for tool in TOOLS {
        rig.tool(tool);
        assert_eq!(
            rig.ws.grid.selection.len(),
            picked,
            "{} dropped the selection",
            tool.label()
        );
    }
}

fn esc_cancels_with_nothing_left(tool: GridTool, from: (i32, i32), to: (i32, i32)) {
    let mut rig = GridRig::top("ux-esc", flat_project(4, 4, false));
    if tool == GridTool::Paint {
        let material = rig.ws.project.add_resource(
            "brick",
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        );
        rig.ws.grid.material = Some(material);
    }
    rig.tool(tool);
    let before = rig.ws.project.clone();
    let can_undo = rig.ws.history.can_undo();
    rig.drag_hold(
        rig.cell_centre(from.0, from.1),
        rig.cell_centre(to.0, to.1),
        egui::Modifiers::NONE,
    );
    assert!(
        rig.ws.project != before,
        "{}: the stroke did nothing",
        tool.label()
    );
    rig.key(egui::Key::Escape);
    rig.release(rig.cell_centre(to.0, to.1), egui::Modifiers::NONE);
    assert!(
        rig.ws.project == before,
        "{}: the document changed",
        tool.label()
    );
    assert_eq!(
        rig.ws.history.can_undo(),
        can_undo,
        "{}: undo step left",
        tool.label()
    );
    assert!(
        !rig.ws.history.can_redo(),
        "{}: redo step left",
        tool.label()
    );
}

#[test]
fn esc_cancels_a_paint_stroke_and_restores_the_document() {
    esc_cancels_with_nothing_left(GridTool::Paint, (0, 0), (3, 0));
}

#[test]
fn esc_cancels_a_cells_stroke_and_restores_the_document() {
    esc_cancels_with_nothing_left(GridTool::Cells, (0, 0), (3, 3));
}

#[test]
fn a_cells_stroke_is_one_undo_step() {
    let mut rig = GridRig::top("ux-cells-one-step", flat_project(4, 4, false));
    let before = rig.ws.project.clone();
    rig.tool(GridTool::Cells);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(3, 3));
    assert!(rig.ws.project != before);
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}
