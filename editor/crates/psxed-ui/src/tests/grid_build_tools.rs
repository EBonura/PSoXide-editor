//! The Build tools replayed through real input in the plan and the 3D view:
//! the acceptance tasks of the review they cover (room beside another, room
//! on its own storey height, doorway, window, corridor between two rooms),
//! each with its hint, its refusal and its single undo step.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::{InteractableKind, LogicNodeKind, NodeKind};

fn hint(rig: &GridRig) -> String {
    rig.ws.grid.build.hint.clone()
}

fn rooms(rig: &GridRig) -> usize {
    rig.world().rooms.len()
}

fn nodes(rig: &GridRig) -> usize {
    rig.ws.project.active_scene().nodes().len()
}

fn at(rig: &GridRig, x: f32, y: f32, z: f32) -> Pos2 {
    rig.screen_of([x * S, y, z * S])
}

fn tall() -> ProjectDocument {
    let mut project = derived_project_for_tiling(5, 5);
    let room = &mut project.active_scene_mut().world.grid_mut().unwrap().rooms[0];
    for z in 0..5 {
        for x in 0..5 {
            if let Some(c) = room.sector_mut(x, z).and_then(|s| s.ground_ceiling_mut()) {
                c.heights = [2048; 4];
            }
        }
    }
    project
}

#[test]
fn edit_is_the_default_tool_and_the_build_keys_choose_the_build_tools() {
    let mut rig = GridRig::top("build-keys", derived_project_for_tiling(5, 5));
    assert_eq!(rig.ws.grid.tool, GridTool::Edit, "Edit is the default tool");
    for (key, tool) in [
        (egui::Key::B, GridTool::Room),
        (egui::Key::K, GridTool::Corridor),
        (egui::Key::I, GridTool::Opening),
        (egui::Key::Y, GridTool::Arcade),
        (egui::Key::J, GridTool::Bridge),
    ] {
        rig.key(key);
        assert_eq!(rig.ws.grid.tool, tool);
    }
}

#[test]
fn a_room_beside_another_is_two_gestures_in_the_plan() {
    let mut rig = GridRig::top("build-room-plan", derived_project_for_tiling(5, 5));
    let before = rig.ws.project.clone();
    rig.key(egui::Key::B);
    rig.drag(rig.cell_centre(6, 1), rig.cell_centre(9, 3));
    assert_eq!(rooms(&rig), 2, "{}", rig.ws.status);
    assert!(
        rig.ws.status.starts_with("New room of 12 sectors"),
        "{}",
        rig.ws.status
    );
    rig.ws.do_undo();
    assert!(rig.ws.project == before, "one undo step puts it all back");
}

#[test]
fn a_room_in_3d_lifts_with_a_typed_number_and_escape_restores() {
    let mut rig = GridRig::persp("build-room-3d", derived_project_for_tiling(5, 5));
    let before = rig.ws.project.clone();
    rig.tool(GridTool::Room);
    let from = at(&rig, 6.5, 0.0, 1.5);
    let to = at(&rig, 8.5, 0.0, 3.5);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    assert_eq!(rooms(&rig), 2, "{}", hint(&rig));
    rig.key(egui::Key::Num5);
    assert_eq!(rig.ws.grid.build.opts.room.height, 1280, "{}", hint(&rig));
    rig.key(egui::Key::Escape);
    assert!(rig.ws.project == before, "Esc restores exactly");
    assert!(rig.ws.history.pop_last().is_none(), "and leaves no step");
    // Typed again and released: the height is kept.
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    rig.key(egui::Key::Num8);
    rig.release(to, egui::Modifiers::NONE);
    assert_eq!(rooms(&rig), 2);
    let ceiling = rig
        .room(1)
        .leaves()
        .next()
        .and_then(|(_, s)| s.ground_ceiling().map(|c| c.heights));
    assert_eq!(ceiling, Some([2048; 4]));
}

#[test]
fn a_build_drag_says_its_size_in_sectors_and_player_heights() {
    let mut rig = GridRig::top("build-ruler", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Room);
    let from = rig.cell_centre(6, 0);
    let to = rig.cell_centre(11, 5);
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    let text = hint(&rig);
    assert!(
        text.contains("sectors") && text.contains("player heights"),
        "{text}"
    );
    rig.release(to, egui::Modifiers::NONE);
}

#[test]
fn an_octagon_room_has_cut_corners() {
    let mut rig = GridRig::top("build-room-oct", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Room);
    rig.ws.grid.build.opts.room.shape = crate::grid_tools::build::options::RoomShape::Octagon;
    rig.drag(rig.cell_centre(6, 0), rig.cell_centre(11, 5));
    assert_eq!(rooms(&rig), 2, "{}", rig.ws.status);
    assert!(rig.room(1).leaf_count() < 36, "corners are cut");
}

#[test]
fn a_corridor_is_clicks_then_enter_and_one_undo_step() {
    let mut rig = GridRig::top("build-corridor", derived_project_for_tiling(5, 5));
    let before = rig.ws.project.clone();
    rig.key(egui::Key::K);
    rig.click(rig.cell_centre(5, 2));
    rig.click(rig.cell_centre(8, 2));
    // The ghost is already in the document.
    assert_eq!(rooms(&rig), 2, "{}", hint(&rig));
    rig.key(egui::Key::Enter);
    assert_eq!(rooms(&rig), 2);
    assert!(rig.ws.status.starts_with("Corridor"), "{}", rig.ws.status);
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

#[test]
fn a_door_in_3d_is_one_drag_along_a_wall() {
    let mut rig = GridRig::persp("build-opening", tall());
    let before = rig.ws.project.clone();
    rig.key(egui::Key::I);
    rig.drag(at(&rig, 1.25, 512.0, 0.0), at(&rig, 1.75, 512.0, 0.0));
    assert!(
        rig.ws.status.starts_with("Door cut"),
        "{} / {}",
        rig.ws.status,
        hint(&rig)
    );
    assert!(rig.ws.project != before);
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

#[test]
fn a_window_is_a_chip_away() {
    let mut rig = GridRig::persp("build-window", tall());
    rig.tool(GridTool::Opening);
    rig.ws.grid.build.opts.opening.window = true;
    rig.ws.grid.build.opts.opening.head = crate::grid_tools::build::options::OpeningHead::Pointed;
    rig.drag(at(&rig, 2.0, 1024.0, 0.0), at(&rig, 2.5, 1024.0, 0.0));
    assert!(rig.ws.status.starts_with("Window cut"), "{}", rig.ws.status);
}

#[test]
fn an_opening_refuses_in_plain_words() {
    let mut rig = GridRig::persp("build-opening-refuse", tall());
    rig.tool(GridTool::Opening);
    // Over the floor, not a wall.
    rig.drag(at(&rig, 2.0, 0.0, 2.0), at(&rig, 2.5, 0.0, 2.0));
    assert!(rig.ws.status.contains("wall"), "{}", rig.ws.status);
}

#[test]
fn an_arcade_is_a_drag_along_the_floor() {
    let mut rig = GridRig::top("build-arcade", derived_project_for_tiling(5, 5));
    let before = rig.ws.project.clone();
    rig.key(egui::Key::Y);
    rig.drag(rig.cell_centre(0, 2), rig.cell_centre(4, 2));
    assert!(rig.ws.status.starts_with("Arcade"), "{}", rig.ws.status);
    assert!(rig.ws.project != before);
}

#[test]
fn a_bridge_is_two_clicks() {
    let mut rig = GridRig::top("build-bridge", derived_project_for_tiling(5, 5));
    rig.key(egui::Key::J);
    rig.click(rig.cell_centre(0, 0));
    rig.click(rig.cell_centre(4, 0));
    assert!(rig.ws.status.starts_with("Bridge"), "{}", rig.ws.status);
}

#[test]
fn a_building_a_terrain_patch_and_a_ruin_each_take_a_drag() {
    let mut rig = GridRig::top("build-more", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Building);
    rig.drag(rig.cell_centre(7, 0), rig.cell_centre(12, 5));
    assert!(rig.ws.status.starts_with("Building"), "{}", rig.ws.status);
    rig.tool(GridTool::Terrain);
    rig.ws.grid.build.opts.terrain.amplitude = 2;
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(4, 4));
    assert!(rig.ws.status.starts_with("Terrain"), "{}", rig.ws.status);
    rig.tool(GridTool::Ruin);
    rig.ws.grid.build.opts.ruin.whole_room = true;
    rig.click(rig.cell_centre(2, 2));
    assert!(rig.ws.status.starts_with("Ruin"), "{}", rig.ws.status);
}

#[test]
fn the_seed_rerolls_with_shift_x() {
    let mut rig = GridRig::top("build-seed", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Terrain);
    let seed = rig.ws.grid.build.opts.terrain.seed;
    rig.key_with(egui::Key::X, egui::Modifiers::SHIFT);
    assert_ne!(rig.ws.grid.build.opts.terrain.seed, seed);
    assert_eq!(rig.ws.grid.tool, GridTool::Terrain);
}

#[test]
fn a_door_tool_cuts_the_doorway_and_places_the_sliding_door() {
    let mut rig = GridRig::persp("build-door", tall());
    let before = nodes(&rig);
    rig.tool(GridTool::Door);
    rig.ws.grid.build.opts.door.lock =
        crate::grid_tools::build::options::Lock::OneWay(psxed_project::DoorSide::North);
    rig.ws.grid.build.opts.door.refusal = "It is barred from this side.".to_string();
    rig.drag(at(&rig, 1.0, 512.0, 0.0), at(&rig, 2.0, 512.0, 0.0));
    assert!(rig.ws.status.starts_with("Door 1"), "{}", rig.ws.status);
    assert_eq!(nodes(&rig), before + 2, "the door and its slab");
    let scene = rig.ws.project.active_scene();
    let door = scene
        .nodes()
        .iter()
        .find_map(|n| match &n.kind {
            NodeKind::Logic {
                kind: LogicNodeKind::Door { lock, refusal, .. },
                persistent,
                ..
            } => Some((*lock, refusal.clone(), *persistent)),
            _ => None,
        })
        .expect("a door node");
    assert!(matches!(door.0, psxed_project::DoorLock::OneWay { .. }));
    assert_eq!(door.1, "It is barred from this side.");
    assert!(door.2, "remembered");
}

#[test]
fn a_lever_is_placed_then_bound_to_the_door_clicked_next() {
    let mut rig = GridRig::persp("build-lever", tall());
    rig.tool(GridTool::Door);
    rig.drag(at(&rig, 1.0, 512.0, 0.0), at(&rig, 2.0, 512.0, 0.0));
    rig.tool(GridTool::Lever);
    rig.click(at(&rig, 3.5, 0.0, 3.5));
    assert!(
        rig.ws.status.contains("click the door"),
        "{}",
        rig.ws.status
    );
    // Two sectors from the door is not the door: nothing is bound.
    rig.click(at(&rig, 3.5, 0.0, 0.5));
    assert!(rig.ws.status.contains("not a door"), "{}", rig.ws.status);
    rig.click(at(&rig, 1.5, 0.0, 0.5));
    let target = rig
        .ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find_map(|n| match &n.kind {
            NodeKind::Interactable {
                kind: InteractableKind::Switch { target, .. },
                ..
            } => Some(target.clone()),
            _ => None,
        })
        .expect("a lever");
    assert_eq!(target, "Door 1", "{}", rig.ws.status);
}

#[test]
fn escape_removes_a_lever_still_waiting_for_its_door() {
    let mut rig = GridRig::persp("build-lever-esc", tall());
    let before = rig.ws.project.clone();
    rig.tool(GridTool::Lever);
    rig.click(at(&rig, 3.5, 0.0, 3.5));
    assert!(rig.ws.project != before);
    rig.key(egui::Key::Escape);
    assert!(rig.ws.project == before);
}

#[test]
fn a_checkpoint_is_one_click() {
    let mut rig = GridRig::top("build-checkpoint", derived_project_for_tiling(5, 5));
    let before = nodes(&rig);
    rig.tool(GridTool::Checkpoint);
    rig.click(rig.cell_centre(2, 2));
    assert_eq!(nodes(&rig), before + 2);
}

#[test]
fn capture_bay_stores_a_style_that_the_picker_lists() {
    let mut rig = GridRig::top("build-capture", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Room);
    rig.ws.grid.selection.set(vec![
        psxed_project::grid::edit::GridElement::Cell {
            room: 0,
            cell: [0, 0],
        },
        psxed_project::grid::edit::GridElement::Cell {
            room: 0,
            cell: [1, 1],
        },
    ]);
    rig.ws.grid.build.capture_name = "My bay".to_string();
    rig.ws.capture_bay_style();
    assert!(rig.ws.status.contains("Captured"), "{}", rig.ws.status);
    assert!(
        rig.ws
            .grid
            .build
            .styles
            .iter()
            .any(|e| e.name == "My bay (bay)" && e.project),
        "{:?}",
        rig.ws
            .grid
            .build
            .styles
            .iter()
            .map(|e| &e.name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_wall_api_reads_a_wall_the_same_whatever_made_it_and_resets_it() {
    use crate::grid_tools::wall_section::{apply, WallEdit};
    use psxed_project::grid::edit::wall_api::{walls_at, Prop};
    use psxed_project::grid::{LeafKey, WallEdge};
    let mut rig = GridRig::persp("build-wall-api", tall());
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    let view = walls_at(world, 0, LeafKey::sector([2, 0]), WallEdge::North)
        .into_iter()
        .next()
        .expect("the closure wall");
    assert!(matches!(view.heights, Prop::Auto(_)));
    let top = view.heights.value()[2];
    let edit = WallEdit {
        heights: Some([None, None, Some(top - 512), Some(top - 512)]),
        reset: false,
        addr: Some(view.addr),
    };
    let text = apply(world, &edit).expect("a status");
    assert!(text.starts_with("Wall heights set"), "{text}");
    let after = walls_at(world, 0, LeafKey::sector([2, 0]), WallEdge::North);
    let set = after
        .iter()
        .find(|v| v.heights.is_set())
        .expect("a set wall");
    assert_eq!(set.heights.value()[2], top - 512);
    let reset = WallEdit {
        heights: None,
        reset: true,
        addr: Some(set.addr),
    };
    assert_eq!(
        apply(world, &reset).as_deref(),
        Some("Wall reset to automatic")
    );
    let back = walls_at(world, 0, LeafKey::sector([2, 0]), WallEdge::North);
    assert!(back.iter().all(|v| !v.heights.is_set()));
}

#[test]
fn play_from_here_moves_the_player_for_one_cook_and_asks_for_play() {
    let mut rig = GridRig::persp("build-play-here", tall());
    rig.ws.play_from_here([1280.0, 0.0, 2560.0], 0.0);
    assert!(rig.ws.status.contains("Play from here"));
    assert!(rig.ws.take_playtest_request().is_some());
}

/// Captures of each Build tool's ghost for review: `PSXED_BUILD_CAPTURE=<dir>`
/// writes the 3D view with the gesture in flight, one file per tool. Without
/// the variable this does nothing.
#[test]
fn build_tool_captures() {
    use super::grid_live::WindowRig;
    use super::grid_pick_shots::{look_from, sector, settle, three_d, wire};
    let Some(dir) = std::env::var_os("PSXED_BUILD_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("capture dir");
    let mut rig = WindowRig::new();
    three_d(&mut rig);
    let s = sector(&rig);
    look_from(
        &mut rig,
        [15.0 * s, 13000.0, 22.0 * s],
        [15.0 * s, 0.0, 5.0 * s],
    );
    settle(&mut rig);
    let p = |rig: &WindowRig, x: f32, y: f32, z: f32| rig.persp_screen([x * s, y, z * s]).unwrap();
    // Each shot: choose the tool, press at `a`, drag to `b`, capture, Esc.
    type Shot = (GridTool, (f32, f32, f32), (f32, f32, f32), &'static str);
    let shots: [Shot; 5] = [
        (GridTool::Room, (16.5, 0.0, 1.5), (20.5, 0.0, 4.5), "room"),
        (
            GridTool::Opening,
            (7.0, 1536.0, 3.2),
            (7.0, 1536.0, 3.9),
            "opening",
        ),
        (
            GridTool::Arcade,
            (16.5, 0.0, 6.5),
            (22.5, 0.0, 6.5),
            "arcade",
        ),
        (
            GridTool::Building,
            (22.5, 0.0, 1.5),
            (26.5, 0.0, 5.5),
            "building",
        ),
        (
            GridTool::Terrain,
            (6.5, 0.0, 14.5),
            (11.5, 0.0, 18.5),
            "terrain",
        ),
    ];
    for (tool, a, b, name) in shots {
        rig.ws.set_grid_tool(tool);
        settle(&mut rig);
        let (from, to) = (p(&rig, a.0, a.1, a.2), p(&rig, b.0, b.1, b.2));
        rig.move_to(from);
        let down = rig.press(from, true);
        rig.frame(vec![down]);
        for step in 1..=8 {
            rig.move_to(from + (to - from) * (step as f32 / 8.0));
        }
        settle(&mut rig);
        wire(&mut rig);
        rig.capture(&dir.join(format!("build-{name}.png")));
        rig.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let up = rig.press(to, false);
        rig.frame(vec![up]);
        settle(&mut rig);
    }
}

#[test]
fn play_from_here_restarts_a_running_playtest_at_the_new_spot() {
    let mut rig = GridRig::top("build-play-restart", derived_project_for_tiling(5, 5));
    rig.ws.playtest_running = true;
    rig.ws.play_from_here([1280.0, 0.0, 2560.0], 0.0);
    assert!(matches!(
        rig.ws.pending_playtest_request,
        Some(crate::EditorPlaytestRequest::Rebuild)
    ));
    assert!(rig.ws.status.contains("restarting"), "{}", rig.ws.status);
}
