//! What a new user tries first in the full editor window: keys that act on
//! the picked light (Delete, Cmd+Z), and the empty palette.

use super::grid_rig::*;
use super::grid_ui::full_context;
use super::*;

/// Press and release `key` in the full editor window (the global shortcut
/// path, not the rig's grid-only one).
fn press(ws: &mut EditorWorkspace, ctx: &egui::Context, key: egui::Key, mods: egui::Modifiers) {
    let texture = ctx.load_texture(
        "newuser-viewport",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    for pressed in [true, false] {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0))),
            modifiers: mods,
            events: vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: mods,
            }],
            ..egui::RawInput::default()
        };
        let _ = ctx.run(input, |c| {
            ws.draw(c, viewport.clone(), EditorPlaytestStatus::Idle)
        });
    }
}

fn with_a_light() -> (EditorWorkspace, egui::Context) {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("newuser-keys"), flat_project(4, 4, true));
    ws.place_kind = PlaceKind::PointLightMarker;
    ws.place_at_world_hit([2.5 * S, 0.0, 2.5 * S]);
    assert_ne!(
        ws.selection.selected_node,
        NodeId::ROOT,
        "the light is picked"
    );
    (ws, full_context())
}

#[test]
fn delete_removes_the_light_that_was_just_placed() {
    let (mut ws, ctx) = with_a_light();
    let nodes = ws.project.active_scene().nodes().len();
    press(&mut ws, &ctx, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(
        ws.project.active_scene().nodes().len(),
        nodes - 1,
        "Delete left the picked light in place (status {:?})",
        ws.status
    );
    // And Cmd+Z brings it back.
    press(&mut ws, &ctx, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(ws.project.active_scene().nodes().len(), nodes);
}

#[test]
fn delete_still_removes_the_picked_faces_and_not_a_light() {
    let (mut ws, ctx) = with_a_light();
    ws.clear_node_selection_state();
    ws.apply_grid_pose("tool=select;select=floors:0,0:0,0")
        .unwrap();
    let nodes = ws.project.active_scene().nodes().len();
    press(&mut ws, &ctx, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(ws.project.active_scene().nodes().len(), nodes);
    let world = ws.project.active_scene().world.grid().unwrap();
    assert!(world.rooms[0]
        .sector(0, 0)
        .unwrap()
        .ground_floor()
        .is_none());
}

#[test]
fn an_empty_palette_says_how_to_get_a_material() {
    let mut project = flat_project(4, 4, false);
    project.resources.clear();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("newuser-palette"), project);
    let text = super::grid_layout::screen(&mut ws);
    assert!(text.contains("No materials"), "{text}");
    assert!(
        text.contains("import") || text.contains("Import"),
        "the empty palette gives no way forward:\n{text}"
    );
}

#[test]
fn a_floor_drawn_far_away_says_which_room_it_joined() {
    use crate::grid_tools::GridTool;
    let mut rig = GridRig::top("newuser-floor-room", flat_project(3, 3, false));
    rig.tool(GridTool::Floor);
    rig.drag(rig.cell_centre(7, 0), rig.cell_centre(8, 1));
    assert!(
        rig.ws.status.contains("Created 4 floors in hall"),
        "{}",
        rig.ws.status
    );
}
