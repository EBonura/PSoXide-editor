//! Duplicate (Cmd+D) and Extrude, the 2026-10-20 requests from Manny, plus
//! the two tool rules that came with them: a tool switch drops the selection
//! (Select itself keeps it) and Follow ceiling starts off.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, GridElement};

#[test]
fn choosing_any_tool_but_select_drops_the_selection() {
    for tool in GridTool::ALL {
        if tool == GridTool::Select {
            continue;
        }
        let mut rig = GridRig::persp("dupext-clear", flat_project(4, 4, true));
        rig.tool(GridTool::Select);
        rig.ws
            .grid
            .selection
            .replace(GridElement::Face(FaceRef::floor(0, [1, 1])));
        rig.ws.grid.mesh_sel = Some(7);
        rig.tool(tool);
        assert!(
            rig.ws.grid.selection.is_empty(),
            "{tool:?} kept the face selection"
        );
        assert!(
            rig.ws.grid.plane_sel.is_empty() && rig.ws.grid.mesh_sel.is_none(),
            "{tool:?} kept the plane or mesh selection"
        );
    }
}

#[test]
fn going_back_to_select_keeps_what_is_selected() {
    let mut rig = GridRig::persp("dupext-keep", flat_project(4, 4, true));
    rig.tool(GridTool::Floor);
    rig.ws
        .grid
        .selection
        .replace(GridElement::Face(FaceRef::floor(0, [1, 1])));
    rig.tool(GridTool::Select);
    assert_eq!(rig.ws.grid.selection.len(), 1);
}

#[test]
fn follow_ceiling_starts_off() {
    let rig = GridRig::persp("dupext-follow", flat_project(4, 4, true));
    assert!(!rig.ws.grid.verb.follow_ceiling);
}
