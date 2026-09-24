//! Regression tests for the editor consistency audit (AUDIT.md): each one
//! reproduces a user-visible inconsistency through the real editor paths
//! and stays red on the pre-fix code.

use super::*;

/// A full editor frame driver: the whole `EditorWorkspace::draw` pass with
/// real egui input, the same entry point the native window uses.
struct EditorFrames {
    ctx: egui::Context,
    viewport: EditorViewport3dPresentation,
    time: f64,
    _texture: egui::TextureHandle,
}

impl EditorFrames {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        let proportional = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap();
        fonts
            .families
            .insert(egui::FontFamily::Name("lucide".into()), proportional);
        ctx.set_fonts(fonts);
        let texture = ctx.load_texture(
            "consistency-viewport",
            egui::ColorImage::new([1, 1], egui::Color32::BLACK),
            egui::TextureOptions::NEAREST,
        );
        let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
        Self {
            ctx,
            viewport,
            time: 0.0,
            _texture: texture,
        }
    }

    fn run(
        &mut self,
        workspace: &mut EditorWorkspace,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1800.0, 900.0))),
            time: Some(self.time),
            modifiers,
            events,
            ..egui::RawInput::default()
        };
        let viewport = self.viewport.clone();
        self.ctx.run(input, |ctx| {
            workspace.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle)
        })
    }

    fn idle(&mut self, workspace: &mut EditorWorkspace) -> egui::FullOutput {
        self.run(workspace, Vec::new(), egui::Modifiers::NONE)
    }

    fn key(&mut self, workspace: &mut EditorWorkspace, key: egui::Key, modifiers: egui::Modifiers) {
        self.run(
            workspace,
            vec![
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers,
                },
            ],
            modifiers,
        );
    }
}

fn single_brush_workspace(label: &str) -> EditorWorkspace {
    let mut project = ProjectDocument::new(label);
    project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, 0, 0],
            [1024, 512, 1024],
        ));
    let mut workspace = EditorWorkspace::with_project(test_temp_dir(label), project);
    workspace.active_workspace = WorkspaceView::Room;
    workspace.active_tool = ViewTool::Select;
    workspace
}

#[test]
fn showing_a_face_uv_scale_does_not_rewrite_it() {
    // A scale the UV canvas gesture or "Fit" can author (300/256 = 117.19%)
    // is not a whole percent. Merely displaying it in the Inspector must not
    // truncate it back through the percent field every frame.
    let mut workspace = single_brush_workspace("uv-scale-display");
    workspace.brush_edit_mode = BrushEditMode::Face;
    workspace.project.active_scene_mut().brushes[0].faces[0]
        .uv
        .scale_q8 = [300, -300];
    workspace.replace_brush_selection(0, Some(0));
    workspace.dirty = false;
    let mut frames = EditorFrames::new();
    for _ in 0..30 {
        frames.idle(&mut workspace);
    }
    assert_eq!(
        workspace.project.active_scene().brushes[0].faces[0]
            .uv
            .scale_q8,
        [300, -300],
        "the Inspector drifted an untouched face UV scale"
    );
    assert!(
        !workspace.is_dirty(),
        "displaying a face marked the project dirty"
    );

    // Fit on a large face authors scales past the field's +/-1600% range;
    // displaying one must not clamp it either.
    workspace.project.active_scene_mut().brushes[0].faces[1]
        .uv
        .scale_q8 = [5000, 256];
    workspace.replace_brush_selection(0, Some(1));
    for _ in 0..3 {
        frames.idle(&mut workspace);
    }
    assert_eq!(
        workspace.project.active_scene().brushes[0].faces[1]
            .uv
            .scale_q8,
        [5000, 256],
        "the Inspector clamped an out-of-range face UV scale it only displayed"
    );
    assert!(!workspace.is_dirty());
}
