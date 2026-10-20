//! Taking a split back (Manny, 2026-10-20: "let's say I split a wall but I
//! don't need the split anymore"): Merge joins two bands of a wall into one
//! and the cells of a split sector into the whole one. Shift+M does it in the
//! Bands, Cells and Select tools; each has a Merge button in its options row.
//! One undo step each.

use super::grid_derived::derived_project_for_tiling;
use super::grid_live::WindowRig;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::effective_set;
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::{Anchor, WallEdge};
use psxed_project::{MaterialResource, ResourceData, ResourceId};

fn shift_m(rig: &mut GridRig) {
    rig.key_with(egui::Key::M, egui::Modifiers::SHIFT);
}

fn material(rig: &mut GridRig, name: &str) -> ResourceId {
    rig.ws
        .project
        .add_resource(name, ResourceData::Material(MaterialResource::opaque(None)))
}

fn north(x: i32) -> FaceRef {
    FaceRef::derived(0, [x, 0], WallEdge::North, DerivedKind::Closure)
}

/// Screen position on the north wall of cell `x` at `y` editor units.
fn on_wall(rig: &GridRig, x: i32, y: f32) -> Pos2 {
    rig.screen_of([(x as f32 + 0.5) * S, y, 0.0])
}

/// A 5 x 5 derived room 1536 high, the Bands tool chosen, the north wall run
/// cut at 768: a lower band (0..768) and an upper one (768..1536). The cut
/// is the selected line.
fn cut_wall() -> GridRig {
    let mut project = derived_project_for_tiling(5, 5);
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        for z in 0..5 {
            for x in 0..5 {
                world.rooms[0]
                    .sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_ceiling(Some(psxed_project::grid::GridSurface::flat(1536, None)));
            }
        }
    }
    let mut rig = GridRig::persp("merge-bands", project);
    rig.tool(GridTool::Bands);
    rig.click(on_wall(&rig, 2, 768.0));
    assert_eq!(lines(&rig, 2), vec![Anchor::Abs(768)], "{}", rig.ws.status);
    rig
}

fn lines(rig: &GridRig, x: i32) -> Vec<Anchor> {
    effective_set(rig.world(), north(x))
        .map(|(_, set)| set.lines.clone())
        .unwrap_or_default()
}

fn bands_of(rig: &GridRig, x: i32) -> Vec<Option<ResourceId>> {
    effective_set(rig.world(), north(x))
        .map(|(_, set)| set.bands.iter().map(|b| b.material).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------
// Bands tool.

#[test]
fn shift_m_in_the_bands_tool_removes_the_selected_cut_as_one_undo_step() {
    let mut rig = cut_wall();
    let cut = rig.ws.project.clone();
    shift_m(&mut rig);
    assert_eq!(rig.ws.grid.tool, GridTool::Bands, "M is not the Rough tool");
    for x in 0..5 {
        assert_eq!(lines(&rig, x), Vec::<Anchor>::new(), "cell {x}");
        assert_eq!(faces_on_north(&rig, x), 1, "cell {x}");
    }
    assert!(rig.ws.history.can_undo());
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, cut);
}

#[test]
fn shift_m_with_no_cut_picked_says_what_to_do_and_changes_nothing() {
    let mut rig = cut_wall();
    rig.ws.grid.band_line = None;
    // The pointer is off the cut, on the upper band.
    rig.move_to(on_wall(&rig, 2, 1200.0), egui::Modifiers::NONE);
    let before = rig.ws.project.clone();
    shift_m(&mut rig);
    assert_eq!(rig.ws.project, before);
    assert!(rig.ws.status.contains("cut"), "{}", rig.ws.status);
}

#[test]
fn the_lower_band_keeps_its_look_when_a_cut_is_merged() {
    let mut rig = cut_wall();
    let (brick, moss) = (material(&mut rig, "brick"), material(&mut rig, "moss"));
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 2, 300.0));
    rig.ws.choose_palette_material(brick);
    rig.click(on_wall(&rig, 2, 1200.0));
    rig.ws.choose_palette_material(moss);
    assert_eq!(bands_of(&rig, 2), vec![Some(brick), Some(moss)]);
    rig.tool(GridTool::Bands);
    rig.ws.grid.band_line = Some((north(2), 0));
    shift_m(&mut rig);
    assert_eq!(bands_of(&rig, 2), vec![Some(brick)], "{}", rig.ws.status);
    // The other segments of the run lost the cut too.
    assert_eq!(lines(&rig, 0), Vec::<Anchor>::new());
}

// ---------------------------------------------------------------------
// Select tool: two neighbouring bands of a wall.

fn two_bands_selected(first_upper: bool) -> (GridRig, ResourceId, ResourceId) {
    let mut rig = cut_wall();
    let (brick, moss) = (material(&mut rig, "brick"), material(&mut rig, "moss"));
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 2, 300.0));
    rig.ws.choose_palette_material(brick);
    rig.click(on_wall(&rig, 2, 1200.0));
    rig.ws.choose_palette_material(moss);
    let (first, second) = if first_upper {
        (1200.0, 300.0)
    } else {
        (300.0, 1200.0)
    };
    rig.click(on_wall(&rig, 2, first));
    rig.click_with(on_wall(&rig, 2, second), egui::Modifiers::COMMAND);
    assert_eq!(
        rig.ws.grid.selection.bands(rig.world()).len(),
        2,
        "{:?}",
        rig.ws.grid.selection.items()
    );
    (rig, brick, moss)
}

#[test]
fn two_selected_bands_merge_and_the_first_selected_keeps_its_material() {
    let (mut rig, brick, moss) = two_bands_selected(true);
    let before = rig.ws.project.clone();
    shift_m(&mut rig);
    assert_eq!(rig.ws.grid.tool, GridTool::Select);
    assert_eq!(bands_of(&rig, 2), vec![Some(moss)], "{}", rig.ws.status);
    // One undo step.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, before);
    // The other way round, the other material wins.
    let (mut rig, brick2, _) = two_bands_selected(false);
    let _ = (brick, brick2);
    shift_m(&mut rig);
    assert_eq!(bands_of(&rig, 2).len(), 1, "{}", rig.ws.status);
    assert_eq!(bands_of(&rig, 2), vec![Some(brick2)]);
}

#[test]
fn one_selected_band_is_not_enough_to_merge() {
    let mut rig = cut_wall();
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 2, 300.0));
    let before = rig.ws.project.clone();
    shift_m(&mut rig);
    assert_eq!(rig.ws.project, before);
    assert!(rig.ws.status.contains("two"), "{}", rig.ws.status);
}

// ---------------------------------------------------------------------
// Cells.

fn halls() -> GridRig {
    GridRig::top("merge-cells", derived_project_for_tiling(8, 8))
}

#[test]
fn shift_m_in_the_cells_tool_joins_the_cell_under_the_pointer_with_its_siblings() {
    let mut rig = halls();
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(4, 4));
    assert_eq!(rig.room(0).leaf_count(), 67);
    let split = rig.ws.project.clone();
    rig.move_to(rig.cell_centre(4, 4), egui::Modifiers::NONE);
    shift_m(&mut rig);
    assert_eq!(rig.ws.grid.tool, GridTool::Cells);
    assert_eq!(rig.room(0).leaf_count(), 64, "{}", rig.ws.status);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, split);
}

#[test]
fn shift_m_in_the_select_tool_merges_the_selected_sibling_cells() {
    let mut rig = halls();
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(4, 4));
    rig.tool(GridTool::Select);
    // Two of the four quarter cells.
    let q = S / 4.0;
    rig.click(rig.top_at(4.0 * S + q, 4.0 * S + q));
    rig.click_with(
        rig.top_at(4.0 * S + 3.0 * q, 4.0 * S + q),
        egui::Modifiers::COMMAND,
    );
    assert!(rig.ws.grid.selection.len() >= 2, "{}", rig.ws.status);
    let split = rig.ws.project.clone();
    shift_m(&mut rig);
    assert_eq!(rig.ws.grid.tool, GridTool::Select);
    assert_eq!(rig.room(0).leaf_count(), 64, "{}", rig.ws.status);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, split);
}

// ---------------------------------------------------------------------
// The buttons and the help.

fn settle(rig: &mut WindowRig) {
    for _ in 0..3 {
        rig.frame(vec![]);
    }
}

#[test]
fn the_bands_options_row_has_a_merge_button_for_the_selected_cut() {
    let mut rig = WindowRig::new();
    let button = rig.at("3D");
    rig.click(button);
    settle(&mut rig);
    rig.ws.set_grid_tool(GridTool::Bands);
    settle(&mut rig);
    // No cut picked: the button is there but the document is untouched.
    assert!(rig.find("Merge bands").is_some());
    let wall = FaceRef::derived(1, [10, -1], WallEdge::North, DerivedKind::Closure);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        assert!(psxed_project::grid::edit::bands::edit_bands(
            world,
            wall,
            psxed_project::grid::edit::bands::BandScope::Run,
            |set| set.insert_line(0, Anchor::Abs(1536))
        ));
    }
    rig.ws.grid_changed();
    rig.ws.grid.band_line = Some((wall, 0));
    settle(&mut rig);
    let at = rig.at("Merge bands");
    rig.click(at);
    settle(&mut rig);
    let set = effective_set(rig.world(), wall);
    assert!(
        set.is_none_or(|(_, s)| s.lines.is_empty()),
        "{}",
        rig.ws.status
    );
}

#[test]
fn the_select_options_row_offers_merge_for_bands_and_for_cells_only_when_they_can() {
    let mut rig = WindowRig::new();
    let button = rig.at("3D");
    rig.click(button);
    settle(&mut rig);
    rig.ws.set_grid_tool(GridTool::Select);
    settle(&mut rig);
    assert!(rig.find("Merge bands").is_none());
    assert!(rig.find("Merge cells").is_none());
    // Two neighbouring bands of one wall.
    let wall = FaceRef::derived(1, [10, -1], WallEdge::North, DerivedKind::Closure);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        assert!(psxed_project::grid::edit::bands::edit_bands(
            world,
            wall,
            psxed_project::grid::edit::bands::BandScope::Run,
            |set| set.insert_line(0, Anchor::Abs(1536))
        ));
    }
    rig.ws.grid_changed();
    rig.ws.grid.selection.set([
        GridElement::Band {
            face: wall,
            band: 0,
        },
        GridElement::Band {
            face: wall,
            band: 1,
        },
    ]);
    settle(&mut rig);
    let at = rig.find("Merge bands").expect("a Merge bands button");
    rig.click(at);
    settle(&mut rig);
    assert!(effective_set(rig.world(), wall).is_none_or(|(_, s)| s.lines.is_empty()));
    // Sibling cells of a split sector.
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let room = &mut world.rooms[1];
        let key = psxed_project::grid::LeafKey::sector([10, 2]);
        psxed_project::grid::tree::subdivide(room, key).unwrap();
    }
    rig.ws.grid_changed();
    rig.ws.grid.selection.set([GridElement::Cell {
        room: 1,
        cell: [10, 2],
    }]);
    settle(&mut rig);
    let before = rig.world().rooms[1].leaf_count();
    let at = rig.find("Merge cells").expect("a Merge cells button");
    rig.click(at);
    settle(&mut rig);
    assert_eq!(rig.world().rooms[1].leaf_count(), before - 4 + 1);
}

#[test]
fn the_help_page_names_the_merge_key() {
    use super::grid_ui::{draw_editor_sized, full_context};
    let ctx = full_context();
    let mut ws = EditorWorkspace::with_project(
        test_temp_dir("merge-help"),
        derived_project_for_tiling(4, 4),
    );
    ws.apply_grid_pose("help").unwrap();
    draw_editor_sized(&mut ws, &ctx, true, Vec2::new(1920.0, 5000.0));
    let out = draw_editor_sized(&mut ws, &ctx, true, Vec2::new(1920.0, 5000.0));
    let text: String = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for needle in ["Shift+M", "Merge"] {
        assert!(text.contains(needle), "{needle}:\n{text}");
    }
}

/// How many faces the north wall of cell `x` is drawn with.
fn faces_on_north(rig: &GridRig, x: i32) -> usize {
    psxed_project::grid::faces::all_faces(rig.world())
        .iter()
        .filter(|f| {
            f.cell == [x, 0]
                && matches!(
                    f.kind,
                    psxed_project::grid::GridFaceKind::Derived {
                        edge: WallEdge::North,
                        ..
                    }
                )
        })
        .count()
}
