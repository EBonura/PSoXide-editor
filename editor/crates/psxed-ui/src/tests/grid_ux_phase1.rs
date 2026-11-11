//! Phase 1 of the UX redesign, driven as state x input sequences.

use super::grid_rig::*;
use crate::grid_tools::GridTool;

const TOOLS: [GridTool; 6] = [
    GridTool::Height,
    GridTool::Paint,
    GridTool::Floor,
    GridTool::Wall,
    GridTool::Extrude,
    GridTool::Select,
];

#[test]
fn the_selection_survives_every_tool_switch() {
    let mut rig = GridRig::top("ux-select-survives", flat_project(4, 4, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    let picked = rig.ws.grid.selection.len();
    assert_eq!(picked, 1);
    for tool in TOOLS {
        rig.tool(tool);
        assert_eq!(
            rig.ws.grid.selection.len(),
            picked,
            "{} dropped the selection",
            tool.label()
        );
    }
}

fn esc_cancels_with_nothing_left(tool: GridTool, from: (i32, i32), to: (i32, i32)) {
    let mut rig = GridRig::top("ux-esc", flat_project(4, 4, false));
    if tool == GridTool::Paint {
        let material = rig.ws.project.add_resource(
            "brick",
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        );
        rig.ws.grid.material = Some(material);
    }
    rig.tool(tool);
    let before = rig.ws.project.clone();
    let can_undo = rig.ws.history.can_undo();
    rig.drag_hold(
        rig.cell_centre(from.0, from.1),
        rig.cell_centre(to.0, to.1),
        egui::Modifiers::NONE,
    );
    assert!(
        rig.ws.project != before,
        "{}: the stroke did nothing",
        tool.label()
    );
    rig.key(egui::Key::Escape);
    rig.release(rig.cell_centre(to.0, to.1), egui::Modifiers::NONE);
    assert!(
        rig.ws.project == before,
        "{}: the document changed",
        tool.label()
    );
    assert_eq!(
        rig.ws.history.can_undo(),
        can_undo,
        "{}: undo step left",
        tool.label()
    );
    assert!(
        !rig.ws.history.can_redo(),
        "{}: redo step left",
        tool.label()
    );
}

#[test]
fn esc_cancels_a_paint_stroke_and_restores_the_document() {
    esc_cancels_with_nothing_left(GridTool::Paint, (0, 0), (3, 0));
}

#[test]
fn esc_cancels_a_cells_stroke_and_restores_the_document() {
    esc_cancels_with_nothing_left(GridTool::Cells, (0, 0), (3, 3));
}

#[test]
fn a_cells_stroke_is_one_undo_step() {
    let mut rig = GridRig::top("ux-cells-one-step", flat_project(4, 4, false));
    let before = rig.ws.project.clone();
    rig.tool(GridTool::Cells);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(3, 3));
    assert!(rig.ws.project != before);
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

#[test]
fn the_rooms_plus_makes_a_walled_room_beside_the_current_one_at_its_floor() {
    let mut project = flat_project(4, 4, false);
    project.active_scene_mut().world.grid_mut().unwrap().rooms[0].y_offset = 512;
    let mut rig = GridRig::top("ux-add-room", project);
    let before = rig.ws.project.clone();
    rig.ws.grid_add_room();
    let world = rig.world();
    assert_eq!(world.rooms.len(), 2);
    let (first, new) = (&world.rooms[0], &world.rooms[1]);
    assert_eq!(new.y_offset, first.y_offset, "at the floor level");
    assert!(
        new.origin[0] >= first.origin[0] + first.width as i32,
        "beside"
    );
    assert_eq!(new.sector_count(), 36);
    let sector = new.sector(0, 0).unwrap();
    assert!(sector.ground_floor().is_some() && sector.ground_ceiling().is_some());
    // The room's walls are the derived closure walls: it is a derived room.
    assert!(matches!(
        new.wall_mode,
        psxed_project::grid::WallMode::Derived
    ));
    // A second one does not land on the first.
    rig.ws.grid_add_room();
    let world = rig.world();
    let (a, b) = (&world.rooms[1], &world.rooms[2]);
    assert!(b.origin[0] >= a.origin[0] + a.width as i32);
    rig.ws.do_undo();
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

#[test]
fn a_height_press_on_empty_space_boxes_and_one_on_nothing_movable_says_why() {
    let mut rig = GridRig::top("ux-height-box", flat_project(4, 4, false));
    rig.tool(GridTool::Height);
    // Off the room: a box selection, not a dead press.
    let off = rig.cell_centre(-1, -1);
    rig.drag_hold(off, rig.cell_centre(-1, 0), egui::Modifiers::NONE);
    assert!(
        matches!(
            rig.ws.grid.gesture,
            crate::grid_tools::Gesture::Marquee { .. }
        ),
        "no box started"
    );
    rig.key(egui::Key::Escape);
    rig.release(rig.cell_centre(-1, 0), egui::Modifiers::NONE);
    assert!(matches!(
        rig.ws.grid.gesture,
        crate::grid_tools::Gesture::Idle
    ));
    assert!(rig.ws.status.contains("Cancelled"));
}

#[test]
fn dragging_a_floor_face_in_height_leaves_a_flat_platform() {
    let mut project = flat_project(3, 3, false);
    // A sloped floor under the pointer: one corner already higher.
    {
        let room = &mut project.active_scene_mut().world.grid_mut().unwrap().rooms[0];
        let floor = room
            .sector_mut(1, 1)
            .and_then(|s| s.ground_floor_mut())
            .unwrap();
        floor.heights = [0, 0, 256, 0];
    }
    let mut rig = GridRig::top("ux-flat-face", project);
    rig.tool(GridTool::Height);
    let centre = rig.cell_centre(1, 1);
    rig.drag(
        centre,
        centre + egui::Vec2::new(0.0, -2.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK),
    );
    let heights = floor_heights(rig.world(), 0, [1, 1]).unwrap();
    assert!(
        heights.iter().all(|h| *h == heights[0]) && heights[0] > 256,
        "not a flat platform: {heights:?}"
    );
}

#[test]
fn the_far_view_preset_sets_the_citadel_values_and_leaves_the_rest() {
    let mut stream = psxed_project::grid::StreamRadii::DEFAULT;
    stream.flood.skin = true;
    crate::grid_tools::world_panel::apply_far_view_preset(&mut stream);
    assert_eq!(
        (stream.draw_far, stream.active_radius, stream.lead_radius),
        (12, 8, 4)
    );
    assert!(stream.ring && stream.flood.on);
    assert_eq!(
        (
            stream.flood.near_radius,
            stream.flood.res_radius,
            stream.flood.panels
        ),
        (12, 13, 10)
    );
    assert!(stream.flood.skin, "an unrelated setting stays");
}

#[test]
fn edit_delete_selection_deletes_picked_faces_and_a_chosen_room() {
    let mut rig = GridRig::top("ux-delete-selection", flat_project(4, 4, false));
    rig.ws.grid_add_room();
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    assert!(rig.ws.grid_selection_deletable());
    let before = rig.world().rooms[0].sector_count();
    assert!(rig.ws.delete_grid_selection());
    assert!(rig.world().rooms[0].sector_count() < before, "faces went");
    rig.ws.select_room(1);
    assert!(rig.ws.grid_selection_deletable());
    assert!(rig.ws.delete_grid_selection());
    assert_eq!(rig.world().rooms.len(), 1, "the room went");
    rig.ws.do_undo();
    assert_eq!(rig.world().rooms.len(), 2);
}
