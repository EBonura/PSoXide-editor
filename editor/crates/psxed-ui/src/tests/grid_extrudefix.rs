//! Extrude as one gesture, in the whole editor window on the dp-world
//! stand-in (a hand-walled Starter room and the derived Machine Hall):
//! 2026-10-21, "I was able to extrude once in the floor, but now clicking
//! again on the floor or a wall does nothing!".
//!
//! There is no "Moves" chip: pressing a floor, a ceiling or a wall piece
//! picks it and starts the drag in the same press, and what was pressed
//! decides what moves.

use super::grid_live::WindowRig;
use super::grid_toolbar::{options_texts, texts};
use super::*;
use crate::grid_tools::extrude_tool::ExtrudeSurface;
use crate::grid_tools::{Gesture, GridTool};

/// The hall's room index in the stand-in.
const HALL: usize = 1;

fn fresh() -> WindowRig {
    rig()
}

fn rig() -> WindowRig {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig.ws.set_grid_tool(GridTool::Extrude);
    rig.ws.grid_active_room = Some(HALL);
    rig.frame(vec![]);
    look_over_the_hall(&mut rig);
    rig
}

fn look(rig: &mut WindowRig, from: [f32; 3], at: [f32; 3]) {
    let (yaw, pitch) = crate::editor_helpers::camera_angles_to_look_at(
        from.map(|v| v as i32),
        at.map(|v| v as i32),
    )
    .unwrap();
    rig.ws.camera_rig.mode = ViewportCameraMode::Free;
    rig.ws.camera_rig.free_initialized = true;
    rig.ws.camera_rig.free_position = from.map(|v| v as i32);
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    for _ in 0..2 {
        rig.frame(vec![]);
    }
}

fn eu(rig: &WindowRig, grid: i32) -> f32 {
    rig.world().scale.to_editor(grid) as f32
}

/// Editor-space point over world cell `cell` (its middle) at `gu` grid units.
fn over(rig: &WindowRig, cell: [i32; 2], gu: i32) -> [f32; 3] {
    [
        eu(rig, cell[0] * 1024 + 512),
        eu(rig, gu),
        eu(rig, cell[1] * 1024 + 512),
    ]
}

/// The hall is world cells x 7..15, z -1..7. Seen from above and the south,
/// its north wall and its floor are in view.
fn look_over_the_hall(rig: &mut WindowRig) {
    let mid = over(rig, [11, 3], 0);
    look(
        rig,
        [mid[0], eu(rig, 7000), eu(rig, 13 * 1024)],
        [mid[0], eu(rig, 800), eu(rig, 2 * 1024)],
    );
}

/// Inside the hall, looking up at the ceiling over `cell`.
fn look_up_at_the_ceiling(rig: &mut WindowRig, cell: [i32; 2]) {
    let at = over(rig, cell, 3072);
    look(rig, [at[0], eu(rig, 900), at[2] + eu(rig, 3 * 1024)], at);
}

fn at_floor(rig: &WindowRig, cell: [i32; 2], gu: i32) -> Pos2 {
    rig.persp_screen(over(rig, cell, gu))
        .unwrap_or_else(|| panic!("{cell:?} at {gu} is off screen"))
}

/// A point on the north wall of `cell` at `gu`.
fn north_wall(rig: &WindowRig, cell: [i32; 2], gu: i32) -> Pos2 {
    let mut p = over(rig, cell, gu);
    p[2] = eu(rig, cell[1] * 1024);
    rig.persp_screen(p)
        .unwrap_or_else(|| panic!("north wall of {cell:?} at {gu} is off screen"))
}

fn floor(rig: &WindowRig, cell: [i32; 2]) -> Option<i32> {
    let room = &rig.world().rooms[HALL];
    let [x, z] = room.local(cell)?;
    room.sector(x, z)?.ground_floor().map(|f| f.heights[0])
}

fn ceiling(rig: &WindowRig, cell: [i32; 2]) -> Option<i32> {
    let room = &rig.world().rooms[HALL];
    let [x, z] = room.local(cell)?;
    room.sector(x, z)?.ground_ceiling().map(|f| f.heights[0])
}

fn with_shift(rig: &mut WindowRig, f: impl FnOnce(&mut WindowRig)) {
    rig.modifiers = egui::Modifiers::SHIFT;
    f(rig);
    rig.modifiers = egui::Modifiers::NONE;
}

fn region_len(rig: &WindowRig) -> usize {
    rig.ws.grid.verb.region.as_ref().map_or(0, |r| r.len())
}

fn idle(rig: &WindowRig) -> bool {
    matches!(rig.ws.grid.gesture, Gesture::Idle)
}

fn bar(rig: &WindowRig) -> String {
    options_texts(rig)
        .into_iter()
        .map(|(t, _)| t)
        .collect::<Vec<_>>()
        .join(" | ")
}

const MIXED: &str = "only floors, only ceilings or only walls";

#[test]
fn one_press_drag_on_a_floor_nobody_picked_extrudes_it() {
    let mut rig = rig();
    assert_eq!(floor(&rig, [9, 3]), Some(0));
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    assert!(
        floor(&rig, [9, 3]).is_some_and(|h| h >= 512),
        "{}",
        rig.ws.status
    );
    assert!(idle(&rig));
}

#[test]
fn the_next_press_drag_on_another_floor_extrudes_that_one_and_drops_the_old_pick() {
    let mut rig = rig();
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    let first = floor(&rig, [9, 3]).unwrap();
    rig.drag(at_floor(&rig, [12, 4], 0), at_floor(&rig, [12, 4], 512));
    assert!(
        floor(&rig, [12, 4]).is_some_and(|h| h > 0),
        "{}",
        rig.ws.status
    );
    assert_eq!(
        floor(&rig, [9, 3]),
        Some(first),
        "the first block is left as it was"
    );
    assert_eq!(
        region_len(&rig),
        1,
        "the stale pick is replaced, not extended"
    );
}

#[test]
fn pressing_the_raised_block_again_raises_it_further() {
    let mut rig = rig();
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    let first = floor(&rig, [9, 3]).unwrap();
    rig.drag(
        at_floor(&rig, [9, 3], first),
        at_floor(&rig, [9, 3], first + 512),
    );
    assert!(
        floor(&rig, [9, 3]).is_some_and(|h| h > first),
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_plain_click_only_picks_and_says_what_comes_next() {
    let mut rig = rig();
    let before = rig.ws.project.clone();
    rig.click(at_floor(&rig, [9, 3], 0));
    assert_eq!(rig.ws.project, before, "a click changes nothing");
    assert_eq!(region_len(&rig), 1);
    assert!(idle(&rig));
    assert!(rig.ws.status.contains("drag"), "{}", rig.ws.status);
}

#[test]
fn a_click_elsewhere_replaces_the_pick() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    rig.click(at_floor(&rig, [12, 4], 0));
    assert_eq!(region_len(&rig), 1);
    assert!(rig.ws.grid.verb.region.as_ref().unwrap().contains([12, 4]));
}

#[test]
fn a_ceiling_a_wall_and_a_floor_each_extrude_with_no_chip_in_between() {
    let mut rig = rig();
    // A floor.
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    assert!(
        floor(&rig, [9, 3]).is_some_and(|h| h > 0),
        "{}",
        rig.ws.status
    );
    // A ceiling, from inside the hall.
    look_up_at_the_ceiling(&mut rig, [11, 1]);
    let from = at_floor(&rig, [11, 1], 3072);
    rig.drag(from, from - Vec2::new(0.0, 50.0));
    assert!(
        ceiling(&rig, [11, 1]).is_some_and(|h| h != 3072),
        "{}",
        rig.ws.status
    );
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Ceiling);
    assert_eq!(floor(&rig, [11, 1]), Some(0), "the floor is left alone");
    // A wall piece: the hall's north wall pushed out one sector.
    look_over_the_hall(&mut rig);
    rig.drag(
        north_wall(&rig, [10, -1], 600),
        north_wall(&rig, [10, -2], 600),
    );
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert!(floor(&rig, [10, -2]).is_some(), "{}", rig.ws.status);
    // And a floor again: this is what used to do nothing.
    rig.drag(at_floor(&rig, [13, 4], 0), at_floor(&rig, [13, 4], 1024));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
    assert!(
        floor(&rig, [13, 4]).is_some_and(|h| h > 0),
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.grid.selection.is_empty(), "the wall pick is gone");
}

#[test]
fn clicking_a_floor_after_a_wall_picks_the_floor() {
    let mut rig = rig();
    rig.click(north_wall(&rig, [10, -1], 600));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert_eq!(rig.ws.grid.selection.len(), 1);
    rig.click(at_floor(&rig, [9, 3], 0));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
    assert_eq!(region_len(&rig), 1, "{}", rig.ws.status);
    assert!(rig.ws.grid.selection.is_empty());
}

#[test]
fn a_wall_press_drag_picks_and_pushes_in_one_go_and_a_second_wall_does_the_same() {
    let mut rig = rig();
    rig.drag(
        north_wall(&rig, [10, -1], 600),
        north_wall(&rig, [10, -2], 600),
    );
    assert!(floor(&rig, [10, -2]).is_some(), "{}", rig.ws.status);
    // A wall the first push did not touch: picked and pushed by one press.
    rig.drag(
        north_wall(&rig, [13, -1], 600),
        north_wall(&rig, [13, -2], 600),
    );
    assert!(floor(&rig, [13, -2]).is_some(), "{}", rig.ws.status);
    assert_eq!(
        rig.ws.grid.selection.len(),
        1,
        "only the new wall is picked"
    );
}

#[test]
fn a_wall_pick_then_out_pushes_it() {
    let mut rig = rig();
    rig.click(north_wall(&rig, [10, -1], 600));
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    assert!(floor(&rig, [10, -2]).is_some(), "{}", rig.ws.status);
}

#[test]
fn shift_adding_a_wall_to_a_floor_pick_is_refused_in_words() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    let before = rig.ws.project.clone();
    with_shift(&mut rig, |rig| {
        rig.click(north_wall(rig, [10, -1], 600));
    });
    assert!(rig.ws.status.contains(MIXED), "{}", rig.ws.status);
    assert_eq!(rig.ws.project, before);
    assert_eq!(region_len(&rig), 1, "the floor pick stays");
    assert!(rig.ws.grid.selection.is_empty(), "the wall was not added");
}

#[test]
fn shift_adding_a_floor_to_a_wall_pick_is_refused_in_words() {
    let mut rig = rig();
    rig.click(north_wall(&rig, [10, -1], 600));
    with_shift(&mut rig, |rig| {
        rig.click(at_floor(rig, [9, 3], 0));
    });
    assert!(rig.ws.status.contains(MIXED), "{}", rig.ws.status);
    assert_eq!(rig.ws.grid.selection.len(), 1, "the wall pick stays");
    assert_eq!(region_len(&rig), 0);
}

#[test]
fn shift_press_drag_on_the_wrong_kind_changes_nothing_and_says_why() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    let before = rig.ws.project.clone();
    with_shift(&mut rig, |rig| {
        rig.drag(
            north_wall(rig, [10, -1], 600),
            north_wall(rig, [10, -2], 600),
        );
    });
    assert_eq!(rig.ws.project, before);
    assert!(rig.ws.status.contains(MIXED), "{}", rig.ws.status);
}

#[test]
fn shift_adds_floors_and_the_set_is_dragged_together() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    with_shift(&mut rig, |rig| {
        rig.click(at_floor(rig, [10, 3], 0));
    });
    assert_eq!(region_len(&rig), 2, "{}", rig.ws.status);
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    let (a, b) = (floor(&rig, [9, 3]).unwrap(), floor(&rig, [10, 3]).unwrap());
    assert!(a > 0 && a == b, "{a} {b}: {}", rig.ws.status);
}

#[test]
fn shift_drag_sweeps_a_rectangle_of_floors() {
    let mut rig = rig();
    with_shift(&mut rig, |rig| {
        rig.drag(at_floor(rig, [9, 3], 0), at_floor(rig, [10, 4], 0));
    });
    assert_eq!(region_len(&rig), 4, "{}", rig.ws.status);
}

#[test]
fn the_moves_chips_are_gone() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    assert!(rig.find("Moves").is_none());
    let texts: Vec<String> = texts(&rig).into_iter().map(|(t, _)| t).collect();
    for chip in ["Floor", "Ceiling", "Wall"] {
        // The tool row still names them; the options row must not repeat
        // them as a chip.
        let in_options = options_texts(&rig).into_iter().any(|(t, _)| t == chip);
        assert!(!in_options, "{chip} chip in the options bar: {texts:?}");
    }
}

#[test]
fn the_options_bar_names_what_the_pick_will_move() {
    let mut rig = rig();
    rig.click(at_floor(&rig, [9, 3], 0));
    assert!(bar(&rig).contains("floor"), "{}", bar(&rig));
    rig.click(north_wall(&rig, [10, -1], 600));
    rig.frame(vec![]);
    assert!(bar(&rig).contains("Out"), "{}", bar(&rig));
    assert!(bar(&rig).contains("wall"), "{}", bar(&rig));
}

#[test]
fn a_hand_placed_wall_says_so_when_it_is_clicked_and_in_the_options_bar() {
    let mut rig = rig();
    // The Starter room (room 0) keeps its hand-placed outline walls.
    rig.ws.grid_active_room = Some(0);
    let mid = over(&rig, [3, 3], 0);
    let (from, at) = (
        [mid[0], eu(&rig, 6000), eu(&rig, 12 * 1024)],
        [mid[0], eu(&rig, 800), eu(&rig, 1024)],
    );
    look(&mut rig, from, at);
    let before = rig.ws.project.clone();
    rig.click(north_wall(&rig, [2, 0], 600));
    assert_eq!(rig.ws.grid.selection.len(), 1, "{}", rig.ws.status);
    assert!(rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
    rig.frame(vec![]);
    assert!(bar(&rig).contains("hand-placed"), "{}", bar(&rig));
    // A drag says the same and changes nothing.
    rig.drag(
        north_wall(&rig, [2, 0], 600),
        north_wall(&rig, [2, -1], 600),
    );
    assert!(rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
    assert_eq!(rig.ws.project, before);
}

#[test]
fn the_plan_takes_a_floor_pick_over_a_wall_pick() {
    let mut rig = rig();
    rig.click(north_wall(&rig, [10, -1], 600));
    assert_eq!(rig.ws.grid.selection.len(), 1);
    rig.ws.apply_grid_pose("view=plan").unwrap();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig.click(rig.cell(9, 3));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
    assert!(rig.ws.grid.selection.is_empty(), "{}", rig.ws.status);
    assert_eq!(region_len(&rig), 1, "{}", rig.ws.status);
}

#[test]
fn cells_smaller_than_a_sector_refuse_walls_and_ceilings_in_words() {
    let mut rig = rig();
    rig.ws.grid.cell_size = crate::grid_tools::cell_size::CellSize::Half;
    rig.frame(vec![]);
    let before = rig.ws.project.clone();
    // A wall: picked, and the reason is on the status line and in the bar.
    rig.click(north_wall(&rig, [10, -1], 600));
    assert!(rig.ws.status.contains("whole sectors"), "{}", rig.ws.status);
    rig.frame(vec![]);
    assert!(bar(&rig).contains("whole sectors"), "{}", bar(&rig));
    rig.drag(
        north_wall(&rig, [10, -1], 600),
        north_wall(&rig, [10, -2], 600),
    );
    assert!(rig.ws.status.contains("whole sectors"), "{}", rig.ws.status);
    // A ceiling.
    look_up_at_the_ceiling(&mut rig, [11, 1]);
    let from = at_floor(&rig, [11, 1], 3072);
    rig.drag(from, from - Vec2::new(0.0, 50.0));
    assert!(rig.ws.status.contains("whole sectors"), "{}", rig.ws.status);
    assert_eq!(rig.ws.project, before);
}

/// Captures for the review page:
/// `PSXED_EXTRUDEFIX_CAPTURE=<dir> PSXED_EXTRUDEFIX_LABEL=before|after`
/// writes PNGs of the whole window; without the variable it does nothing.
#[test]
fn extrudefix_captures() {
    use super::grid_pick_shots::{settle, wire};
    let Some(dir) = std::env::var_os("PSXED_EXTRUDEFIX_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let label = std::env::var("PSXED_EXTRUDEFIX_LABEL").unwrap_or_else(|_| "after".into());
    let shot = |rig: &mut WindowRig, name: &str| {
        settle(rig);
        wire(rig);
        rig.capture(&dir.join(format!("{name}-{label}.png")));
    };
    let hold = |rig: &mut WindowRig, from: Pos2, to: Pos2| {
        rig.move_to(from);
        let e = rig.press(from, true);
        rig.frame(vec![e]);
        for step in 1..=6 {
            rig.move_to(from + (to - from) * (step as f32 / 6.0));
        }
    };

    // The report: a floor, a wall, then the floor again does nothing.
    let mut rig = fresh();
    rig.drag(at_floor(&rig, [9, 3], 0), at_floor(&rig, [9, 3], 1024));
    rig.click(north_wall(&rig, [10, -1], 600));
    rig.click(at_floor(&rig, [12, 4], 0));
    shot(&mut rig, "extrude-1-floor-click-after-a-wall");

    // One press: pick and drag an unpicked floor.
    let mut rig = fresh();
    let (from, to) = (at_floor(&rig, [11, 4], 0), at_floor(&rig, [11, 4], 1024));
    hold(&mut rig, from, to);
    shot(&mut rig, "extrude-2-press-drag-an-unpicked-floor");
    let e = rig.press(to, false);
    rig.frame(vec![e]);

    // A hand-placed wall says so.
    let mut rig = fresh();
    rig.ws.grid_active_room = Some(0);
    let mid = over(&rig, [3, 3], 0);
    let (from, at) = (
        [mid[0], eu(&rig, 6000), eu(&rig, 12 * 1024)],
        [mid[0], eu(&rig, 800), eu(&rig, 1024)],
    );
    look(&mut rig, from, at);
    rig.click(north_wall(&rig, [2, 0], 600));
    shot(&mut rig, "extrude-3-hand-placed-wall");

    // Floors and walls do not mix.
    let mut rig = fresh();
    rig.click(at_floor(&rig, [9, 3], 0));
    with_shift(&mut rig, |rig| {
        rig.click(north_wall(rig, [10, -1], 600));
    });
    shot(&mut rig, "extrude-4-mixed-pick");
}
