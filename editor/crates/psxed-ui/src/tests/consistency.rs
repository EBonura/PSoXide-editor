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

#[test]
fn reload_starts_a_fresh_undo_timeline() {
    let dir = test_temp_dir("reload-history");
    let _guard = ScratchProjectDir::new(dir.clone());
    let project = ProjectDocument::new("reload-history");
    let mut workspace = EditorWorkspace::with_project(dir, project);
    workspace.save().expect("save the on-disk baseline");
    let on_disk_nodes = scene_node_count(&workspace);

    // Two in-memory edits the Reload is about to discard.
    for _ in 0..2 {
        workspace.push_undo();
        add_marker_node(&mut workspace);
        workspace.mark_dirty();
    }

    workspace.reload();
    assert_eq!(scene_node_count(&workspace), on_disk_nodes);

    workspace.do_undo();
    assert_eq!(
        scene_node_count(&workspace),
        on_disk_nodes,
        "Cmd+Z after Reload resurrected edits Reload discarded"
    );
    assert!(
        !workspace.is_dirty(),
        "a no-op undo after Reload marked the project dirty"
    );
}

fn plain_workspace(label: &str) -> EditorWorkspace {
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir(label), ProjectDocument::new(label));
    workspace.active_workspace = WorkspaceView::Room;
    workspace.active_tool = ViewTool::Select;
    workspace
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
            let mut workspace = plain_workspace("room-keys-elsewhere");
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
    let mut workspace = plain_workspace("modal-shortcuts");
    let root = workspace.project.active_scene().root;
    let entity =
        workspace
            .project
            .active_scene_mut()
            .add_node(root, "Behind The Dialog", NodeKind::Entity);
    // One recorded edit that Cmd+Z could roll back.
    workspace.push_undo();
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
    let rows = super::egui_driver::text_shape_centers(&output.shapes, label);
    let version = super::egui_driver::text_shape_centers(&output.shapes, "Version name");
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
    let mut workspace = plain_workspace("top-view-node-drag");
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

#[test]
fn light_radius_inspector_speaks_the_units_the_cook_uses() {
    // PointLight radius is stored in sectors and the cook, the brush bake
    // and the viewport preview all multiply it by the World sector size.
    let mut workspace = plain_workspace("light-radius-units");
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
        !super::egui_driver::text_shape_centers(&output.shapes, "Radius 512 units").is_empty(),
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
    add_marker_node(&mut workspace);
    let nodes_after_edit = scene_node_count(&workspace);
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
        scene_node_count(&workspace),
        nodes_after_edit,
        "Cmd+Z after an import must not also roll back the edit before it"
    );
}

#[test]
fn new_projects_keep_the_player_weapon_appearance_tracks_pointing_at_weapons() {
    // Aletha's animation set materialises her blades through weapon
    // appearance tracks. A New Project hydrates that set from the default
    // project; its track weapon ids must land on the copied weapons, not on
    // whatever resource happens to hold the template's raw id.
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let name = format!(
        "Weapon Tracks {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let target = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    let _scratch = ScratchProjectDir::new(target.clone());
    workspace.create_and_open_project(&name).unwrap();

    let project = workspace.project();
    let mut tracks = 0;
    for resource in &project.resources {
        let ResourceData::AnimationSet(set) = &resource.data else {
            continue;
        };
        for track in &set.weapon_appearance_tracks {
            tracks += 1;
            let weapon = project.resource(track.weapon).unwrap_or_else(|| {
                panic!(
                    "{}: track weapon {:?} is missing",
                    resource.name, track.weapon
                )
            });
            let ResourceData::Weapon(weapon_data) = &weapon.data else {
                panic!(
                    "{}: track weapon {:?} is '{}', not a Weapon",
                    resource.name, track.weapon, weapon.name
                );
            };
            let model = weapon_data
                .model
                .and_then(|id| project.resource(id))
                .unwrap_or_else(|| panic!("weapon '{}' has no model", weapon.name));
            let ResourceData::Model(model_data) = &model.data else {
                panic!("weapon '{}' model is not a Model", weapon.name);
            };
            for path in std::iter::once(&model_data.model_path).chain(&model_data.texture_path) {
                assert!(
                    target.join(path).is_file(),
                    "weapon '{}' asset {path} was not copied",
                    weapon.name
                );
            }
        }
    }
    assert!(
        tracks > 0,
        "the starter player has weapon appearance tracks"
    );
}

#[test]
fn cmd_z_in_a_focused_text_field_undoes_the_text_not_the_project() {
    let mut project = ProjectDocument::new("text-field-undo");
    let material = project.add_resource(
        "Zircon",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("text-field-undo"), project);
    // One recorded project edit the text-field Cmd+Z must leave alone.
    workspace.push_undo();
    add_marker_node(&mut workspace);
    let nodes_after_edit = scene_node_count(&workspace);
    workspace.active_workspace = WorkspaceView::Material;
    assert!(workspace.focus_material_resource(material));
    let mut frames = EditorFrames::new();
    let output = frames.idle(&mut workspace);
    let name_field = material_lab_field(&output, "Name");
    frames.click(&mut workspace, name_field);
    frames.type_text(&mut workspace, " Slab");
    frames.key(&mut workspace, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(
        scene_node_count(&workspace),
        nodes_after_edit,
        "Cmd+Z in a focused text field undid the project edit"
    );
    frames.key(&mut workspace, egui::Key::Enter, egui::Modifiers::NONE);
    frames.idle(&mut workspace);
    assert_eq!(
        workspace.project.resource(material).unwrap().name,
        "Zircon",
        "Cmd+Z in the field should undo the typed text"
    );

    // With no text field focused, Cmd+Z is project undo again.
    frames.key(&mut workspace, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(
        scene_node_count(&workspace),
        nodes_after_edit - 1,
        "with no text field focused, Cmd+Z is project undo again"
    );
}

#[test]
fn box_prop_one_to_one_texels_repeat_one_tile_per_world_sector() {
    // "1:1 Texels" means one texture tile per World sector, the unit the
    // ArchProp cook tiles by. It used to divide by the node-translation
    // unit (1 in BSP scenes) and always clamp to the maximum 255 span.
    let mut project = ProjectDocument::new("box-prop-one-to-one");
    let material = project.add_resource(
        "Crate",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let root = project.active_scene().root;
    let world_sector = project.world_sector_size_for_node(root);
    let prop = project.active_scene_mut().add_node(
        root,
        "Crate",
        NodeKind::BoxProp {
            materials: [Some(material); psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [psxed_project::UvTransform::default(); psxed_project::BOX_PROP_FACE_COUNT],
            vertices: psxed_project::box_prop_vertices_for_size(world_sector as u16),
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir("box-prop-one-to-one"), project);
    workspace.active_workspace = WorkspaceView::Room;
    workspace.replace_node_selection(prop);
    let handle = egui::Context::default().load_texture(
        "box-prop-one-to-one",
        egui::ColorImage::new([1, 1], egui::Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    workspace.texture_thumbs.insert(
        material,
        ThumbnailEntry {
            signature: workspace.test_material_thumbnail_signature(material),
            handle,
            image: egui::ColorImage::new([1, 1], egui::Color32::WHITE),
            stats: PsxtStats {
                width: 64,
                height: 64,
                depth_bits: 4,
                clut_entries: 16,
                index_zero_transparent: false,
                pixel_bytes: 1,
                clut_bytes: 32,
                file_bytes: 45,
            },
        },
    );
    super::egui_driver::run_real_egui_workspace_click_on_label(&mut workspace, "1:1 Texels");
    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::BoxProp { uvs, .. } = &node.kind else {
        unreachable!()
    };
    // A one-sector face repeats a 64 px texture once: inclusive span 63.
    assert_eq!(uvs[0].span, [63, 63]);
}

#[test]
fn arch_props_snap_to_world_sector_tiles_in_world_units() {
    // The cook expands an ArchProp in tiles of the World sector size around
    // its world-unit anchor; the editor snapped the anchor as if it were in
    // tiles of 1 unit, so it landed on half-units.
    let geometry = psxed_project::ArchPropGeometry {
        span_tiles: 3,
        depth_tiles: 2,
        ..psxed_project::ArchPropGeometry::default()
    };
    let mut transform = psxed_project::Transform3 {
        translation: [1000.0, 100.0, 1000.0],
        ..psxed_project::Transform3::default()
    };
    crate::inspector_transform_node::snap_arch_prop_transform(&mut transform, geometry, 1024);
    // Odd span centres on a tile, even depth on a tile line.
    assert_eq!(transform.translation, [512.0, 128.0, 1024.0]);
}

#[test]
fn every_move_path_lands_nodes_on_the_same_grid_step() {
    use crate::inspector_transform_node::{node_gizmo_plane_translation, node_snap_step};
    // One rule: positional nodes land on the grid, whatever moves them.
    let mut scratch = ProjectDocument::new("cylinder-plane-snap");
    let root = scratch.active_scene().root;
    let id = scratch.active_scene_mut().add_node(
        root,
        "Pillar",
        NodeKind::CylinderProp {
            materials: Default::default(),
            uvs: Default::default(),
            geometry: psxed_project::CylinderPropGeometry::default(),
            collision_enabled: true,
        },
    );
    let cylinder = scratch.active_scene().node(id).unwrap().clone();
    // The plane gizmo used to leave Cylinder Props off the grid.
    assert_eq!(
        node_gizmo_plane_translation(
            &cylinder,
            [3.0, 0.0, 5.0],
            crate::gizmo::NodeGizmoPlane::XZ,
            [20.0, 0.0, 20.0],
            16,
        ),
        [16.0, 0.0, 16.0]
    );
    for kind in [
        NodeKind::SpawnPoint {
            player: true,
            character: None,
        },
        NodeKind::ParticleEmitter {
            settings: psxed_project::ParticleEmitterSettings::default(),
        },
    ] {
        assert_eq!(node_snap_step(&kind, 16), Some(16), "{}", kind.label());
    }
    assert_eq!(node_snap_step(&NodeKind::Group, 16), None);

    // Placement lands on the grid too; the picked height stays exact.
    let project = ProjectDocument::new("placement-snap");
    let root = project.active_scene().root;
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("placement-snap"), project);
    workspace.place_kind = PlaceKind::ParticleEmitter;
    workspace.place_node_at_world_hit(root, [509.0, 37.0, 522.0]);
    let node = workspace
        .project
        .active_scene()
        .node(workspace.selected_node_id())
        .expect("placed emitter is selected");
    assert_eq!(node.transform.translation, [512.0, 37.0, 528.0]);
}

/// A copy of the New Project template to open, save and close for real.
fn template_copy(label: &str) -> PathBuf {
    let dir = test_temp_dir(label);
    let _ = std::fs::remove_dir_all(&dir);
    crate::starter_catalogue::copy_dir_recursive(&psxed_project::new_project_template_dir(), &dir)
        .unwrap();
    let _ = std::fs::remove_file(dir.join(psxed_project::EDITOR_VIEW_STATE_FILE));
    dir
}

#[test]
fn looking_around_never_edits_the_project_and_the_view_comes_back_on_open() {
    let dir = template_copy("view-state-sidecar");
    let before = std::fs::read(dir.join("project.ron")).unwrap();
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.active_workspace = WorkspaceView::Room;
    workspace.rotate_viewport_3d_camera(Vec2::new(40.0, -12.0));
    workspace.scroll_viewport_3d_camera(120.0);
    workspace.show_grid = !workspace.show_grid;
    workspace.snap_units = 32;
    assert!(
        !workspace.is_dirty(),
        "orbiting the camera marked the project unsaved"
    );
    let view = workspace.editor_view_state();

    // Closing with nothing unsaved is allowed at once and writes only the
    // view file.
    assert!(workspace.request_close());
    assert_eq!(std::fs::read(dir.join("project.ron")).unwrap(), before);
    assert!(dir.join(psxed_project::EDITOR_VIEW_STATE_FILE).is_file());

    let reopened = EditorWorkspace::open_directory(&dir).unwrap();
    assert_eq!(reopened.editor_view_state(), view);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_with_unsaved_edits_asks_save_discard_or_cancel() {
    let dir = template_copy("unsaved-close-prompt");
    let before = std::fs::read(dir.join("project.ron")).unwrap();
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.active_workspace = WorkspaceView::Room;
    workspace.push_undo();
    add_marker_node(&mut workspace);
    workspace.mark_dirty();

    // Cancel keeps the edit and the window.
    assert!(!workspace.request_close());
    assert!(matches!(workspace.modal, Modal::UnsavedChanges { .. }));
    super::egui_driver::run_real_egui_workspace_click_on_label(&mut workspace, "Cancel");
    assert!(matches!(workspace.modal, Modal::None));
    assert!(workspace.is_dirty());

    // Discard closes without writing project.ron.
    assert!(!workspace.request_close());
    super::egui_driver::run_real_egui_workspace_click_on_label(&mut workspace, "Discard and Close");
    assert!(
        workspace.request_close(),
        "the answered prompt lets the close through"
    );
    assert_eq!(std::fs::read(dir.join("project.ron")).unwrap(), before);

    // Save writes the edit, then closes.
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    let nodes = scene_node_count(&workspace);
    workspace.push_undo();
    add_marker_node(&mut workspace);
    workspace.mark_dirty();
    assert!(!workspace.request_close());
    super::egui_driver::run_real_egui_workspace_click_on_label(&mut workspace, "Save and Close");
    assert!(!workspace.is_dirty());
    assert!(workspace.request_close());
    let saved = ProjectDocument::load_from_path(dir.join("project.ron")).unwrap();
    assert_eq!(saved.active_scene().nodes().len(), nodes + 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reload_with_unsaved_edits_asks_first() {
    let dir = template_copy("unsaved-reload-prompt");
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.active_workspace = WorkspaceView::Room;
    let nodes = scene_node_count(&workspace);
    workspace.push_undo();
    add_marker_node(&mut workspace);
    workspace.mark_dirty();

    workspace.request_reload();
    assert!(matches!(workspace.modal, Modal::UnsavedChanges { .. }));
    super::egui_driver::run_real_egui_workspace_click_on_label(&mut workspace, "Cancel");
    assert_eq!(scene_node_count(&workspace), nodes + 1);

    workspace.request_reload();
    super::egui_driver::run_real_egui_workspace_click_on_label(
        &mut workspace,
        "Discard and Reload",
    );
    assert_eq!(scene_node_count(&workspace), nodes);
    assert!(!workspace.is_dirty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn after_save_the_open_document_is_what_reopening_the_file_gives() {
    let dir = template_copy("save-normalises-open-document");
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.push_undo();
    add_marker_node(&mut workspace);
    workspace.mark_dirty();
    workspace.save().unwrap();
    let on_disk = ProjectDocument::load_from_path(dir.join("project.ron")).unwrap();
    assert!(
        on_disk == workspace.project,
        "the editor kept a document that differs from the file it just saved"
    );
    let _ = std::fs::remove_dir_all(dir);
}
