//! The Material tab of a grid project edits the palette's paint material and
//! its Inspector is about that material, not the World's sky.

use super::grid_rig::*;
use super::grid_ui::full_context;
use super::*;

fn two_materials() -> (ProjectDocument, ResourceId, ResourceId) {
    let mut project = flat_project(4, 4, false);
    let material =
        || psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None));
    let a = project.add_resource("Graybox / Brick", material());
    let b = project.add_resource("Graybox / Moss", material());
    (project, a, b)
}

fn draw_material_tab(ws: &mut EditorWorkspace) -> String {
    let ctx = full_context();
    let texture = ctx.load_texture(
        "material-tab",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    ws.active_workspace = WorkspaceView::Material;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0))),
            ..egui::RawInput::default()
        },
        |ctx| ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle),
    );
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

#[test]
fn the_material_tab_opens_on_the_paint_material() {
    let (project, _a, b) = two_materials();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("mat-tab-open"), project);
    ws.grid.material = Some(b);
    let _ = draw_material_tab(&mut ws);
    assert_eq!(ws.material_lab.focused_material, Some(b));
}

#[test]
fn choosing_a_material_in_the_tab_makes_it_the_paint_material() {
    let (project, a, b) = two_materials();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("mat-tab-pick"), project);
    ws.grid.material = Some(b);
    let _ = draw_material_tab(&mut ws);
    assert!(ws.focus_material_resource(a));
    assert_eq!(ws.grid.material, Some(a));
    let _ = draw_material_tab(&mut ws);
    assert_eq!(
        ws.material_lab.focused_material,
        Some(a),
        "the tab keeps it"
    );
}

#[test]
fn the_material_tab_inspector_is_about_the_material_and_not_the_world() {
    let (project, _a, b) = two_materials();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("mat-tab-inspector"), project);
    ws.grid.material = Some(b);
    let text = draw_material_tab(&mut ws);
    for world in ["Physics", "Far Vista", "World Message", "Sky Texture"] {
        assert!(!text.contains(world), "the World form is drawn: {world}");
    }
    assert!(text.contains("Used on"), "{text}");
    assert!(text.contains("Paint with it"), "{text}");
}
