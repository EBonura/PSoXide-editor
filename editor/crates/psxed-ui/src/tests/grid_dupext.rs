//! Duplicate (Cmd+D) and Extrude, the 2026-10-20 requests from Manny, plus
//! the two tool rules that came with them: a tool switch drops the selection
//! (Select itself keeps it) and Follow ceiling starts off.

use super::grid_rig::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::{NodeId, NodeKind};

#[test]
fn choosing_any_tool_but_select_drops_the_selection() {
    for tool in GridTool::ALL {
        if tool == GridTool::Select {
            continue;
        }
        let mut rig = GridRig::persp("dupext-clear", flat_project(4, 4, true));
        rig.tool(GridTool::Select);
        rig.ws
            .grid
            .selection
            .replace(GridElement::Face(FaceRef::floor(0, [1, 1])));
        rig.ws.grid.mesh_sel = Some(7);
        rig.tool(tool);
        assert!(
            rig.ws.grid.selection.is_empty(),
            "{tool:?} kept the face selection"
        );
        assert!(
            rig.ws.grid.plane_sel.is_empty() && rig.ws.grid.mesh_sel.is_none(),
            "{tool:?} kept the plane or mesh selection"
        );
    }
}

#[test]
fn going_back_to_select_keeps_what_is_selected() {
    let mut rig = GridRig::persp("dupext-keep", flat_project(4, 4, true));
    rig.tool(GridTool::Floor);
    rig.ws
        .grid
        .selection
        .replace(GridElement::Face(FaceRef::floor(0, [1, 1])));
    rig.tool(GridTool::Select);
    assert_eq!(rig.ws.grid.selection.len(), 1);
}

#[test]
fn follow_ceiling_starts_off() {
    let rig = GridRig::persp("dupext-follow", flat_project(4, 4, true));
    assert!(!rig.ws.grid.verb.follow_ceiling);
}

// ---------------------------------------------------------------------
// Duplicate: Cmd+D grabs a copy of the selection and moves it with the pointer.

use psxed_project::grid::edit::{add_asset, place_mesh, FaceTarget, SurfaceKind};
use psxed_project::grid::mesh::{ArchSpec, GridMesh, MeshAsset, MeshKind};
use psxed_project::grid::GridPlane;

fn cmd_d(rig: &mut GridRig) {
    rig.key_with(egui::Key::D, egui::Modifiers::COMMAND);
}

fn light(rig: &mut GridRig, name: &str, at: [f32; 3]) -> NodeId {
    let id = rig.ws.project.active_scene_mut().add_node(
        NodeId::ROOT,
        name,
        NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    rig.ws
        .project
        .active_scene_mut()
        .node_mut(id)
        .unwrap()
        .transform
        .translation = at;
    id
}

fn nodes_named(rig: &GridRig, prefix: &str) -> Vec<(NodeId, String, [f32; 3])> {
    rig.ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .filter(|n| n.name.starts_with(prefix))
        .map(|n| (n.id, n.name.clone(), n.transform.translation))
        .collect()
}

fn move_pointer(rig: &mut GridRig, from: (i32, i32), to: (i32, i32)) {
    rig.move_to(rig.cell_centre(from.0, from.1), egui::Modifiers::NONE);
    rig.move_to(rig.cell_centre(to.0, to.1), egui::Modifiers::NONE);
}

#[test]
fn cmd_d_on_a_light_grabs_a_copy_called_light_2_that_follows_the_pointer_and_drops_on_click() {
    let mut rig = GridRig::top("dup-light", flat_project(8, 8, true));
    let source = light(&mut rig, "Light 1", [1536.0, 600.0, 1536.0]);
    rig.ws.replace_node_selection(source);
    rig.tool(GridTool::Select);
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    assert!(rig.ws.grid.grab.is_some(), "{}", rig.ws.status);
    let lights = nodes_named(&rig, "Light");
    assert_eq!(lights.len(), 2);
    let copy = lights.iter().find(|l| l.0 != source).unwrap();
    assert_eq!(copy.1, "Light 2");
    // Two sectors east and one south of where the pointer first was.
    move_pointer(&mut rig, (1, 1), (3, 2));
    let copy_now = nodes_named(&rig, "Light 2")[0].2;
    // On the editor's node snap (16 units), as far as the pointer travelled
    // (pixels are whole, a sector is 102.4 of them).
    let (dx, dz) = (copy_now[0] - 1536.0, copy_now[2] - 1536.0);
    assert!(
        (dx - 2048.0).abs() <= 16.0 && (dz - 1024.0).abs() <= 16.0,
        "{copy_now:?}"
    );
    assert_eq!((dx % 16.0, dz % 16.0, copy_now[1]), (0.0, 0.0, 600.0));
    assert_eq!(nodes_named(&rig, "Light 1")[0].2, [1536.0, 600.0, 1536.0]);
    rig.click(rig.cell_centre(3, 2));
    assert!(rig.ws.grid.grab.is_none());
    // The click drops it where the pointer is (a pixel off is a snap step).
    let dropped = nodes_named(&rig, "Light 2")[0].2;
    assert!((dropped[0] - copy_now[0]).abs() <= 16.0 && dropped[2] == copy_now[2]);
    // The copy is the selection; one undo takes it away.
    assert!(rig.ws.node_is_selected(copy.0));
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(nodes_named(&rig, "Light").len(), 2);
}

#[test]
fn escape_during_the_grab_leaves_nothing_and_selects_the_original_again() {
    let mut rig = GridRig::top("dup-esc", flat_project(8, 8, true));
    let source = light(&mut rig, "Light 1", [1536.0, 600.0, 1536.0]);
    rig.ws.replace_node_selection(source);
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    move_pointer(&mut rig, (1, 1), (4, 4));
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.grab.is_none());
    assert_eq!(rig.ws.project, original);
    assert!(rig.ws.node_is_selected(source));
    assert!(!rig.ws.history.can_undo(), "no undo step for a cancel");
}

#[test]
fn copies_of_props_and_actors_get_clean_numbered_names() {
    let mut rig = GridRig::top("dup-names", flat_project(8, 8, true));
    let crate_ =
        rig.ws
            .project
            .active_scene_mut()
            .add_node(NodeId::ROOT, "Crate", NodeKind::Entity);
    rig.ws.replace_node_selection(crate_);
    cmd_d(&mut rig);
    rig.click(rig.cell_centre(2, 2));
    // The copy is selected: duplicating it again counts on.
    cmd_d(&mut rig);
    rig.click(rig.cell_centre(2, 2));
    let mut names: Vec<String> = nodes_named(&rig, "Crate")
        .into_iter()
        .map(|n| n.1)
        .collect();
    names.sort();
    assert_eq!(names, vec!["Crate", "Crate 2", "Crate 3"]);
}

#[test]
fn cmd_d_with_nothing_selected_says_so_and_grabs_nothing() {
    let mut rig = GridRig::top("dup-none", flat_project(4, 4, true));
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    assert!(rig.ws.grid.grab.is_none());
    assert!(
        rig.ws.status.contains("Select something"),
        "{}",
        rig.ws.status
    );
    assert_eq!(rig.ws.project, original);
}

fn mesh_rig() -> (GridRig, u16) {
    let mut rig = GridRig::top("dup-mesh", flat_project(8, 8, false));
    let id = {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let asset = add_asset(
            world,
            MeshAsset::new("arch", MeshKind::Arch(ArchSpec::default())),
        )
        .unwrap();
        let mut mesh = GridMesh::new(0, asset, [1536, 0, 1536]);
        mesh.rotation = [90, 0, 0];
        place_mesh(world, 0, mesh).unwrap()
    };
    rig.ws.grid_changed();
    rig.ws.grid.mesh_sel = Some(id);
    (rig, id)
}

#[test]
fn cmd_d_on_a_static_mesh_grabs_a_copy_with_the_same_turn_and_drops_it_on_the_floor() {
    let (mut rig, id) = mesh_rig();
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    assert_eq!(rig.room(0).meshes.len(), 2, "{}", rig.ws.status);
    move_pointer(&mut rig, (1, 1), (4, 3));
    rig.click(rig.cell_centre(4, 3));
    let meshes = &rig.room(0).meshes;
    assert_eq!(meshes.len(), 2);
    let copy = meshes.iter().find(|m| m.id != id).unwrap();
    assert_eq!(copy.rotation, [90, 0, 0]);
    assert_eq!((copy.position[0] % 256, copy.position[2] % 256), (0, 0));
    assert!(
        copy.position[0] > 3000 && copy.position[2] > 2500,
        "{copy:?}"
    );
    assert_eq!(copy.position[1], 0);
    assert_eq!(rig.ws.grid.mesh_sel, Some(copy.id));
    // The original did not move.
    assert_eq!(
        meshes.iter().find(|m| m.id == id).unwrap().position,
        [1536, 0, 1536]
    );
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn escape_on_a_mesh_grab_removes_the_copy() {
    let (mut rig, _id) = mesh_rig();
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    move_pointer(&mut rig, (1, 1), (4, 3));
    rig.key(egui::Key::Escape);
    assert_eq!(rig.ws.project, original);
    assert!(rig.ws.grid.mesh_sel.is_some());
}

#[test]
fn cmd_d_on_a_free_plane_hangs_a_ghost_copy_and_leaves_the_clipboard_alone() {
    let mut rig = GridRig::top("dup-plane", flat_project(8, 8, false));
    let id = {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let mut p = GridPlane::new([3072, 512, 3072], [2048, 1024]);
        p.rotation = [0, 90, 0];
        psxed_project::grid::edit::add_plane(world, 0, p).unwrap()
    };
    rig.ws.grid_changed();
    rig.ws.select_planes(vec![id]);
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    assert!(rig.ws.grid.plane_paste.is_some(), "{}", rig.ws.status);
    assert!(rig.ws.grid.plane_clip.is_none() && rig.ws.grid.clipboard.is_none());
    rig.click(rig.top_at(1024.0, 1024.0));
    assert_eq!(rig.room(0).planes.len(), 2);
    assert_eq!(rig.room(0).planes[1].size, [2048, 1024]);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    // Escape leaves nothing.
    rig.ws.select_planes(vec![id]);
    cmd_d(&mut rig);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.plane_paste.is_none());
    assert_eq!(rig.ws.project, original);
}

// ---- grid geometry ----

/// A 8x8 hall with a raised, sloped two-sector platform at (1,1) and (2,1),
/// a wall on its north edge, a light standing on it and a second light on a
/// sector that is not selected.
fn platform() -> (GridRig, NodeId, NodeId) {
    let mut rig = GridRig::top("dup-cells", flat_project(8, 8, true));
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let room = &mut world.rooms[0];
        for x in [1usize, 2] {
            let s = room.sector_or_insert(x, 1).unwrap();
            let floor = s.ground_floor_mut().unwrap();
            floor.heights = [256, 256, 512, 512];
        }
        room.sector_or_insert(1, 1)
            .unwrap()
            .north
            .push(psxed_project::grid::GridWall::span(0, 512, None));
    }
    let on = light(&mut rig, "Light 1", [1536.0, 600.0, 1536.0]);
    let off = light(&mut rig, "Light 2", [6656.0, 600.0, 6656.0]);
    rig.ws.grid_changed();
    rig.tool(GridTool::Select);
    rig.ws.grid.selection.set([
        GridElement::Cell {
            room: 0,
            cell: [1, 1],
        },
        GridElement::Cell {
            room: 0,
            cell: [2, 1],
        },
    ]);
    (rig, on, off)
}

#[test]
fn cmd_d_on_cells_stamps_the_region_where_it_is_dropped_with_heights_walls_and_nodes() {
    let (mut rig, on, off) = platform();
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    assert!(rig.ws.grid.paste.is_some(), "{}", rig.ws.status);
    assert!(rig.ws.grid.grab.is_some());
    // The ghost is centred on the pointer: (4,3) puts a 2x1 clip at (3,3).
    rig.move_to(rig.cell_centre(4, 3), egui::Modifiers::NONE);
    rig.click(rig.cell_centre(4, 3));
    assert!(rig.ws.grid.paste.is_none() && rig.ws.grid.grab.is_none());
    assert_eq!(
        floor_heights(rig.world(), 0, [3, 3]),
        Some([256, 256, 512, 512])
    );
    assert_eq!(
        floor_heights(rig.world(), 0, [4, 3]),
        Some([256, 256, 512, 512])
    );
    assert_eq!(
        walls_on(rig.world(), 0, [3, 3], psxed_project::grid::WallEdge::North).len(),
        1
    );
    // The source is untouched.
    assert_eq!(
        floor_heights(rig.world(), 0, [1, 1]),
        Some([256, 256, 512, 512])
    );
    // The light on the platform came along; the other stayed single.
    let lights = nodes_named(&rig, "Light");
    assert_eq!(lights.len(), 3, "{lights:?}");
    let copy = lights
        .iter()
        .find(|l| l.0 != on && l.0 != off)
        .expect("a copy of the light on the platform");
    assert_eq!(copy.2, [1536.0 + 2048.0, 600.0, 1536.0 + 2048.0]);
    // One undo step for all of it.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn escape_on_a_cell_grab_leaves_nothing_behind_not_even_the_node_copies() {
    let (mut rig, _on, _off) = platform();
    let original = rig.ws.project.clone();
    cmd_d(&mut rig);
    rig.move_to(rig.cell_centre(5, 5), egui::Modifiers::NONE);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.paste.is_none() && rig.ws.grid.grab.is_none());
    assert_eq!(rig.ws.project, original);
    assert!(!rig.ws.grid.selection.is_empty(), "the selection stays");
}

#[test]
fn the_arrow_keys_lift_the_cell_grab_a_click_at_a_time() {
    let (mut rig, _on, _off) = platform();
    cmd_d(&mut rig);
    rig.move_to(rig.cell_centre(4, 3), egui::Modifiers::NONE);
    rig.key(egui::Key::ArrowUp);
    rig.key(egui::Key::ArrowUp);
    rig.key(egui::Key::ArrowDown);
    rig.click(rig.cell_centre(4, 3));
    assert_eq!(
        floor_heights(rig.world(), 0, [3, 3]),
        Some([512, 512, 768, 768]),
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_cell_grab_stamps_in_the_room_it_came_from_whichever_room_is_active() {
    let (mut rig, _on, _off) = platform();
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let mut other = psxed_project::grid::GridRoom::new("annex", 2, 2);
        other.origin = [20, 20];
        world.rooms.push(other);
    }
    rig.ws.grid_active_room = Some(1);
    cmd_d(&mut rig);
    rig.move_to(rig.cell_centre(4, 3), egui::Modifiers::NONE);
    rig.click(rig.cell_centre(4, 3));
    assert!(floor_heights(rig.world(), 0, [3, 3]).is_some());
    assert_eq!(rig.room(1).width * rig.room(1).depth, 4, "annex untouched");
    let _ = (FaceTarget::floor(0), SurfaceKind::Floor);
}

#[test]
fn in_the_3d_view_a_light_copy_follows_the_pointer_over_its_own_height_plane() {
    let mut rig = GridRig::persp("dup-light-3d", flat_project(8, 8, true));
    let source = light(&mut rig, "Light 1", [2560.0, 600.0, 2560.0]);
    rig.ws.replace_node_selection(source);
    cmd_d(&mut rig);
    let a = rig.screen_of([2560.0, 600.0, 2560.0]);
    let b = rig.screen_of([3584.0, 600.0, 2048.0]);
    rig.move_to(a, egui::Modifiers::NONE);
    rig.move_to(b, egui::Modifiers::NONE);
    let copy = nodes_named(&rig, "Light 2")[0].2;
    assert!((copy[0] - 3584.0).abs() <= 32.0, "{copy:?}");
    assert!((copy[2] - 2048.0).abs() <= 32.0, "{copy:?}");
    assert_eq!(copy[1], 600.0);
    rig.click(b);
    assert!(rig.ws.grid.grab.is_none());
}

#[test]
fn in_the_3d_view_a_cell_selection_stamps_where_the_ghost_is_dropped() {
    let (mut rig, _on, _off) = platform();
    let mut rig3 = GridRig::persp("dup-cells-3d", rig.ws.project.clone());
    rig3.ws.grid.selection = rig.ws.grid.selection.clone();
    cmd_d(&mut rig3);
    let target = rig3.screen_of([4.5 * S, 0.0, 3.5 * S]);
    rig3.move_to(target, egui::Modifiers::NONE);
    rig3.click(target);
    assert!(rig3.ws.grid.grab.is_none(), "{}", rig3.ws.status);
    assert!(floor_heights(rig3.world(), 0, [3, 3]).is_some_and(|h| h[0] == 256));
    let _ = &mut rig;
}
