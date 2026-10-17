//! The grid chrome (strip, palette, inspector, status bar) and the poses
//! the headless captures use, drawn through the whole editor.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;

pub(super) fn full_context() -> egui::Context {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    let proportional = fonts.families[&egui::FontFamily::Proportional].clone();
    fonts
        .families
        .insert(egui::FontFamily::Name("lucide".into()), proportional);
    ctx.set_fonts(fonts);
    ctx
}

pub(super) fn draw_editor(
    ws: &mut EditorWorkspace,
    ctx: &egui::Context,
    view_2d: bool,
) -> egui::FullOutput {
    draw_editor_sized(ws, ctx, view_2d, Vec2::new(1600.0, 900.0))
}

/// [`draw_editor`] on a screen of `size` (the help page needs a tall one: its
/// rows below the screen are not drawn).
pub(super) fn draw_editor_sized(
    ws: &mut EditorWorkspace,
    ctx: &egui::Context,
    view_2d: bool,
    size: Vec2,
) -> egui::FullOutput {
    let texture = ctx.load_texture(
        "grid-ui",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    ws.view_2d = view_2d;
    ws.active_workspace = WorkspaceView::Room;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        ..egui::RawInput::default()
    };
    ctx.run(input, |ctx| {
        ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
    })
}

#[test]
fn the_whole_editor_draws_for_every_tool_in_both_views() {
    let ctx = full_context();
    let mut project = flat_project(4, 4, true);
    project.add_resource(
        "brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let mut ws = EditorWorkspace::with_project(test_temp_dir("grid-ui-all"), project);
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    for tool in GridTool::ALL {
        ws.set_grid_tool(tool);
        for view_2d in [true, false] {
            let output = draw_editor(&mut ws, &ctx, view_2d);
            assert!(!output.shapes.is_empty());
        }
    }
}

#[test]
fn a_selection_shows_in_the_inspector_and_a_hint_shows_in_the_status_bar() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-ui-sel"), flat_project(3, 3, false));
    ws.apply_grid_pose("tool=select;select=floors:0,0:1,1")
        .unwrap();
    assert_eq!(ws.grid.selection.len(), 4);
    let output = draw_editor(&mut ws, &ctx, true);
    let text: String = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("4 faces"),
        "inspector summary missing:\n{text}"
    );
    assert!(text.contains("Shift range"), "key hints missing:\n{text}");
    assert!(text.contains("sectors"), "budget readout missing:\n{text}");
    assert!(text.contains("Materials"), "palette missing");
}

#[test]
fn poses_set_the_state_a_mouse_would() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-pose"), flat_project(6, 6, true));
    ws.apply_grid_pose("tool=floor;rect=floor:1,1:3,2:2")
        .unwrap();
    match &ws.grid.gesture {
        crate::grid_tools::Gesture::Rect(g) => {
            assert_eq!((g.anchor, g.current, g.height), ([1, 1], [3, 2], 512));
        }
        other => panic!("{other:?}"),
    }
    ws.apply_grid_pose("tool=wall;walls=north:0,0:3,0").unwrap();
    assert!(matches!(
        ws.grid.gesture,
        crate::grid_tools::Gesture::Walls(_)
    ));
    ws.apply_grid_pose("select=cells:0,0:1,1;copy;paste=3,3")
        .unwrap();
    assert!(ws.grid.paste.is_some());
    assert!(ws.apply_grid_pose("nonsense=1").is_err());
    assert!(ws.apply_grid_pose("tool=lasso").is_err());
}

#[test]
fn rooms_can_be_added_and_deleted_with_undo() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-rooms"), flat_project(3, 3, false));
    ws.grid_add_room();
    let world = ws.project.active_scene().world.grid().unwrap();
    assert_eq!(world.rooms.len(), 2);
    assert_eq!(world.rooms[1].y_offset, 3072);
    assert_eq!(ws.grid_active_room, Some(1));
    ws.grid_hidden_rooms.insert(1);
    ws.grid_delete_room(0);
    assert_eq!(
        ws.project.active_scene().world.grid().unwrap().rooms.len(),
        1
    );
    assert_eq!(ws.grid_active_room, Some(0));
    assert!(ws.grid_hidden_rooms.contains(&0));
    ws.do_undo();
    assert_eq!(
        ws.project.active_scene().world.grid().unwrap().rooms.len(),
        2
    );
    // The last room cannot be deleted.
    ws.grid_delete_room(1);
    ws.grid_delete_room(0);
    assert_eq!(
        ws.project.active_scene().world.grid().unwrap().rooms.len(),
        1
    );
}

#[test]
fn dot_frames_the_selection_in_both_views() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-frame-sel"), flat_project(8, 8, false));
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.apply_grid_pose("select=cells:6,6:7,7").unwrap();
    ws.view_2d = true;
    ws.frame_viewport();
    // The selection centre is (7, 7) sectors at scale 1:1.
    assert!(
        (ws.orthographic_focus[0] - 7.0 * 1024.0).abs() < 1.0,
        "{:?}",
        ws.orthographic_focus
    );
    ws.view_2d = false;
    ws.frame_viewport();
    assert!((ws.camera_rig.target[0] as f32 - 7.0 * 1024.0).abs() < 2.0);
}

#[test]
fn a_rectangle_that_would_overfill_the_room_says_so_instead_of_freezing() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-oversize"), flat_project(3, 3, false));
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    // Two hundred sectors across, dragged in one go.
    ws.apply_grid_pose("tool=floor;rect=floor:0,0:200,3")
        .unwrap();
    for view_2d in [true, false] {
        let output = draw_editor(&mut ws, &ctx, view_2d);
        let text: String = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if view_2d {
            assert!(text.contains("the cap is 128"), "{text}");
        }
    }
}

fn shapes_text(output: &egui::FullOutput) -> String {
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

pub(super) fn draw_with_events(
    ws: &mut EditorWorkspace,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let texture = ctx.load_texture(
        "grid-ui-events",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    ws.view_2d = true;
    ws.active_workspace = WorkspaceView::Room;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
        events,
        ..egui::RawInput::default()
    };
    ctx.run(input, |ctx| {
        ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
    })
}

#[test]
fn the_rooms_panel_lists_rooms_and_hiding_one_reaches_the_preview() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-rooms-panel"), flat_project(3, 3, false));
    ws.grid_add_room();
    let output = draw_editor(&mut ws, &ctx, true);
    let text = shapes_text(&output);
    assert!(text.contains("Rooms"), "{text}");
    assert!(text.contains("hall"), "{text}");
    assert!(text.contains("Room 1"), "{text}");
    assert!(crate::grid_preview_hidden_rooms().is_empty());
    ws.grid_hidden_rooms.insert(1);
    draw_editor(&mut ws, &ctx, false);
    assert_eq!(crate::grid_preview_hidden_rooms(), vec![1]);
    // A brush world publishes nothing hidden.
    ws.project.active_scene_mut().world = psxed_project::grid::SceneWorld::Empty;
    draw_editor(&mut ws, &ctx, false);
    assert!(crate::grid_preview_hidden_rooms().is_empty());
}

#[test]
fn the_question_mark_opens_the_key_help_and_escape_closes_it() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-help"), flat_project(3, 3, false));
    let key = |key, pressed, shift| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers {
            shift,
            ..egui::Modifiers::NONE
        },
    };
    let closed = shapes_text(&draw_editor(&mut ws, &ctx, true));
    assert!(!closed.contains("Grid editor keys"));
    let open = shapes_text(&draw_with_events(
        &mut ws,
        &ctx,
        vec![
            key(egui::Key::Slash, true, true),
            key(egui::Key::Slash, false, true),
        ],
    ));
    assert!(ws.grid.help_open);
    // egui sizes a new window invisibly first; the next frame shows it.
    drop(open);
    let open = shapes_text(&draw_editor_sized(
        &mut ws,
        &ctx,
        true,
        Vec2::new(1920.0, 2600.0),
    ));
    assert!(open.contains("Grid editor keys"), "{open}");
    assert!(open.contains("Shift+drag"), "{open}");
    for title in ["Raise (U)", "Ramp (A)", "Stairs (T)", "Overlays"] {
        assert!(open.contains(title), "{title}: {open}");
    }
    assert!(open.contains("Ramp (A)"), "{open}");
    draw_with_events(
        &mut ws,
        &ctx,
        vec![
            key(egui::Key::Escape, true, false),
            key(egui::Key::Escape, false, false),
        ],
    );
    assert!(!ws.grid.help_open);
}

#[test]
fn no_brush_menus_are_drawn_for_grid_or_empty_worlds() {
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-menus"), flat_project(2, 2, false));
    let grid_text = shapes_text(&draw_editor(&mut ws, &ctx, true));
    assert!(grid_text.contains("Edit"), "{grid_text}");
    assert!(!grid_text.contains("Brush"), "{grid_text}");
    ws.project.active_scene_mut().world = psxed_project::grid::SceneWorld::Empty;
    let empty_text = shapes_text(&draw_editor(&mut ws, &ctx, true));
    assert!(!empty_text.contains("Brush"), "{empty_text}");
}

#[test]
fn new_grid_project_makes_a_playable_starter_and_opens_it() {
    let projects = psxed_project::projects_dir();
    let name = format!("Test Grid Starter {}", std::process::id());
    let dir = projects.join(psxed_project::project_file_stem(&name));
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("grid-new"), flat_project(2, 2, false));
    let result = ws.create_and_open_project(&name);
    let world_rooms = ws
        .project
        .active_scene()
        .world
        .grid()
        .map(|w| (w.rooms.len(), w.sector_count()));
    let had_player = ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .any(|n| matches!(n.kind, NodeKind::Entity));
    let again = ws.create_and_open_project(&name);
    let _ = std::fs::remove_dir_all(&dir);
    result.expect("the starter is created");
    assert_eq!(world_rooms, Some((1, 36)));
    assert!(had_player, "the Graybox Reach player is in the scene");
    assert!(again.unwrap_err().contains("already exists"));
}
