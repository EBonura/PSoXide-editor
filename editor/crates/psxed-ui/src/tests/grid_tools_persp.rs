//! The grid tools driven in the 3D view, with real depth-aware picking.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::WallEdge;

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

fn ground(rig: &GridRig, x: f32, z: f32, y: f32) -> Pos2 {
    rig.screen_of([x * S, y, z * S])
}

#[test]
fn the_floor_tool_drags_a_rectangle_over_the_plane_in_3d() {
    let mut rig = GridRig::persp("persp-floor", empty_project());
    rig.tool(GridTool::Floor);
    rig.drag(ground(&rig, 1.5, 1.5, 0.0), ground(&rig, 3.5, 2.5, 0.0));
    let room = rig.room(0);
    assert_eq!((room.origin, room.width, room.depth), ([1, 1], 3, 2));
    assert_eq!(floor_heights(rig.world(), 0, [3, 2]), Some([0; 4]));
}

#[test]
fn shift_in_3d_turns_pointer_height_into_surface_height() {
    let mut rig = GridRig::persp("persp-floor-height", empty_project());
    rig.tool(GridTool::Floor);
    let a = ground(&rig, 1.5, 1.5, 0.0);
    let b = ground(&rig, 2.5, 1.5, 0.0);
    rig.drag_hold(a, b, egui::Modifiers::NONE);
    rig.move_to(b, shift());
    // Move the pointer up the screen by roughly one click's projected size.
    let one_click = (rig.screen_of([2.0 * S, 0.0, 1.0 * S]).y
        - rig.screen_of([2.0 * S, 256.0, 1.0 * S]).y)
        .abs();
    rig.move_to(b + Vec2::new(0.0, -2.0 * one_click), shift());
    let Gesture::Rect(g) = &rig.ws.grid.gesture else {
        panic!("rect gesture expected");
    };
    assert_eq!(g.height, 512, "{g:?}");
    rig.release(b, shift());
    assert_eq!(floor_heights(rig.world(), 0, [1, 1]), Some([512; 4]));
}

#[test]
fn select_picks_the_vertex_then_the_edge_then_the_face() {
    let mut rig = GridRig::persp("persp-pick", flat_project(5, 5, false));
    rig.tool(GridTool::Select);
    // On a lattice vertex (shared by four floors): a vertex.
    let corner = ground(&rig, 2.0, 2.0, 0.0);
    rig.click(corner);
    assert!(
        matches!(
            rig.ws.grid.selection.primary(),
            Some(GridElement::Vertex(_))
        ),
        "{:?}",
        rig.ws.grid.selection
    );
    // Half way along a cell edge, away from corners: an edge.
    let edge_mid = ground(&rig, 2.5, 2.0, 0.0);
    rig.click(edge_mid);
    assert!(
        matches!(rig.ws.grid.selection.primary(), Some(GridElement::Edge(_))),
        "{:?}",
        rig.ws.grid.selection
    );
    // The middle of a cell: the floor face.
    let middle = ground(&rig, 2.5, 2.5, 0.0);
    rig.click(middle);
    assert_eq!(
        rig.ws.grid.selection.items(),
        &[GridElement::Face(FaceRef::floor(0, [2, 2]))]
    );
}

#[test]
fn a_vertex_hidden_behind_a_wall_is_not_picked_but_the_wall_is() {
    let mut rig = GridRig::persp("persp-depth", flat_project(5, 5, false));
    // A tall wall on the south edge of cell (2,3): between the camera
    // (south) and the floor vertices of the lattice line z = 3.
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms[0]
        .sector_mut(2, 3)
        .unwrap()
        .north
        .push(psxed_project::grid::GridWall::span(0, 3072, None));
    rig.tool(GridTool::Select);
    // The floor vertex at (2, 2) is behind the wall at z = 3 as seen from
    // the camera far to the south, but only if the wall covers it on
    // screen. Aim at the wall's face instead and expect the wall.
    let wall_mid = rig.screen_of([2.5 * S, 1000.0, 3.0 * S]);
    rig.click(wall_mid);
    match rig.ws.grid.selection.primary() {
        Some(GridElement::Face(f)) => {
            assert!(matches!(
                f.target,
                psxed_project::grid::edit::FaceTarget::Wall { .. }
            ));
        }
        other => panic!("expected the wall face, got {other:?}"),
    }
    // A point on the floor right behind the wall's base, hidden by it, must
    // not offer the floor vertex lying behind.
    let behind = rig.screen_of([2.0 * S, 0.0, 2.0 * S]);
    let hit = rig.ws.pick_element_3d(
        rig.persp_rect(),
        behind,
        crate::grid_tools::pick::PickLevel::Vertex,
    );
    if let Some((GridElement::Vertex(v), _)) = hit {
        panic!("picked a hidden vertex {v:?}");
    }
}

#[test]
fn shift_drag_raises_a_selected_vertex_by_clicks_in_3d() {
    let mut rig = GridRig::persp("persp-shift-drag", flat_project(5, 5, false));
    rig.tool(GridTool::Select);
    let corner = ground(&rig, 2.0, 2.0, 0.0);
    rig.click(corner);
    let one_click = (rig.screen_of([2.0 * S, 0.0, 2.0 * S]).y
        - rig.screen_of([2.0 * S, 256.0, 2.0 * S]).y)
        .abs();
    rig.drag_with(corner, corner + Vec2::new(0.0, -3.0 * one_click), shift());
    let w = rig.world();
    // The four floors that meet at the vertex all lean up by 3 clicks.
    assert_eq!(floor_heights(w, 0, [1, 1]).unwrap()[2], 768);
    assert_eq!(floor_heights(w, 0, [2, 1]).unwrap()[3], 768);
    assert_eq!(floor_heights(w, 0, [1, 2]).unwrap()[1], 768);
    assert_eq!(floor_heights(w, 0, [2, 2]).unwrap()[0], 768);
    assert_eq!(floor_heights(w, 0, [0, 0]).unwrap(), [0; 4]);
}

#[test]
fn the_height_tool_lifts_a_face_in_3d_and_undo_restores_it() {
    let mut rig = GridRig::persp("persp-height-face", flat_project(5, 5, false));
    rig.tool(GridTool::Height);
    let middle = ground(&rig, 2.5, 2.5, 0.0);
    let one_click = (rig.screen_of([2.5 * S, 0.0, 2.5 * S]).y
        - rig.screen_of([2.5 * S, 256.0, 2.5 * S]).y)
        .abs();
    rig.drag(middle, middle + Vec2::new(0.0, -2.0 * one_click));
    assert_eq!(floor_heights(rig.world(), 0, [2, 2]).unwrap(), [512; 4]);
    rig.ws.do_undo();
    assert_eq!(floor_heights(rig.world(), 0, [2, 2]).unwrap(), [0; 4]);
}

#[test]
fn walls_face_the_side_the_camera_is_on() {
    let mut rig = GridRig::persp("persp-wall-facing", flat_project(5, 5, true));
    rig.tool(GridTool::Wall);
    // The camera is south of the grid. Click the floor just inside the
    // north edge of cell (2,2): the wall belongs to (2,2) and faces +z.
    rig.click(ground(&rig, 2.5, 2.1, 0.0));
    let from_inside = walls_on(rig.world(), 0, [2, 2], WallEdge::North);
    assert_eq!(from_inside.len(), 1);
    assert_eq!(from_inside[0].facing, WallFacing::Front);
    // Clicking the floor just north of that same line puts the wall on the
    // south edge of (2,1), whose inside is away from the camera: it faces
    // the camera as a Back wall.
    let mut rig = GridRig::persp("persp-wall-back", flat_project(5, 5, true));
    rig.tool(GridTool::Wall);
    rig.click(ground(&rig, 2.5, 1.9, 0.0));
    let from_far_side = walls_on(rig.world(), 0, [2, 1], WallEdge::South);
    assert_eq!(from_far_side.len(), 1);
    assert_eq!(from_far_side[0].facing, WallFacing::Back);
}

#[test]
fn hovering_the_wall_tool_shows_the_gap_it_would_fill() {
    let mut rig = GridRig::persp("persp-wall-hover", flat_project(5, 5, true));
    rig.tool(GridTool::Wall);
    rig.move_to(ground(&rig, 2.5, 2.9, 0.0), egui::Modifiers::NONE);
    match &rig.ws.grid.hover {
        crate::grid_tools::GridHover::Wall {
            cell,
            edge,
            heights,
            ..
        } => {
            assert_eq!((*cell, *edge), ([2, 2], WallEdge::South));
            assert_eq!(*heights, Some([0, 0, 3072, 3072]));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_marquee_in_3d_selects_the_facing_faces_inside_it() {
    let mut rig = GridRig::persp("persp-marquee", flat_project(5, 5, false));
    rig.tool(GridTool::Select);
    // A screen-space box from one ground point to another.
    let (from, to) = (ground(&rig, 0.7, 0.7, 0.0), ground(&rig, 3.3, 3.3, 0.0));
    rig.drag(from, to);
    let boxed = Rect::from_two_pos(from, to);
    let selected: Vec<_> = rig.ws.grid.selection.items().to_vec();
    assert!(selected.len() >= 4, "{}", selected.len());
    for element in &selected {
        let GridElement::Face(f) = element else {
            panic!("faces expected, got {element:?}");
        };
        let c = f.cell();
        let centre = rig.screen_of([(c[0] as f32 + 0.5) * S, 0.0, (c[1] as f32 + 0.5) * S]);
        assert!(boxed.contains(centre), "{f:?} outside the box");
    }
    // Cells well outside the box are not in.
    assert!(!rig
        .ws
        .grid
        .selection
        .contains(&GridElement::Face(FaceRef::floor(0, [4, 4]))));
}

#[test]
fn paint_in_3d_paints_the_face_under_the_pointer() {
    let mut rig = GridRig::persp("persp-paint", flat_project(5, 5, false));
    let id = rig.ws.project.add_resource(
        "brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    rig.ws.grid.material = Some(id);
    rig.tool(GridTool::Paint);
    rig.click(ground(&rig, 2.5, 2.5, 0.0));
    let painted = |cell: [i32; 2]| {
        let r = rig.room(0);
        let [x, z] = r.local(cell).unwrap();
        r.sector(x, z)
            .unwrap()
            .ground_floor()
            .unwrap()
            .tri_a
            .material
    };
    assert_eq!(painted([2, 2]), Some(id));
    assert_eq!(painted([1, 2]), None);
}

#[test]
fn the_place_tool_stands_on_the_floor_height_under_the_pointer_in_3d() {
    let mut rig = GridRig::persp("persp-place", flat_project(5, 5, false));
    rig.tool(GridTool::Place);
    rig.ws.place_kind = PlaceKind::PointLightMarker;
    rig.click(ground(&rig, 2.3, 2.6, 0.0));
    let node = rig
        .ws
        .project
        .active_scene()
        .nodes()
        .last()
        .unwrap()
        .clone();
    let t = node.transform.translation;
    assert_eq!((t[0], t[2]), (2.5 * S, 2.5 * S));
}

#[test]
fn in_a_grid_world_wasd_only_flies_the_free_camera_while_looking() {
    let mut rig = GridRig::persp("persp-wasd", flat_project(3, 3, false));
    let before = rig.ws.camera_rig.free_position;
    rig.move_to(Pos2::new(400.0, 300.0), egui::Modifiers::NONE);
    rig.key(egui::Key::W);
    assert_eq!(rig.ws.grid.tool, GridTool::Wall, "W is the Wall tool");
    assert_eq!(rig.ws.camera_rig.free_position, before, "and does not fly");
    rig.key(egui::Key::D);
    assert_eq!(rig.ws.grid.tool, GridTool::Split);
}

#[test]
fn pointing_at_an_existing_floor_makes_the_next_rectangle_continue_at_its_height() {
    let mut project = flat_project(3, 3, false);
    for (_, sector) in project.active_scene_mut().world.grid_mut().unwrap().rooms[0]
        .leaves
        .iter_mut()
    {
        sector.set_ground_floor(Some(psxed_project::grid::GridSurface::flat(512, None)));
    }
    let mut rig = GridRig::persp("persp-continue-height", project);
    rig.tool(GridTool::Floor);
    assert_eq!(rig.ws.grid.floor_height, 0);
    // Hovering the platform shows the ghost at its level and changes no
    // setting.
    rig.move_to(ground(&rig, 1.5, 1.5, 512.0), egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid.floor_height, 0);
    assert!(matches!(
        rig.ws.grid.hover,
        crate::grid_tools::GridHover::Cell { height: 512, .. }
    ));
    // Pressing on it takes its level, and the drag out over the void next to
    // it makes cells level with it.
    rig.drag(ground(&rig, 1.5, 1.5, 512.0), ground(&rig, 4.5, 1.5, 512.0));
    assert_eq!(rig.ws.grid.floor_height, 512);
    assert_eq!(floor_heights(rig.world(), 0, [3, 1]), Some([512; 4]));
    assert_eq!(floor_heights(rig.world(), 0, [4, 1]), Some([512; 4]));
}
