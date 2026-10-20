//! Extrude through the editor: the one tool that raises floors, moves
//! ceilings and pushes wall pieces out and in. Real pointer and key input in
//! the 3D rig (walls) and the plan (floors, ceilings).

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::extrude_tool::ExtrudeSurface;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::{edit_bands, BandScope};
use psxed_project::grid::edit::FaceRef;
use psxed_project::grid::{Anchor, GridSurface, WallEdge};

/// A 5 x 5 derived room, 1536 high, so the whole north wall is in view.
fn room() -> ProjectDocument {
    let mut project = derived_project_for_tiling(5, 5);
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        for z in 0..5 {
            for x in 0..5 {
                world.rooms[0]
                    .sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_ceiling(Some(GridSurface::flat(1536, None)));
            }
        }
    }
    project
}

fn rig() -> GridRig {
    let mut rig = GridRig::persp("extrude", room());
    rig.tool(GridTool::Extrude);
    rig
}

/// A point on the north wall of cell (2, 0) at `y` editor units.
fn wall_point(y: f32) -> [f32; 3] {
    [2.5 * S, y, 0.0]
}

fn floor_at(rig: &GridRig, cell: [i32; 2]) -> Option<[i32; 4]> {
    floor_heights(rig.world(), 0, cell)
}

fn ceiling_at(rig: &GridRig, cell: [i32; 2]) -> Option<[i32; 4]> {
    let r = rig.room(0);
    let [x, z] = r.local(cell)?;
    r.sector(x, z)?.ground_ceiling().map(|c| c.heights)
}

/// Press on the wall at `y` and drag `sectors` along its outward normal.
fn push(rig: &mut GridRig, y: f32, sectors: f32, hold: bool) -> (Pos2, Pos2) {
    let from = rig.screen_of(wall_point(y));
    let to = rig.screen_of([2.5 * S, y, -sectors * S]);
    if hold {
        rig.drag_hold(from, to, egui::Modifiers::NONE);
    } else {
        rig.drag(from, to);
    }
    (from, to)
}

#[test]
fn the_tool_is_called_extrude_and_keeps_raises_key() {
    assert_eq!(GridTool::Extrude.label(), "Extrude");
    assert_eq!(GridTool::Extrude.key(), egui::Key::U);
    assert!(GridTool::ALL.iter().all(|t| t.label() != "Raise"));
}

#[test]
fn pointing_at_a_wall_piece_lights_it_up() {
    let mut rig = rig();
    rig.move_to(rig.screen_of(wall_point(500.0)), egui::Modifiers::NONE);
    match &rig.ws.grid.hover {
        crate::grid_tools::GridHover::Elements(list) => assert_eq!(list.len(), 1),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dragging_a_wall_out_along_its_normal_makes_an_alcove_live_and_in_one_undo_step() {
    let mut rig = rig();
    let original = rig.ws.project.clone();
    let (_, to) = push(&mut rig, 500.0, 1.0, true);
    assert!(
        matches!(rig.ws.grid.gesture, Gesture::Verb(_)),
        "{}",
        rig.ws.status
    );
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert_eq!(floor_at(&rig, [2, -1]), Some([0; 4]), "{}", rig.ws.status);
    assert_eq!(ceiling_at(&rig, [2, -1]), Some([1536; 4]));
    assert!(rig.ws.status.contains("out 1"), "{}", rig.ws.status);
    rig.release(to, egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(floor_at(&rig, [2, -1]), Some([0; 4]));
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(floor_at(&rig, [2, -1]), Some([0; 4]));
}

#[test]
fn dragging_two_sectors_out_and_back_to_none_gives_the_document_back() {
    let mut rig = rig();
    let original = rig.ws.project.clone();
    let from = rig.screen_of(wall_point(500.0));
    rig.drag_hold(
        from,
        rig.screen_of([2.5 * S, 500.0, -2.0 * S]),
        egui::Modifiers::NONE,
    );
    assert!(floor_at(&rig, [2, -2]).is_some(), "{}", rig.ws.status);
    rig.move_to(from, egui::Modifiers::NONE);
    assert_eq!(
        rig.ws.project, original,
        "back at the press point is zero steps"
    );
    rig.release(from, egui::Modifiers::NONE);
}

#[test]
fn dragging_a_wall_in_fills_the_sector_in_front_of_it() {
    let mut rig = rig();
    push(&mut rig, 500.0, -1.0, false);
    assert!(floor_at(&rig, [2, 0]).is_none(), "{}", rig.ws.status);
    assert!(floor_at(&rig, [2, 1]).is_some());
}

#[test]
fn escape_during_the_drag_puts_everything_back() {
    let mut rig = rig();
    let original = rig.ws.project.clone();
    push(&mut rig, 500.0, 1.0, true);
    assert!(floor_at(&rig, [2, -1]).is_some());
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
}

#[test]
fn a_band_goes_out_as_a_niche_at_the_height_of_the_band() {
    let mut rig = rig();
    let face = FaceRef::derived(0, [2, 0], WallEdge::North, DerivedKind::Closure);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        assert!(edit_bands(world, face, BandScope::Run, |set| set
            .insert_line(0, Anchor::Abs(768))));
    }
    rig.ws.grid_changed();
    // The upper band, 768 to 1536.
    push(&mut rig, 1200.0, 1.0, false);
    assert_eq!(floor_at(&rig, [2, -1]), Some([768; 4]), "{}", rig.ws.status);
    assert_eq!(ceiling_at(&rig, [2, -1]), Some([1536; 4]));
}

#[test]
fn shift_click_adds_a_piece_and_the_buttons_push_them_all_together() {
    let mut rig = rig();
    rig.click(rig.screen_of(wall_point(500.0)));
    rig.click_with(rig.screen_of([1.5 * S, 500.0, 0.0]), egui::Modifiers::SHIFT);
    assert_eq!(rig.ws.grid.selection.len(), 2, "{}", rig.ws.status);
    let original = rig.ws.project.clone();
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    assert!(floor_at(&rig, [2, -1]).is_some() && floor_at(&rig, [1, -1]).is_some());
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original, "one undo step for both");
}

#[test]
fn equals_and_minus_push_a_sector_and_the_pieces_follow_the_new_wall() {
    let mut rig = rig();
    rig.click(rig.screen_of(wall_point(500.0)));
    rig.key(egui::Key::Equals);
    rig.key(egui::Key::Equals);
    assert!(floor_at(&rig, [2, -2]).is_some(), "{}", rig.ws.status);
    rig.key(egui::Key::Minus);
    assert!(floor_at(&rig, [2, -2]).is_none() && floor_at(&rig, [2, -1]).is_some());
}

#[test]
fn a_box_over_the_wall_picks_the_pieces_in_it() {
    let mut rig = rig();
    rig.ws.grid.verb.surface = ExtrudeSurface::Wall;
    // From a corner of the view where nothing is, across all of it.
    let rect = RIG_VIEWPORT;
    rig.drag(
        rect.min + Vec2::new(4.0, 4.0),
        rect.max - Vec2::new(4.0, 4.0),
    );
    assert!(rig.ws.grid.selection.len() >= 5, "{}", rig.ws.status);
}

#[test]
fn another_room_in_the_way_is_said_and_nothing_changes() {
    let mut project = room();
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        let mut annex = psxed_project::grid::GridRoom::new_derived("annex", 5, 1);
        annex.origin = [0, -1];
        for x in 0..5 {
            let s = annex.sector_or_insert(x, 0).unwrap();
            s.set_ground_floor(Some(GridSurface::flat(0, None)));
            s.set_ground_ceiling(Some(GridSurface::flat(1536, None)));
        }
        world.rooms.push(annex);
    }
    let mut rig = GridRig::persp("extrude-blocked", project);
    rig.tool(GridTool::Extrude);
    let original = rig.ws.project.clone();
    rig.click(rig.screen_of(wall_point(500.0)));
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    assert!(rig.ws.status.contains("another room"), "{}", rig.ws.status);
    assert_eq!(rig.ws.project, original);
}

#[test]
fn a_ceiling_region_drags_up_and_leaves_the_floor_alone() {
    let mut rig = GridRig::top("extrude-ceiling", room());
    rig.tool(GridTool::Extrude);
    rig.ws.grid.verb.surface = ExtrudeSurface::Ceiling;
    let original = rig.ws.project.clone();
    rig.drag(rig.cell_centre(1, 1), rig.cell_centre(2, 2));
    let from = rig.cell_centre(1, 1);
    let to = from - Vec2::new(0.0, 2.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK);
    rig.drag(from, to);
    assert_eq!(
        ceiling_at(&rig, [1, 1]),
        Some([2048; 4]),
        "{}",
        rig.ws.status
    );
    assert_eq!(ceiling_at(&rig, [0, 0]), Some([1536; 4]));
    assert_eq!(floor_at(&rig, [1, 1]), Some([0; 4]));
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn pressing_on_a_ceiling_in_3d_switches_the_tool_to_ceilings_and_a_floor_back() {
    let mut rig = rig();
    rig.ws.grid.verb.surface = ExtrudeSurface::Floor;
    // The camera inside the room, looking up at the far ceiling.
    let (from, at) = ([2.5 * S, 700.0, 4.8 * S], [2.5 * S, 1536.0, 1.5 * S]);
    let (yaw, pitch) = crate::editor_helpers::camera_angles_to_look_at(
        from.map(|v| v as i32),
        at.map(|v| v as i32),
    )
    .unwrap();
    rig.ws.camera_rig.free_position = from.map(|v| v as i32);
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    rig.pump(vec![], egui::Modifiers::NONE);
    rig.click(rig.screen_of(at));
    assert_eq!(
        rig.ws.grid.verb.surface,
        ExtrudeSurface::Ceiling,
        "{}",
        rig.ws.status
    );
    // And a click on the floor goes back.
    let (from, at) = ([2.5 * S, 700.0, 4.8 * S], [2.5 * S, 0.0, 2.5 * S]);
    let (yaw, pitch) = crate::editor_helpers::camera_angles_to_look_at(
        from.map(|v| v as i32),
        at.map(|v| v as i32),
    )
    .unwrap();
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    rig.pump(vec![], egui::Modifiers::NONE);
    rig.click(rig.screen_of(at));
    assert_eq!(
        rig.ws.grid.verb.surface,
        ExtrudeSurface::Floor,
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_floor_still_extrudes_as_raise_always_did() {
    let mut rig = GridRig::top("extrude-floor", room());
    rig.tool(GridTool::Extrude);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(3, 3));
    let from = rig.cell_centre(2, 2);
    rig.drag(
        from,
        from - Vec2::new(0.0, crate::grid_tools::view::TOP_PIXELS_PER_CLICK),
    );
    assert_eq!(floor_at(&rig, [2, 2]), Some([256; 4]), "{}", rig.ws.status);
}

#[test]
fn a_tool_switch_forgets_the_picked_wall_pieces() {
    let mut rig = rig();
    rig.click(rig.screen_of(wall_point(500.0)));
    assert!(!rig.ws.grid.selection.is_empty());
    rig.tool(GridTool::Ramp);
    assert!(rig.ws.grid.selection.is_empty());
}
