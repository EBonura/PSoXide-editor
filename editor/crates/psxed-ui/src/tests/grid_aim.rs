//! The tools follow the storey cursor: the surface and level verbs edit the
//! floor the cursor names in a column of several, faces and picks carry their
//! span, the cursor follows an edit, and the spiral tool winds stairs round a
//! ring. Driven by real pointer and key events, in both views.

use super::grid_derived::derived_project_for_tiling;
use super::grid_levels::{alt_down, alt_up, column, CLICK_PX};
use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::edit::spans::insert_slab;
use psxed_project::grid::edit::{CellRect, FaceRef, FaceTarget, GridElement, SurfaceKind};
use psxed_project::grid::WallEdge;

/// A derived hall, 8 by 8 sectors at scale 1:1: floor 0, ceiling 12 clicks,
/// with a slab at 6 clicks over sectors (2, 2) to (5, 5): two storeys there.
fn two_floors() -> ProjectDocument {
    let mut project = derived_project_for_tiling(8, 8);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    let outcome = insert_slab(
        world,
        0,
        CellRect::from_corners([2, 2], [5, 5]),
        1536,
        256,
        None,
    );
    assert_eq!(outcome.changed, 16);
    project
}

/// The cursor on the upper storey (two presses of Alt+Up).
fn upper_level(rig: &mut GridRig) {
    alt_up(rig);
    alt_up(rig);
    assert_eq!(rig.ws.grid.level.y, Some(1536), "{}", rig.ws.status);
}

/// Pick sectors (2, 2) to (5, 5) with the active verb.
fn pick_slab_block(rig: &mut GridRig) {
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(5, 5));
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(16));
}

#[test]
fn a_verb_over_a_column_of_two_floors_refuses_until_a_level_is_set() {
    let mut rig = GridRig::top("aim-refuse", two_floors());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Raise);
    pick_slab_block(&mut rig);
    let from = rig.cell_centre(2, 2);
    rig.drag(from, from - Vec2::new(0.0, 2.0 * CLICK_PX));
    assert_eq!(rig.ws.project, original, "{}", rig.ws.status);
    assert!(
        rig.ws.status.contains("no level is set"),
        "the refusal names the way out: {}",
        rig.ws.status
    );
    // Single-span cells are still edited with no level at all.
    rig.key(egui::Key::Escape);
    rig.key(egui::Key::Escape);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(1, 1));
    let from = rig.cell_centre(0, 0);
    rig.drag(from, from - Vec2::new(0.0, CLICK_PX));
    assert_eq!(column(&rig, [0, 0])[0].0, Some(256), "{}", rig.ws.status);
}

#[test]
fn raise_edits_the_floor_at_the_level_and_the_cursor_follows_it() {
    let mut rig = GridRig::top("aim-raise", two_floors());
    upper_level(&mut rig);
    rig.tool(GridTool::Raise);
    pick_slab_block(&mut rig);
    let from = rig.cell_centre(2, 2);
    rig.drag(from, from - Vec2::new(0.0, 2.0 * CLICK_PX));
    for cell in [[2, 2], [4, 3], [5, 5]] {
        // The upper floor rose two clicks and took its ceiling along; the
        // ground floor and the slab underside did not move.
        assert_eq!(
            column(&rig, cell),
            vec![(Some(0), Some(1280)), (Some(2048), Some(3584))],
            "{cell:?}: {}",
            rig.ws.status
        );
    }
    // The cursor stayed with the floor it was raising.
    assert_eq!(rig.ws.grid.level.y, Some(2048), "{}", rig.ws.status);
    // One undo step puts it all back.
    rig.ws.do_undo();
    assert_eq!(column(&rig, [3, 3])[1].0, Some(1536));
}

#[test]
fn raise_at_the_ground_level_leaves_the_floor_above_alone() {
    let mut rig = GridRig::top("aim-ground", two_floors());
    alt_up(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    rig.tool(GridTool::Raise);
    pick_slab_block(&mut rig);
    let from = rig.cell_centre(2, 2);
    rig.drag(from, from - Vec2::new(0.0, CLICK_PX));
    assert_eq!(
        column(&rig, [3, 3]),
        vec![(Some(256), Some(1536)), (Some(1536), Some(3072))],
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_raise_that_would_break_the_column_is_refused_and_a_sliver_is_filled_solid() {
    let mut rig = GridRig::top("aim-sliver", two_floors());
    alt_up(&mut rig);
    rig.tool(GridTool::Raise);
    pick_slab_block(&mut rig);
    let before = rig.ws.project.clone();
    // The ground floor and its ceiling rise together toward the floor above:
    // the drag goes as far as the column stays in order and says why it stops.
    let from = rig.cell_centre(2, 2);
    rig.drag(from, from - Vec2::new(0.0, 8.0 * CLICK_PX));
    assert!(rig.ws.status.contains("out of order"), "{}", rig.ws.status);
    let held = column(&rig, [3, 3]);
    assert_eq!(held.len(), 2, "{held:?}");
    assert!(
        held[0].1 <= held[1].0,
        "the column stays in order: {held:?}"
    );
    assert_eq!(
        held[1],
        (Some(1536), Some(3072)),
        "the floor above is not moved"
    );
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, before);

    // Hold the ceiling still: the floor stops a click under it, which leaves
    // less than three clicks to stand in, so the span is filled solid.
    rig.ws.grid.verb.follow_ceiling = false;
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    assert_eq!(
        column(&rig, [3, 3]),
        vec![(Some(1536), Some(3072))],
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.status.contains("too thin"), "{}", rig.ws.status);
}

#[test]
fn ramp_and_stairs_work_on_the_floor_at_the_level() {
    let mut rig = GridRig::top("aim-ramp", two_floors());
    upper_level(&mut rig);
    rig.tool(GridTool::Ramp);
    pick_slab_block(&mut rig);
    // Press near the north side to keep it, drag up one click.
    let from = rig.cell_centre(3, 2) - Vec2::new(0.0, 30.0);
    rig.drag(from, from - Vec2::new(0.0, CLICK_PX));
    let upper = |rig: &GridRig, cell| {
        let room = rig.room(0);
        let [x, z] = room.local(cell).unwrap();
        room.sector(x, z).unwrap().spans()[1]
            .floor
            .as_ref()
            .unwrap()
            .heights
    };
    let north = upper(&rig, [3, 2]);
    let south = upper(&rig, [3, 5]);
    assert!(
        north.iter().chain(south.iter()).any(|h| *h > 1536),
        "the upper floor slopes: {north:?} {south:?} {}",
        rig.ws.status
    );
    assert_eq!(
        column(&rig, [3, 3])[0].0,
        Some(0),
        "the ground floor is flat"
    );

    // A flight along the slab: treads climb from the upper floor.
    let mut rig = GridRig::top("aim-stairs", two_floors());
    upper_level(&mut rig);
    rig.tool(GridTool::Stairs);
    rig.click(rig.cell_centre(2, 3));
    rig.click(rig.cell_centre(5, 3));
    rig.key(egui::Key::Enter);
    let from = rig.cell_centre(3, 3);
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    let floors: Vec<i32> = (2..=5)
        .map(|x| column(&rig, [x, 3])[1].0.unwrap())
        .collect();
    assert_eq!(floors, vec![1792, 2048, 2304, 2560], "{}", rig.ws.status);
    assert_eq!(column(&rig, [3, 3])[0].0, Some(0));
}

#[test]
fn the_floor_tool_makes_floors_at_the_cursor_and_the_ceiling_tool_edits_its_span() {
    let mut rig = GridRig::top("aim-floor", two_floors());
    upper_level(&mut rig);
    // The Floor tool's height follows the cursor: a floor made past the hall
    // is made at the level.
    rig.tool(GridTool::Floor);
    rig.click(rig.cell_centre(8, 3));
    assert_eq!(
        floor_heights(rig.world(), 0, [8, 3]),
        Some([1536; 4]),
        "{}",
        rig.ws.status
    );
    assert_eq!(rig.ws.grid.level.y, Some(1536));
    // Ctrl and the Ceiling tool take the ceiling of the span at the level
    // out; the underside of the slab stays.
    rig.tool(GridTool::Ceiling);
    rig.click_with(rig.cell_centre(3, 3), egui::Modifiers::COMMAND);
    assert_eq!(
        column(&rig, [3, 3]),
        vec![(Some(0), Some(1280)), (Some(1536), None)],
        "{}",
        rig.ws.status
    );
}

#[test]
fn the_wall_tool_fills_the_gap_of_the_span_at_the_cursor() {
    let mut rig = GridRig::top("aim-wall", two_floors());
    // Without a cursor the gap is the ground span's, below the slab.
    rig.tool(GridTool::Wall);
    let west_of_3_3 = |rig: &GridRig, dx: f32| {
        let c = rig.cell_centre(3, 3);
        c - Vec2::new(dx, 0.0)
    };
    upper_level(&mut rig);
    rig.click(west_of_3_3(&rig, 45.0));
    let walls = walls_on(rig.world(), 0, [3, 3], WallEdge::West);
    assert_eq!(walls.len(), 1, "{}", rig.ws.status);
    assert_eq!(walls[0].heights, [1536, 1536, 3072, 3072]);
    // At the ground level the same edge takes a wall under the slab.
    alt_down(&mut rig);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    rig.click(west_of_3_3(&rig, 45.0));
    let walls = walls_on(rig.world(), 0, [3, 3], WallEdge::West);
    assert_eq!(walls.len(), 2, "{}", rig.ws.status);
    assert!(walls.iter().any(|w| w.heights == [0, 0, 1280, 1280]));
}

#[test]
fn a_pick_in_3d_carries_its_span_and_the_cutaway_picks_the_storey() {
    let mut rig = GridRig::persp("aim-pick", two_floors());
    let mid = rig.screen_of([3.5 * S, 1536.0, 3.5 * S]);
    let rect = rig.persp_rect();
    let ground = rig.screen_of([3.5 * S, 0.0, 3.5 * S]);
    // Nothing cut: the slab's top, span 1.
    let hit = rig.ws.pick_face_3d(rect, mid).expect("a hit");
    assert_eq!(hit.face, FaceRef::floor_in(0, [3, 3], 1), "{hit:?}");
    // The cursor on the ground storey cuts the slab away: span 0.
    alt_up(&mut rig);
    let hit = rig.ws.pick_face_3d(rect, ground).expect("a hit");
    assert_eq!(hit.face, FaceRef::floor_in(0, [3, 3], 0), "{hit:?}");

    // The Select tool selects exactly that face, and the Inspector height of
    // the face is the span's own.
    rig.key(egui::Key::Z);
    rig.tool(GridTool::Select);
    rig.click(mid);
    let selected: Vec<_> = rig.ws.grid.selection.items().to_vec();
    assert_eq!(
        selected,
        vec![GridElement::Face(FaceRef::floor_in(0, [3, 3], 1))],
        "{}",
        rig.ws.status
    );
    let face = FaceRef::floor_in(0, [3, 3], 1);
    assert_eq!(face.heights(rig.world(), 0), Some([1536; 4]));
    assert_eq!(
        FaceRef::floor_in(0, [3, 3], 0).heights(rig.world(), 0),
        Some([0; 4])
    );
    assert!(matches!(
        face.target,
        FaceTarget::Surface {
            kind: SurfaceKind::Floor,
            span: 1
        }
    ));
}

#[test]
fn a_press_on_an_upper_floor_in_3d_moves_the_cursor_to_it() {
    let mut rig = GridRig::persp("aim-follow-pick", two_floors());
    alt_up(&mut rig);
    rig.key(egui::Key::Z);
    assert_eq!(rig.ws.grid.level.y, Some(0));
    rig.tool(GridTool::Raise);
    let mid = rig.screen_of([3.5 * S, 1536.0, 3.5 * S]);
    rig.click(mid);
    assert_eq!(
        rig.ws.grid.level.y,
        Some(1536),
        "the click edits the storey under the pointer: {}",
        rig.ws.status
    );
    // The Floor tool's height went with it.
    assert_eq!(rig.ws.grid.floor_height, 1536);
}

#[test]
fn painting_an_upper_floor_leaves_the_floor_below_its_material_alone() {
    let mut rig = GridRig::persp("aim-paint", two_floors());
    let id = rig.ws.project.add_resource(
        "test paint",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let material_of = |rig: &GridRig, span| {
        psxed_project::grid::edit::material_of(
            rig.world(),
            FaceRef::floor_in(0, [3, 3], span),
            None,
        )
    };
    let before_ground = material_of(&rig, 0);
    rig.ws.grid.material = Some(id);
    rig.tool(GridTool::Paint);
    let mid = rig.screen_of([3.5 * S, 1536.0, 3.5 * S]);
    rig.click(mid);
    assert_eq!(material_of(&rig, 1), Some(id), "{}", rig.ws.status);
    assert_eq!(material_of(&rig, 0), before_ground);
}

#[test]
fn the_top_view_draws_the_slice_at_the_cursor() {
    use crate::grid_view::{draw_grid_world, GridRoomView};
    let project = two_floors();
    let world = project.active_scene().world.grid().unwrap().clone();
    let labels = |level: Option<i32>| -> Vec<String> {
        let ctx = egui::Context::default();
        let hidden = HashSet::new();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 900.0))),
                ..egui::RawInput::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let rect = ui.max_rect();
                    let painter = ui.painter_at(rect);
                    let transform = ViewportTransform::from_focus(
                        rect,
                        OrthographicView::Top,
                        [4.0 * S, 4.0 * S],
                        0.1,
                    );
                    draw_grid_world(
                        &painter,
                        transform,
                        &world,
                        true,
                        &GridRoomView {
                            hidden: &hidden,
                            active: None,
                            level,
                        },
                    );
                });
            },
        );
        output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };
    let count = |texts: &[String], what: &str| texts.iter().filter(|t| *t == what).count();
    // No cursor: every column shows its ground floor, 0 clicks.
    let none = labels(None);
    assert_eq!(count(&none, "6"), 0, "{none:?}");
    // The upper level: the sixteen slab columns show their upper floor, 6
    // clicks, and the rest of the hall, which has no floor there, is only
    // what it is (its single span).
    let upper = labels(Some(1536));
    assert_eq!(count(&upper, "6"), 16, "{upper:?}");
    // The ground level: the slab columns show the ground floor again.
    let ground = labels(Some(0));
    assert_eq!(count(&ground, "6"), 0, "{ground:?}");
}

#[test]
fn the_preview_view_is_one_typed_value_with_the_cutaway_and_hidden_rooms() {
    let ctx = full_context();
    let mut rig = GridRig::persp("aim-preview-view", two_floors());
    assert_eq!(
        crate::grid_preview_view(),
        crate::GridPreviewView::default()
    );
    alt_up(&mut rig);
    rig.ws.grid_hidden_rooms.insert(1);
    draw_editor(&mut rig.ws, &ctx, false);
    let view = crate::grid_preview_view();
    assert_eq!(view.cutaway, Some(1536), "the next storey's floor");
    assert_eq!(view.hidden_rooms, vec![1]);
    assert!(view.hides_room(1) && !view.hides_room(0));
    assert!(view.cuts(1536.0) && !view.cuts(1535.0));
    // The cutaway off: nothing is cut.
    rig.key(egui::Key::Z);
    draw_editor(&mut rig.ws, &ctx, false);
    assert_eq!(crate::grid_preview_view().cutaway, None);
}

/// A 12 click tall hall for the spiral: floor 0, ceiling 3072 at scale 1:1,
/// 8 by 8 sectors, with a taller 12 by 12 world above it set by `ceiling`.
fn tall_hall(ceiling: i32) -> ProjectDocument {
    let mut project = derived_project_for_tiling(8, 8);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    for z in 0..8 {
        for x in 0..8 {
            let sector = world.rooms[0].sector_mut(x, z).unwrap();
            sector.ground_ceiling_mut().unwrap().heights.fill(ceiling);
        }
    }
    project
}

#[test]
fn the_spiral_tool_winds_stairs_round_a_ring_and_esc_puts_them_back() {
    let mut rig = GridRig::top("aim-spiral", tall_hall(12 * 256));
    let original = rig.ws.project.clone();
    rig.key_with(egui::Key::T, egui::Modifiers::SHIFT);
    assert_eq!(rig.ws.grid.tool, GridTool::Spiral, "{}", rig.ws.status);
    // The corners of a square ring, Enter closes it.
    for (x, z) in [(1, 1), (6, 1), (6, 5), (1, 5)] {
        rig.click(rig.cell_centre(x, z));
    }
    rig.key(egui::Key::Enter);
    assert_eq!(rig.ws.grid.verb.stairs_path.len(), 18, "{}", rig.ws.status);
    assert_eq!(rig.ws.grid.verb.region.as_ref().map(|r| r.len()), Some(18));
    // Drag up for a climb of 4 clicks a storey, two storeys.
    rig.ws.grid.verb.spiral.storeys = 2;
    let from = rig.cell_centre(3, 1);
    rig.drag_hold(
        from,
        from - Vec2::new(0.0, 4.0 * CLICK_PX),
        egui::Modifiers::NONE,
    );
    assert!(
        matches!(rig.ws.grid.gesture, Gesture::Verb(_)),
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.status.contains("8 treads"), "{}", rig.ws.status);
    // The first straight cell after the corner takes the first tread.
    assert_eq!(column(&rig, [2, 1])[0].0, Some(256), "{}", rig.ws.status);
    assert_eq!(column(&rig, [3, 1])[0].0, Some(512));
    // Fourteen straight cells make a lap, so eight treads never come round.
    assert_eq!(column(&rig, [2, 1]).len(), 1);
    // Esc puts the document back byte for byte.
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original, "{}", rig.ws.status);
    // And a finished gesture is one undo step.
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    assert_ne!(rig.ws.project, original);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn a_spiral_with_many_storeys_passes_a_sector_more_than_once() {
    let mut rig = GridRig::top("aim-spiral-laps", tall_hall(40 * 256));
    rig.ws
        .apply_grid_pose("tool=spiral;stairs-path=1,1:6,1:6,5:1,5;storeys=2;verb-drag=16")
        .unwrap_or_else(|e| panic!("{e}"));
    // 16 clicks a storey, a click a tread: 32 treads, 14 to a lap. The
    // second straight sector of the ring is tread 2, 16 and 30.
    let found: Vec<Option<i32>> = column(&rig, [3, 1]).into_iter().map(|c| c.0).collect();
    assert_eq!(
        found,
        vec![Some(2 * 256), Some(16 * 256), Some(30 * 256)],
        "{}",
        rig.ws.status
    );
}
