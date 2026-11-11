//! The Library drawer: open it with its button, open each tab, pick an item,
//! and place it, in every view.

use super::grid_toolbar::frames;
use super::tool_audit::*;
use super::tool_flows::*;
use super::tool_matrix::{lib_open, lib_row, lib_row_nth, lib_swatch};
use super::*;
use crate::grid_tools::GridTool;

fn rig_for(v: View) -> super::grid_live::WindowRig {
    let mut rig = v.rig(Fixture::Starter);
    v.aim(&mut rig, [-1, -3], [22, 8]);
    rig
}

/// (tab, view, result) for each tab of the drawer in each view.
pub(super) fn cells() -> Vec<(&'static str, View, Result<(), String>)> {
    let mut out = Vec::new();
    for v in View::ALL {
        let mut check = |what: &'static str, r: Result<(), String>| out.push((what, v, r));
        // Materials: a swatch arms Paint, the next face clicked takes it.
        check("Materials", (|| {
            let mut rig = rig_for(v);
            if !lib_open(&mut rig, "Materials") {
                return Err("the drawer did not open".into());
            }
            let mut painted = false;
            for n in 1..6 {
                let _ = start(&mut rig, GridTool::Select, Start::Strip);
                if !lib_swatch(&mut rig, n) || rig.ws.grid.tool != GridTool::Paint {
                    continue;
                }
                let before = rig.ws.project.clone();
                v.click_cell(&mut rig, [2, 2]);
                if rig.ws.project != before {
                    painted = true;
                    break;
                }
            }
            if painted { Ok(()) } else { Err("no swatch armed Paint and painted".into()) }
        })());
        // Meshes: an arch made first, then the Meshes tab arms Mesh.
        check("Meshes", (|| {
            let mut rig = rig_for(v);
            if !start(&mut rig, GridTool::Arcade, Start::Strip) {
                return Err("Arcade".into());
            }
            let (a, b) = (v.cell(&rig, [1, 2]), v.cell(&rig, [4, 2]));
            rig.drag(a, b);
            if !lib_open(&mut rig, "Meshes") || !lib_row(&mut rig, crate::icons::CUBOID) {
                return Err("no mesh row".into());
            }
            if rig.ws.grid.tool != GridTool::Mesh {
                return Err("the row did not arm Mesh".into());
            }
            let before = rig.ws.project.clone();
            v.click_cell(&mut rig, [2, 4]);
            if rig.ws.project != before { Ok(()) } else { Err(format!("nothing placed: {}", rig.ws.status)) }
        })());
        // Actors: a row arms Place with that actor.
        check("Actors", (|| {
            let mut rig = rig_for(v);
            if !lib_open(&mut rig, "Actors") || !lib_row(&mut rig, crate::icons::PERSON_STANDING) {
                let all: Vec<String> = super::grid_toolbar::texts(&rig).into_iter().filter(|(_, r)| r.center().y > rig.size.y * 0.7).map(|(t, _)| t).collect();
                return Err(format!("no actor row; the drawer shows {all:?}"));
            }
            if rig.ws.grid.tool != GridTool::Place {
                return Err(format!("the row did not arm Place: tool {:?} status {}", rig.ws.grid.tool, rig.ws.status));
            }
            let n = rig.ws.project.active_scene().nodes().len();
            v.click_cell(&mut rig, [4, 2]);
            if rig.ws.project.active_scene().nodes().len() > n { Ok(()) } else { Err(format!("nothing placed: {}", rig.ws.status)) }
        })());
        // Styles: a row arms Corridor with that style.
        check("Styles", (|| {
            let mut rig = rig_for(v);
            // The first row is Plain; the second is a built-in style.
            if !lib_open(&mut rig, "Styles") || !lib_row_nth(&mut rig, crate::icons::SWATCH_BOOK, 1) {
                return Err("no style row".into());
            }
            if rig.ws.grid.tool != GridTool::Corridor {
                return Err("the row did not arm Corridor".into());
            }
            for c in [[5, 2], [9, 2]] {
                v.click_cell(&mut rig, c);
            }
            key(&mut rig, egui::Key::Enter);
            if rig.ws.status.contains("in style") { Ok(()) } else { Err(format!("not in style: {}", rig.ws.status)) }
        })());
        // Buildings: a preset arms Building, Cathedral arms its tool.
        check("Buildings", (|| {
            let mut rig = rig_for(v);
            if !lib_open(&mut rig, "Buildings") || !lib_row(&mut rig, crate::icons::BUILDING) {
                return Err("no building row".into());
            }
            if rig.ws.grid.tool != GridTool::Building {
                return Err("the row did not arm Building".into());
            }
            let rooms = rig.ws.project.active_scene().world.grid().map_or(0, |w| w.rooms.len());
            v.drag_cells(&mut rig, [10, 0], [14, 4]);
            let after = rig.ws.project.active_scene().world.grid().map_or(0, |w| w.rooms.len());
            if after != rooms + 1 {
                return Err(format!("no building: {}", rig.ws.status));
            }
            if !lib_row(&mut rig, crate::icons::CHURCH) || rig.ws.grid.tool != GridTool::Cathedral {
                return Err("the Cathedral row did not arm Cathedral".into());
            }
            frames(&mut rig, 1);
            Ok(())
        })());
    }
    out
}

#[test]
fn the_library_tabs_arm_their_tools_and_place_in_every_view() {
    let bad: Vec<String> = cells()
        .into_iter()
        .filter_map(|(what, v, r)| r.err().map(|e| format!("{what} [{v:?}]: {e}")))
        .collect();
    assert!(bad.is_empty(), "{} cells:\n{}", bad.len(), bad.join("\n"));
}
