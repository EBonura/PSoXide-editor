//! The walkability overlay through the editor: the key, the strip button, the
//! drawing in both views, and the cache that follows the document.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;
use crate::grid_tools::GridTool;

fn text_of(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A derived 6 by 4 hall at scale 1:1 with a steep sector, a low one, a tall
/// step and an island behind a gap.
fn fixture() -> ProjectDocument {
    let mut project = derived_project_for_tiling(6, 4);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    let room = &mut world.rooms[0];
    // Steep: 85 runtime units of rise over a sector is 1088 grid units at
    // the default scale, but this world is 1:1, where 16 grid units are one
    // runtime unit: 80 units a sector is 1280.
    room.sector_mut(1, 1)
        .unwrap()
        .ground_floor_mut()
        .unwrap()
        .heights = [0, 0, 1360, 1360];
    // Low headroom: a 600 unit ceiling is 37 runtime units.
    room.sector_mut(3, 1)
        .unwrap()
        .ground_ceiling_mut()
        .unwrap()
        .heights = [600; 4];
    // A tall step: 1024 grid units is 64 runtime units.
    for z in 2..4 {
        room.sector_mut(4, z)
            .unwrap()
            .ground_floor_mut()
            .unwrap()
            .heights = [1024; 4];
    }
    // Remove a column to make an island of sector (5, 0).
    room.sector_mut(4, 0).unwrap().set_ground_floor(None);
    room.sector_mut(4, 1).unwrap().set_ground_floor(None);
    project
}

#[test]
fn alt_w_toggles_the_overlay_and_the_strip_button_agrees() {
    let mut rig = GridRig::top("walk-key", fixture());
    assert!(!rig.ws.grid.overlays.walk);
    rig.key_with(egui::Key::W, egui::Modifiers::ALT);
    assert!(rig.ws.grid.overlays.walk, "{}", rig.ws.status);
    // Alt+W is not the Wall tool key: the tool stays.
    assert_eq!(rig.ws.grid.tool, GridTool::Select);
    rig.key_with(egui::Key::W, egui::Modifiers::ALT);
    assert!(!rig.ws.grid.overlays.walk);
    // A plain W still picks the Wall tool.
    rig.key(egui::Key::W);
    assert_eq!(rig.ws.grid.tool, GridTool::Wall);
}

#[test]
fn the_overlay_counts_what_the_fixture_has_in_both_views() {
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("walk-draw"), fixture());
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.grid.overlays.walk = true;
    for view_2d in [true, false] {
        let output = draw_editor(&mut ws, &ctx, view_2d);
        let text = text_of(&output);
        assert!(text.contains("Walkability"), "{text}");
        assert!(text.contains("too steep to stand on: 1"), "{text}");
        assert!(text.contains("headroom under 48 ru: 1"), "{text}");
        assert!(text.contains("step taller than 40 ru (2 clicks)"), "{text}");
    }
}

#[test]
fn the_analysis_is_kept_per_revision_and_dropped_when_the_document_changes() {
    let mut rig = GridRig::top("walk-cache", fixture());
    rig.ws.grid.overlays.walk = true;
    let first = rig.ws.walk_run().unwrap();
    let again = rig.ws.walk_run().unwrap();
    assert!(
        std::rc::Rc::ptr_eq(&first.analysis, &again.analysis),
        "one run per revision"
    );
    // An edit bumps the revision.
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        world.rooms[0]
            .sector_mut(1, 1)
            .unwrap()
            .ground_floor_mut()
            .unwrap()
            .heights = [0; 4];
    }
    rig.ws.grid_changed();
    let after = rig.ws.walk_run().unwrap();
    assert!(!std::rc::Rc::ptr_eq(&first.analysis, &after.analysis));
    assert_eq!(
        after.analysis.counts()[0],
        0,
        "the steep sector was levelled"
    );
}

#[test]
fn a_ramp_made_at_the_limit_turns_green_and_one_past_it_orange_in_the_overlay() {
    let mut rig = GridRig::top("walk-ramp", derived_project_for_tiling(8, 4));
    rig.ws.grid.overlays.walk = true;
    rig.tool(GridTool::Ramp);
    rig.ws
        .apply_grid_pose("region=1,1:6,2;ramp-from=west;ramp-clamp;verb-drag=40")
        .unwrap();
    // Clamped: the largest walkable rise, so nothing is steep.
    let walk = rig.ws.walk_run().unwrap();
    assert_eq!(walk.analysis.counts()[0], 0, "{}", rig.ws.status);
    rig.key(egui::Key::Escape);
    rig.ws.grid.verb.ramp_clamp = false;
    rig.ws.apply_grid_pose("verb-drag=40").unwrap();
    let steep = rig.ws.walk_run().unwrap();
    assert!(steep.analysis.counts()[0] > 0, "{}", rig.ws.status);
}
