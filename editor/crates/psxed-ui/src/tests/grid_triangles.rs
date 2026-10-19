//! Triangle selection: both halves of a cracked, voided or flipped quad are
//! selectable on their own, in the plan and in 3D (the Select tool), and the
//! tools that act on a selection act on the triangle.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, FaceTarget, GridElement};
use psxed_project::grid::LeafKey;

fn alt() -> egui::Modifiers {
    egui::Modifiers {
        alt: true,
        ..egui::Modifiers::NONE
    }
}

fn floor_of_22() -> FaceRef {
    FaceRef::new(0, LeafKey::sector([2, 2]), FaceTarget::floor(0))
}

/// A 6 x 6 hall whose sector (2, 2) is cracked: triangle B sits two clicks
/// above triangle A.
fn cracked(rig: &mut GridRig) {
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    let sector = world.rooms[0].sector_mut(2, 2).unwrap();
    let floor = sector.ground_floor_mut().unwrap();
    floor.heights_b = Some([512; 4]);
    rig.ws.grid_changed();
}

fn void_b(rig: &mut GridRig) {
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    psxed_project::grid::edit::natural::set_void(
        world,
        0,
        [2, 2],
        psxed_project::grid::edit::SurfaceKind::Floor,
        psxed_project::grid::edit::natural::Triangles::B,
        true,
    );
    rig.ws.grid_changed();
}

fn picked(rig: &GridRig) -> Vec<GridElement> {
    rig.ws.grid.selection.items().to_vec()
}

fn tri(face: FaceRef, tri: u8) -> GridElement {
    GridElement::Triangle { face, tri }
}

/// Plan point in triangle A (upper right of the NW to SE diagonal) and B.
fn plan_a(rig: &GridRig) -> Pos2 {
    rig.top_at(2.8 * S, 2.2 * S)
}
fn plan_b(rig: &GridRig) -> Pos2 {
    rig.top_at(2.2 * S, 2.8 * S)
}

#[test]
fn the_halves_of_a_cracked_quad_select_separately_in_the_plan() {
    let mut rig = GridRig::top("tri-plan", flat_project(6, 6, false));
    cracked(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(plan_a(&rig));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 0)]);
    rig.click(plan_b(&rig));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
    // Ctrl adds the other half.
    rig.click_with(
        plan_a(&rig),
        egui::Modifiers {
            command: true,
            ctrl: true,
            ..egui::Modifiers::NONE
        },
    );
    assert_eq!(picked(&rig).len(), 2);
}

#[test]
fn the_halves_of_a_cracked_quad_select_separately_in_3d() {
    let mut rig = GridRig::persp("tri-3d", flat_project(6, 6, false));
    cracked(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(rig.screen_of([2.8 * S, 0.0, 2.2 * S]));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 0)]);
    rig.click(rig.screen_of([2.2 * S, 512.0, 2.8 * S]));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
}

#[test]
fn a_flat_quad_is_one_face_and_alt_picks_a_half() {
    let mut rig = GridRig::top("tri-flat", flat_project(6, 6, false));
    rig.tool(GridTool::Select);
    rig.click(plan_a(&rig));
    assert!(matches!(
        picked(&rig).as_slice(),
        [GridElement::Cell { .. }]
    ));
    rig.click_with(plan_b(&rig), alt());
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
    let mut rig = GridRig::persp("tri-flat-3d", flat_project(6, 6, false));
    rig.tool(GridTool::Select);
    rig.click(rig.screen_of([2.8 * S, 0.0, 2.2 * S]));
    assert!(matches!(picked(&rig).as_slice(), [GridElement::Face(_)]));
    rig.click_with(rig.screen_of([2.2 * S, 0.0, 2.8 * S]), alt());
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
}

#[test]
fn a_voided_triangle_can_be_picked_in_the_plan_and_in_3d() {
    let mut rig = GridRig::top("tri-void", flat_project(6, 6, false));
    void_b(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(plan_b(&rig));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
    let mut rig = GridRig::persp("tri-void-3d", flat_project(6, 6, false));
    void_b(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(rig.screen_of([2.2 * S, 0.0, 2.8 * S]));
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
}

#[test]
fn a_material_goes_on_the_selected_triangle_only() {
    let mut project = flat_project(6, 6, false);
    let brick = project.add_resource(
        "Graybox / Brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let mut rig = GridRig::top("tri-material", project);
    rig.tool(GridTool::Select);
    rig.click_with(plan_b(&rig), alt());
    assert_eq!(rig.ws.apply_grid_material_to_selection(brick), 1);
    let world = rig.world();
    let floor = floor_of_22().surface(world).unwrap();
    assert_eq!(floor.tri(1).material, Some(brick));
    assert_ne!(floor.tri(0).material, Some(brick));
}

#[test]
fn delete_voids_the_selected_triangle_and_undo_brings_it_back() {
    let mut rig = GridRig::top("tri-delete", flat_project(6, 6, false));
    rig.tool(GridTool::Select);
    rig.click_with(plan_b(&rig), alt());
    rig.key(egui::Key::Delete);
    let floor = floor_of_22().surface(rig.world()).unwrap().clone();
    assert!(floor.is_void(1) && !floor.is_void(0), "{}", rig.ws.status);
    rig.ws.do_undo();
    assert!(!floor_of_22().surface(rig.world()).unwrap().is_void(1));
}

#[test]
fn shift_drag_lifts_only_the_corners_of_the_selected_triangle() {
    let mut rig = GridRig::persp("tri-lift", flat_project(6, 6, false));
    rig.tool(GridTool::Select);
    // Triangle B is NW, SE, SW; NE (index 1) is not on it.
    let centre_b = rig.screen_of([2.25 * S, 0.0, 2.75 * S]);
    rig.click_with(centre_b, alt());
    assert_eq!(picked(&rig), vec![tri(floor_of_22(), 1)]);
    let one_click = (rig.screen_of([2.5 * S, 0.0, 2.5 * S]).y
        - rig.screen_of([2.5 * S, 256.0, 2.5 * S]).y)
        .abs();
    // Shift-drag on a selected corner raises the selection's corners.
    let nw = rig.screen_of([2.0 * S, 0.0, 2.0 * S]);
    rig.ws.grid.link = false;
    rig.drag_with(
        centre_b,
        centre_b + Vec2::new(0.0, -2.0 * one_click),
        egui::Modifiers::SHIFT,
    );
    let _ = nw;
    let floor = floor_of_22().surface(rig.world()).unwrap().clone();
    let b = floor.heights_for(1);
    assert_eq!(b[0], 512, "NW is on triangle B: {floor:?}");
    assert_eq!(b[2], 512);
    assert_eq!(b[3], 512);
    assert_eq!(floor.heights_for(0)[1], 0, "NE is not on it: {floor:?}");
}

/// Writes a hall with a cracked, a voided and a flipped quad to
/// `$PSXED_AUDIT_OUT/project.ron` for a headless capture. Does nothing
/// without the variable.
#[test]
fn write_the_triangle_hall_for_a_capture() {
    let Ok(out) = std::env::var("PSXED_AUDIT_OUT") else {
        return;
    };
    let mut rig = GridRig::top("tri-capture", flat_project(6, 6, false));
    cracked(&mut rig);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let floor = world.rooms[0]
            .sector_mut(4, 2)
            .unwrap()
            .ground_floor_mut()
            .unwrap();
        floor.set_void(1, true);
        let floor = world.rooms[0]
            .sector_mut(2, 4)
            .unwrap()
            .ground_floor_mut()
            .unwrap();
        floor.split = psxed_project::grid::GridSplit::NeSw;
    }
    std::fs::create_dir_all(&out).unwrap();
    rig.ws
        .project
        .save_to_path(std::path::Path::new(&out).join("project.ron"))
        .unwrap();
}
