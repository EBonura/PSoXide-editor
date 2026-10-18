//! Derived walls through the editor: seeing, picking and painting a face
//! the room computes, the wall tool's gap logic, and the Convert command.

use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;
use crate::grid_tools::{GridHover, GridTool};
use psxed_project::grid::derive::{self, DerivedKind};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::faces::all_faces;
use psxed_project::grid::{GridSurface, WallEdge, WallMode};

fn ground(rig: &GridRig, x: f32, z: f32, y: f32) -> Pos2 {
    rig.screen_of([x * S, y, z * S])
}

/// `flat_project` with its room switched to derived walls.
pub(super) fn derived_project_for_tiling(width: usize, depth: usize) -> ProjectDocument {
    derived_project(width, depth)
}

fn derived_project(width: usize, depth: usize) -> ProjectDocument {
    let mut project = flat_project(width, depth, true);
    project.active_scene_mut().world.grid_mut().unwrap().rooms[0].wall_mode = WallMode::Derived;
    project
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

#[test]
fn a_derived_room_has_no_wall_records_but_draws_its_closure() {
    let rig = GridRig::persp("derived-closure", derived_project(5, 5));
    let faces = all_faces(rig.world());
    // 25 floors, 25 ceilings, 20 outline edges: one face each, however tall.
    assert_eq!(faces.len(), 25 + 25 + 20);
    assert!(rig
        .room(0)
        .iter()
        .all(|(_, _, sector)| sector.wall_count() == 0));
}

#[test]
fn painting_a_derived_wall_in_3d_writes_the_override_not_a_wall_record() {
    let mut rig = GridRig::persp("derived-paint", derived_project(5, 5));
    let id = rig.ws.project.add_resource(
        "brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    rig.ws.grid.material = Some(id);
    rig.tool(GridTool::Paint);
    // The camera is south of the room looking north, so it sees the inside
    // of the north outline wall of cell (2, 0).
    rig.click(ground(&rig, 2.5, 0.0, 1500.0));
    let key = derive::edge_key([2, 0], WallEdge::North);
    let painted = rig.room(0).edge_overrides.get(&key);
    assert_eq!(
        painted.and_then(|o| o.material),
        Some(id),
        "{}",
        rig.ws.status
    );
    let sector = rig.room(0).sector(2, 0).unwrap();
    assert_eq!(sector.wall_count(), 0, "no wall record was created");
    // Neighbouring segments are untouched, and one undo step reverts it.
    assert_eq!(rig.room(0).edge_overrides.len(), 1);
    rig.ws.do_undo();
    assert!(rig.room(0).edge_overrides.is_empty());
}

#[test]
fn the_select_tool_picks_a_derived_wall_line_in_the_top_view() {
    let mut rig = GridRig::top("derived-pick", derived_project(5, 5));
    rig.tool(GridTool::Select);
    // The west outline of cell (0, 2): click on its edge.
    rig.click(rig.top_at(0.0, 2.5 * S));
    let want = FaceRef::derived(0, [0, 2], WallEdge::West, DerivedKind::Closure);
    assert!(
        rig.ws.grid.selection.contains(&GridElement::Face(want)),
        "{:?}",
        rig.ws.grid.selection.items()
    );
}

#[test]
fn a_raised_platform_shows_its_risers_and_hiding_one_survives_undo() {
    let mut rig = GridRig::persp("derived-platform", derived_project(5, 5));
    let before = all_faces(rig.world()).len();
    {
        let room = &mut rig
            .ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .rooms[0];
        room.sector_or_insert(2, 2)
            .unwrap()
            .set_ground_floor(Some(GridSurface::flat(512, None)));
    }
    rig.ws.grid_changed();
    assert_eq!(all_faces(rig.world()).len(), before + 4);
    // Select one riser from the Top-view style pick and delete it: it hides.
    let riser = FaceRef::derived(0, [2, 1], WallEdge::South, DerivedKind::Riser);
    assert!(riser.exists(rig.world()));
    rig.ws.grid.selection.set(vec![GridElement::Face(riser)]);
    rig.tool(GridTool::Select);
    rig.key(egui::Key::Delete);
    assert_eq!(all_faces(rig.world()).len(), before + 3);
    rig.ws.do_undo();
    assert_eq!(all_faces(rig.world()).len(), before + 4);
}

#[test]
fn the_wall_tool_sees_a_derived_wall_as_already_standing() {
    let mut rig = GridRig::persp("derived-wall-gap", derived_project(5, 5));
    rig.tool(GridTool::Wall);
    // Just inside the north outline of cell (2, 0): the room closes it.
    rig.move_to(ground(&rig, 2.5, 0.1, 0.0), egui::Modifiers::NONE);
    match &rig.ws.grid.hover {
        GridHover::Wall {
            cell,
            edge,
            heights,
            ..
        } => {
            assert_eq!((*cell, *edge), ([2, 0], WallEdge::North));
            assert_eq!(*heights, None, "the derived wall covers it");
        }
        other => panic!("{other:?}"),
    }
    rig.click(ground(&rig, 2.5, 0.1, 0.0));
    assert_eq!(rig.room(0).sector(2, 0).unwrap().wall_count(), 0);
    // An authored room offers the full gap on the same edge.
    let mut legacy = GridRig::persp("authored-wall-gap", flat_project(5, 5, true));
    legacy.tool(GridTool::Wall);
    legacy.move_to(ground(&legacy, 2.5, 0.1, 0.0), egui::Modifiers::NONE);
    assert!(matches!(
        legacy.ws.grid.hover,
        GridHover::Wall {
            heights: Some(_),
            ..
        }
    ));
}

#[test]
fn convert_makes_a_legacy_room_derived_in_one_undoable_step() {
    let mut project = flat_project(4, 4, true);
    // A legacy outline: a wall on every north edge of the top row.
    {
        let room = &mut project.active_scene_mut().world.grid_mut().unwrap().rooms[0];
        for x in 0..4 {
            room.sector_or_insert(x, 0)
                .unwrap()
                .walls_mut(WallEdge::North)
                .push(psxed_project::grid::GridWall::span(0, 3072, None));
        }
    }
    let mut rig = GridRig::top("derived-convert", project);
    let faces_before = all_faces(rig.world()).len();
    let snapshot = rig.ws.project.clone();
    rig.ws
        .grid_convert_room(0, psxed_project::grid::edit::ConvertMode::PreserveLook);
    assert_eq!(rig.room(0).wall_mode, WallMode::Derived);
    // Exactly the four north walls became derived faces; the three other
    // sides stay open as they were. A derived wall is one face where the
    // authored one was cut into three courses, so the count fell by two a
    // wall.
    assert_eq!(
        rig.room(0)
            .iter()
            .map(|(_, _, s)| s.wall_count())
            .sum::<usize>(),
        0
    );
    assert_eq!(all_faces(rig.world()).len(), faces_before - 4 * 2);
    assert!(
        rig.ws.status.contains("4 walls converted"),
        "{}",
        rig.ws.status
    );
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, snapshot);
}

#[test]
fn the_rooms_panel_offers_convert_for_legacy_rooms_and_the_defaults_for_derived_ones() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("derived-panel"), flat_project(3, 3, true));
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    let legacy = text_of(&draw_editor(&mut ws, &ctx, true));
    assert!(legacy.contains("authored (legacy)"), "{legacy}");
    assert!(legacy.contains("Convert to derived"), "{legacy}");
    ws.grid_convert_room(0, psxed_project::grid::edit::ConvertMode::FillNewWalls);
    let derived = text_of(&draw_editor(&mut ws, &ctx, true));
    assert!(derived.contains("derived from heights"), "{derived}");
    assert!(derived.contains("Open wall"), "{derived}");
    assert!(!derived.contains("Convert to derived"), "{derived}");
}
