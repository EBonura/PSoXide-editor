//! Free planes driven through real pointer input: place, select, stretch,
//! turn, move, paint, copy, delete and undo, in the Top view and in 3D.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{add_plane, PlaneId};
use psxed_project::grid::{GridPlane, PlaneCollision};

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

/// A flat 2048 x 1024 plane centred at `(3072, 256, 3072)` lying down.
fn flat_plane() -> GridPlane {
    let mut plane = GridPlane::new([3072, 256, 3072], [2048, 1024]);
    plane.rotation = [0, 90, 0];
    plane
}

/// A rig with an 8x8 floor and one selected plane.
fn rig_with_plane(label: &str, persp: bool, plane: GridPlane) -> (GridRig, PlaneId) {
    let mut rig = if persp {
        GridRig::persp(label, flat_project(8, 8, false))
    } else {
        GridRig::top(label, flat_project(8, 8, false))
    };
    let id = {
        let world = rig
            .ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .expect("grid");
        add_plane(world, 0, plane).expect("room for the plane")
    };
    rig.ws.grid_changed();
    rig.tool(GridTool::Select);
    rig.ws.select_planes(vec![id]);
    rig.pump(vec![], egui::Modifiers::NONE);
    (rig, id)
}

fn plane(rig: &GridRig, id: PlaneId) -> GridPlane {
    id.get(rig.world()).expect("the plane exists").clone()
}

#[test]
fn a_click_with_the_plane_tool_puts_a_default_plane_on_the_floor() {
    let mut rig = GridRig::top("plane-click", flat_project(6, 6, false));
    rig.tool(GridTool::Plane);
    rig.click(rig.cell_centre(2, 2));
    let planes = &rig.room(0).planes;
    assert_eq!(planes.len(), 1, "{}", rig.ws.status);
    let p = &planes[0];
    // Snapped to the 256 lattice, lifted off the floor along its normal,
    // laid flat facing up, one sector square.
    assert_eq!(p.center[0] % 256, 0);
    assert_eq!(p.center[2] % 256, 0);
    assert!((p.center[1] - 24).abs() <= 1, "{:?}", p.center);
    assert_eq!(p.rotation, [0, 90, 0]);
    assert_eq!(p.size, [1024, 1024]);
    assert_eq!(rig.ws.grid.plane_sel.len(), 1);
    assert!(rig.ws.status.contains("Placed plane"), "{}", rig.ws.status);
    // One undo step removes it.
    rig.ws.do_undo();
    assert!(rig.room(0).planes.is_empty());
}

#[test]
fn dragging_with_the_plane_tool_sizes_the_plane_from_the_press_point() {
    let mut rig = GridRig::top("plane-drag", flat_project(8, 8, false));
    rig.tool(GridTool::Plane);
    rig.drag(rig.top_at(1.0 * S, 1.0 * S), rig.top_at(3.0 * S, 2.0 * S));
    let p = &rig.room(0).planes[0];
    assert_eq!(p.size, [2048, 1024]);
    assert_eq!(p.center[0], 2048);
    assert_eq!(p.center[2], 1536);
    // New planes take the strip's collision class and sidedness.
    rig.ws.grid.new_plane.collision = PlaneCollision::Walkable;
    rig.ws.grid.new_plane.double_sided = true;
    rig.drag(rig.top_at(4.0 * S, 4.0 * S), rig.top_at(5.0 * S, 5.0 * S));
    let q = &rig.room(0).planes[1];
    assert_eq!(q.collision, PlaneCollision::Walkable);
    assert!(q.double_sided);
}

#[test]
fn a_plane_placed_beyond_the_room_grows_the_room() {
    let mut rig = GridRig::top("plane-grow", flat_project(2, 2, false));
    rig.tool(GridTool::Plane);
    rig.drag(rig.top_at(4.0 * S, 1.0 * S), rig.top_at(6.0 * S, 2.0 * S));
    let room = rig.room(0);
    assert_eq!(room.planes.len(), 1);
    let (lo, hi) = room.planes[0].cell_span();
    assert!(
        room.local(lo).is_some() && room.local(hi).is_some(),
        "{room:?}"
    );
}

#[test]
fn a_click_in_3d_places_on_the_surface_it_hits_facing_out() {
    let mut rig = GridRig::persp("plane-3d", flat_project(8, 8, false));
    rig.tool(GridTool::Plane);
    rig.click(rig.screen_of([3.5 * S, 0.0, 3.5 * S]));
    let p = &rig.room(0).planes[0];
    assert_eq!(p.rotation[1], 90, "{p:?}");
    assert!(p.center[1] > 0 && p.center[1] < 100, "{p:?}");
    // Now the same click lands on the new plane (it is in front of the
    // floor): the tool selects it instead of stacking a second one.
    rig.click(rig.screen_of([p.center[0] as f32, p.center[1] as f32, p.center[2] as f32]));
    assert_eq!(rig.room(0).planes.len(), 1);
    assert_eq!(rig.ws.grid.plane_sel.len(), 1);
}

#[test]
fn a_wall_placed_on_a_wall_surface_stands_upright_facing_out() {
    let mut project = flat_project(6, 6, true);
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        world.rooms[0]
            .sector_or_insert(2, 2)
            .unwrap()
            .north
            .push(psxed_project::grid::GridWall::span(0, 3072, None));
    }
    let mut rig = GridRig::persp("plane-on-wall", project);
    rig.tool(GridTool::Plane);
    // The wall is the north edge of cell (2, 2): z = 2048, x 2048..3072. Its
    // front faces +z (into the room), toward the camera.
    rig.click(rig.screen_of([2.5 * S, 1024.0, 2.0 * S]));
    let p = &rig.room(0).planes[0];
    assert_eq!(p.rotation[1], 0, "{p:?}");
    assert!(p.center[2] > 2048 && p.center[2] < 2100, "{p:?}");
}

#[test]
fn dragging_an_edge_handle_stretches_one_side_and_keeps_the_other() {
    let (mut rig, id) = rig_with_plane("plane-stretch", false, flat_plane());
    // The right edge midpoint is at x = 4096. Pull it to 5120.
    let from = rig.top_at(4096.0, 3072.0);
    let to = rig.top_at(5120.0, 3072.0);
    rig.drag(from, to);
    let p = plane(&rig, id);
    assert_eq!(p.size, [3072, 1024], "{p:?}");
    assert_eq!(p.center[0], 3584);
    assert_eq!(p.center[2], 3072);
    rig.ws.do_undo();
    assert_eq!(plane(&rig, id).size, [2048, 1024]);
}

#[test]
fn a_corner_handle_stretches_both_ways_and_alt_snaps_finer() {
    let (mut rig, id) = rig_with_plane("plane-corner", false, flat_plane());
    // Corners of the flat plane: x 2048..4096, z 2560..3584. Pull the
    // corner at (4096, 3584) out by 300 and 300 with the fine snap.
    let from = rig.top_at(4096.0, 3584.0);
    let to = rig.top_at(4096.0 + 300.0, 3584.0 + 300.0);
    rig.drag_with(from, to, egui::Modifiers::ALT);
    let p = plane(&rig, id);
    // 2048 + 300 snapped to 64s: 2368; 1024 + 300: 1344.
    assert_eq!(p.size, [2368, 1344], "{p:?}");
}

#[test]
fn the_yaw_knob_turns_the_plane_in_fifteen_degree_steps() {
    let (mut rig, id) = rig_with_plane("plane-yaw", false, flat_plane());
    let radius = (2048.0f32).hypot(1024.0) / 2.0 + 192.0;
    let knob = rig.top_at(3072.0, 3072.0 + radius);
    // Drag it to due east of the centre, a little off 90.
    let to = rig.top_at(3072.0 + 900.0, 3072.0 - 60.0);
    rig.drag(knob, to);
    let p = plane(&rig, id);
    assert_eq!(p.rotation[0], 90, "{p:?}");
    // Shift+Alt gives whole degrees.
    let (mut rig, id) = rig_with_plane("plane-yaw-free", false, flat_plane());
    let knob = rig.top_at(3072.0, 3072.0 + radius);
    let to = rig.top_at(3072.0 + 900.0, 3072.0 + 300.0);
    rig.drag_with(knob, to, egui::Modifiers::ALT | egui::Modifiers::SHIFT);
    let p = plane(&rig, id);
    assert_eq!(p.rotation[0], 72, "{p:?}");
}

#[test]
fn dragging_the_body_slides_it_with_edges_on_the_lattice() {
    let (mut rig, id) = rig_with_plane("plane-move", false, flat_plane());
    let from = rig.top_at(3072.0, 3072.0);
    let to = rig.top_at(3072.0 + 780.0, 3072.0 - 410.0);
    rig.drag(from, to);
    let p = plane(&rig, id);
    assert_eq!(p.center, [3840, 256, 2560], "{p:?}");
    // Shift moves it up: three clicks of pixels is 768 units.
    let from = rig.top_at(3840.0, 2560.0);
    let up = from + Vec2::new(0.0, -3.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK);
    rig.drag_with(from, up, shift());
    let p = plane(&rig, id);
    assert_eq!(p.center[1], 256 + 768, "{p:?}");
    assert_eq!((p.center[0], p.center[2]), (3840, 2560));
}

#[test]
fn escape_during_a_drag_puts_the_plane_back() {
    let (mut rig, id) = rig_with_plane("plane-escape", false, flat_plane());
    let from = rig.top_at(3072.0, 3072.0);
    let to = rig.top_at(3072.0 + 780.0, 3072.0);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    assert_ne!(plane(&rig, id).center[0], 3072);
    rig.key(egui::Key::Escape);
    assert_eq!(plane(&rig, id), flat_plane());
}

#[test]
fn selecting_deleting_copying_and_pasting_planes() {
    let (mut rig, id) = rig_with_plane("plane-clip", false, flat_plane());
    // Click empty ground deselects, a click on the plane selects it again.
    rig.click(rig.cell_centre(0, 0));
    assert!(rig.ws.grid.plane_sel.is_empty());
    rig.click(rig.top_at(3072.0, 3072.0));
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
    // Copy, paste with a click elsewhere.
    rig.key_with(egui::Key::C, egui::Modifiers::COMMAND);
    rig.key_with(egui::Key::V, egui::Modifiers::COMMAND);
    assert!(rig.ws.grid.plane_paste.is_some());
    rig.click(rig.top_at(1024.0, 1024.0));
    assert_eq!(rig.room(0).planes.len(), 2);
    let pasted = &rig.room(0).planes[1];
    assert_eq!((pasted.center[0], pasted.center[2]), (1024, 1024));
    assert_eq!(pasted.size, [2048, 1024]);
    // Delete removes the selection (the pasted plane), one undo restores.
    rig.key(egui::Key::Delete);
    assert_eq!(rig.room(0).planes.len(), 1);
    rig.ws.do_undo();
    assert_eq!(rig.room(0).planes.len(), 2);
}

#[test]
fn the_paint_tool_and_the_palette_apply_materials_to_planes() {
    let mut project = flat_project(8, 8, false);
    let brick = project.add_resource(
        "brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let moss = project.add_resource(
        "moss",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let mut rig = GridRig::persp("plane-paint", project);
    let id = {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        let mut p = GridPlane::new([3584, 512, 3584], [1024, 1024]);
        p.rotation = [0, 90, 0];
        add_plane(world, 0, p).unwrap()
    };
    rig.ws.grid_changed();
    rig.tool(GridTool::Paint);
    rig.ws.grid.material = Some(brick);
    rig.click(rig.screen_of([3584.0, 512.0, 3584.0]));
    assert_eq!(plane(&rig, id).material, Some(brick));
    // Ctrl+click samples the plane's material back.
    rig.ws.grid.material = None;
    rig.click_with(
        rig.screen_of([3584.0, 512.0, 3584.0]),
        egui::Modifiers::COMMAND,
    );
    assert_eq!(rig.ws.grid.material, Some(brick));
    // A palette click applies to the selected plane.
    rig.ws.select_planes(vec![id]);
    assert_eq!(rig.ws.apply_grid_material_to_selection(moss), 1);
    assert_eq!(plane(&rig, id).material, Some(moss));
    // The faces behind a plane are untouched by painting the plane.
    assert!(rig
        .room(0)
        .sector(0, 0)
        .unwrap()
        .floor
        .as_ref()
        .unwrap()
        .tri_a
        .material
        .is_none());
}

#[test]
fn nudge_and_turn_keys_move_selected_planes() {
    let (mut rig, id) = rig_with_plane("plane-keys", false, flat_plane());
    rig.key(egui::Key::PageUp);
    assert_eq!(plane(&rig, id).center[1], 256 + 256);
    rig.key(egui::Key::OpenBracket);
    assert_eq!(plane(&rig, id).center[1], 256);
    rig.key(egui::Key::R);
    assert_eq!(plane(&rig, id).rotation[0], 90);
    // N chooses the Plane tool.
    rig.key(egui::Key::N);
    assert_eq!(rig.ws.grid.tool, GridTool::Plane);
}

#[test]
fn select_picks_a_plane_in_front_of_the_floor_in_3d_and_not_one_behind() {
    let mut p = GridPlane::new([3584, 768, 3584], [1024, 1024]);
    p.rotation = [0, 90, 0];
    let (mut rig, id) = rig_with_plane("plane-pick-3d", true, p);
    rig.click(rig.screen_of([0.5 * S, 0.0, 0.5 * S]));
    assert!(rig.ws.grid.plane_sel.is_empty());
    rig.click(rig.screen_of([3584.0, 768.0, 3584.0]));
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
}

#[test]
fn the_whole_editor_draws_with_planes_selected_in_both_views() {
    use super::grid_ui::{draw_editor, full_context};
    let ctx = full_context();
    let mut project = flat_project(8, 8, true);
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        for (rotation, collision) in [
            ([0, 90, 0], PlaneCollision::Walkable),
            ([30, 0, 0], PlaneCollision::Solid),
            ([0, 0, 0], PlaneCollision::Decor),
        ] {
            let mut p = GridPlane::new([3072, 1024, 3072], [2048, 1024]);
            p.rotation = rotation;
            p.collision = collision;
            add_plane(world, 0, p).unwrap();
        }
    }
    let mut ws = EditorWorkspace::with_project(test_temp_dir("plane-ui-draw"), project);
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    for tool in GridTool::ALL {
        ws.set_grid_tool(tool);
        ws.select_planes(vec![
            PlaneId { room: 0, index: 0 },
            PlaneId { room: 0, index: 1 },
        ]);
        for view_2d in [true, false] {
            let output = draw_editor(&mut ws, &ctx, view_2d);
            assert!(!output.shapes.is_empty());
        }
    }
}

/// A 2048 x 1024 wall standing at z = 3072 facing +z, toward the rig camera.
fn wall_plane() -> GridPlane {
    GridPlane::new([4096, 512, 3072], [2048, 1024])
}

fn at3(rig: &GridRig, x: f32, y: f32, z: f32) -> Pos2 {
    rig.screen_of([x, y, z])
}

#[test]
fn handles_work_in_the_3d_view_too() {
    let (mut rig, id) = rig_with_plane("plane-3d-handles", true, wall_plane());
    // Stretch: the right edge from x = 5120 out to 6144; the left stays at 3072.
    rig.drag(
        at3(&rig, 5120.0, 512.0, 3072.0),
        at3(&rig, 6144.0, 512.0, 3072.0),
    );
    let p = plane(&rig, id);
    assert_eq!(p.size[0], 3072, "{p:?}");
    assert_eq!(p.center[0], 4608);
    // Tilt: the arrow tip is half the longer side plus 128 in front of the
    // centre. Aim it straight up.
    let tip = at3(&rig, 4608.0, 512.0, 3072.0 + 1536.0 + 128.0);
    rig.drag(tip, at3(&rig, 4608.0, 512.0 + 1500.0, 3072.0));
    let p = plane(&rig, id);
    assert_eq!(p.rotation[1], 90, "{p:?}");
    // Back to upright by dragging the tip level again, then turn the ring
    // knob: centre to due east.
    let mut upright = plane(&rig, id);
    upright.rotation = [0, 0, 0];
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        *id.get_mut(world).unwrap() = upright;
    }
    rig.ws.grid_changed();
    rig.pump(vec![], egui::Modifiers::NONE);
    let radius = (3072.0f32).hypot(1024.0) / 2.0 + 192.0;
    let knob = at3(&rig, 4608.0, 512.0, 3072.0 + radius);
    rig.drag(knob, at3(&rig, 4608.0 + 900.0, 512.0, 3072.0 - 30.0));
    assert_eq!(plane(&rig, id).rotation[0], 90);
}

#[test]
fn the_body_slides_in_its_own_plane_in_3d() {
    let (mut rig, id) = rig_with_plane("plane-3d-move", true, wall_plane());
    rig.drag(
        at3(&rig, 4096.0, 512.0, 3072.0),
        at3(&rig, 4096.0 + 1024.0, 512.0 + 256.0, 3072.0),
    );
    let p = plane(&rig, id);
    assert_eq!(p.center[2], 3072, "a wall slides along itself: {p:?}");
    assert_eq!(p.center[0], 5120);
    assert_eq!(p.center[1], 768);
}
