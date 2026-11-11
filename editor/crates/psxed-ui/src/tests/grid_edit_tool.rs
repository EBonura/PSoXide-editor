//! The Edit tool replayed through real input in the 3D view: the acceptance
//! tasks of the review (part c) the tool covers, plus the cases the owner
//! asked about (slide a wall keeping its height, tilt a selected room by one
//! edge, raise a rectangle as a platform in two steps).

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::surface::GridWall;
use psxed_project::grid::WallEdge;

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

fn at(rig: &GridRig, x: f32, y: f32, z: f32) -> Pos2 {
    rig.screen_of([x * S, y * S, z * S])
}

fn edit_rig(label: &str, project: ProjectDocument) -> GridRig {
    let mut rig = GridRig::persp(label, project);
    rig.tool(GridTool::Edit);
    rig
}

fn hint(rig: &GridRig) -> String {
    rig.ws.grid.edit.hint.clone()
}

fn heights(rig: &GridRig, cell: [i32; 2]) -> Option<[i32; 4]> {
    floor_heights(rig.world(), 0, cell)
}

fn ceiling(rig: &GridRig, cell: [i32; 2]) -> Option<[i32; 4]> {
    let r = rig.room(0);
    let [x, z] = r.local(cell)?;
    r.sector(x, z)?.ground_ceiling().map(|c| c.heights)
}

/// A flat room with a hand-placed wall on the north edge of cell (2, 2).
fn walled() -> ProjectDocument {
    let mut project = flat_project(5, 5, true);
    let room = &mut project.active_scene_mut().world.grid_mut().unwrap().rooms[0];
    room.sector_or_insert(2, 2)
        .unwrap()
        .walls_mut(WallEdge::North)
        .push(GridWall::span(0, 1024, None));
    project
}

/// A derived room with a low ceiling, so its walls fit the default view.
fn low_derived() -> ProjectDocument {
    let mut project = derived_project_for_tiling(5, 5);
    let room = &mut project.active_scene_mut().world.grid_mut().unwrap().rooms[0];
    for z in 0..5 {
        for x in 0..5 {
            if let Some(sector) = room.sector_mut(x, z) {
                if let Some(c) = sector.ground_ceiling_mut() {
                    c.heights = [1024; 4];
                }
            }
        }
    }
    project
}

fn walls(rig: &GridRig) -> Vec<GridWall> {
    walls_on(rig.world(), 0, [2, 2], WallEdge::North)
}

#[test]
fn hover_names_what_a_drag_will_do_before_any_press() {
    let mut rig = edit_rig("edit-hover", flat_project(5, 5, true));
    rig.move_to(at(&rig, 2.5, 0.0, 2.5), egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: raise floor"),
        "{}",
        hint(&rig)
    );
    assert!(matches!(
        rig.ws.grid.hover,
        crate::grid_tools::GridHover::Elements(ref e) if e == &[GridElement::Face(FaceRef::floor(0, [2, 2]))]
    ));
    // Near a floor edge: a slope.
    rig.move_to(at(&rig, 2.5, 0.0, 2.0), egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: slope from this edge"),
        "{}",
        hint(&rig)
    );
    // On a lattice corner: a lift.
    rig.move_to(at(&rig, 2.0, 0.0, 2.0), egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: lift this corner"),
        "{}",
        hint(&rig)
    );
    // Shift boxes.
    rig.move_to(at(&rig, 2.5, 0.0, 2.5), shift());
    assert!(hint(&rig).contains("box select"), "{}", hint(&rig));
}

#[test]
fn raising_one_tile_is_one_undo_step_and_selects_it() {
    let mut rig = edit_rig("edit-raise", flat_project(5, 5, true));
    let before = rig.ws.project.clone();
    let from = at(&rig, 2.5, 0.0, 2.5);
    rig.drag(from, at(&rig, 2.5, 512.0 / S, 2.5));
    assert_eq!(heights(&rig, [2, 2]), Some([512; 4]));
    assert_eq!(
        rig.ws.grid.selection.items(),
        &[GridElement::Face(FaceRef::floor(0, [2, 2]))]
    );
    // Neighbours stay: weld is off.
    assert_eq!(heights(&rig, [1, 2]), Some([0; 4]));
    rig.ws.do_undo();
    assert!(rig.ws.project == before, "undo restores the document");
}

#[test]
fn a_far_tile_picks_as_a_face_and_never_tilts() {
    let mut rig = edit_rig("edit-far", flat_project(5, 5, true));
    // Back the camera off until a tile is about 14 pixels wide.
    rig.ws.camera_rig.free_position = [3 * S as i32, 60000, 40 * S as i32];
    let (yaw, pitch) = camera_angles_to_look_at(
        rig.ws.camera_rig.free_position,
        [3 * S as i32, 0, 2 * S as i32],
    )
    .unwrap();
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    rig.pump(vec![], egui::Modifiers::NONE);
    let width = (at(&rig, 3.0, 0.0, 2.5) - at(&rig, 2.0, 0.0, 2.5)).length();
    assert!(width < 20.0, "tile is {width} px wide");
    let centre = at(&rig, 2.5, 0.0, 2.5);
    rig.move_to(centre, egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: raise floor"),
        "{}",
        hint(&rig)
    );
    // Even a press a pixel from the edge is the face.
    let near_edge = at(&rig, 2.5, 0.0, 2.04);
    rig.move_to(near_edge, egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: raise floor"),
        "{}",
        hint(&rig)
    );
}

#[test]
fn a_box_picks_four_tiles_and_a_drag_lifts_them_together() {
    let mut rig = edit_rig("edit-box", flat_project(5, 5, true));
    rig.drag_with(at(&rig, 1.1, 0.0, 1.1), at(&rig, 2.9, 0.0, 2.9), shift());
    assert_eq!(
        rig.ws.grid.selection.len(),
        4,
        "{:?}",
        rig.ws.grid.selection
    );
    let history = rig.ws.history.can_undo();
    rig.drag(at(&rig, 1.5, 0.0, 1.5), at(&rig, 1.5, 512.0 / S, 1.5));
    for cell in [[1, 1], [2, 1], [1, 2], [2, 2]] {
        assert_eq!(heights(&rig, cell), Some([512; 4]), "{cell:?}");
    }
    assert_eq!(heights(&rig, [3, 3]), Some([0; 4]));
    // One undo step for the lift (the box made none).
    rig.ws.do_undo();
    assert_eq!(heights(&rig, [1, 1]), Some([0; 4]));
    assert_eq!(rig.ws.history.can_undo(), history);
}

#[test]
fn a_press_on_an_unselected_tile_replaces_the_selection_and_acts_alone() {
    let mut rig = edit_rig("edit-unselected", flat_project(5, 5, true));
    rig.click(at(&rig, 1.5, 0.0, 1.5));
    rig.drag(at(&rig, 3.5, 0.0, 3.5), at(&rig, 3.5, 256.0 / S, 3.5));
    assert_eq!(heights(&rig, [3, 3]), Some([256; 4]));
    assert_eq!(heights(&rig, [1, 1]), Some([0; 4]));
    assert_eq!(
        rig.ws.grid.selection.items(),
        &[GridElement::Face(FaceRef::floor(0, [3, 3]))]
    );
}

#[test]
fn an_edge_slopes_from_the_opposite_edge() {
    let mut rig = edit_rig("edit-edge", flat_project(5, 5, true));
    rig.drag(at(&rig, 2.5, 0.0, 2.04), at(&rig, 2.5, 256.0 / S, 2.04));
    // North edge: corners 0 and 1 rise, the south ones stay.
    assert_eq!(heights(&rig, [2, 2]), Some([256, 256, 0, 0]));
}

fn tilted_room_rig(label: &str, width: usize) -> GridRig {
    let mut rig = edit_rig(label, flat_project(width, 4, true));
    // The room's whole floor, by double click.
    let p = at(&rig, 1.5, 0.0, 1.5);
    rig.click(p);
    rig.click(p);
    rig
}

#[test]
fn a_selected_room_tilts_about_the_far_edge_taking_its_share_per_cell() {
    let mut rig = tilted_room_rig("edit-tilt", 4);
    assert_eq!(
        rig.ws.grid.selection.len(),
        16,
        "{:?}",
        rig.ws.grid.selection.len()
    );
    let before = rig.ws.project.clone();
    // The east edge of the room, middle of its last cell.
    rig.drag(at(&rig, 4.0, 0.0, 1.5), at(&rig, 4.0, 512.0 / S, 1.5));
    assert_eq!(heights(&rig, [0, 1]), Some([0, 128, 128, 0]));
    assert_eq!(heights(&rig, [3, 1]), Some([384, 512, 512, 384]));
    assert!(
        rig.ws.status.contains("clicks") || rig.ws.status.contains("Slope"),
        "{}",
        rig.ws.status
    );
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

#[test]
fn a_platform_inside_a_tilted_room_keeps_its_offset() {
    let mut rig = edit_rig("edit-tilt-platform", flat_project(4, 4, true));
    rig.drag(at(&rig, 0.5, 0.0, 0.5), at(&rig, 0.5, 256.0 / S, 0.5));
    let p = at(&rig, 1.5, 0.0, 1.5);
    rig.click(p);
    rig.click(p);
    rig.drag(at(&rig, 4.0, 0.0, 1.5), at(&rig, 4.0, 512.0 / S, 1.5));
    assert_eq!(heights(&rig, [0, 0]), Some([256, 384, 384, 256]));
    assert_eq!(heights(&rig, [0, 1]), Some([0, 128, 128, 0]));
}

#[test]
fn too_gentle_a_slope_is_refused_and_changes_nothing() {
    let mut rig = tilted_room_rig("edit-gentle", 7);
    let before = rig.ws.project.clone();
    rig.drag(at(&rig, 7.0, 0.0, 1.5), at(&rig, 7.0, 256.0 / S, 1.5));
    assert!(rig.ws.project == before, "refused slope left a change");
    assert!(rig.ws.status.contains("Too gentle"), "{}", rig.ws.status);
}

#[test]
fn a_hand_placed_wall_stretches_from_its_top_edge() {
    let mut rig = edit_rig("edit-wall-top", walled());
    rig.drag(
        at(&rig, 2.5, 1024.0 / S, 2.0),
        at(&rig, 2.5, 1280.0 / S, 2.0),
    );
    assert_eq!(walls(&rig)[0].heights, [0, 0, 1280, 1280]);
}

#[test]
fn a_closing_wall_stretches_without_moving_the_ceiling_and_says_so() {
    let mut rig = edit_rig("edit-closure-top", low_derived());
    let top = at(&rig, 2.5, 1024.0 / S, 0.0);
    rig.move_to(top, egui::Modifiers::NONE);
    assert!(
        hint(&rig).contains("the floor and the ceiling stay"),
        "{}",
        hint(&rig)
    );
    rig.drag(top, at(&rig, 2.5, 768.0 / S, 0.0));
    // The ceiling stays, the wall is shorter and says what it left open.
    assert_eq!(ceiling(&rig, [2, 0]), Some([1024; 4]));
    assert_eq!(heights(&rig, [2, 0]), Some([0; 4]));
    assert!(rig.ws.status.contains("gap"), "{}", rig.ws.status);
}

#[test]
fn the_arrow_slides_a_hand_placed_wall_keeping_its_height() {
    let mut rig = edit_rig("edit-slide-wall", walled());
    // Hover the wall: its arrow shows above the top edge.
    rig.move_to(at(&rig, 2.5, 512.0 / S, 2.0), egui::Modifiers::NONE);
    let arrow = at(&rig, 2.5, 1024.0 / S, 2.0) - Vec2::new(0.0, 22.0);
    rig.move_to(arrow, egui::Modifiers::NONE);
    assert!(hint(&rig).contains("slide the wall"), "{}", hint(&rig));
    let one = (at(&rig, 2.5, 0.0, 2.0).y - at(&rig, 2.5, 256.0 / S, 2.0).y).abs();
    rig.drag(arrow, arrow - Vec2::new(0.0, one));
    assert_eq!(walls(&rig)[0].heights, [256, 256, 1280, 1280]);
}

#[test]
fn the_arrow_slides_a_closing_wall_keeping_its_height_and_the_floor_stays() {
    let mut rig = edit_rig("edit-slide-closure", low_derived());
    // The west wall of cell (0, 3) faces the camera's side of the room.
    rig.move_to(at(&rig, 0.0, 512.0 / S, 3.5), egui::Modifiers::NONE);
    let arrow = at(&rig, 0.0, 1024.0 / S, 3.5) - Vec2::new(0.0, 22.0);
    rig.move_to(arrow, egui::Modifiers::NONE);
    assert!(hint(&rig).contains("slide the wall"), "{}", hint(&rig));
    let one = (at(&rig, 0.0, 0.0, 3.5).y - at(&rig, 0.0, 256.0 / S, 3.5).y).abs();
    rig.drag(arrow, arrow - Vec2::new(0.0, one));
    assert_eq!(
        heights(&rig, [0, 3]),
        Some([0; 4]),
        "the floor does not move"
    );
    assert_eq!(ceiling(&rig, [0, 3]), Some([1024; 4]), "nor the ceiling");
    let walls = walls_on(rig.world(), 0, [0, 3], WallEdge::West);
    assert_eq!(walls.len(), 1, "the wall is its own now");
    assert_eq!(walls[0].heights, [256, 256, 1280, 1280]);
    // The selection followed the wall to its new address.
    assert!(rig
        .ws
        .grid
        .selection
        .faces(rig.world())
        .iter()
        .all(|f| f.exists(rig.world())));
}

#[test]
fn a_generated_wall_and_a_hand_placed_one_give_the_same_faces() {
    // Hand placed: a wall of the same size as the generated one.
    let mut hand = flat_project(5, 5, true);
    hand.active_scene_mut().world.grid_mut().unwrap().rooms[0]
        .sector_or_insert(2, 2)
        .unwrap()
        .walls_mut(WallEdge::North)
        .push(GridWall::span(0, 1024, None));
    let mut a = edit_rig("edit-parity-hand", hand);
    a.move_to(at(&a, 2.5, 512.0 / S, 2.0), egui::Modifiers::NONE);
    let arrow = at(&a, 2.5, 1024.0 / S, 2.0) - Vec2::new(0.0, 22.0);
    a.move_to(arrow, egui::Modifiers::NONE);
    let one = (at(&a, 2.5, 0.0, 2.0).y - at(&a, 2.5, 256.0 / S, 2.0).y).abs();
    a.drag(arrow, arrow - Vec2::new(0.0, one));
    let hand_walls = walls_on(a.world(), 0, [2, 2], WallEdge::North);

    let mut b = edit_rig("edit-parity-gen", low_derived());
    b.move_to(at(&b, 2.5, 512.0 / S, 0.0), egui::Modifiers::NONE);
    let arrow = at(&b, 2.5, 1024.0 / S, 0.0) - Vec2::new(0.0, 22.0);
    b.move_to(arrow, egui::Modifiers::NONE);
    let one = (at(&b, 2.5, 0.0, 0.0).y - at(&b, 2.5, 256.0 / S, 0.0).y).abs();
    b.drag(arrow, arrow - Vec2::new(0.0, one));
    let gen_walls = walls_on(b.world(), 0, [2, 0], WallEdge::North);
    assert_eq!(hand_walls.len(), 1);
    assert_eq!(gen_walls.len(), 1);
    assert_eq!(hand_walls[0].heights, gen_walls[0].heights);
}

#[test]
fn a_wall_body_pushes_the_room_out_a_sector() {
    let mut rig = edit_rig("edit-push", low_derived());
    let body = at(&rig, 2.5, 512.0 / S, 0.0);
    rig.move_to(body, egui::Modifiers::NONE);
    assert!(hint(&rig).contains("push the room"), "{}", hint(&rig));
    rig.drag(body, at(&rig, 2.5, 512.0 / S, -1.0));
    assert!(heights(&rig, [2, -1]).is_some(), "{}", rig.ws.status);
}

#[test]
fn typing_an_amount_sets_it_exactly_and_backspace_never_deletes() {
    let mut rig = edit_rig("edit-typed", flat_project(5, 5, true));
    let from = at(&rig, 2.5, 0.0, 2.5);
    rig.drag_hold(from, at(&rig, 2.5, 256.0 / S, 2.5), egui::Modifiers::NONE);
    rig.key(egui::Key::Minus);
    rig.key(egui::Key::Num3);
    assert_eq!(
        heights(&rig, [2, 2]),
        Some([-768; 4]),
        "live: {}",
        rig.ws.status
    );
    rig.key(egui::Key::Backspace);
    rig.key(egui::Key::Num2);
    assert_eq!(
        rig.ws.grid.selection.len(),
        1,
        "backspace kept the selection"
    );
    assert_eq!(heights(&rig, [2, 2]), Some([-512; 4]));
    rig.key(egui::Key::Enter);
    assert_eq!(heights(&rig, [2, 2]), Some([-512; 4]));
    assert!(rig.room(0).sector(2, 2).unwrap().ground_floor().is_some());
    rig.release(from, egui::Modifiers::NONE);
    assert_eq!(heights(&rig, [2, 2]), Some([-512; 4]));
    rig.ws.do_undo();
    assert_eq!(heights(&rig, [2, 2]), Some([0; 4]));
}

#[test]
fn esc_puts_the_document_back_exactly() {
    for (label, from, to) in [
        ("face", (2.5, 0.0, 2.5), (2.5, 512.0, 2.5)),
        ("edge", (2.5, 0.0, 2.0), (2.5, 512.0, 2.0)),
        ("corner", (2.0, 0.0, 2.0), (2.0, 512.0, 2.0)),
    ] {
        let mut rig = edit_rig("edit-esc", flat_project(5, 5, true));
        let before = rig.ws.project.clone();
        let a = at(&rig, from.0, from.1, from.2);
        rig.drag_hold(a, at(&rig, to.0, to.1 / S, to.2), egui::Modifiers::NONE);
        assert!(rig.ws.project != before, "{label} moved");
        rig.key(egui::Key::Escape);
        assert!(rig.ws.project == before, "{label}: Esc restores");
        rig.release(a, egui::Modifiers::NONE);
        assert!(rig.ws.project == before, "{label}: release after Esc");
        assert!(!rig.ws.history.can_undo(), "{label}: no step left");
    }
}

#[test]
fn a_typed_number_with_a_selection_offsets_it() {
    let mut rig = edit_rig("edit-offset", flat_project(5, 5, true));
    rig.click(at(&rig, 2.5, 0.0, 2.5));
    rig.key(egui::Key::Num2);
    rig.key(egui::Key::Enter);
    assert_eq!(heights(&rig, [2, 2]), Some([512; 4]));
    rig.ws.do_undo();
    assert_eq!(heights(&rig, [2, 2]), Some([0; 4]));
}

#[test]
fn r_repeats_the_last_action_on_the_new_selection() {
    let mut rig = edit_rig("edit-repeat", flat_project(5, 5, true));
    rig.drag(at(&rig, 1.5, 0.0, 1.5), at(&rig, 1.5, 256.0 / S, 1.5));
    rig.click(at(&rig, 3.5, 0.0, 3.5));
    rig.key(egui::Key::R);
    assert_eq!(heights(&rig, [3, 3]), Some([256; 4]), "{}", rig.ws.status);
}

#[test]
fn a_double_click_selects_the_rooms_floor() {
    let mut rig = edit_rig("edit-double", flat_project(5, 5, true));
    let p = at(&rig, 2.5, 0.0, 2.5);
    rig.click_with(p, egui::Modifiers::COMMAND);
    rig.click(p);
    assert_eq!(rig.ws.grid.selection.len(), 25, "{}", rig.ws.status);
}

#[test]
fn the_move_handle_carries_the_selection_a_sector() {
    let mut rig = edit_rig("edit-move", flat_project(5, 5, true));
    rig.click(at(&rig, 4.5, 0.0, 2.5));
    let handle = at(&rig, 4.5, 64.0 / S, 2.5);
    rig.drag(handle, at(&rig, 5.5, 64.0 / S, 2.5));
    assert!(heights(&rig, [4, 2]).is_none(), "{}", rig.ws.status);
    assert_eq!(heights(&rig, [5, 2]), Some([0; 4]));
}

#[test]
fn the_move_handle_refuses_a_collision_and_says_which_cell() {
    let mut rig = edit_rig("edit-move-collide", flat_project(5, 5, true));
    rig.click(at(&rig, 2.5, 0.0, 2.5));
    let before = rig.ws.project.clone();
    let handle = at(&rig, 2.5, 64.0 / S, 2.5);
    rig.drag(handle, at(&rig, 3.5, 64.0 / S, 2.5));
    assert!(rig.ws.project == before, "a refused move left a change");
    assert!(rig.ws.status.contains("in the way"), "{}", rig.ws.status);
}

#[test]
fn raising_a_rectangle_as_a_platform_takes_two_steps() {
    let mut rig = edit_rig("edit-platform", flat_project(5, 5, true));
    // 1: box the rectangle. 2: drag it up.
    rig.drag_with(at(&rig, 1.1, 0.0, 1.1), at(&rig, 2.9, 0.0, 1.9), shift());
    rig.drag(at(&rig, 1.5, 0.0, 1.5), at(&rig, 1.5, 256.0 / S, 1.5));
    assert_eq!(heights(&rig, [1, 1]), Some([256; 4]));
    assert_eq!(heights(&rig, [2, 1]), Some([256; 4]));
    assert_eq!(heights(&rig, [1, 2]), Some([0; 4]));
}

#[test]
fn in_the_plan_a_floor_cell_rises_eighteen_pixels_a_click() {
    let mut rig = GridRig::top("edit-plan", flat_project(5, 5, true));
    rig.tool(GridTool::Edit);
    let from = rig.cell_centre(2, 2);
    rig.move_to(from, egui::Modifiers::NONE);
    assert!(
        hint(&rig).starts_with("Drag: raise floor"),
        "{}",
        hint(&rig)
    );
    rig.drag(from, from - Vec2::new(0.0, 36.0));
    assert_eq!(heights(&rig, [2, 2]), Some([512; 4]));
    assert_eq!(rig.ws.grid.selection.len(), 1);
}

#[test]
fn in_the_plan_a_room_slopes_from_an_edge_of_a_double_clicked_floor() {
    let mut rig = GridRig::top("edit-plan-tilt", flat_project(4, 4, true));
    rig.tool(GridTool::Edit);
    let p = rig.cell_centre(1, 1);
    rig.click(p);
    rig.click(p);
    assert_eq!(rig.ws.grid.selection.len(), 16);
    // The east edge of the room, a few pixels inside its last cell.
    let edge = rig.top_at(3.97 * S, 1.5 * S);
    rig.drag(edge, edge - Vec2::new(0.0, 36.0));
    assert_eq!(heights(&rig, [3, 1]), Some([384, 512, 512, 384]));
}

#[test]
fn a_walk_limit_warning_names_floors_too_steep_to_stand_on() {
    let mut rig = tilted_room_rig("edit-steep", 2);
    // Ten clicks over two sectors is steeper than a body can stand on.
    let from = at(&rig, 2.0, 0.0, 1.5);
    rig.drag_hold(from, at(&rig, 2.0, 2560.0 / S, 1.5), egui::Modifiers::NONE);
    assert!(rig.ws.status.contains("too steep"), "{}", rig.ws.status);
    rig.key(egui::Key::Escape);
}

/// Captures of the Edit tool for review: `PSXED_EDIT_CAPTURE=<dir>` writes a
/// hover over a floor, a hover over a wall with its arrow, and a slope in
/// flight, from the whole window. Without the variable this does nothing.
#[test]
fn edit_tool_captures() {
    use super::grid_live::WindowRig;
    use super::grid_pick_shots::{look_from, sector, settle, three_d, wire};
    let Some(dir) = std::env::var_os("PSXED_EDIT_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let mut rig = WindowRig::new();
    three_d(&mut rig);
    let s = sector(&rig);
    look_from(
        &mut rig,
        [11.0 * s, 8500.0, 15.0 * s],
        [11.0 * s, 0.0, 3.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Edit);
    settle(&mut rig);
    let p = |rig: &WindowRig, x: f32, y: f32, z: f32| rig.persp_screen([x * s, y, z * s]).unwrap();
    // 1. Hover over a floor tile.
    let tile = p(&rig, 11.5, 0.0, 3.5);
    rig.move_to(tile);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join("edit-hover-floor.png"));
    // 2. Hover near an edge, then a wall and its arrow.
    let wall = p(&rig, 7.0, 1536.0, 3.5);
    rig.move_to(wall);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join("edit-hover-wall.png"));
    let arrow = p(&rig, 7.0, 3072.0, 3.5) - Vec2::new(0.0, 22.0);
    rig.move_to(arrow);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join("edit-hover-wall-arrow.png"));
    // 3. The room's floor, then its east edge half way up.
    rig.click(tile);
    rig.click(tile);
    settle(&mut rig);
    let edge = p(&rig, 14.97, 0.0, 3.5);
    rig.move_to(edge);
    let down = rig.press(edge, true);
    rig.frame(vec![down]);
    let up = p(&rig, 14.97, 768.0, 3.5);
    for step in 1..=6 {
        rig.move_to(edge + (up - edge) * (step as f32 / 6.0));
    }
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join("edit-drag-tilt.png"));
    let release = rig.press(up, false);
    rig.frame(vec![release]);
}

/// A hall with a slab: two storeys over sectors (2, 2) to (5, 5).
fn two_storeys() -> ProjectDocument {
    use psxed_project::grid::edit::spans::insert_slab;
    use psxed_project::grid::edit::CellRect;
    let mut project = derived_project_for_tiling(8, 8);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    let outcome = insert_slab(
        world,
        0,
        CellRect::from_corners([0, 2], [3, 5]),
        1536,
        256,
        None,
    );
    assert_eq!(outcome.changed, 16);
    project
}

#[test]
fn a_wall_on_the_upper_storey_slides_like_one_on_the_ground_floor() {
    use crate::grid_tools::edit::plan::WallPlan;
    use crate::grid_tools::edit::wall_handle::{WallHandle, WallMove};
    use psxed_project::grid::edit::wall_api::walls_at;
    use psxed_project::grid::LeafKey;
    let mut project = two_storeys();
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    let leaf = LeafKey::sector([0, 2]);
    let views = walls_at(world, 0, leaf, WallEdge::West);
    let spans: Vec<u8> = views.iter().map(|v| v.addr.span).collect();
    assert!(spans.contains(&0) && spans.contains(&1), "{spans:?}");
    let floors = |w: &psxed_project::grid::GridWorld| {
        let r = &w.rooms[0];
        let s = r.sector(0, 2).unwrap();
        format!(
            "{:?}",
            (
                s.ground_floor().map(|f| f.heights),
                s.ground_ceiling().map(|c| c.heights)
            )
        )
    };
    let before_floors = floors(world);
    for span in [0u8, 1] {
        let mut world = world.clone();
        let view = walls_at(&world, 0, leaf, WallEdge::West)
            .into_iter()
            .find(|v| v.addr.span == span)
            .unwrap();
        let face = FaceRef {
            room: 0,
            leaf,
            target: if view.addr.authored {
                psxed_project::grid::edit::FaceTarget::Wall {
                    edge: WallEdge::West,
                    index: view.addr.index,
                }
            } else {
                psxed_project::grid::edit::FaceTarget::Derived {
                    edge: WallEdge::West,
                    kind: psxed_project::grid::derive::DerivedKind::Closure,
                    seg: 0,
                    span,
                }
            },
        };
        let Some(handle) = WallHandle::of(face) else {
            panic!("no handle")
        };
        let plan = WallPlan::new(&world, &[handle], WallMove::Slide).unwrap();
        let out = plan.apply(&mut world, 256);
        assert!(out.changed, "span {span}: {:?}", out.refused);
        assert_eq!(
            floors(&world),
            before_floors,
            "span {span}: the floor stays"
        );
        let after = walls_at(&world, 0, leaf, WallEdge::West);
        let moved = after
            .iter()
            .find(|v| v.addr.span == span && v.addr.authored)
            .unwrap()
            .heights
            .value();
        let base = view.heights.value();
        assert_eq!(moved, base.map(|h| h + 256), "span {span}");
    }
}

// ---- acceptance: the rest of the review's checklist the tool owns ----

#[test]
fn a_right_click_opens_the_menu_and_a_right_drag_orbits_without_one() {
    let mut rig = edit_rig("edit-menu", flat_project(5, 5, true));
    let before = rig.ws.viewport_3d_camera();
    rig.secondary_drag(
        at(&rig, 2.5, 0.0, 2.5),
        at(&rig, 2.5, 0.0, 2.5) + Vec2::new(30.0, 0.0),
    );
    assert!(
        !rig.ws.grid.edit.menu_is_open(),
        "a right-drag opened the menu"
    );
    assert!(
        rig.ws.viewport_3d_camera() != before,
        "the camera did not orbit"
    );
    rig.secondary_click(at(&rig, 2.5, 0.0, 2.5));
    assert!(rig.ws.grid.edit.menu_is_open(), "a right click opens it");
}

#[test]
fn reset_wall_to_automatic_is_one_undo_step() {
    let mut rig = edit_rig("edit-reset", low_derived());
    let original = rig.ws.project.clone();
    rig.move_to(at(&rig, 0.0, 512.0 / S, 3.5), egui::Modifiers::NONE);
    let arrow = at(&rig, 0.0, 1024.0 / S, 3.5) - Vec2::new(0.0, 22.0);
    rig.move_to(arrow, egui::Modifiers::NONE);
    let one = (at(&rig, 0.0, 0.0, 3.5).y - at(&rig, 0.0, 256.0 / S, 3.5).y).abs();
    rig.drag(arrow, arrow - Vec2::new(0.0, one));
    let slid = rig.ws.project.clone();
    assert!(slid != original);
    let face = rig.ws.grid.selection.faces(rig.world())[0];
    crate::grid_tools::edit::menu::reset_wall(&mut rig.ws, face);
    assert!(rig.ws.project != slid, "{}", rig.ws.status);
    rig.ws.do_undo();
    assert!(
        rig.ws.project == slid,
        "one undo step puts the slid wall back"
    );
}

#[test]
fn a_press_on_the_void_starts_a_box_and_replaces_the_selection() {
    let mut rig = edit_rig("edit-void-box", flat_project(5, 5, true));
    rig.click(at(&rig, 0.5, 0.0, 0.5));
    assert_eq!(rig.ws.grid.selection.len(), 1);
    // Beside the room, over nothing: drag a box across two tiles.
    let from = at(&rig, -1.0, 0.0, 1.0);
    rig.drag(from, at(&rig, 1.9, 0.0, 2.5));
    let n = rig.ws.grid.selection.len();
    assert!(n >= 2, "{n} faces boxed: {}", rig.ws.status);
}

#[test]
fn painting_ten_materials_on_ten_faces_takes_no_more_than_21_steps() {
    // Today: P, then (swatch, face) ten times is 21. Now the Library opens
    // (1) and a swatch click arms Paint, so it is the same 21.
    let mut project = flat_project(5, 5, false);
    let ids: Vec<_> = (0..10)
        .map(|n| {
            project.add_resource(
                format!("Graybox / Acceptance {n}"),
                psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(
                    None,
                )),
            )
        })
        .collect();
    let mut rig = GridRig::top("edit-paint-ten", project);
    let mut steps = 0;
    rig.ws
        .open_library(crate::grid_tools::LibraryTab::Materials);
    steps += 1;
    for (n, id) in ids.iter().enumerate() {
        rig.ws.choose_palette_material(*id);
        steps += 1;
        rig.click(rig.cell_centre((n % 5) as i32, (n / 5) as i32));
        steps += 1;
    }
    assert_eq!(
        rig.ws.grid.tool,
        GridTool::Paint,
        "the first swatch armed Paint"
    );
    assert!(steps <= 21, "{steps} steps");
    let world = rig.world();
    let painted: std::collections::BTreeSet<String> = (0..10)
        .filter_map(|n| {
            let face = FaceRef::floor_in(0, [n % 5, n / 5], 0);
            psxed_project::grid::edit::material_of(world, face, None).map(|id| format!("{id:?}"))
        })
        .collect();
    assert_eq!(painted.len(), 10, "ten different materials landed");
}

#[test]
fn no_two_tools_share_a_key_binding() {
    let mut seen = std::collections::BTreeMap::new();
    for tool in GridTool::ALL {
        if !tool.has_key() {
            continue;
        }
        let binding = (format!("{:?}", tool.key()), tool.shift());
        if let Some(other) = seen.insert(binding.clone(), tool) {
            panic!("{other:?} and {tool:?} both take {binding:?}");
        }
    }
}
