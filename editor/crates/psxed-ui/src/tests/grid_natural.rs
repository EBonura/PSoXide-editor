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
    r.sector(x, z).unwrap().floor.clone().unwrap()
}

/// A point inside triangle B (south-west half of a NW to SE split) of `cell`.
fn in_tri_b(rig: &GridRig, cell: [i32; 2]) -> Pos2 {
    rig.top_at((cell[0] as f32 + 0.25) * S, (cell[1] as f32 + 0.75) * S)
}

/// A point inside triangle A (north-east half).
fn in_tri_a(rig: &GridRig, cell: [i32; 2]) -> Pos2 {
    rig.top_at((cell[0] as f32 + 0.75) * S, (cell[1] as f32 + 0.25) * S)
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
        if sector.floor.as_ref().is_some_and(|f| f.heights_b.is_some()) {
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
