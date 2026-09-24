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

impl EditorFrames {
    fn click(&mut self, workspace: &mut EditorWorkspace, pos: Pos2) {
        self.run(
            workspace,
            vec![egui::Event::PointerMoved(pos)],
            egui::Modifiers::NONE,
        );
        for pressed in [true, false] {
            self.run(
                workspace,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
                egui::Modifiers::NONE,
            );
        }
    }

    fn type_text(&mut self, workspace: &mut EditorWorkspace, text: &str) {
        for ch in text.chars() {
            self.run(
                workspace,
                vec![egui::Event::Text(ch.to_string())],
                egui::Modifiers::NONE,
            );
        }
    }
}

/// Screen point inside the Material Lab text field on the row labelled
/// `label`, far enough right to land past the current text.
fn material_lab_field(output: &egui::FullOutput, label: &str) -> Pos2 {
    let rows = super::brush_tools::text_shape_centers(&output.shapes, label);
    let version = super::brush_tools::text_shape_centers(&output.shapes, "Version name");
    let anchor = *version
        .first()
        .expect("Material Lab shows its Version name row");
    let row = rows
        .into_iter()
        .filter(|point| point.y <= anchor.y + 1.0 && anchor.y - point.y < 40.0)
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .expect("Material Lab row label");
    row + Vec2::new(200.0, 0.0)
}

#[test]
fn material_lab_names_accept_spaces_while_typing() {
    let mut project = ProjectDocument::new("material-lab-names");
    let material = project.add_resource(
        "Zircon",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("material-lab-names"), project);
    workspace.active_workspace = WorkspaceView::Material;
    assert!(workspace.focus_material_resource(material));
    let mut frames = EditorFrames::new();
    let output = frames.idle(&mut workspace);
    let name_field = material_lab_field(&output, "Name");
    frames.click(&mut workspace, name_field);
    frames.type_text(&mut workspace, " Slab");
    frames.key(&mut workspace, egui::Key::Enter, egui::Modifiers::NONE);
    frames.idle(&mut workspace);
    assert_eq!(
        workspace.project.resource(material).unwrap().name,
        "Zircon Slab",
        "the Material Lab name field swallowed the space"
    );

    let version_before = match &workspace.project.resource(material).unwrap().data {
        ResourceData::Material(material) => material.active_version_name.clone(),
        _ => unreachable!(),
    };
    // Step past egui's double-click window so the next click only places
    // the cursor instead of selecting a word.
    frames.time += 1.0;
    let output = frames.idle(&mut workspace);
    let version_field = material_lab_field(&output, "Version name");
    frames.click(&mut workspace, version_field);
    frames.type_text(&mut workspace, " two");
    frames.key(&mut workspace, egui::Key::Enter, egui::Modifiers::NONE);
    frames.idle(&mut workspace);
    let version_after = match &workspace.project.resource(material).unwrap().data {
        ResourceData::Material(material) => material.active_version_name.clone(),
        _ => unreachable!(),
    };
    assert_eq!(version_after, format!("{version_before} two"));
}

#[test]
fn slow_top_view_node_drags_move_and_undo_as_one_step() {
    let mut workspace = single_brush_workspace("top-view-node-drag");
    let root = workspace.project.active_scene().root;
    // Placing the node is the previous undoable edit.
    workspace.push_undo();
    let entity = workspace
        .project
        .active_scene_mut()
        .add_node(root, "Crate", NodeKind::Entity);
    workspace.replace_node_selection(entity);
    workspace.snap_units = 16;
    workspace.viewport_zoom = 1.0;
    let start = workspace
        .project
        .active_scene()
        .node(entity)
        .unwrap()
        .transform
        .translation;
    // 48 frames of 2 px each: 96 world units in total, but never more than
    // an eighth of a grid step in any one frame (a slow, careful drag).
    for _ in 0..48 {
        workspace.drag_selected_node(Vec2::new(2.0, 0.0));
    }
    workspace.end_node_drag_2d();
    let moved = workspace
        .project
        .active_scene()
        .node(entity)
        .unwrap()
        .transform
        .translation;
    assert_eq!(
        moved[0],
        start[0] + 96.0,
        "a slow Top-view drag must still move the node"
    );

    workspace.do_undo();
    let undone = workspace
        .project
        .active_scene()
        .node(entity)
        .unwrap()
        .transform
        .translation;
    assert_eq!(undone, start, "one Cmd+Z undoes the whole Top-view drag");
    assert!(
        workspace.project.active_scene().node(entity).is_some(),
        "the undo must not also roll back the node's creation"
    );
}

fn committed_primitive(
    shape: BrushDrawShape,
    grid_step: i32,
    max: [i32; 3],
    tweak: impl FnOnce(&mut BrushDrawSettings),
) -> EditorWorkspace {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("primitive-shortfall"),
        ProjectDocument::new("primitive-shortfall"),
    );
    workspace.project.active_scene_mut().brushes.clear();
    let mut settings = BrushDrawSettings {
        shape,
        ..BrushDrawSettings::default()
    };
    tweak(&mut settings);
    workspace.brush_drag = Some(BrushDrag {
        anchor: [0, 0, 0],
        current: [max[0], 0, max[2]],
        view: OrthographicView::Top,
        grid_step,
        height_end: max[1],
        stage: BrushCreateStage::Height,
        height_press_y: 0,
        height_press_end: max[1],
        height_dragging: true,
        settings,
    });
    workspace.commit_brush_drag();
    workspace
}

#[test]
fn primitives_that_lose_pieces_to_the_grid_say_so() {
    // 24 voussoirs of a 512-wide arch on the 64 grid: the snapped corners
    // of several coincide and `convex_prism` rejects the degenerate slivers.
    let arch = committed_primitive(BrushDrawShape::DoorwayArch, 64, [512, 512, 64], |s| {
        s.arch_segments = 24;
        s.arch_thickness = 64;
    });
    let built = arch.project.active_scene().brushes.len();
    assert!(built > 0, "the arch still commits what it could build");
    assert!(
        built < 24 + 2,
        "fixture must actually lose pieces (built {built})"
    );
    assert!(
        arch.status.contains(&format!("{built} of 26")),
        "status must report the missing arch pieces, got {:?}",
        arch.status
    );

    // A 16-sided cylinder 512 across on the 64 grid snaps to far fewer sides.
    let cylinder = committed_primitive(BrushDrawShape::Cylinder, 64, [512, 256, 512], |s| {
        s.cylinder_sides = 16;
    });
    let brushes = &cylinder.project.active_scene().brushes;
    assert_eq!(brushes.len(), 1);
    let sides = brushes[0].faces.len() - 2;
    assert!(sides < 16, "fixture must actually lose sides (got {sides})");
    assert!(
        cylinder.status.contains(&format!("{sides} of 16 sides")),
        "status must report the collapsed cylinder sides, got {:?}",
        cylinder.status
    );

    // 256 across, nothing survives at all: the refusal names the grid.
    let nothing = committed_primitive(BrushDrawShape::Cylinder, 64, [256, 256, 256], |s| {
        s.cylinder_sides = 16;
    });
    assert!(nothing.project.active_scene().brushes.is_empty());
    assert!(
        nothing.status.contains("Grid 64"),
        "an empty result must name the grid that collapsed it, got {:?}",
        nothing.status
    );

    // A clean primitive keeps the plain message.
    let clean = committed_primitive(BrushDrawShape::DoorwayArch, 16, [2048, 2048, 64], |s| {
        s.arch_segments = 6;
        s.arch_thickness = 128;
    });
    assert_eq!(clean.project.active_scene().brushes.len(), 8);
    assert_eq!(clean.status, "Created Doorway Arch");
}

#[test]
fn light_radius_inspector_speaks_the_units_the_cook_uses() {
    // PointLight radius is stored in sectors and the cook, the brush bake
    // and the viewport preview all multiply it by the World sector size.
    let mut workspace = single_brush_workspace("light-radius-units");
    let root = workspace.project.active_scene().root;
    let sector = workspace.project.world_sector_size_for_node(root);
    assert_eq!(sector, 1024, "fixture assumes the default World sector");
    let light = workspace.project.active_scene_mut().add_node(
        root,
        "Half Sector Light",
        NodeKind::PointLight {
            color: [255, 255, 255],
            intensity: 1.0,
            radius: 0.5,
        },
    );
    workspace.replace_node_selection(light);
    workspace.dirty = false;
    let mut frames = EditorFrames::new();
    let mut output = frames.idle(&mut workspace);
    for _ in 0..3 {
        output = frames.idle(&mut workspace);
    }
    let radius = match &workspace.project.active_scene().node(light).unwrap().kind {
        NodeKind::PointLight { radius, .. } => *radius,
        _ => unreachable!(),
    };
    assert_eq!(radius, 0.5, "selecting the light rewrote its radius");
    assert!(
        !workspace.is_dirty(),
        "selecting the light marked the project dirty"
    );
    assert!(
        !super::brush_tools::text_shape_centers(&output.shapes, "Radius 512 units").is_empty(),
        "the Inspector must show the 512 world units the cook bakes"
    );
}

#[test]
fn texture_import_is_its_own_undo_step() {
    let dir = test_temp_dir("texture-import-undo");
    let _guard = ScratchProjectDir::new(dir.clone());
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("checker.png");
    image::RgbaImage::from_fn(16, 16, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgba([255, 255, 255, 255])
        } else {
            image::Rgba([0, 0, 0, 255])
        }
    })
    .save(&source)
    .unwrap();

    let mut workspace = EditorWorkspace::with_project(dir, ProjectDocument::new("import-undo"));
    // The edit before the import.
    workspace.push_undo();
    workspace
        .project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, 0, 0],
            [256, 256, 256],
        ));
    let brushes_after_edit = workspace.project.active_scene().brushes.clone();
    let resources_before = workspace.project.resources.len();

    workspace.texture_import_dialog.source_path = source.display().to_string();
    workspace.texture_import_dialog.output_name = "checker".to_string();
    workspace.commit_texture_import();
    assert_eq!(
        workspace.project.resources.len(),
        resources_before + 1,
        "import added the material: {}",
        workspace.status
    );

    workspace.do_undo();
    assert_eq!(
        workspace.project.resources.len(),
        resources_before,
        "Cmd+Z removes the imported material"
    );
    assert_eq!(
        workspace.project.active_scene().brushes,
        brushes_after_edit,
        "Cmd+Z after an import must not also roll back the edit before it"
    );
}

#[test]
fn snap_to_grid_snaps_every_selected_brush_like_its_neighbour_buttons() {
    // Duplicate and Delete beside it act on the whole brush selection.
    let mut workspace = single_brush_workspace("snap-selection");
    workspace.project.active_scene_mut().brushes = vec![
        psxed_project::brush::Brush::cuboid([3, 0, 5], [509, 256, 250]),
        psxed_project::brush::Brush::cuboid([1027, 0, 7], [1533, 256, 249]),
    ];
    workspace.snap_units = 16;
    workspace.replace_brush_selection(0, None);
    workspace.selected_brushes = vec![0, 1];
    let before = workspace.project.active_scene().brushes.clone();
    workspace.snap_selected_brush();
    let after = workspace.project.active_scene().brushes.clone();
    for (index, brush) in after.iter().enumerate() {
        assert_ne!(*brush, before[index], "brush {index} was left off the grid");
        for face in &brush.faces {
            for point in face.points {
                assert!(
                    point.iter().all(|value| value % 16 == 0),
                    "brush {index} point {point:?} is off Grid 16"
                );
            }
        }
    }
    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().brushes,
        before,
        "one undo step restores the whole selection"
    );
}
