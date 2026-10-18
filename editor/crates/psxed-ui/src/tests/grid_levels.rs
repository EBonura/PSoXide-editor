//! The storey cursor, the go-to-level pick, the cutaway and the Slab verb
//! through the editor, driven by real pointer and key events in both views.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;
use crate::grid_tools::slab_tool::SlabMode;
use crate::grid_tools::GridTool;
use psxed_project::grid::storeys::storeys;

/// Pixels the Top view spends on one click of height.
pub(super) const CLICK_PX: f32 = crate::grid_tools::view::TOP_PIXELS_PER_CLICK;

/// A derived hall, 8 by 8 sectors at scale 1:1, floor at 0, ceiling at 3072
/// (twelve clicks): tall enough to hold two storeys.
fn hall() -> ProjectDocument {
    derived_project_for_tiling(8, 8)
}

pub(super) fn spans_at(rig: &GridRig, cell: [i32; 2]) -> usize {
    let room = rig.room(0);
    let [x, z] = room.local(cell).unwrap();
    room.sector(x, z).unwrap().span_count()
}

/// The floor and ceiling heights of every span of a column.
pub(super) fn column(rig: &GridRig, cell: [i32; 2]) -> Vec<(Option<i32>, Option<i32>)> {
    let room = rig.room(0);
    let [x, z] = room.local(cell).unwrap();
    room.sector(x, z)
        .unwrap()
        .spans()
        .iter()
        .map(|s| {
            (
                s.floor.as_ref().map(|f| f.heights[0]),
                s.ceiling.as_ref().map(|c| c.heights[0]),
            )
        })
        .collect()
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

/// Pick sectors (2,2) to (5,5) with the Slab tool and drag a slab `clicks`
/// up, releasing.
fn insert_slab(rig: &mut GridRig, clicks: f32) {
    rig.tool(GridTool::Slab);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(5, 5));
    let from = rig.cell_centre(2, 2);
    rig.drag(from, from - Vec2::new(0.0, clicks * CLICK_PX));
}

pub(super) fn alt_up(rig: &mut GridRig) {
    rig.key_with(egui::Key::ArrowUp, egui::Modifiers::ALT);
}

pub(super) fn alt_down(rig: &mut GridRig) {
    rig.key_with(egui::Key::ArrowDown, egui::Modifiers::ALT);
}

#[test]
fn shift_q_picks_the_slab_tool_and_q_alone_stays_the_cells_tool() {
    let mut rig = GridRig::top("slab-key", hall());
    rig.key(egui::Key::Q);
    assert_eq!(rig.ws.grid.tool, GridTool::Cells);
    rig.key_with(egui::Key::Q, egui::Modifiers::SHIFT);
    assert_eq!(rig.ws.grid.tool, GridTool::Slab, "{}", rig.ws.status);
    // A verb with a region first: Tab cycles the mode.
    rig.key(egui::Key::Tab);
    assert_eq!(rig.ws.grid.verb.slab.mode, SlabMode::Cut);
}

#[test]
fn a_vertical_drag_inserts_a_slab_in_one_undo_step_and_esc_puts_it_back() {
    let mut rig = GridRig::top("slab-insert", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Slab);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(5, 5));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(16));
    // Held half way: the slab is live, and Esc takes it back exactly.
    let from = rig.cell_centre(2, 2);
    let to = from - Vec2::new(0.0, 6.0 * CLICK_PX);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    assert_eq!(spans_at(&rig, [3, 3]), 2, "{}", rig.ws.status);
    rig.key(egui::Key::Escape);
    rig.release(to, egui::Modifiers::NONE);
    assert_eq!(rig.ws.project, original, "{}", rig.ws.status);

    // Released: sixteen columns, the slab's top six clicks up and its
    // underside one click lower, and everything else untouched.
    rig.drag(from, to);
    for cell in [[2, 2], [5, 5], [3, 4]] {
        assert_eq!(
            column(&rig, cell),
            vec![(Some(0), Some(1280)), (Some(1536), Some(3072))],
            "{cell:?}: {}",
            rig.ws.status
        );
    }
    assert_eq!(spans_at(&rig, [1, 1]), 1);
    assert_eq!(spans_at(&rig, [6, 6]), 1);
    assert!(rig.ws.status.contains("16 sector(s)"), "{}", rig.ws.status);
    // One undo step puts the document back; one redo restores the slab.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(spans_at(&rig, [3, 3]), 2);
}

#[test]
fn a_slab_at_a_height_with_nothing_open_changes_nothing_and_says_so() {
    let mut rig = GridRig::top("slab-skip", hall());
    let original = rig.ws.project.clone();
    // Twelve clicks is the ceiling: no span of the column is open there.
    insert_slab(&mut rig, 12.0);
    assert_eq!(rig.ws.project, original);
    assert!(
        rig.ws
            .status
            .contains("skipped: 16 with no span at that height"),
        "{}",
        rig.ws.status
    );
}

#[test]
fn the_storey_cursor_steps_through_the_derived_storeys_with_alt_arrows() {
    let mut rig = GridRig::top("level-step", hall());
    insert_slab(&mut rig, 6.0);
    let found = storeys(rig.world());
    assert_eq!(
        found.iter().map(|s| s.height).collect::<Vec<_>>(),
        vec![0, 1536]
    );
    assert_eq!(rig.ws.grid.level.y, None);
    // The first press from nowhere lands on the lowest storey, either way.
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0), "{}", rig.ws.status);
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(1536), "{}", rig.ws.status);
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(1536));
    assert!(
        rig.ws.status.contains("No storey above"),
        "{}",
        rig.ws.status
    );
    alt_down(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    alt_down(&mut rig);
    assert!(
        rig.ws.status.contains("No storey below"),
        "{}",
        rig.ws.status
    );

    // The chip says which level and how high, in both views.
    alt_up(&mut rig);
    let ctx = full_context();
    for view_2d in [true, false] {
        let text = text_of(&draw_editor(&mut rig.ws, &ctx, view_2d));
        assert!(text.contains("Level 1"), "{text}");
        assert!(text.contains("+6.00 clicks"), "{text}");
    }
}

#[test]
fn a_level_with_one_storey_has_no_levels_to_step_to() {
    let mut rig = GridRig::top("level-none", hall());
    // 64 sectors at one height is one storey: the cursor lands on it, and
    // there is nothing above or below.
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    alt_up(&mut rig);
    assert!(
        rig.ws.status.contains("No storey above"),
        "{}",
        rig.ws.status
    );
    // Nothing is cut away on a level with one storey.
    assert_eq!(rig.ws.cutaway_plane(), None);
}

#[test]
fn z_switches_the_cutaway_and_its_plane_is_the_next_storey_up() {
    let mut rig = GridRig::persp("level-cut-plane", hall());
    insert_slab_3d(&mut rig);
    // No cursor, nothing hidden.
    assert_eq!(rig.ws.cutaway_plane(), None);
    alt_up(&mut rig);
    assert_eq!(rig.ws.cutaway_plane(), Some(1536));
    rig.key(egui::Key::Z);
    assert_eq!(rig.ws.cutaway_plane(), None, "{}", rig.ws.status);
    assert!(rig.ws.status.contains("Cutaway off"));
    rig.key(egui::Key::Z);
    assert_eq!(rig.ws.cutaway_plane(), Some(1536));
    // On the top storey there is nothing above to hide.
    alt_up(&mut rig);
    assert_eq!(rig.ws.cutaway_plane(), None);
}

#[test]
fn the_cutaway_decides_what_a_3d_click_hits() {
    let mut rig = GridRig::persp("level-cut-pick", hall());
    insert_slab_3d(&mut rig);
    // Aim at the middle of sector (3, 3), where the slab's top is at 1536.
    let mid = rig.screen_of([3.5 * S, 1536.0, 3.5 * S]);
    let rect = rig.persp_rect();
    let hit = rig.ws.pick_face_3d(rect, mid).expect("a hit");
    assert!(
        (hit.point[1] - 1536.0).abs() < 2.0,
        "without a cutaway the slab is hit: {hit:?}"
    );
    // The cursor on the ground storey cuts everything from 1536 up: the same
    // click goes through to the ground floor.
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    let hit = rig.ws.pick_face_3d(rect, mid).expect("a hit");
    assert!(hit.point[1].abs() < 2.0, "the ground floor: {hit:?}");
    // Off again: the slab.
    rig.key(egui::Key::Z);
    let hit = rig.ws.pick_face_3d(rect, mid).expect("a hit");
    assert!((hit.point[1] - 1536.0).abs() < 2.0, "{hit:?}");
}

/// The Slab tool's marquee and drag in the 3D view: the slab is inserted from
/// a pose, since the 3D view measures the drag against the perspective.
fn insert_slab_3d(rig: &mut GridRig) {
    rig.tool(GridTool::Slab);
    rig.ws
        .apply_grid_pose("region=2,2:5,5;verb-drag=6")
        .unwrap();
    // The posed drag is in flight: let go of it where it is.
    rig.ws.grid.gesture = crate::grid_tools::Gesture::Idle;
    assert_eq!(spans_at(rig, [3, 3]), 2, "{}", rig.ws.status);
}

#[test]
fn g_then_a_click_sets_the_level_to_the_surface_under_it() {
    let mut rig = GridRig::top("level-goto", hall());
    insert_slab(&mut rig, 6.0);
    rig.key(egui::Key::G);
    assert!(rig.ws.grid.level.picking);
    // The click goes to the cursor, not to the Slab tool: the region stays.
    let region = rig.ws.grid.verb.region.clone();
    rig.click(rig.cell_centre(3, 3));
    assert!(!rig.ws.grid.level.picking);
    assert_eq!(rig.ws.grid.level.y, Some(1536), "{}", rig.ws.status);
    assert_eq!(rig.ws.grid.verb.region, region);
    // On a column of one span the top floor is the ground floor.
    rig.key(egui::Key::G);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(rig.ws.grid.level.y, Some(0));
    // Esc cancels a pick that is waiting.
    rig.key(egui::Key::G);
    rig.key(egui::Key::Escape);
    assert!(!rig.ws.grid.level.picking);
    assert_eq!(rig.ws.grid.level.y, Some(0));
}

#[test]
fn cut_takes_the_first_slab_above_the_level_and_fill_closes_it_again() {
    let mut rig = GridRig::top("slab-cut-fill", hall());
    insert_slab(&mut rig, 6.0);
    let with_slab = rig.ws.project.clone();
    // Level 0: the slab over a 2 by 2 block comes out on a click. Esc drops
    // the region the insert left, so the next drag starts a new one.
    alt_up(&mut rig);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.verb.region.is_none());
    rig.key(egui::Key::Tab);
    assert_eq!(rig.ws.grid.verb.slab.mode, SlabMode::Cut);
    rig.drag(rig.cell_centre(3, 3), rig.cell_centre(4, 4));
    assert_eq!(
        rig.ws.grid.verb.region.as_ref().map(|r| r.len()),
        Some(4),
        "{}",
        rig.ws.status
    );
    rig.click(rig.cell_centre(3, 3));
    assert_eq!(spans_at(&rig, [3, 3]), 1, "{}", rig.ws.status);
    assert_eq!(spans_at(&rig, [4, 4]), 1);
    assert_eq!(spans_at(&rig, [2, 2]), 2, "outside the region");
    assert_eq!(column(&rig, [3, 3]), vec![(Some(0), Some(3072))]);
    // Fill is the inverse: closed up to the next storey above the level.
    rig.key(egui::Key::Tab);
    assert_eq!(rig.ws.grid.verb.slab.mode, SlabMode::Fill);
    rig.click(rig.cell_centre(3, 3));
    assert_eq!(
        column(&rig, [3, 3]),
        vec![(Some(0), Some(1280)), (Some(1536), Some(3072))],
        "{}",
        rig.ws.status
    );
    assert_eq!(
        rig.ws.project.active_scene().world,
        with_slab.active_scene().world
    );
    // Each press was one undo step.
    rig.ws.do_undo();
    assert_eq!(spans_at(&rig, [3, 3]), 1);
    rig.ws.do_undo();
    assert_eq!(spans_at(&rig, [3, 3]), 2);
}

#[test]
fn the_slab_options_and_help_draw_and_a_pose_shows_the_cutaway() {
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("slab-draw"), hall());
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.apply_grid_pose("tool=slab;region=2,2:5,5;verb-drag=6;cutaway=cut")
        .unwrap();
    ws.apply_grid_pose("level=0").unwrap();
    for view_2d in [true, false] {
        let text = text_of(&draw_editor(&mut ws, &ctx, view_2d));
        assert!(text.contains("Insert"), "{text}");
        assert!(text.contains("Level 0"), "{text}");
        assert!(
            text.contains("Slab at +6.00 clicks over the level"),
            "{text}"
        );
    }
}
