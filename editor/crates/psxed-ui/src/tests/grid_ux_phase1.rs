//! Phase 1 of the UX redesign, driven as state x input sequences.

use super::grid_rig::*;
use super::*;
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
