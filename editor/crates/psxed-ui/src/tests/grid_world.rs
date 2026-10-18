//! Grid worlds inside the editor workspace: undo and the Top view drawing.

use super::*;
use psxed_project::grid::{GridRoom, GridSurface, GridWall, GridWorld, SceneWorld};

fn grid_project() -> ProjectDocument {
    let mut room = GridRoom::new("hall", 3, 2);
    for x in 0..3 {
        for z in 0..2 {
            room.sector_or_insert(x, z)
                .unwrap()
                .set_floor(Some(GridSurface::flat(256 * x as i32, None)));
        }
    }
    room.sector_or_insert(1, 1)
        .unwrap()
        .north
        .push(GridWall::span(0, 1024, None));
    let mut project = ProjectDocument::new("grid undo");
    project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
        rooms: vec![room],
        ..GridWorld::new()
    });
    project
}

fn raise(workspace: &mut EditorWorkspace, x: usize, z: usize, height: i32) {
    workspace.push_undo();
    let world = workspace
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .expect("grid scene");
    world.rooms[0]
        .sector_mut(x, z)
        .unwrap()
        .floor_mut()
        .unwrap()
        .heights = [height; 4];
    workspace.mark_dirty();
}

fn floor_height(workspace: &EditorWorkspace, x: usize, z: usize) -> i32 {
    workspace.project.active_scene().world.grid().unwrap().rooms[0]
        .sector(x, z)
        .unwrap()
        .floor()
        .unwrap()
        .heights[0]
}

#[test]
fn grid_edits_undo_and_redo_through_the_document_snapshots() {
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("grid-undo"), grid_project());
    let before = workspace.project.clone();

    raise(&mut workspace, 0, 0, 768);
    raise(&mut workspace, 2, 1, 1024);
    assert_eq!(floor_height(&workspace, 0, 0), 768);
    assert_eq!(floor_height(&workspace, 2, 1), 1024);

    workspace.do_undo();
    assert_eq!(floor_height(&workspace, 2, 1), 512);
    assert_eq!(floor_height(&workspace, 0, 0), 768);
    workspace.do_undo();
    assert_eq!(workspace.project, before);
    assert!(workspace.project.active_scene().world.grid().is_some());

    workspace.do_redo();
    workspace.do_redo();
    assert_eq!(floor_height(&workspace, 0, 0), 768);
    assert_eq!(floor_height(&workspace, 2, 1), 1024);
    workspace.do_undo();
    raise(&mut workspace, 1, 0, 256);
    workspace.do_redo();
    assert_eq!(
        floor_height(&workspace, 2, 1),
        512,
        "a new edit forks the history, so redo has nothing to restore"
    );
}

#[test]
fn growing_a_room_is_one_undo_step() {
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("grid-grow"), grid_project());
    workspace.push_undo();
    let room = &mut workspace
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms[0];
    room.grow_to_include(-2, 4);
    room.sector_or_insert(0, 0)
        .unwrap()
        .set_floor(Some(GridSurface::flat(0, None)));
    workspace.mark_dirty();
    let grown = &workspace.project.active_scene().world.grid().unwrap().rooms[0];
    assert_eq!((grown.width, grown.depth, grown.origin), (5, 5, [-2, 0]));

    workspace.do_undo();
    let restored = &workspace.project.active_scene().world.grid().unwrap().rooms[0];
    assert_eq!(
        (restored.width, restored.depth, restored.origin),
        (3, 2, [0, 0])
    );
}

#[test]
fn the_top_view_draws_every_sector_and_wall_of_a_grid_world() {
    let project = grid_project();
    let world = project.active_scene().world.grid().unwrap().clone();
    let ctx = egui::Context::default();
    let mut drawn = 0usize;
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.max_rect();
            let transform = crate::viewport2d::ViewportTransform::from_focus(
                rect,
                crate::viewport2d::OrthographicView::Top,
                [2000.0, 1000.0],
                0.05,
            );
            let painter = ui.painter_at(rect);
            let hidden = HashSet::new();
            crate::grid_view::draw_grid_world(
                &painter,
                transform,
                &world,
                true,
                &crate::grid_view::GridRoomView {
                    hidden: &hidden,
                    active: None,
                },
            );
            drawn = world.sector_count();
        });
    });
    // One fill per sector at least, plus outlines, the wall and the room frame.
    let shapes = output.shapes.len();
    assert_eq!(drawn, 6);
    assert!(shapes >= drawn, "{shapes} shapes for {drawn} sectors");
}

#[test]
fn opening_a_grid_project_frames_the_whole_world_in_the_top_view() {
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("grid-frame"), grid_project());
    workspace.last_viewport_size = Vec2::new(1280.0, 700.0);
    workspace.show_room_orthographic();
    // The 3x2 room at the default scale spans 3840 x 2560 editor units
    // around (1920, 1280) in X and Z.
    assert!(
        (workspace.orthographic_focus[0] - 1920.0).abs() < 1.0,
        "{:?}",
        workspace.orthographic_focus
    );
    assert!(
        (workspace.orthographic_focus[2] - 1280.0).abs() < 1.0,
        "{:?}",
        workspace.orthographic_focus
    );
    let zoom = workspace.viewport_zoom;
    assert!(
        zoom > 0.0 && 3840.0 * zoom <= 1280.0 && 2560.0 * zoom <= 700.0,
        "zoom {zoom}"
    );
}

#[test]
fn stacked_rooms_draw_bottom_up_with_the_active_room_last_and_hidden_ones_skipped() {
    use crate::grid_view::GridRoomView;
    let mut world = GridWorld::new();
    for (name, y) in [("high", 2048), ("low", 0), ("mid", 1024)] {
        let mut room = GridRoom::new(name, 1, 1);
        room.y_offset = y;
        world.rooms.push(room);
    }
    let none = HashSet::new();
    let view = GridRoomView {
        hidden: &none,
        active: None,
    };
    assert_eq!(view.draw_order(&world), vec![1, 2, 0]);
    // Rooms above the lowest are translucent so the ones beneath show.
    assert_eq!(view.fade(&world, 1), 1.0);
    assert!(view.fade(&world, 0) < 1.0);

    let hidden = HashSet::from([2usize]);
    let view = GridRoomView {
        hidden: &hidden,
        active: Some(1),
    };
    assert_eq!(view.draw_order(&world), vec![0, 1]);
    assert_eq!(view.fade(&world, 1), 1.0);
    assert!(view.fade(&world, 0) < 1.0);
}
