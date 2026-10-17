//! The level verbs through the editor: Raise/Lower with a marquee, a brush
//! and a path, in both views, with live derived sides, exact cancel and undo.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::faces::all_faces;

/// A derived hall with a ceiling, 8 by 8 sectors, at scale 1:1.
fn hall() -> ProjectDocument {
    derived_project_for_tiling(8, 8)
}

fn floor_at(rig: &GridRig, cell: [i32; 2]) -> [i32; 4] {
    floor_heights(rig.world(), 0, cell).unwrap()
}

fn ceiling_at(rig: &GridRig, cell: [i32; 2]) -> [i32; 4] {
    let r = rig.room(0);
    let [x, z] = r.local(cell).unwrap();
    r.sector(x, z).unwrap().ceiling.as_ref().unwrap().heights
}

/// Pixels the Top view spends on one click of height.
const CLICK_PX: f32 = crate::grid_tools::view::TOP_PIXELS_PER_CLICK;

#[test]
fn a_marquee_then_a_vertical_drag_raises_the_region_with_live_sides_and_undoes_exactly() {
    let mut rig = GridRig::top("verb-raise-top", hall());
    let original = rig.ws.project.clone();
    let faces_before = all_faces(rig.world()).len();
    rig.tool(GridTool::Raise);
    // Marquee over sectors (2,2) to (3,3).
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(3, 3));
    let region = rig.ws.grid.verb.region.clone().expect("a region");
    assert_eq!(region.len(), 4);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    // Press on the region and drag up two clicks, holding.
    let from = rig.cell_centre(2, 2);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 2.0 * CLICK_PX),
        egui::Modifiers::NONE,
    );
    assert!(matches!(rig.ws.grid.gesture, Gesture::Verb(_)));
    assert_eq!(floor_at(&rig, [2, 2]), [512; 4], "{}", rig.ws.status);
    assert_eq!(floor_at(&rig, [1, 2]), [0; 4]);
    // The ceiling followed, and the derived faces appear while dragging:
    // 8 risers and 8 drops around the 2 by 2 block.
    assert_eq!(ceiling_at(&rig, [2, 2]), [3584; 4]);
    assert_eq!(all_faces(rig.world()).len(), faces_before + 16);
    rig.release(from - Vec2::new(0.0, 2.0 * CLICK_PX), egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(floor_at(&rig, [3, 3]), [512; 4]);
    // One undo step puts the document back, one redo restores the raise.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(floor_at(&rig, [2, 2]), [512; 4]);
}

#[test]
fn dragging_down_lowers_and_alt_snaps_to_the_fine_step() {
    let mut rig = GridRig::top("verb-lower", hall());
    rig.tool(GridTool::Raise);
    // A click on a sector is a one sector region.
    rig.click(rig.cell_centre(1, 1));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(1));
    let from = rig.cell_centre(1, 1);
    rig.drag_with(from, from + Vec2::new(0.0, CLICK_PX), egui::Modifiers::NONE);
    assert_eq!(
        floor_at(&rig, [1, 1]),
        [-256; 4],
        "{} {:?}",
        rig.ws.status,
        rig.ws.grid.verb.region
    );
    // Alt: a quarter of a click is a fine step.
    rig.ws.do_undo();
    rig.drag_with(from, from - Vec2::new(0.0, 6.5), egui::Modifiers::ALT);
    assert_eq!(floor_at(&rig, [1, 1]), [64; 4]);
}

#[test]
fn escape_mid_drag_puts_the_document_back_exactly() {
    let mut rig = GridRig::top("verb-cancel", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Raise);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(4, 2));
    let from = rig.cell_centre(3, 2);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 3.0 * CLICK_PX),
        egui::Modifiers::NONE,
    );
    assert_ne!(rig.ws.project, original);
    rig.key(egui::Key::Escape);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(rig.ws.project, original, "{}", rig.ws.status);
    // The region survives a cancelled drag; a second Esc clears it.
    assert!(rig.ws.grid.verb.region.is_some());
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.verb.region.is_none());
}

#[test]
fn tab_switches_to_level_mode_mid_drag() {
    let mut rig = GridRig::top("verb-tab", hall());
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        world.rooms[0]
            .sector_mut(2, 2)
            .unwrap()
            .floor
            .as_mut()
            .unwrap()
            .heights = [0, 0, 256, 256];
    }
    rig.ws.grid_changed();
    rig.tool(GridTool::Raise);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(3, 2));
    let from = rig.cell_centre(2, 2);
    rig.drag_hold(from, from - Vec2::new(0.0, CLICK_PX), egui::Modifiers::NONE);
    // A non flat region defaults to Offset: the slope stays a slope.
    assert_eq!(floor_at(&rig, [2, 2]), [256, 256, 512, 512]);
    rig.key(egui::Key::Tab);
    // Level: the reference is the height under the press (the cell's mean,
    // 128 snapped to 128), plus a click.
    let flat = floor_at(&rig, [2, 2]);
    assert!(flat.iter().all(|h| *h == flat[0]), "{flat:?}");
    assert_eq!(floor_at(&rig, [3, 2]), flat);
    rig.key(egui::Key::Tab);
    assert_eq!(floor_at(&rig, [2, 2]), [256, 256, 512, 512]);
}

#[test]
fn the_brush_and_a_path_make_regions() {
    let mut rig = GridRig::top("verb-sources", hall());
    rig.tool(GridTool::Raise);
    rig.ws.grid.verb.source = crate::grid_tools::verb_state::RegionSource::Brush;
    rig.ws.grid.verb.brush_radius = 1;
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(4, 2));
    // A plus around each of three centres: 3 columns of 3 + the end caps.
    let cells: Vec<[i32; 2]> = rig.ws.grid.verb.region.as_ref().unwrap().cells().collect();
    assert_eq!(
        cells.len(),
        11,
        "plus shapes merged along the stroke: {cells:?}"
    );
    // `[` and `]` change the radius.
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.ws.grid.verb.brush_radius, 2);
    // A path: clicks, then Enter.
    rig.ws.grid.verb.region = None;
    rig.ws.grid.verb.source = crate::grid_tools::verb_state::RegionSource::Path;
    rig.click(rig.cell_centre(1, 1));
    rig.click(rig.cell_centre(5, 1));
    rig.click(rig.cell_centre(5, 4));
    // Five sectors east along z = 1, then three south along x = 5.
    assert_eq!(rig.ws.grid.verb.region.as_ref().unwrap().len(), 8);
    rig.key(egui::Key::Enter);
    assert!(rig.ws.grid.verb.path.is_empty());
    // Press on the finished path and raise it.
    let from = rig.cell_centre(3, 1);
    rig.drag(from, from - Vec2::new(0.0, CLICK_PX));
    assert_eq!(floor_at(&rig, [3, 1]), [256; 4]);
    assert_eq!(floor_at(&rig, [3, 2]), [0; 4]);
}

#[test]
fn the_3d_view_raises_a_region_too() {
    let mut rig = GridRig::persp("verb-raise-3d", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Raise);
    let ground = |rig: &GridRig, x: f32, z: f32| rig.screen_of([x * S, 0.0, z * S]);
    rig.drag(ground(&rig, 3.5, 3.5), ground(&rig, 4.5, 4.5));
    let region = rig.ws.grid.verb.region.clone().expect("a region");
    assert_eq!(region.len(), 4, "{}", rig.ws.status);
    let from = ground(&rig, 3.5, 3.5);
    // Drag up the screen: the amount follows the pointer at the grab rate.
    rig.drag_hold(from, from - Vec2::new(0.0, 60.0), egui::Modifiers::NONE);
    assert!(
        matches!(rig.ws.grid.gesture, Gesture::Verb(_)),
        "{}",
        rig.ws.status
    );
    let raised = floor_at(&rig, [3, 3])[0];
    assert!(raised > 0 && raised % 256 == 0, "{raised}");
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
}

#[test]
fn a_typed_amount_is_one_undo_step() {
    let mut rig = GridRig::top("verb-typed", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Raise);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(1, 1));
    rig.ws.grid.verb.typed_clicks = 3.0;
    crate::grid_tools::verb_tool::apply_typed(&mut rig.ws);
    assert_eq!(floor_at(&rig, [0, 0]), [768; 4]);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn raising_with_a_selection_adopts_it_as_the_region() {
    let mut rig = GridRig::top("verb-adopt", hall());
    rig.ws
        .apply_grid_pose("tool=select;select=floors:1,1:2,1")
        .unwrap();
    rig.tool(GridTool::Raise);
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(2));
}

/// Cost of a verb gesture on the Cathedral, printed, not asserted. Run with
/// `cargo test -p psxed-ui --release --lib measure_verb_gesture -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn measure_verb_gesture_cost_on_the_cathedral() {
    use psxed_project::grid::edit::verbs::raise::{RaiseDrag, RaiseMode, RaiseParams};
    use psxed_project::grid::edit::{CellRect, Region};
    use std::time::Instant;
    let path = psxed_project::projects_dir()
        .join("grid-cathedral")
        .join("project.ron");
    let project = ProjectDocument::load_from_path(path).expect("the Cathedral loads");
    let world = project.active_scene().world.grid().unwrap().clone();
    let room = &world.rooms[0];
    let region = Region::from_rect(
        0,
        CellRect {
            min: room.origin,
            max: [
                room.origin[0] + room.width as i32 - 1,
                room.origin[1] + room.depth as i32 - 1,
            ],
        },
    );
    let (cells, _) = region.with_floors(&world);
    let params = RaiseParams {
        mode: RaiseMode::Offset,
        follow_ceiling: true,
        wall_material: None,
        reference: 0,
    };
    let mut w = world.clone();
    let drag = RaiseDrag::begin(&w, &region, params).unwrap();
    let n = 200;
    let t = Instant::now();
    for i in 0..n {
        drag.apply(&mut w, 256 * (1 + (i % 4)));
    }
    let apply = t.elapsed().as_secs_f64() * 1e3 / n as f64;
    let t = Instant::now();
    let mut faces = 0;
    for _ in 0..n {
        faces = all_faces(&w).len();
    }
    let all = t.elapsed().as_secs_f64() * 1e3 / n as f64;
    // A whole editor frame in the Top view with the region shown.
    let mut rig = GridRig::top("verb-measure", project);
    rig.tool(GridTool::Raise);
    rig.ws.grid.verb.region = Some(region);
    let t = Instant::now();
    for _ in 0..30 {
        rig.pump(vec![], egui::Modifiers::NONE);
    }
    let frame = t.elapsed().as_secs_f64() * 1e3 / 30.0;
    eprintln!(
        "cathedral region {} sectors, {faces} faces: apply {apply:.3} ms, all_faces {all:.3} ms, top-view editor frame {frame:.3} ms",
        cells.len()
    );
}

#[test]
fn a_ramp_keeps_the_side_nearest_the_press_and_the_other_side_rises() {
    let mut rig = GridRig::top("verb-ramp", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Ramp);
    rig.drag(rig.cell_centre(2, 1), rig.cell_centre(4, 3));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(9));
    // Hover near the north row: that side would keep its height.
    rig.move_to(rig.cell_centre(3, 1), egui::Modifiers::NONE);
    assert_eq!(
        rig.ws.grid.verb.ramp_hover,
        Some(psxed_project::grid::edit::verbs::ramp::RampDir::North)
    );
    let from = rig.cell_centre(3, 1);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 3.0 * CLICK_PX),
        egui::Modifiers::NONE,
    );
    assert!(
        matches!(rig.ws.grid.gesture, Gesture::Verb(_)),
        "{}",
        rig.ws.status
    );
    // Three rows, three clicks: 256 a row, north row first.
    assert_eq!(
        floor_at(&rig, [3, 1]),
        [0, 0, 256, 256],
        "{}",
        rig.ws.status
    );
    assert_eq!(floor_at(&rig, [3, 3]), [512, 512, 768, 768]);
    assert_eq!(floor_at(&rig, [1, 1]), [0; 4], "outside stays");
    assert!(rig.ws.status.contains("walkable"), "{}", rig.ws.status);
    rig.release(from - Vec2::new(0.0, 3.0 * CLICK_PX), egui::Modifiers::NONE);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn r_turns_the_kept_side_and_a_steep_ramp_is_called_out_and_clamped_on_request() {
    let mut rig = GridRig::top("verb-ramp-turn", hall());
    rig.tool(GridTool::Ramp);
    rig.drag(rig.cell_centre(2, 1), rig.cell_centre(2, 1));
    rig.click(rig.cell_centre(2, 1));
    rig.move_to(rig.cell_centre(2, 1), egui::Modifiers::NONE);
    rig.key(egui::Key::R);
    assert_eq!(
        rig.ws.grid.verb.ramp_from,
        Some(psxed_project::grid::edit::verbs::ramp::RampDir::East)
    );
    // One sector deep along X, twelve clicks up: far too steep.
    let from = rig.cell_centre(2, 1);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 12.0 * CLICK_PX),
        egui::Modifiers::NONE,
    );
    assert!(rig.ws.status.contains("too steep"), "{}", rig.ws.status);
    let steep = floor_at(&rig, [2, 1]);
    assert!(steep.iter().any(|h| *h >= 3000), "{steep:?}");
    // Tab holds it to what the motor stands on: 1024 over one sector here.
    rig.key(egui::Key::Tab);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 12.0 * CLICK_PX + 1.0),
        egui::Modifiers::NONE,
    );
    let held = floor_at(&rig, [2, 1]);
    assert_eq!(held.iter().max(), Some(&1024), "{held:?} {}", rig.ws.status);
    assert!(rig.ws.status.contains("walkable"), "{}", rig.ws.status);
}

#[test]
fn the_ramp_pose_and_typed_amount_work_without_a_pointer() {
    let mut rig = GridRig::top("verb-ramp-pose", hall());
    rig.ws
        .apply_grid_pose("tool=ramp;region=1,1:3,2;ramp-from=west;verb-drag=2")
        .unwrap();
    // Three columns deep, two clicks: 512 over three, snapped to 64 a column.
    let f = floor_at(&rig, [3, 1]);
    assert_eq!(f[1], 512, "{f:?}");
    rig.key(egui::Key::Escape);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(floor_at(&rig, [3, 1]), [0; 4]);
}
