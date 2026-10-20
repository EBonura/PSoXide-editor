//! The natural terrain tools through the editor: voiding and cracking a
//! triangle, the lasso, Break and Rough, in both views, with exact cancel
//! and one undo step each.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::edit::FaceRef;
use psxed_project::grid::faces::all_faces;
use psxed_project::grid::surface::{VOID_A, VOID_B};
use psxed_project::grid::{GridSplit, GridSurface};

/// A derived hall with a ceiling, 8 by 8 sectors, at scale 1:1.
fn hall() -> ProjectDocument {
    derived_project_for_tiling(8, 8)
}

/// Pixels the Top view spends on one click of height.
const CLICK_PX: f32 = crate::grid_tools::view::TOP_PIXELS_PER_CLICK;

fn floor_of(rig: &GridRig, cell: [i32; 2]) -> GridSurface {
    let r = rig.room(0);
    let [x, z] = r.local(cell).unwrap();
    r.sector(x, z).unwrap().ground_floor().cloned().unwrap()
}

/// A point inside triangle B (south-west half of a NW to SE split) of `cell`.
fn in_tri_b(rig: &GridRig, cell: [i32; 2]) -> Pos2 {
    rig.top_at((cell[0] as f32 + 0.25) * S, (cell[1] as f32 + 0.75) * S)
}

/// A point inside triangle A (north-east half).
fn in_tri_a(rig: &GridRig, cell: [i32; 2]) -> Pos2 {
    rig.top_at((cell[0] as f32 + 0.75) * S, (cell[1] as f32 + 0.25) * S)
}

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

const SHIFT: egui::Modifiers = egui::Modifiers::SHIFT;
const ALT: egui::Modifiers = egui::Modifiers::ALT;

#[test]
fn shift_click_in_split_voids_a_triangle_and_again_restores_it() {
    let mut rig = GridRig::top("natural-void", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Split);
    rig.click_with(in_tri_b(&rig, [2, 2]), SHIFT);
    assert_eq!(floor_of(&rig, [2, 2]).void, VOID_B, "{}", rig.ws.status);
    assert!(
        rig.ws.status.contains("Triangle B voided"),
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.status.contains("closes itself"), "{}", rig.ws.status);
    assert_eq!(floor_of(&rig, [2, 3]).void, 0);
    // One undo step.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(floor_of(&rig, [2, 2]).void, VOID_B);
    // Shift+click again restores it; A is a separate triangle.
    rig.click_with(in_tri_b(&rig, [2, 2]), SHIFT);
    assert_eq!(floor_of(&rig, [2, 2]).void, 0);
    rig.click_with(in_tri_a(&rig, [2, 2]), SHIFT);
    assert_eq!(floor_of(&rig, [2, 2]).void, VOID_A);
}

#[test]
fn a_plain_click_in_split_still_flips_the_diagonal() {
    let mut rig = GridRig::top("natural-flip", hall());
    rig.tool(GridTool::Split);
    let before = floor_of(&rig, [4, 4]).split;
    rig.click(rig.cell_centre(4, 4));
    let after = floor_of(&rig, [4, 4]);
    assert_ne!(after.split, before);
    assert_eq!(after.void, 0);
}

#[test]
fn alt_drag_in_height_moves_one_triangle_and_shows_a_diagonal_wall() {
    let mut rig = GridRig::top("natural-tri-top", hall());
    let original = rig.ws.project.clone();
    let faces_before = all_faces(rig.world()).len();
    rig.tool(GridTool::Height);
    let from = in_tri_b(&rig, [3, 3]);
    rig.drag_hold(from, from - Vec2::new(0.0, 2.0 * CLICK_PX), ALT);
    let f = floor_of(&rig, [3, 3]);
    assert_eq!(f.split, GridSplit::NwSe);
    // Triangle B is corners 0, 2, 3; the fourth corner stays.
    assert_eq!(f.heights, [0; 4], "A did not move: {}", rig.ws.status);
    assert_eq!(f.heights_b, Some([512, 0, 512, 512]), "{}", rig.ws.status);
    // The derived diagonal riser appears live.
    assert!(all_faces(rig.world()).len() > faces_before);
    let diag = psxed_project::grid::derive::cell_walls(rig.room(0), [3, 3])
        .into_iter()
        .any(|w| w.edge.is_diagonal());
    assert!(diag, "a diagonal wall between the halves");
    assert!(rig.ws.status.contains("Triangle B"), "{}", rig.ws.status);
    rig.release(from - Vec2::new(0.0, 2.0 * CLICK_PX), ALT);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    // One undo step covers the split and the move.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn escape_and_a_zero_drag_put_a_triangle_drag_back_exactly() {
    let mut rig = GridRig::top("natural-tri-cancel", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Height);
    let from = in_tri_a(&rig, [2, 5]);
    rig.drag_hold(from, from - Vec2::new(0.0, CLICK_PX), ALT);
    assert_ne!(rig.ws.project, original);
    // Back to where it began: the surface is not split-level any more.
    rig.move_to(from, ALT);
    assert_eq!(rig.ws.project, original, "{}", rig.ws.status);
    rig.move_to(from - Vec2::new(0.0, CLICK_PX), ALT);
    assert_ne!(rig.ws.project, original);
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
}

#[test]
fn shift_is_the_fine_step_inside_a_triangle_drag() {
    let mut rig = GridRig::top("natural-tri-fine", hall());
    rig.tool(GridTool::Height);
    let from = in_tri_b(&rig, [1, 1]);
    rig.drag_hold(from, from - Vec2::new(0.0, 1.0), ALT);
    let fine = egui::Modifiers {
        alt: true,
        shift: true,
        ..egui::Modifiers::NONE
    };
    // A click is 18 px, so 9 px is half a click: 128 units on the fine step.
    rig.move_to(from - Vec2::new(0.0, 9.0), fine);
    let heights = floor_of(&rig, [1, 1]).heights_b.unwrap();
    assert_eq!(heights[0], 128, "{}", rig.ws.status);
}

#[test]
fn alt_drag_works_in_the_3d_view_too() {
    let mut rig = GridRig::persp("natural-tri-3d", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Height);
    let at = |rig: &GridRig, x: f32, z: f32| rig.screen_of([x * S, 0.0, z * S]);
    let from = at(&rig, 3.25, 3.75);
    rig.drag_hold(from, from - Vec2::new(0.0, 60.0), ALT);
    let f = floor_of(&rig, [3, 3]);
    let b = f.heights_b.unwrap_or_else(|| panic!("{}", rig.ws.status));
    assert_eq!(f.heights, [0; 4]);
    assert_eq!(b[1], 0);
    assert!(b[0] > 0 && b[0] % 256 == 0, "{b:?}");
    assert_eq!([b[0], b[2], b[3]], [b[0]; 3]);
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_triangle_drag_pose_puts_one_in_flight() {
    let mut rig = GridRig::top("natural-tri-pose", hall());
    rig.ws.apply_grid_pose("triangle-drag=2,2,1,1").unwrap();
    assert!(matches!(rig.ws.grid.gesture, Gesture::Height(_)));
    assert_eq!(rig.ws.grid.tool, GridTool::Height);
    assert_eq!(floor_of(&rig, [2, 2]).heights_b, Some([256, 0, 256, 256]));
    let _ = FaceRef::floor(0, [2, 2]);
}

fn corner_cracks(rig: &GridRig) -> Vec<[i32; 2]> {
    let mut out = Vec::new();
    for (x, z, sector) in rig.room(0).iter() {
        if sector.ground_floor().is_some_and(|f| f.heights_b.is_some()) {
            out.push(rig.room(0).world_cell(x, z));
        }
    }
    out
}

#[test]
fn the_lasso_closes_into_the_polygon_sectors_and_shift_snaps_to_45_degrees() {
    use crate::grid_tools::verb_state::RegionSource;
    use psxed_project::grid::edit::outline::polygon_cells;
    let mut rig = GridRig::top("natural-lasso", hall());
    rig.tool(GridTool::Raise);
    rig.ws.grid.verb.source = RegionSource::Lasso;
    let at = |rig: &GridRig, x: f32, z: f32| rig.top_at(x * S, z * S);
    rig.click(at(&rig, 1.0, 1.0));
    rig.click_with(at(&rig, 6.0, 1.2), SHIFT);
    rig.click_with(at(&rig, 4.2, 3.1), SHIFT);
    let points = rig.ws.grid.verb.lasso.clone();
    assert_eq!(points.len(), 3, "{}", rig.ws.status);
    // The second segment is level, the third a 45 degree line of two sectors.
    assert!((points[1][1] - points[0][1]).abs() < 1e-3, "{points:?}");
    assert!(
        (points[2][0] - points[1][0] + 2.0).abs() < 1e-3
            && (points[2][1] - points[1][1] - 2.0).abs() < 1e-3,
        "{points:?}"
    );
    // The shape is not a region until Enter.
    assert!(rig.ws.grid.verb.region.is_none());
    rig.key(egui::Key::Enter);
    let region = rig.ws.grid.verb.region.clone().expect("a region");
    let expect = polygon_cells(&points);
    assert!(!expect.is_empty());
    assert_eq!(
        region.cells().collect::<Vec<_>>(),
        expect.into_iter().collect::<Vec<_>>()
    );
    assert!(rig.ws.grid.verb.lasso.is_empty());
    // Press on the lasso region and raise it like any other.
    let first = region.cells().next().unwrap();
    let from = rig.cell_centre(first[0], first[1]);
    rig.drag(from, from - Vec2::new(0.0, CLICK_PX));
    let raised = floor_of(&rig, first).heights;
    assert_eq!(raised, [256; 4], "{}", rig.ws.status);
}

#[test]
fn escape_drops_a_lasso_in_progress_before_the_region() {
    use crate::grid_tools::verb_state::RegionSource;
    let mut rig = GridRig::top("natural-lasso-esc", hall());
    rig.tool(GridTool::Raise);
    rig.ws.grid.verb.source = RegionSource::Lasso;
    rig.click(rig.cell_centre(1, 1));
    rig.click(rig.cell_centre(4, 1));
    assert_eq!(rig.ws.grid.verb.lasso.len(), 2);
    // Fewer than three corners cannot close.
    rig.key(egui::Key::Enter);
    assert_eq!(rig.ws.grid.verb.lasso.len(), 2);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.verb.lasso.is_empty());
    assert!(rig.ws.grid.verb.region.is_none());
}

#[test]
fn the_lasso_pose_draws_a_half_made_shape() {
    let mut rig = GridRig::top("natural-lasso-pose", hall());
    rig.ws
        .apply_grid_pose("tool=raise;lasso=1,1:5.5,1:5.5,4.5")
        .unwrap();
    assert_eq!(rig.ws.grid.verb.lasso.len(), 3);
    assert_eq!(rig.ws.grid.verb.lasso[1], [5.5, 1.0]);
    rig.pump(vec![], egui::Modifiers::NONE);
}

#[test]
fn cut_corners_cracks_the_convex_corners_of_the_region_in_one_undo_step() {
    let mut rig = GridRig::top("natural-cut-corners", hall());
    rig.tool(GridTool::Raise);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(4, 4));
    let from = rig.cell_centre(3, 3);
    rig.drag(from, from - Vec2::new(0.0, 2.0 * CLICK_PX));
    let raised = rig.ws.project.clone();
    assert!(corner_cracks(&rig).is_empty());
    crate::grid_tools::verb_tool::cut_corners(&mut rig.ws);
    let mut cracked = corner_cracks(&rig);
    cracked.sort();
    assert_eq!(
        cracked,
        vec![[2, 2], [2, 4], [4, 2], [4, 4]],
        "{}",
        rig.ws.status
    );
    // The last raise (two clicks) is how far each outer triangle drops.
    let f = floor_of(&rig, [2, 2]);
    let (low, high) = (
        f.heights.iter().min().unwrap(),
        f.heights_b.unwrap().iter().min().copied().unwrap(),
    );
    assert_eq!(*low.min(&high), 0, "{f:?}");
    assert!(rig.ws.status.contains("Cut 4 corners"), "{}", rig.ws.status);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, raised);
}

/// Pixels of upward travel for one percent of Break intensity.
const BREAK_PX: f32 = 2.0;
/// Pixels for one Rough level.
const ROUGH_PX: f32 = 24.0;

/// A Top rig on the hall with the Break tool and the region (1,1) to (6,6).
fn break_rig(label: &str) -> GridRig {
    let mut rig = GridRig::top(label, hall());
    rig.tool(GridTool::Break);
    rig.drag(rig.cell_centre(1, 1), rig.cell_centre(6, 6));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(36));
    rig
}

/// Press inside the region and hold `percent` up.
fn hold_break(rig: &mut GridRig, percent: f32) -> Pos2 {
    let from = rig.cell_centre(3, 3);
    let to = from - Vec2::new(0.0, percent * BREAK_PX);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    to
}

fn break_seed(rig: &GridRig) -> u32 {
    rig.ws.grid.verb.brk.seed
}

#[test]
fn a_break_drag_then_escape_restores_the_document_byte_for_byte() {
    let mut rig = break_rig("natural-break-cancel");
    let original = rig.ws.project.clone();
    let before = original.to_ron_string().unwrap();
    hold_break(&mut rig, 100.0);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Verb(_)));
    assert_ne!(rig.ws.project, original, "{}", rig.ws.status);
    assert!(
        rig.ws.status.starts_with("Break seed 1, 100 percent:"),
        "{}",
        rig.ws.status
    );
    rig.key(egui::Key::Escape);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(rig.ws.project, original);
    assert_eq!(rig.ws.project.to_ron_string().unwrap(), before);
    // The region survives, and undo has nothing of the cancelled drag.
    assert!(rig.ws.grid.verb.region.is_some());
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn releasing_a_break_commits_one_undo_step() {
    let mut rig = break_rig("natural-break-commit");
    let original = rig.ws.project.clone();
    let faces_before = all_faces(rig.world()).len();
    let to = hold_break(&mut rig, 100.0);
    // Back and forth in the same drag: still one step.
    rig.move_to(to + Vec2::new(0.0, 80.0), egui::Modifiers::NONE);
    rig.move_to(to, egui::Modifiers::NONE);
    rig.release(to, egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    let broken = rig.ws.project.clone();
    assert_ne!(broken, original);
    assert!(all_faces(rig.world()).len() > faces_before);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(rig.ws.project, broken);
}

#[test]
fn shift_x_rerolls_the_seed_and_the_same_seed_repeats() {
    let mut rig = break_rig("natural-break-seed");
    let original = rig.ws.project.clone();
    hold_break(&mut rig, 100.0);
    let first = rig.ws.project.clone();
    assert_eq!(break_seed(&rig), 1);
    rig.key_with(egui::Key::X, SHIFT);
    assert_eq!(break_seed(&rig), 2, "{}", rig.ws.status);
    assert_ne!(rig.ws.project, first, "a new seed, a new break");
    assert!(
        rig.ws.status.starts_with("Break seed 2, 100 percent:"),
        "{}",
        rig.ws.status
    );
    // The tool stayed Break: Shift+X is the re-roll, not a tool switch.
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
    // The option row's seed, typed, repeats the first result exactly.
    rig.ws.grid.verb.brk.seed = 1;
    hold_break(&mut rig, 100.0);
    assert_eq!(rig.ws.project, first);
    rig.key(egui::Key::Escape);
}

#[test]
fn tab_cycles_the_targets_live_and_enter_commits() {
    let mut rig = break_rig("natural-break-tab");
    let original = rig.ws.project.clone();
    let to = hold_break(&mut rig, 100.0);
    assert_eq!(rig.ws.grid.verb.brk.targets_label(), "Walls+floor");
    let ceiling = |rig: &GridRig| -> Vec<[i32; 4]> {
        rig.room(0)
            .iter()
            .filter_map(|(_, _, s)| s.ground_ceiling().map(|c| c.heights))
            .collect()
    };
    let flat_ceiling = ceiling(&rig);
    assert!(flat_ceiling.iter().all(|h| *h == [3072; 4]));
    for _ in 0..3 {
        rig.key(egui::Key::Tab);
    }
    assert_eq!(rig.ws.grid.verb.brk.targets_label(), "Ceiling");
    assert_ne!(ceiling(&rig), flat_ceiling, "{}", rig.ws.status);
    // Floors are untouched while only the ceiling breaks.
    for (_, _, s) in rig.room(0).iter() {
        assert_eq!(s.ground_floor().unwrap().heights, [0; 4]);
    }
    rig.key(egui::Key::Enter);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    let committed = rig.ws.project.clone();
    assert_ne!(committed, original);
    // The release after Enter does nothing more.
    rig.release(to, egui::Modifiers::NONE);
    assert_eq!(rig.ws.project, committed);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_walkability_overlay_keeps_working_while_a_break_is_in_flight() {
    let mut rig = break_rig("natural-break-walk");
    hold_break(&mut rig, 60.0);
    rig.key_with(egui::Key::W, ALT);
    assert!(rig.ws.grid.overlays.walk);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Verb(_)));
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    rig.move_to(
        rig.cell_centre(3, 3) - Vec2::new(0.0, 100.0),
        egui::Modifiers::NONE,
    );
    assert!(rig.ws.walk_run().is_some());
}

#[test]
fn the_3d_view_breaks_a_region_too() {
    let mut rig = GridRig::persp("natural-break-3d", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Break);
    let at = |rig: &GridRig, x: f32, z: f32| rig.screen_of([x * S, 0.0, z * S]);
    rig.drag(at(&rig, 2.5, 2.5), at(&rig, 5.5, 5.5));
    assert!(rig.ws.grid.verb.region.is_some(), "{}", rig.ws.status);
    let from = at(&rig, 3.5, 3.5);
    rig.drag_hold(from, from - Vec2::new(0.0, 200.0), egui::Modifiers::NONE);
    assert!(
        matches!(rig.ws.grid.gesture, Gesture::Verb(_)),
        "{}",
        rig.ws.status
    );
    assert_ne!(rig.ws.project, original);
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_break_pose_puts_a_seeded_drag_in_flight_and_the_status_names_it() {
    let mut rig = GridRig::top("natural-break-pose", hall());
    let original = rig.ws.project.clone();
    rig.ws
        .apply_grid_pose("region=1,1:6,6;break-drag=60:7")
        .unwrap();
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Verb(_)));
    assert!(
        rig.ws.status.starts_with("Break seed 7, 60 percent: "),
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.status.contains("faces"), "{}", rig.ws.status);
    assert_ne!(rig.ws.project, original);
    assert!(rig.ws.apply_grid_pose("break-drag=oops").is_err());
}

#[test]
fn rough_levels_follow_the_drag_and_one_undo_puts_everything_back() {
    let mut rig = GridRig::top("natural-rough", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Rough);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(3, 3));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(4));
    let level = |rig: &GridRig, cell: [i32; 2]| {
        let r = rig.room(0);
        let [x, z] = r.local(cell).unwrap();
        r.sector(x, z).unwrap().rough
    };
    let from = rig.cell_centre(2, 2);
    // Two levels up.
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 2.0 * ROUGH_PX),
        egui::Modifiers::NONE,
    );
    assert_eq!(level(&rig, [2, 2]), 2, "{}", rig.ws.status);
    assert_eq!(level(&rig, [3, 3]), 2);
    assert_eq!(level(&rig, [4, 4]), 0);
    // The room got default amplitudes for the first rough level.
    assert!(rig.room(0).rough.jitters());
    // More than the top is held to level 3.
    rig.move_to(from - Vec2::new(0.0, 6.0 * ROUGH_PX), egui::Modifiers::NONE);
    assert_eq!(level(&rig, [2, 2]), 3);
    // Back to the start: nothing changed at all.
    rig.move_to(from, egui::Modifiers::NONE);
    assert_eq!(rig.ws.project, original);
    rig.move_to(from - Vec2::new(0.0, ROUGH_PX), egui::Modifiers::NONE);
    assert_eq!(level(&rig, [2, 2]), 1);
    rig.release(from - Vec2::new(0.0, ROUGH_PX), egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    // One undo step, amplitudes included.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(level(&rig, [3, 2]), 1);
    // The next drag starts from the level the region holds: down one clears.
    rig.drag(from, from + Vec2::new(0.0, ROUGH_PX));
    assert_eq!(level(&rig, [2, 2]), 0, "{}", rig.ws.status);
}

#[test]
fn escape_restores_a_rough_drag_exactly_and_the_pose_shows_one() {
    let mut rig = GridRig::top("natural-rough-esc", hall());
    let original = rig.ws.project.clone();
    rig.ws
        .apply_grid_pose("region=2,2:4,4;rough-drag=3")
        .unwrap();
    assert_eq!(rig.ws.grid.tool, GridTool::Rough);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Verb(_)));
    assert!(rig.room(0).rough.jitters());
    assert!(rig.ws.status.contains("Rough level 3"), "{}", rig.ws.status);
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_rough_settings_edit_through_set_params_in_the_rooms_panel_and_the_option_row() {
    use super::grid_ui::{draw_editor, full_context};
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("natural-rough-ui"), hall());
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.set_grid_tool(GridTool::Rough);
    let output = draw_editor(&mut ws, &ctx, true);
    let text = text_of(&output);
    assert!(text.contains("Seed"), "{text}");
    assert!(text.contains("Tint"), "{text}");
    assert!(text.contains("Rough"), "{text}");
    // The settings themselves go through the edit function.
    let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
    assert!(psxed_project::grid::edit::rough::set_params(
        world,
        0,
        Some(9),
        Some([5, 10, 60]),
        Some(30)
    ));
}

#[test]
fn the_help_page_names_the_natural_terrain_keys() {
    use super::grid_ui::{draw_editor_sized, full_context};
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("natural-help"), hall());
    ws.apply_grid_pose("help").unwrap();
    // egui sizes a new window invisibly first; the next frame shows it.
    draw_editor_sized(&mut ws, &ctx, true, Vec2::new(1920.0, 5000.0));
    let text = text_of(&draw_editor_sized(
        &mut ws,
        &ctx,
        true,
        Vec2::new(1920.0, 5000.0),
    ));
    for needle in [
        "Break (X)",
        "Rough (M)",
        "Shift+X",
        "Tab",
        "Diagonal: Shift+click",
        "Alt+drag a floor or ceiling face",
        "Lasso: click corners, Enter",
        "Cut corners",
        "X Break, M Rough",
        "S B K I Y G",
    ] {
        assert!(text.contains(needle), "{needle}:\n{text}");
    }
}

#[test]
fn the_strip_has_break_and_rough_in_the_shape_group_with_their_keys() {
    use crate::grid_tools::ToolGroup;
    assert_eq!(GridTool::Break.label(), "Break");
    assert_eq!(GridTool::Rough.label(), "Rough");
    assert_eq!(GridTool::Break.group(), ToolGroup::Shape);
    assert_eq!(GridTool::Rough.group(), ToolGroup::Shape);
    assert_eq!(GridTool::Break.key(), egui::Key::X);
    assert_eq!(GridTool::Rough.key(), egui::Key::M);
    assert!(GridTool::Break.is_verb() && GridTool::Rough.is_verb());
    // The keys switch tools, in the Top view rig.
    let mut rig = GridRig::top("natural-keys", hall());
    rig.key(egui::Key::X);
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    rig.key(egui::Key::M);
    assert_eq!(rig.ws.grid.tool, GridTool::Rough);
    // The poses parse the labels.
    rig.ws.apply_grid_pose("tool=break").unwrap();
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    rig.ws.apply_grid_pose("tool=rough").unwrap();
    assert_eq!(rig.ws.grid.tool, GridTool::Rough);
}
