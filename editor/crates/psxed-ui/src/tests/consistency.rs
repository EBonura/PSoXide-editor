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

/// Press a gizmo arrow and drag part way, leaving the button held.
fn begin_held_gizmo_drag(rig: &mut super::interaction_rig::MouseRig) -> (Pos2, Pos2) {
    let (grab, to) = rig.gizmo_drag_vector(0);
    rig.move_to(grab);
    rig.press(grab);
    rig.move_to(grab + (to - grab) * 0.3);
    rig.move_to(grab + (to - grab) * 0.6);
    (grab, to)
}

#[test]
fn deleting_the_dragged_brush_mid_gesture_leaves_other_brushes_alone() {
    // Delete/Backspace fire while the pointer is still held on a gizmo arrow
    // or a free face/edge/vertex handle. The gesture holds brush index 0;
    // the delete shifts the bystander into that slot.
    let cases = [
        (BrushEditMode::Move, None),
        (BrushEditMode::Face, Some([256.0, 256.0, 128.0])),
        (BrushEditMode::Edge, Some([256.0, 256.0, 0.0])),
        (BrushEditMode::Vertex, Some([0.0, 256.0, 0.0])),
    ];
    for (mode, handle) in cases {
        let mut rig = super::interaction_rig::MouseRig::single_cube("delete-mid-drag");
        let bystander = psxed_project::brush::Brush::cuboid([4096, 0, 4096], [4608, 256, 4352]);
        rig.workspace
            .project
            .active_scene_mut()
            .brushes
            .push(bystander.clone());
        rig.workspace.set_brush_edit_mode(mode);
        let body = rig.world_to_screen([256.0, 256.0, 128.0]);
        rig.click(body);
        assert_eq!(rig.workspace.selected_brush, Some(0), "{mode:?}");

        let to = match handle {
            None => begin_held_gizmo_drag(&mut rig).1,
            Some(anchor) => {
                let grab = rig.world_to_screen(anchor);
                let to = grab + Vec2::new(48.0, -48.0);
                rig.move_to(grab);
                rig.press(grab);
                rig.move_to(grab + (to - grab) * 0.5);
                rig.move_to(to);
                to
            }
        };
        rig.key(to, egui::Key::Delete);
        rig.move_to(to + Vec2::new(8.0, 0.0));
        rig.release(to + Vec2::new(8.0, 0.0));

        let brushes = &rig.workspace.project.active_scene().brushes;
        assert_eq!(
            brushes.len(),
            1,
            "{mode:?}: Delete removed the dragged brush"
        );
        assert_eq!(
            brushes[0], bystander,
            "{mode:?}: the drag wrote the deleted brush over the one that took its index"
        );
    }
}

#[test]
fn undo_mid_gesture_cancels_the_drag_instead_of_resurrecting_it() {
    let mut rig = super::interaction_rig::MouseRig::single_cube("undo-mid-drag");
    rig.workspace.set_brush_edit_mode(BrushEditMode::Move);
    let body = rig.world_to_screen([256.0, 256.0, 128.0]);
    rig.click(body);
    // One recorded edit to undo: add a second brush.
    rig.workspace.push_undo();
    rig.workspace
        .project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [4096, 0, 4096],
            [4608, 256, 4352],
        ));
    let before_add = {
        let mut scene = rig.workspace.project.active_scene().brushes.clone();
        scene.pop();
        scene
    };
    let (_, to) = begin_held_gizmo_drag(&mut rig);
    rig.workspace.do_undo();
    rig.move_to(to);
    rig.release(to);
    assert_eq!(
        rig.workspace.project.active_scene().brushes,
        before_add,
        "Cmd+Z mid-drag must land on the undone document, not the drag's result"
    );
}

#[test]
fn reload_starts_a_fresh_undo_timeline_and_drops_stale_brush_selection() {
    let dir = test_temp_dir("reload-history");
    let _guard = ScratchProjectDir::new(dir.clone());
    let mut project = ProjectDocument::new("reload-history");
    project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, 0, 0],
            [512, 256, 512],
        ));
    let mut workspace = EditorWorkspace::with_project(dir, project);
    workspace.save().expect("save the on-disk baseline");
    let on_disk = workspace.project.clone();

    // Two in-memory edits the Reload is about to discard.
    for offset in [1024, 2048] {
        workspace.push_undo();
        workspace
            .project
            .active_scene_mut()
            .brushes
            .push(psxed_project::brush::Brush::cuboid(
                [offset, 0, 0],
                [offset + 512, 256, 512],
            ));
        workspace.mark_dirty();
    }
    workspace.replace_brush_selection(2, Some(0));

    workspace.reload();
    assert_eq!(
        workspace.project.active_scene().brushes,
        on_disk.active_scene().brushes
    );
    assert_eq!(
        workspace.selected_brush, None,
        "brush 3 no longer exists after Reload"
    );
    assert_eq!(workspace.selected_brush_face, None);

    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().brushes,
        on_disk.active_scene().brushes,
        "Cmd+Z after Reload resurrected edits Reload discarded"
    );
    assert!(
        !workspace.is_dirty(),
        "a no-op undo after Reload marked the project dirty"
    );
}

#[test]
fn room_editing_keys_do_nothing_in_the_animation_and_material_workspaces() {
    for view in [WorkspaceView::Animation, WorkspaceView::Material] {
        for (key, modifiers) in [
            (egui::Key::R, egui::Modifiers::NONE),
            (egui::Key::F2, egui::Modifiers::NONE),
            (egui::Key::D, egui::Modifiers::COMMAND),
            (egui::Key::Delete, egui::Modifiers::NONE),
            (egui::Key::Backspace, egui::Modifiers::NONE),
        ] {
            let mut workspace = single_brush_workspace("room-keys-elsewhere");
            let root = workspace.project.active_scene().root;
            let entity = workspace.project.active_scene_mut().add_node(
                root,
                "Hidden Entity",
                NodeKind::Entity,
            );
            workspace.replace_node_selection(entity);
            workspace.active_workspace = view;
            workspace.dirty = false;
            let before = workspace.project.clone();
            let mut frames = EditorFrames::new();
            frames.idle(&mut workspace);
            frames.key(&mut workspace, key, modifiers);
            frames.idle(&mut workspace);
            assert_eq!(
                workspace.project.active_scene(),
                before.active_scene(),
                "{view:?}: {key:?} edited the Room scene the user cannot see"
            );
            assert!(
                workspace.renaming.is_none(),
                "{view:?}: {key:?} began a rename in the hidden scene tree"
            );
            assert!(!workspace.is_dirty(), "{view:?}: {key:?}");
        }
    }
}

#[test]
fn arrow_keys_in_ui_navigation_preview_do_not_nudge_the_layout() {
    let mut project = ProjectDocument::new("ui-nav-preview");
    let scene_id = project.add_ui_scene("Nav Preview");
    let root = project.ui_scene(scene_id).unwrap().root;
    let panel = project.ui_scene_mut(scene_id).unwrap().add_node(
        root,
        "Panel",
        UiNodeKind::Group {
            rect: UiRect::new(20, 30, 100, 50),
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("ui-nav-preview"), project);
    workspace.active_workspace = WorkspaceView::Ui;
    assert!(workspace.focus_ui_scene("Nav Preview"));
    workspace.selection.selected_ui_node = panel;
    let rect = |workspace: &EditorWorkspace| {
        workspace
            .current_ui_scene()
            .and_then(|scene| scene.node(panel))
            .and_then(|node| node.kind.rect())
    };
    let before = rect(&workspace);
    let mut frames = EditorFrames::new();
    frames.idle(&mut workspace);

    // Editing (preview off): arrows nudge the selected node.
    frames.key(&mut workspace, egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert_ne!(
        rect(&workspace),
        before,
        "arrow nudge is the editing baseline"
    );
    workspace.do_undo();
    assert_eq!(rect(&workspace), before);

    // Previewing navigation: the same keys drive the preview focus only.
    workspace.ui_nav_preview = true;
    workspace.dirty = false;
    frames.idle(&mut workspace);
    for key in [egui::Key::ArrowRight, egui::Key::ArrowDown] {
        frames.key(&mut workspace, key, egui::Modifiers::NONE);
    }
    assert_eq!(
        rect(&workspace),
        before,
        "navigation preview arrows moved the selected UI node"
    );
    assert!(!workspace.is_dirty());
}

#[test]
fn editor_shortcuts_stand_down_while_a_modal_dialog_is_open() {
    let mut workspace = single_brush_workspace("modal-shortcuts");
    let root = workspace.project.active_scene().root;
    let entity =
        workspace
            .project
            .active_scene_mut()
            .add_node(root, "Behind The Dialog", NodeKind::Entity);
    // One recorded edit that Cmd+Z could roll back.
    workspace.push_undo();
    workspace.project.active_scene_mut().brushes.clear();
    workspace.replace_node_selection(entity);
    workspace.modal = Modal::DeleteProject { error: None };
    let before = workspace.project.clone();
    let mut frames = EditorFrames::new();
    frames.idle(&mut workspace);
    for (key, modifiers) in [
        (egui::Key::Delete, egui::Modifiers::NONE),
        (egui::Key::R, egui::Modifiers::NONE),
        (egui::Key::D, egui::Modifiers::COMMAND),
        (egui::Key::Z, egui::Modifiers::COMMAND),
    ] {
        frames.key(&mut workspace, key, modifiers);
        assert_eq!(
            workspace.project, before,
            "{key:?} edited the project behind the Delete Project dialog"
        );
    }
    assert!(matches!(workspace.modal, Modal::DeleteProject { .. }));
}
