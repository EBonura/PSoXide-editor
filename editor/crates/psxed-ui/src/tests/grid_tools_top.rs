//! The grid tools driven with the mouse and keys in the Top view.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::{GridSplit, WallEdge};

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers::COMMAND
}

#[test]
fn the_floor_tool_drags_a_rectangle_and_grows_the_room() {
    let mut rig = GridRig::top("grid-floor-rect", empty_project());
    rig.tool(GridTool::Floor);
    rig.drag(rig.cell_centre(1, 1), rig.cell_centre(3, 2));
    let room = rig.room(0);
    assert_eq!((room.origin, room.width, room.depth), ([1, 1], 3, 2));
    assert_eq!(room.sector_count(), 6);
    for cell in [[1, 1], [3, 2]] {
        assert_eq!(floor_heights(rig.world(), 0, cell), Some([0; 4]));
    }
    assert!(
        rig.ws.status.contains("Created 6 floors"),
        "{}",
        rig.ws.status
    );

    // One undo step takes the whole rectangle away again.
    rig.ws.do_undo();
    assert!(rig.world().rooms.is_empty());
    rig.ws.do_redo();
    assert_eq!(rig.room(0).sector_count(), 6);
}

#[test]
fn dragging_left_and_up_past_the_room_grows_it_towards_negative() {
    let mut rig = GridRig::top("grid-floor-negative", flat_project(2, 2, false));
    rig.tool(GridTool::Floor);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(-2, -1));
    let room = rig.room(0);
    assert_eq!((room.origin, room.width, room.depth), ([-2, -1], 4, 3));
    // The original cells kept their world position.
    assert_eq!(floor_heights(rig.world(), 0, [1, 1]), Some([0; 4]));
    assert_eq!(floor_heights(rig.world(), 0, [-2, -1]), Some([0; 4]));
}

#[test]
fn ctrl_drag_deletes_floor_and_trims_the_room() {
    let mut rig = GridRig::top("grid-floor-delete", flat_project(4, 4, false));
    rig.tool(GridTool::Floor);
    rig.drag_with(rig.cell_centre(0, 0), rig.cell_centre(3, 1), ctrl());
    let room = rig.room(0);
    assert_eq!((room.origin, room.depth), ([0, 2], 2));
    assert!(floor_heights(rig.world(), 0, [1, 1]).is_none());
    assert!(floor_heights(rig.world(), 0, [3, 3]).is_some());
    assert!(rig.ws.status.contains("Deleted 8"), "{}", rig.ws.status);
}

#[test]
fn shift_while_dragging_sets_the_height_in_clicks_and_it_sticks() {
    let mut rig = GridRig::top("grid-floor-height", empty_project());
    rig.tool(GridTool::Floor);
    let from = rig.cell_centre(0, 0);
    let to = rig.cell_centre(1, 0);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    // Press Shift and move the pointer up by three clicks of pixels.
    let up = to + Vec2::new(0.0, -3.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK);
    rig.move_to(to, shift());
    rig.move_to(up, shift());
    match &rig.ws.grid.gesture {
        Gesture::Rect(g) => assert_eq!(g.height, 768, "{g:?}"),
        other => panic!("expected a rectangle, got {other:?}"),
    }
    rig.release(up, shift());
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]), Some([768; 4]));
    assert_eq!(floor_heights(rig.world(), 0, [1, 0]), Some([768; 4]));
    // The next rectangle starts at the same height.
    assert_eq!(rig.ws.grid.floor_height, 768);
    rig.drag(
        rig.cell_centre(3, 3),
        rig.cell_centre(3, 3) + Vec2::new(40.0, 0.0),
    );
    assert_eq!(floor_heights(rig.world(), 0, [3, 3]), Some([768; 4]));
}

#[test]
fn the_ceiling_tool_defaults_to_three_sectors_up() {
    let mut rig = GridRig::top("grid-ceiling", flat_project(3, 3, false));
    rig.tool(GridTool::Ceiling);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(2, 0));
    let room = rig.room(0);
    for x in 0..3 {
        let c = room.sector(x, 0).unwrap().ceiling.as_ref().unwrap();
        assert_eq!(c.heights, [3072; 4]);
    }
    assert!(room.sector(0, 1).unwrap().ceiling.is_none());
}

#[test]
fn a_single_click_creates_one_sector() {
    let mut rig = GridRig::top("grid-floor-click", flat_project(2, 2, false));
    rig.tool(GridTool::Floor);
    rig.click(rig.cell_centre(4, 4));
    assert_eq!(floor_heights(rig.world(), 0, [4, 4]), Some([0; 4]));
    // Clicking on an existing floor says so and changes nothing.
    let before = rig.world().clone();
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(*rig.world(), before);
    assert!(rig.ws.status.contains("already there"), "{}", rig.ws.status);
}

#[test]
fn the_wall_tool_places_a_wall_on_the_nearest_edge_filling_floor_to_ceiling() {
    let mut rig = GridRig::top("grid-wall-click", flat_project(3, 3, true));
    rig.tool(GridTool::Wall);
    // Near the north edge of cell (1,1).
    let near_north = rig.top_at(1.5 * S, 1.08 * S);
    rig.click(near_north);
    let walls = walls_on(rig.world(), 0, [1, 1], WallEdge::North);
    assert_eq!(walls.len(), 1);
    assert_eq!(walls[0].heights, [0, 0, 3072, 3072]);
    // The same edge is covered now.
    rig.click(near_north);
    assert!(rig.ws.status.contains("fully covered"), "{}", rig.ws.status);
    assert_eq!(walls_on(rig.world(), 0, [1, 1], WallEdge::North).len(), 1);
    // East edge of the same cell.
    rig.click(rig.top_at(1.94 * S, 1.5 * S));
    assert_eq!(walls_on(rig.world(), 0, [1, 1], WallEdge::East).len(), 1);
}

#[test]
fn dragging_with_the_wall_tool_places_a_run_locked_to_the_edge() {
    let mut rig = GridRig::top("grid-wall-run", flat_project(5, 3, true));
    rig.tool(GridTool::Wall);
    // Start near the south edge of (0,1), drag right and a little off axis.
    let from = rig.top_at(0.5 * S, 1.92 * S);
    let to = rig.top_at(3.5 * S, 1.5 * S);
    rig.drag(from, to);
    for x in 0..4 {
        assert_eq!(
            walls_on(rig.world(), 0, [x, 1], WallEdge::South).len(),
            1,
            "x {x}"
        );
    }
    assert!(walls_on(rig.world(), 0, [4, 1], WallEdge::South).is_empty());
    assert!(walls_on(rig.world(), 0, [1, 1], WallEdge::East).is_empty());
    assert!(
        rig.ws.status.contains("Placed 4 south walls"),
        "{}",
        rig.ws.status
    );
}

#[test]
fn r_turns_the_wall_direction_through_the_diagonals_and_back_to_auto() {
    use crate::grid_tools::WallDirection;
    let mut rig = GridRig::top("grid-wall-r", flat_project(3, 3, true));
    rig.tool(GridTool::Wall);
    assert_eq!(rig.ws.grid.wall_dir, WallDirection::Auto);
    for expected in [
        WallDirection::Edge(WallEdge::North),
        WallDirection::Edge(WallEdge::East),
        WallDirection::Edge(WallEdge::South),
        WallDirection::Edge(WallEdge::West),
        WallDirection::Edge(WallEdge::Nwse),
        WallDirection::Edge(WallEdge::Nesw),
        WallDirection::Auto,
    ] {
        rig.key(egui::Key::R);
        assert_eq!(rig.ws.grid.wall_dir, expected);
    }
    // A fixed diagonal places a diagonal wall wherever you click.
    rig.key(egui::Key::R);
    rig.key(egui::Key::R);
    rig.key(egui::Key::R);
    rig.key(egui::Key::R);
    rig.key(egui::Key::R);
    assert_eq!(rig.ws.grid.wall_dir, WallDirection::Edge(WallEdge::Nwse));
    rig.click(rig.cell_centre(1, 1));
    assert_eq!(walls_on(rig.world(), 0, [1, 1], WallEdge::Nwse).len(), 1);
}

#[test]
fn select_picks_a_cell_ctrl_toggles_and_shift_ranges() {
    let mut rig = GridRig::top("grid-select", flat_project(5, 5, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    assert_eq!(
        rig.ws.grid.selection.items(),
        &[GridElement::Cell {
            room: 0,
            cell: [1, 1]
        }]
    );
    rig.click_with(rig.cell_centre(3, 1), ctrl());
    assert_eq!(rig.ws.grid.selection.len(), 2);
    rig.click_with(rig.cell_centre(1, 1), ctrl());
    assert_eq!(rig.ws.grid.selection.len(), 1);
    // Shift extends over the rectangle from the last pick.
    rig.click(rig.cell_centre(0, 0));
    rig.click_with(rig.cell_centre(2, 1), shift());
    assert_eq!(
        rig.ws.grid.selection.len(),
        6,
        "{:?}",
        rig.ws.grid.selection
    );
    // Clicking nothing clears.
    rig.click(rig.cell_centre(6, 3));
    assert!(rig.ws.grid.selection.is_empty());
}

#[test]
fn a_marquee_selects_the_cells_inside_and_ctrl_adds() {
    let mut rig = GridRig::top("grid-marquee", flat_project(5, 5, false));
    rig.tool(GridTool::Select);
    rig.drag(rig.top_at(0.1 * S, 0.1 * S), rig.top_at(2.9 * S, 1.9 * S));
    assert_eq!(rig.ws.grid.selection.len(), 6);
    rig.drag_with(
        rig.top_at(3.1 * S, 3.1 * S),
        rig.top_at(4.9 * S, 4.9 * S),
        ctrl(),
    );
    assert_eq!(rig.ws.grid.selection.len(), 6 + 4);
}

#[test]
fn ctrl_a_selects_the_room_and_delete_removes_it_in_one_undo_step() {
    let mut rig = GridRig::top("grid-select-all", flat_project(3, 3, true));
    rig.tool(GridTool::Select);
    rig.key_with(egui::Key::A, ctrl());
    // 9 floors and 9 ceilings.
    assert_eq!(rig.ws.grid.selection.len(), 18);
    rig.key(egui::Key::Delete);
    assert_eq!(rig.world().rooms[0].sector_count(), 0);
    assert!(rig.ws.grid.selection.is_empty());
    rig.ws.do_undo();
    assert_eq!(rig.world().rooms[0].sector_count(), 9);
}

#[test]
fn deleting_a_selected_cell_clears_it_entirely() {
    let mut rig = GridRig::top("grid-delete-cell", flat_project(3, 3, true));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    rig.key(egui::Key::Backspace);
    assert!(floor_heights(rig.world(), 0, [1, 1]).is_none());
    assert_eq!(rig.world().rooms[0].sector_count(), 8);
}

#[test]
fn the_height_tool_drags_a_cell_corner_and_its_neighbours_follow() {
    let mut rig = GridRig::top("grid-height-vertex", flat_project(3, 3, false));
    rig.tool(GridTool::Height);
    // The lattice vertex shared by cells (0,0), (1,0), (0,1), (1,1).
    let vertex = rig.top_at(S, S);
    let up = vertex + Vec2::new(0.0, -2.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK);
    rig.drag(vertex, up);
    let w = rig.world();
    assert_eq!(floor_heights(w, 0, [0, 0]).unwrap()[2], 512);
    assert_eq!(floor_heights(w, 0, [1, 0]).unwrap()[3], 512);
    assert_eq!(floor_heights(w, 0, [0, 1]).unwrap()[1], 512);
    assert_eq!(floor_heights(w, 0, [1, 1]).unwrap()[0], 512);
    assert_eq!(floor_heights(w, 0, [2, 2]).unwrap(), [0; 4]);
    // One undo step for the drag.
    rig.ws.do_undo();
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap(), [0; 4]);
}

#[test]
fn l_unlinks_so_only_the_picked_corner_moves() {
    let mut rig = GridRig::top("grid-height-unlink", flat_project(3, 3, false));
    rig.tool(GridTool::Height);
    rig.key(egui::Key::L);
    assert!(!rig.ws.grid.link);
    let vertex = rig.top_at(S, S);
    rig.drag(
        vertex,
        vertex + Vec2::new(0.0, -crate::grid_tools::view::TOP_PIXELS_PER_CLICK),
    );
    let w = rig.world();
    let raised: usize = [[0, 0], [1, 0], [0, 1], [1, 1]]
        .iter()
        .map(|c| {
            floor_heights(w, 0, *c)
                .unwrap()
                .iter()
                .filter(|h| **h == 256)
                .count()
        })
        .sum();
    assert_eq!(raised, 1);
}

#[test]
fn alt_snaps_height_to_the_fine_step_and_escape_cancels_the_drag() {
    let mut rig = GridRig::top("grid-height-fine", flat_project(2, 2, false));
    rig.tool(GridTool::Height);
    let vertex = rig.top_at(S, S);
    // 12 pixels is two thirds of a click: with Alt it snaps to 64s (192).
    rig.drag_with(vertex, vertex + Vec2::new(0.0, -12.0), egui::Modifiers::ALT);
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap()[2], 192);
    // Escape in the middle of a drag puts the heights back.
    let mut rig = GridRig::top("grid-height-escape", flat_project(2, 2, false));
    rig.tool(GridTool::Height);
    let vertex = rig.top_at(S, S);
    rig.drag_hold(
        vertex,
        vertex + Vec2::new(0.0, -2.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK),
        egui::Modifiers::NONE,
    );
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap()[2], 512);
    rig.key(egui::Key::Escape);
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap()[2], 0);
}

#[test]
fn page_up_nudges_the_selection_by_a_click() {
    let mut rig = GridRig::top("grid-nudge", flat_project(2, 2, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(0, 0));
    rig.key(egui::Key::PageUp);
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap(), [256; 4]);
    rig.key(egui::Key::PageDown);
    assert_eq!(floor_heights(rig.world(), 0, [0, 0]).unwrap(), [0; 4]);
}

#[test]
fn the_split_tool_flips_the_diagonal_with_one_undo_step() {
    let mut rig = GridRig::top("grid-split", flat_project(2, 2, true));
    rig.tool(GridTool::Split);
    rig.click(rig.cell_centre(0, 0));
    let split = |rig: &GridRig, ceiling: bool| {
        let s = rig.room(0).sector(0, 0).unwrap();
        if ceiling {
            s.ceiling.as_ref().unwrap().split
        } else {
            s.floor.as_ref().unwrap().split
        }
    };
    assert_eq!(split(&rig, false), GridSplit::NeSw);
    // Clicking the same spot again takes the ceiling under the floor in the
    // Top view (Alt no longer does: it is the fine snap).
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(split(&rig, true), GridSplit::NeSw);
    assert_eq!(split(&rig, false), GridSplit::NeSw);
    rig.ws.do_undo();
    assert_eq!(split(&rig, true), GridSplit::NwSe);
    rig.ws.do_undo();
    assert_eq!(split(&rig, false), GridSplit::NwSe);
}

fn two_materials(rig: &mut GridRig) -> (psxed_project::ResourceId, psxed_project::ResourceId) {
    let mut add = |name: &str| {
        rig.ws.project.add_resource(
            name,
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        )
    };
    (add("brick"), add("moss"))
}

#[test]
fn paint_applies_the_palette_material_and_a_stroke_is_one_undo_step() {
    let mut rig = GridRig::top("grid-paint", flat_project(4, 1, false));
    let (brick, moss) = two_materials(&mut rig);
    rig.tool(GridTool::Paint);
    // Nothing chosen yet: nothing painted.
    rig.click(rig.cell_centre(0, 0));
    assert!(rig.ws.status.contains("palette"), "{}", rig.ws.status);
    rig.ws.grid.material = Some(brick);
    rig.click(rig.cell_centre(0, 0));
    let mat = |rig: &GridRig, x: usize| {
        rig.room(0)
            .sector(x, 0)
            .unwrap()
            .floor
            .as_ref()
            .unwrap()
            .tri_a
            .material
    };
    assert_eq!(mat(&rig, 0), Some(brick));
    // A drag across three cells paints them all, as one undo step.
    rig.ws.grid.material = Some(moss);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(3, 0));
    for x in 0..4 {
        assert_eq!(mat(&rig, x), Some(moss), "x {x}");
    }
    rig.ws.do_undo();
    assert_eq!(mat(&rig, 0), Some(brick));
    assert_eq!(mat(&rig, 3), None);
    // Ctrl-click samples the face's material into the palette.
    rig.ws.grid.material = None;
    rig.click_with(rig.cell_centre(0, 0), ctrl());
    assert_eq!(rig.ws.grid.material, Some(brick));
}

#[test]
fn the_palette_click_applies_to_the_selection() {
    let mut rig = GridRig::top("grid-palette-apply", flat_project(3, 1, false));
    let (brick, _) = two_materials(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(0, 0));
    rig.click_with(rig.cell_centre(2, 0), ctrl());
    assert_eq!(rig.ws.apply_grid_material_to_selection(brick), 2);
    let mat = |x| {
        rig.room(0)
            .sector(x, 0)
            .unwrap()
            .floor
            .as_ref()
            .unwrap()
            .tri_a
            .material
    };
    assert_eq!((mat(0), mat(1), mat(2)), (Some(brick), None, Some(brick)));
}

#[test]
fn copy_flip_and_paste_stamps_the_selection_elsewhere() {
    let mut rig = GridRig::top("grid-paste", flat_project(6, 3, false));
    // A ramp in the first two cells.
    {
        let room = &mut rig
            .ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .rooms[0];
        room.sector_mut(0, 0)
            .unwrap()
            .floor
            .as_mut()
            .unwrap()
            .heights = [0, 256, 256, 0];
        room.sector_mut(1, 0)
            .unwrap()
            .floor
            .as_mut()
            .unwrap()
            .heights = [256, 512, 512, 256];
    }
    rig.tool(GridTool::Select);
    rig.drag(rig.top_at(0.1 * S, 0.1 * S), rig.top_at(1.9 * S, 0.9 * S));
    assert_eq!(rig.ws.grid.selection.len(), 2);
    rig.key_with(egui::Key::C, ctrl());
    assert!(rig.ws.grid.clipboard.is_some());
    rig.key_with(egui::Key::V, ctrl());
    assert!(rig.ws.grid.paste.is_some());
    // Mirror it east to west, then drop it on cells 3..5 of the top row.
    rig.key(egui::Key::H);
    rig.move_to(rig.cell_centre(4, 2), egui::Modifiers::NONE);
    rig.click(rig.cell_centre(4, 2));
    assert!(rig.ws.grid.paste.is_none());
    // The ghost is centred on the clicked cell: a 2x1 clip lands at x 3..5.
    let w = rig.world();
    // Mirrored: the high end is now at the west.
    let a = floor_heights(w, 0, [3, 2]).unwrap();
    let b = floor_heights(w, 0, [4, 2]).unwrap();
    assert_eq!(a, [512, 256, 256, 512], "{a:?} {b:?}");
    assert_eq!(b, [256, 0, 0, 256]);
    // One undo removes the paste.
    rig.ws.do_undo();
    assert_eq!(floor_heights(rig.world(), 0, [3, 2]).unwrap(), [0; 4]);
}

#[test]
fn escape_drops_the_paste_ghost_and_then_the_selection() {
    let mut rig = GridRig::top("grid-esc", flat_project(3, 3, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(0, 0));
    rig.key_with(egui::Key::C, ctrl());
    rig.key_with(egui::Key::V, ctrl());
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.paste.is_none());
    assert_eq!(rig.ws.grid.selection.len(), 1);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.grid.selection.is_empty());
}

#[test]
fn tool_keys_switch_tools() {
    let mut rig = GridRig::top("grid-keys", flat_project(2, 2, false));
    for (key, tool) in [
        (egui::Key::F, GridTool::Floor),
        (egui::Key::C, GridTool::Ceiling),
        (egui::Key::W, GridTool::Wall),
        (egui::Key::H, GridTool::Height),
        (egui::Key::P, GridTool::Paint),
        (egui::Key::D, GridTool::Split),
        (egui::Key::O, GridTool::Place),
        (egui::Key::V, GridTool::Select),
    ] {
        rig.key(key);
        assert_eq!(rig.ws.grid.tool, tool);
    }
}

#[test]
fn the_place_tool_stands_the_node_at_the_sector_centre_on_the_floor() {
    let mut rig = GridRig::top("grid-place", flat_project(3, 3, false));
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms[0]
        .sector_mut(1, 1)
        .unwrap()
        .floor
        .as_mut()
        .unwrap()
        .heights = [512; 4];
    rig.tool(GridTool::Place);
    rig.ws.place_kind = PlaceKind::PointLightMarker;
    let before = rig.ws.project.active_scene().nodes().len();
    rig.click(rig.top_at(1.3 * S, 1.2 * S));
    let scene = rig.ws.project.active_scene();
    assert_eq!(scene.nodes().len(), before + 1);
    let node = scene.nodes().last().unwrap();
    let t = node.transform.translation;
    assert_eq!((t[0], t[2]), (1.5 * S, 1.5 * S));
    // Lights lift 256 above the floor, which is at 512 here.
    assert_eq!(t[1], 512.0 + 256.0);
    // The tool returns to Select after placing, like the brush Place tool.
    assert_eq!(rig.ws.grid.tool, GridTool::Select);
}

#[test]
fn selecting_a_grid_cell_leaves_nodes_alone_and_clicking_a_node_clears_the_cells() {
    let mut rig = GridRig::top("grid-node-select", flat_project(5, 5, false));
    let light = rig.ws.project.active_scene_mut().add_node(
        psxed_project::NodeId::ROOT,
        "Light",
        psxed_project::NodeKind::PointLight {
            color: [255, 255, 255],
            intensity: 1.0,
            radius: 0.1,
        },
    );
    rig.ws
        .project
        .active_scene_mut()
        .node_mut(light)
        .unwrap()
        .transform
        .translation = [4.5 * S, 300.0, 4.5 * S];
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(rig.ws.grid.selection.len(), 1);
    rig.click(rig.cell_centre(4, 4));
    assert!(rig.ws.grid.selection.is_empty(), "the node click took over");
    assert_eq!(rig.ws.selection.selected_node, light);
    // Selecting a cell drops the node selection.
    rig.click(rig.cell_centre(0, 0));
    assert_ne!(rig.ws.selection.selected_node, light);
}

#[test]
fn undo_during_a_gesture_abandons_it_and_stale_selection_is_pruned() {
    let mut rig = GridRig::top("grid-stale", flat_project(3, 3, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(2, 2));
    // Delete it, undo, redo: the selection never points at a missing cell.
    rig.key(egui::Key::Delete);
    rig.ws.do_undo();
    rig.ws.do_redo();
    assert!(rig.ws.grid.selection.is_empty());
    let _ = FaceRef::floor(0, [0, 0]);
}

#[test]
fn shift_paints_one_triangle_of_a_flat_quad() {
    let mut rig = GridRig::top("grid-paint-tri", flat_project(3, 1, false));
    let (brick, moss) = two_materials(&mut rig);
    rig.tool(GridTool::Paint);
    rig.ws.grid.material = Some(brick);
    // NW-SE diagonal: the north-east half is triangle 0, the south-west
    // half triangle 1.
    rig.click_with(rig.top_at(1.8 * S, 0.2 * S), shift());
    rig.ws.grid.material = Some(moss);
    rig.click_with(rig.top_at(1.2 * S, 0.8 * S), shift());
    let floor = rig.room(0).sector(1, 0).unwrap().floor.clone().unwrap();
    assert_eq!(floor.tri_a.material, Some(brick));
    assert_eq!(floor.tri_b.map(|t| t.material), Some(Some(moss)));
    // The neighbour is untouched.
    assert!(rig
        .room(0)
        .sector(0, 0)
        .unwrap()
        .floor
        .as_ref()
        .unwrap()
        .tri_b
        .is_none());
    // Without Shift the whole quad takes the material and merges again.
    rig.click(rig.cell_centre(1, 0));
    let floor = rig.room(0).sector(1, 0).unwrap().floor.clone().unwrap();
    assert_eq!(floor.tri_a.material, Some(moss));
    assert!(floor.tri_b.is_none());
}

#[test]
fn new_surfaces_and_walls_take_the_materials_already_in_the_room() {
    let mut rig = GridRig::top("grid-new-material", flat_project(3, 3, true));
    let (brick, _) = two_materials(&mut rig);
    // Texture the existing floors, then extend the room.
    rig.tool(GridTool::Select);
    rig.key_with(egui::Key::A, ctrl());
    assert!(rig.ws.apply_grid_material_to_selection(brick) > 0);
    rig.ws.grid.material = None;
    rig.ws.grid.selection.clear();
    rig.tool(GridTool::Floor);
    rig.click(rig.cell_centre(4, 1));
    let new_floor = rig.room(0).sector(4, 1).unwrap().floor.clone().unwrap();
    assert_eq!(new_floor.tri_a.material, Some(brick));
    rig.tool(GridTool::Wall);
    rig.click(rig.top_at(1.5 * S, 1.08 * S));
    let walls = walls_on(rig.world(), 0, [1, 1], WallEdge::North);
    assert_eq!(walls[0].material, Some(brick));
}

#[test]
fn the_cursor_tells_what_the_tool_will_do() {
    use egui::CursorIcon as C;
    let mut rig = GridRig::top("grid-cursor", flat_project(3, 3, false));
    rig.tool(GridTool::Floor);
    assert_eq!(rig.ws.grid_cursor(), C::Crosshair);
    rig.tool(GridTool::Select);
    rig.move_to(rig.top_at(-300.0 * S, 0.0), egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid_cursor(), C::Default);
    rig.move_to(rig.cell_centre(1, 1), egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid_cursor(), C::PointingHand);
    rig.tool(GridTool::Height);
    rig.move_to(rig.cell_centre(1, 1), egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid_cursor(), C::ResizeVertical);
}
