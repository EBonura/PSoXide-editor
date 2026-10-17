use super::*;

#[test]
fn build_and_play_requests_preserve_viewport_layout_and_camera() {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("project-build-console"),
        ProjectDocument::new("project build console"),
    );
    workspace.resources_open = false;
    workspace.content_browser_view = ContentBrowserView::Resources;
    workspace.camera_rig.yaw = 1234;
    workspace.camera_rig.pitch = 3456;
    workspace.camera_rig.radius = 789;
    workspace.camera_rig.target = [111, 222, 333];
    let camera = workspace.current_editor_camera_state();

    workspace.request_project_build();

    assert!(!workspace.resources_open);
    assert_eq!(
        workspace.content_browser_view,
        ContentBrowserView::Resources
    );
    assert_eq!(workspace.current_editor_camera_state(), camera);
    assert_eq!(
        workspace.take_playtest_request(),
        Some(EditorPlaytestRequest::BuildProject)
    );

    workspace.request_play_or_rebuild(EditorPlaytestStatus::Idle);
    assert!(!workspace.resources_open);
    assert_eq!(
        workspace.content_browser_view,
        ContentBrowserView::Resources
    );
    assert_eq!(workspace.current_editor_camera_state(), camera);
    assert_eq!(
        workspace.take_playtest_request(),
        Some(EditorPlaytestRequest::Play)
    );
}

#[test]
fn point_of_interest_place_tool_creates_entity_host_and_component() {
    let project = ProjectDocument::new("poi placement");
    let root = project.active_scene().root;
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("poi-placement"), project);
    workspace.set_active_tool_cycle_value((ViewTool::Place, Some(PlaceKind::PointOfInterest)));

    workspace.place_node_at_world_hit(root, [512.0, 1.0, 768.0]);
    let component_id = workspace.selected_node_id();
    let scene = workspace.project().active_scene();
    let component = scene.node(component_id).expect("poi component");
    let host = component.parent.expect("component has an entity host");
    let host_node = scene.node(host).expect("placed entity host");
    assert!(matches!(host_node.kind, NodeKind::Entity));
    assert_eq!(host_node.parent, Some(root));
    assert_eq!(host_node.transform.translation, [512.0, 1.0, 768.0]);
    assert_eq!(host_node.children.len(), 1);
    assert_eq!(host_node.children, vec![component_id]);
    let NodeKind::PointOfInterest {
        pages,
        prompt,
        radius,
        marker_height,
        repeatable,
        reward,
        enabled,
        ..
    } = &component.kind
    else {
        panic!("expected point of interest component");
    };
    assert_eq!(pages, &["ARCHIVE SIGNAL DETECTED."]);
    assert_eq!(prompt, "READ");
    assert_eq!(*radius, 576);
    assert_eq!(*marker_height, 192);
    assert!(*repeatable);
    assert!(reward.is_none());
    assert!(*enabled);
    assert!(workspace.status.contains("replace the default message"));
}

#[test]
fn legacy_and_archive_interactions_share_one_component_slot() {
    let templates = crate::scene_tree::component_templates_for_host(&NodeKind::Entity);
    let legacy = templates
        .iter()
        .find(|(name, _)| *name == "Interactable")
        .map(|(_, kind)| kind)
        .expect("legacy template");
    let archive = templates
        .iter()
        .find(|(name, _)| *name == "Point of Interest")
        .map(|(_, kind)| kind)
        .expect("POI template");
    assert!(!crate::scene_tree::component_can_be_added(
        archive,
        &[legacy]
    ));
    assert!(!crate::scene_tree::component_can_be_added(
        legacy,
        &[archive]
    ));
}

fn save_watch_project(dir: &Path, project: &ProjectDocument) {
    std::fs::create_dir_all(dir).unwrap();
    project.save_to_path(dir.join("project.ron")).unwrap();
}

#[test]
fn external_project_change_never_replaces_open_document_and_play_save_wins() {
    let dir = test_temp_dir("project-watch-conflict");
    let project = ProjectDocument::new("watch baseline");
    save_watch_project(&dir, &project);
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();

    let mut external = project.clone();
    external.name = "clean external change with different length".to_string();
    external.save_to_path(dir.join("project.ron")).unwrap();
    workspace.poll_project_watch(true);
    assert_eq!(workspace.project().name, project.name);
    assert!(workspace.is_dirty());
    assert!(workspace.has_external_project_conflict());
    assert!(workspace.save_if_dirty().unwrap());
    assert_eq!(
        ProjectDocument::load_from_path(dir.join("project.ron"))
            .unwrap()
            .name,
        project.name
    );
    assert!(!workspace.is_dirty());
    assert!(!workspace.has_external_project_conflict());

    let baseline_nodes = scene_node_count(&workspace);
    add_marker_node(&mut workspace);
    workspace.mark_dirty();
    let mut second_external = project.clone();
    push_marker(second_external.active_scene_mut());
    push_marker(second_external.active_scene_mut());
    second_external
        .save_to_path(dir.join("project.ron"))
        .unwrap();
    assert!(workspace.save_if_dirty().unwrap());
    assert_eq!(scene_node_count(&workspace), baseline_nodes + 1);
    assert_eq!(
        ProjectDocument::load_from_path(dir.join("project.ron"))
            .unwrap()
            .active_scene()
            .nodes()
            .len(),
        baseline_nodes + 1
    );
    assert!(!workspace.is_dirty());
    assert!(!workspace.has_external_project_conflict());
    let _ = std::fs::remove_dir_all(dir);
}

/// Repeated external writes must never make the locally authored document
/// unsaveable. Every explicit Save overwrites the disk version and recaptures
/// the watcher baseline for the next edit.
#[test]
fn repeated_external_conflicts_are_resolved_by_explicit_save() {
    let dir = test_temp_dir("project-watch-cycles");
    let project = ProjectDocument::new("cycle baseline");
    save_watch_project(&dir, &project);
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    let path = dir.join("project.ron");

    let node_count = |document: &ProjectDocument| document.active_scene().nodes().len();

    for round in 1..=3usize {
        // A local edit the author has not saved yet.
        let local_before = node_count(workspace.project());
        add_marker_node(&mut workspace);
        workspace.mark_dirty();
        assert!(
            !workspace.has_external_project_conflict(),
            "round {round} started latched"
        );

        // Something outside the editor rewrites project.ron. Start from the
        // last saved document, not the in-memory local edit.
        let mut external = ProjectDocument::load_from_path(&path).unwrap();
        for _ in 0..round {
            push_marker(external.active_scene_mut());
        }
        external.save_to_path(&path).unwrap();

        // Explicit Save keeps the local document and overwrites the external
        // disk version.
        workspace
            .save()
            .unwrap_or_else(|error| panic!("round {round} Save: {error}"));
        assert_eq!(
            node_count(workspace.project()),
            local_before + 1,
            "round {round} lost the unsaved edit"
        );
        assert!(
            !workspace.has_external_project_conflict(),
            "round {round} stayed latched after Save"
        );
        assert!(!workspace.is_dirty());

        // The local bytes are on disk, and polling the freshly captured
        // baseline sees no phantom change.
        assert_eq!(
            node_count(&ProjectDocument::load_from_path(&path).unwrap()),
            local_before + 1,
            "round {round} did not reach disk"
        );
        workspace.poll_project_watch(true);
        assert!(
            !workspace.has_external_project_conflict(),
            "round {round} polled a phantom conflict after Save"
        );
        assert_eq!(node_count(workspace.project()), local_before + 1);
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn same_length_atomic_project_replace_is_detected_even_when_metadata_looks_unchanged() {
    let dir = test_temp_dir("project-watch-atomic-same-length");
    let path = dir.join("project.ron");
    let project = ProjectDocument::new("baseline-name");
    save_watch_project(&dir, &project);
    let baseline_len = std::fs::metadata(&path).unwrap().len();
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();

    let mut external = project.clone();
    external.name = "external-name".to_string();
    let replacement = dir.join("project.ron.atomic-replacement");
    external.save_to_path(&replacement).unwrap();
    assert_eq!(std::fs::metadata(&replacement).unwrap().len(), baseline_len);
    std::fs::rename(&replacement, &path).unwrap();
    // Model the hardest coarse-filesystem case: the watcher sees metadata
    // equal to its baseline. Only the always-bounded project hash can reveal
    // that the atomic replacement contains different bytes.
    workspace.project_watch.project.metadata = watched_file_metadata(&path);
    workspace.poll_project_watch(true);
    assert_eq!(workspace.project().name, project.name);
    assert!(workspace.is_dirty());
    assert!(workspace.has_external_project_conflict());

    // Reload is the only operation that accepts the external document.
    workspace.reload();
    assert_eq!(workspace.project().name, external.name);
    assert!(!workspace.is_dirty());
    assert!(!workspace.has_external_project_conflict());

    workspace.project.active_scene_mut().name = "mine".to_string();
    workspace.mark_dirty();
    let mut second_external = external.clone();
    second_external.active_scene_mut().name = "disk".to_string();
    second_external.save_to_path(&replacement).unwrap();
    assert_eq!(std::fs::metadata(&replacement).unwrap().len(), baseline_len);
    std::fs::rename(&replacement, &path).unwrap();
    workspace.project_watch.project.metadata = watched_file_metadata(&path);
    workspace.save().unwrap();
    assert_eq!(workspace.project().active_scene().name, "mine");
    assert!(!workspace.has_external_project_conflict());
    assert_eq!(
        ProjectDocument::load_from_path(&path)
            .unwrap()
            .active_scene()
            .name,
        "mine"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn resource_watch_is_metadata_only_and_ignores_large_raw_source_resources() {
    let dir = test_temp_dir("resource-watch-scope");
    let runtime_path = dir.join("assets/sfx/hit.wav");
    let unreferenced_path = dir.join("source_assets/huge.glb");
    std::fs::create_dir_all(runtime_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(unreferenced_path.parent().unwrap()).unwrap();
    std::fs::write(&runtime_path, b"runtime-audio").unwrap();
    std::fs::write(&unreferenced_path, vec![0x5a; 2 * 1024 * 1024]).unwrap();
    let mut project = ProjectDocument::new("watch resources");
    project.add_resource(
        "Hit",
        ResourceData::Audio {
            source_path: "assets/sfx/hit.wav".to_string(),
        },
    );
    project.add_resource(
        "Raw source mesh",
        ResourceData::Mesh {
            source_path: "source_assets/huge.glb".to_string(),
        },
    );
    save_watch_project(&dir, &project);
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    assert!(workspace
        .project_watch
        .resources
        .contains_key(&runtime_path));
    assert!(!workspace
        .project_watch
        .resources
        .contains_key(&unreferenced_path));

    let baseline = workspace.project_watch.resources.clone();
    let old_status = workspace.status.clone();
    std::fs::write(&unreferenced_path, vec![0x33; 2 * 1024 * 1024 + 1]).unwrap();
    workspace.poll_project_watch(true);
    assert_eq!(workspace.project_watch.resources, baseline);
    assert_eq!(workspace.status, old_status);

    std::fs::write(&runtime_path, b"runtime-audio-changed").unwrap();
    workspace.poll_project_watch(true);
    assert_eq!(
        workspace.status_text(),
        "Reloaded externally changed project resources"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn referenced_runtime_resource_delete_recreate_and_atomic_replace_are_detected() {
    let dir = test_temp_dir("resource-watch-delete-recreate");
    let runtime_path = dir.join("assets/sfx/hit.wav");
    std::fs::create_dir_all(runtime_path.parent().unwrap()).unwrap();
    std::fs::write(&runtime_path, b"runtime-audio-A").unwrap();
    let mut project = ProjectDocument::new("resource lifecycle");
    project.add_resource(
        "Hit",
        ResourceData::Audio {
            source_path: "assets/sfx/hit.wav".to_string(),
        },
    );
    save_watch_project(&dir, &project);
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    let original = watched_file_metadata(&runtime_path).unwrap();

    std::fs::remove_file(&runtime_path).unwrap();
    workspace.poll_project_watch(true);
    assert_eq!(workspace.project_watch.resources[&runtime_path], None);
    assert_eq!(
        workspace.status_text(),
        "Reloaded externally changed project resources"
    );

    std::fs::write(&runtime_path, b"runtime-audio-B").unwrap();
    workspace.poll_project_watch(true);
    let recreated = workspace.project_watch.resources[&runtime_path].unwrap();
    assert_ne!(recreated, original, "recreated file identity must change");
    assert_eq!(
        workspace.status_text(),
        "Reloaded externally changed project resources"
    );

    let replacement = runtime_path.with_extension("atomic");
    std::fs::write(&replacement, b"runtime-audio-C").unwrap();
    assert_eq!(
        std::fs::metadata(&replacement).unwrap().len(),
        std::fs::metadata(&runtime_path).unwrap().len()
    );
    std::fs::rename(&replacement, &runtime_path).unwrap();
    let atomically_replaced = watched_file_metadata(&runtime_path).unwrap();
    assert_ne!(atomically_replaced, recreated);
    workspace.poll_project_watch(true);
    assert_eq!(
        workspace.project_watch.resources[&runtime_path],
        Some(atomically_replaced)
    );
    assert_eq!(
        workspace.status_text(),
        "Reloaded externally changed project resources"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn centered_aspect_rect_centers_wide_preview_box() {
    let container = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 240.0));

    let rect = centered_aspect_rect(container, VIEWPORT_PREVIEW_ASPECT);

    assert_size_approx(rect.size(), Vec2::new(320.0, 240.0));
    assert_pos_approx(rect.center(), container.center());
}

#[test]
fn centered_aspect_rect_centers_tall_preview_box() {
    let container = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(320.0, 800.0));

    let rect = centered_aspect_rect(container, VIEWPORT_PREVIEW_ASPECT);

    assert_size_approx(rect.size(), Vec2::new(320.0, 240.0));
    assert_pos_approx(rect.center(), container.center());
}

#[test]
fn screen_offset_preview_shift_scales_device_px_to_canvas_px() {
    // No offset -> no shift, regardless of scale.
    assert_eq!(screen_offset_preview_shift(0, 640.0, 320), 0.0);
    // 320-logical canvas drawn at 640 egui px is 2x, so 32 device px -> 64.
    assert_eq!(screen_offset_preview_shift(32, 640.0, 320), 64.0);
    // 1:1 scale passes the device offset straight through, sign preserved.
    assert_eq!(screen_offset_preview_shift(-16, 320.0, 320), -16.0);
    // Degenerate logical width is clamped, never divides by zero.
    assert_eq!(screen_offset_preview_shift(10, 320.0, 0), 3200.0);
}

#[test]
fn free_camera_center_ray_uses_position_and_forward_basis() {
    let camera = ViewportCameraState {
        mode: ViewportCameraMode::Free,
        yaw_q12: 0,
        pitch_q12: 0,
        radius: 1000,
        target: [0, 0, 0],
        position: [10, 20, 30],
    };

    let (origin, dir) = camera.ray_for_normalized_panel_point(0.0, 0.0);

    assert_vec3_approx(origin, [10.0, 20.0, 30.0]);
    assert_vec3_approx(dir, [0.0, 0.0, -1.0]);
    assert_eq!(camera.anchor_i32(), [10, 20, 30]);
    assert_eq!(camera.position_i32(), [10, 20, 30]);
}

#[test]
fn orbit_camera_keeps_target_anchor() {
    let camera = ViewportCameraState {
        mode: ViewportCameraMode::Orbit,
        yaw_q12: 0,
        pitch_q12: 0,
        radius: 1000,
        target: [10, 20, 30],
        position: [0, 0, 0],
    };

    let (origin, dir) = camera.ray_for_normalized_panel_point(0.0, 0.0);

    assert_vec3_approx(origin, [10.0, 20.0, 1030.0]);
    assert_vec3_approx(dir, [0.0, 0.0, -1.0]);
    assert_eq!(camera.anchor_i32(), [10, 20, 30]);
    assert_eq!(camera.position_i32(), [10, 20, 1030]);
}

#[test]
fn orbit_camera_quarter_turn_uses_q12_units() {
    let position = orbit_camera_position_i32(1024, 0, 1000, [10, 20, 30]);

    assert_eq!(position, [1010, 20, 30]);
}

#[test]
fn free_camera_forward_quarter_turn_uses_q12_units() {
    let forward = camera_forward_from_angles(1024, 0);

    assert_vec3_approx(forward, [-1.0, 0.0, 0.0]);
}

#[test]
fn focus_shortcut_fits_orbit_bounds_and_preserves_angle() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    workspace.camera_rig.mode = ViewportCameraMode::Orbit;
    workspace.camera_rig.radius = 12_345;
    workspace.camera_rig.yaw = 256;
    workspace.camera_rig.pitch = 256;

    workspace.frame_3d_bounds([4096.0, 512.0, -2048.0], [100.0, 500.0, 250.0]);

    assert_eq!(workspace.camera_rig.target, [4096, 512, -2048]);
    assert_eq!(workspace.camera_rig.radius, 1600);
    assert_eq!(workspace.camera_rig.yaw, 256);
    assert_eq!(workspace.camera_rig.pitch, 256);
}

#[test]
fn focus_shortcut_in_free_mode_fits_bounds_and_preserves_direction() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    workspace.camera_rig.mode = ViewportCameraMode::Free;
    workspace.camera_rig.free_position = [0, 0, 0];
    workspace.camera_rig.free_yaw = 1024;
    workspace.camera_rig.free_pitch = signed_to_q12(300);
    workspace.camera_rig.free_initialized = true;

    workspace.frame_3d_bounds([0.0, 0.0, -4096.0], [256.0, 256.0, 256.0]);

    assert_eq!(workspace.camera_rig.free_position, [734, -364, -4096]);
    assert_eq!(workspace.camera_rig.target, [0, 0, -4096]);
    assert_eq!(workspace.camera_rig.radius, 819);
    assert_eq!(workspace.camera_rig.free_yaw, 1024);
    assert_eq!(workspace.camera_rig.free_pitch, signed_to_q12(300));
}

#[test]
fn editor_camera_saves_with_project_and_restores_on_open() {
    let project_dir = test_temp_dir("editor-camera");
    let project = ProjectDocument::new("editor-camera");
    let mut workspace = EditorWorkspace::with_project(project_dir.clone(), project);
    workspace.camera_rig.mode = ViewportCameraMode::Free;
    workspace.camera_rig.yaw = 384;
    workspace.camera_rig.pitch = signed_to_q12(-128);
    workspace.camera_rig.radius = 12_288;
    workspace.camera_rig.target = [1024, 512, -2048];
    workspace.camera_rig.free_yaw = 1536;
    workspace.camera_rig.free_pitch = 128;
    workspace.camera_rig.free_position = [-300, 700, 900];
    workspace.camera_rig.free_initialized = true;

    workspace.save().unwrap();

    let reopened = EditorWorkspace::open_directory(&project_dir).unwrap();
    let camera = reopened.viewport_3d_camera();
    assert_eq!(camera.mode, ViewportCameraMode::Free);
    assert_eq!(camera.yaw_q12, 1536);
    assert_eq!(camera.pitch_q12, 128);
    assert_eq!(camera.radius, 12_288);
    assert_eq!(camera.target, [1024, 512, -2048]);
    assert_eq!(camera.position, [-300, 700, 900]);

    let _ = std::fs::remove_dir_all(project_dir);
}

#[test]
fn editor_visibility_saves_with_project_and_restores_on_open() {
    let project_dir = test_temp_dir("editor-visibility");
    let mut workspace =
        EditorWorkspace::with_project(project_dir.clone(), ProjectDocument::new("visibility"));
    workspace.show_grid = false;
    workspace.show_lights = false;
    workspace.preview_bounds = false;
    workspace.show_play_debug_overlays = false;

    workspace.save().unwrap();

    let reopened = EditorWorkspace::open_directory(&project_dir).unwrap();
    assert!(!reopened.show_grid_enabled());
    assert!(!reopened.show_lights_enabled());
    assert!(!reopened.preview_bounds_enabled());
    assert!(!reopened.show_play_debug_overlays);

    let _ = std::fs::remove_dir_all(project_dir);
}

#[test]
fn editor_workspace_saves_with_project_and_restores_on_open() {
    let project_dir = test_temp_dir("editor-workspace");
    let mut workspace =
        EditorWorkspace::with_project(project_dir.clone(), ProjectDocument::new("workspace"));
    workspace.active_workspace = WorkspaceView::Material;

    workspace.save().unwrap();

    let reopened = EditorWorkspace::open_directory(&project_dir).unwrap();
    assert_eq!(reopened.active_workspace, WorkspaceView::Material);

    let _ = std::fs::remove_dir_all(project_dir);
}

#[test]
fn editor_viewport_saves_with_project_and_restores_on_open() {
    let project_dir = test_temp_dir("editor-viewport");
    let mut workspace =
        EditorWorkspace::with_project(project_dir.clone(), ProjectDocument::new("viewport"));
    workspace.view_2d = true;
    workspace.set_orthographic_view(OrthographicView::Top);
    workspace.orthographic_focus = [64.0, 128.0, 256.0];
    workspace.viewport_zoom = 48.0;
    workspace.snap_units = 32;

    workspace.save().unwrap();

    let reopened = EditorWorkspace::open_directory(&project_dir).unwrap();
    assert!(reopened.view_2d, "2D mode restored");
    assert_eq!(reopened.orthographic_view, OrthographicView::Top);
    assert_eq!(reopened.orthographic_focus, [64.0, 128.0, 256.0]);
    assert_eq!(reopened.viewport_zoom, 48.0);
    assert_eq!(reopened.snap_units, 32);
    assert_eq!(reopened.grid_snap_units(), 32);

    // Out-of-range persisted zoom clamps into the interactive range.
    let mut wild = EditorWorkspace::open_directory(&project_dir).unwrap();
    let mut view = wild.editor_view_state();
    view.viewport.viewport_zoom = 100_000.0;
    view.viewport.snap_units = 0;
    wild.apply_editor_view_state(view);
    assert_eq!(wild.viewport_zoom, MAX_VIEWPORT_ZOOM);
    // The grid never goes below one engine unit, and lands on multiples of it.
    assert_eq!(wild.snap_units, 16);
    assert_eq!(wild.grid_snap_units(), 16);
    view.viewport.snap_units = 40;
    wild.apply_editor_view_state(view);
    assert_eq!(wild.snap_units, 48);

    let _ = std::fs::remove_dir_all(project_dir);
}

#[test]
fn opening_tracked_starter_preserves_its_authored_interior_camera() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../archive/fixtures/brush-first-playable");
    let authored = ProjectDocument::load_from_path(fixture_dir.join("project.ron"))
        .unwrap()
        .editor_camera;

    let reopened = EditorWorkspace::open_directory(&fixture_dir).unwrap();

    assert!(authored.orbit_radius > 0);
    assert_eq!(reopened.current_editor_camera_state(), authored);
    assert_eq!(reopened.project().editor_camera, authored);
    assert!(!reopened.is_dirty());
}

#[test]
fn stale_typed_cook_issue_target_falls_back_cleanly() {
    let mut project = ProjectDocument::new("stale diagnostic");
    let root = project.active_scene().root;
    let gone = project
        .active_scene_mut()
        .add_node(root, "Gone", NodeKind::Group);
    project.active_scene_mut().remove_node(gone);
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    assert!(!workspace.focus_playtest_validation_target(
        psxed_project::playtest::PlaytestValidationTarget::Node(gone)
    ));
}

/// A cook error raised by a per-kind cook helper (which only ever sees a node
/// NAME) still blames the authoring node, via the caller-side blame scope.
#[test]
fn cook_errors_blame_the_authoring_node_that_raised_them() {
    use psxed_project::playtest::PlaytestValidationTarget;

    // A Trigger Volume with no target is dead content and fails the cook.
    let (dir, _scratch) = cookable_template_copy("per-error-targets");
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.set_active_tool_cycle_value((ViewTool::Place, Some(PlaceKind::Logic)));
    place_at(&mut workspace, [1024.0, 1024.0]);
    let offender = workspace.selected_node_id();

    let project = workspace.project().clone();
    let (package, report) = psxed_project::playtest::build_package(&project, &dir);
    assert!(package.is_none(), "a targetless volume must fail the cook");
    let blamed: Vec<_> = report
        .errors
        .iter()
        .filter(|error| error.contains("has no target"))
        .map(|error| error.target)
        .collect();
    assert_eq!(
        blamed,
        vec![Some(PlaytestValidationTarget::Node(offender))],
        "the dead volume blames its own node"
    );
    assert_eq!(
        report.focus_target(),
        Some(PlaytestValidationTarget::Node(offender)),
        "the convenience accessor still returns the first focusable error"
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// End to end: a failing cook driven through the editor's own cook command
/// auto-focuses the authored node responsible, and keeps every error's target
/// available so the diagnostics list can focus any row.
#[test]
fn failing_cook_auto_focuses_the_offending_node_and_keeps_every_target() {
    use psxed_project::playtest::PlaytestValidationTarget;

    let (dir, _scratch) = cookable_template_copy("failing-cook-focus");
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.set_active_tool_cycle_value((ViewTool::Place, Some(PlaceKind::Logic)));
    place_at(&mut workspace, [1024.0, 1024.0]);
    let offender = workspace.selected_node_id();
    workspace.replace_node_selection(workspace.project().active_scene().root);

    let cook = test_temp_dir("failing-cook-focus-out");
    let error = workspace
        .cook_playtest_to_dir(&cook)
        .expect_err("a targetless Trigger Volume must fail validation");
    assert!(error.contains("has no target"), "{error}");
    assert_eq!(
        workspace.selected_node_id(),
        offender,
        "the failing cook selected the node the author has to fix"
    );
    let targets: Vec<_> = workspace
        .last_cook_errors
        .iter()
        .map(|error| error.target)
        .collect();
    assert!(
        targets.contains(&Some(PlaytestValidationTarget::Node(offender))),
        "the retained diagnostics keep the offender focusable: {targets:?}"
    );

    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(cook);
}

#[test]
fn blank_point_of_interest_cook_error_focuses_its_message_component() {
    use psxed_project::playtest::PlaytestValidationTarget;

    let (dir, _scratch) = cookable_template_copy("blank-poi-focus");
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    workspace.set_active_tool_cycle_value((ViewTool::Place, Some(PlaceKind::PointOfInterest)));
    place_at(&mut workspace, [1024.0, 1024.0]);
    let message_component = workspace.selected_node_id();
    let NodeKind::PointOfInterest { pages, .. } = &mut workspace
        .project
        .active_scene_mut()
        .node_mut(message_component)
        .expect("placed POI component")
        .kind
    else {
        panic!("placed node must be a POI component");
    };
    pages[0].clear();

    let (package, report) = psxed_project::playtest::build_package(workspace.project(), &dir);
    assert!(package.is_none(), "a blank POI message must fail the cook");
    let error = report
        .errors
        .iter()
        .find(|error| error.contains("page 1 is blank"))
        .expect("blank POI validation error");
    assert_eq!(
        error.target,
        Some(PlaytestValidationTarget::Node(message_component)),
        "Play should select the message component whose page needs editing"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn texture_import_resolution_label_marks_presets_and_custom_sizes() {
    assert_eq!(texture_import_resolution_label(32, 32), "32 x 32");
    assert_eq!(texture_import_resolution_label(40, 24), "Custom 40 x 24");
}

#[test]
fn viewport_3d_pan_delta_tracks_pointer_drag_plane() {
    let camera = ViewportCameraState {
        mode: ViewportCameraMode::Orbit,
        yaw_q12: 0,
        pitch_q12: 0,
        radius: 1000,
        target: [0, 0, 0],
        position: [0, 0, 0],
    };

    let delta = viewport_3d_pan_delta(camera, Vec2::new(1000.0, 750.0), Vec2::new(100.0, 100.0));

    assert_vec3_approx(delta, [-100.0, 100.0, 0.0]);
}

#[test]
fn orbit_camera_rotation_uses_slow_step_and_clamps_pitch() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    workspace.camera_rig.mode = ViewportCameraMode::Orbit;
    workspace.camera_rig.yaw = 0;
    workspace.camera_rig.pitch = signed_to_q12(940);

    workspace.rotate_viewport_3d_camera(Vec2::new(100.0, 200.0));

    assert_eq!(workspace.camera_rig.yaw, 400);
    assert_eq!(workspace.camera_rig.pitch, signed_to_q12(960));
}

#[test]
fn free_camera_rotation_uses_q12_drag_sensitivity() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    workspace.camera_rig.mode = ViewportCameraMode::Free;
    workspace.camera_rig.free_yaw = 1024;
    workspace.camera_rig.free_pitch = 0;

    workspace.rotate_viewport_3d_camera(Vec2::new(100.0, 50.0));

    assert_eq!(workspace.camera_rig.free_yaw, 624);
    assert_eq!(workspace.camera_rig.free_pitch, signed_to_q12(-200));
    assert!(workspace.camera_rig.free_initialized);
}

#[test]
fn command_modifier_blocks_bare_shortcuts() {
    assert!(bare_shortcuts_available(false, egui::Modifiers::NONE));
    assert!(!bare_shortcuts_available(true, egui::Modifiers::NONE));
    assert!(!bare_shortcuts_available(false, egui::Modifiers::COMMAND));
    assert!(!bare_shortcuts_available(false, egui::Modifiers::CTRL));
}

#[test]
fn command_shortcut_consumes_but_ignores_key_repeat() {
    let mut input = egui::InputState::default();
    let shortcut = command_shortcut(egui::Key::Z);
    input.events.push(egui::Event::Key {
        key: egui::Key::Z,
        physical_key: Some(egui::Key::Z),
        pressed: true,
        repeat: true,
        modifiers: egui::Modifiers::COMMAND,
    });
    assert!(!consume_shortcut_once(&mut input, &shortcut));
    assert!(input.events.is_empty());

    input.events.push(egui::Event::Key {
        key: egui::Key::Z,
        physical_key: Some(egui::Key::Z),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    });
    assert!(consume_shortcut_once(&mut input, &shortcut));
    assert!(input.events.is_empty());
}

#[test]
fn cycle_value_wraps_forward_and_backward() {
    const VALUES: &[u8] = &[1, 2, 3];

    assert_eq!(cycle_value(VALUES, 1, false), 2);
    assert_eq!(cycle_value(VALUES, 3, false), 1);
    assert_eq!(cycle_value(VALUES, 1, true), 3);
    assert_eq!(cycle_value(VALUES, 9, false), 1);
}

#[test]
fn visibility_cycle_only_changes_editor_view_items() {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("visibility-cycle"),
        ProjectDocument::new("visibility"),
    );
    workspace.show_grid = true;
    workspace.show_lights = true;
    workspace.preview_bounds = true;
    workspace.show_play_debug_overlays = false;

    workspace.cycle_visibility_group(false);

    assert!(workspace.show_grid);
    assert!(!workspace.show_lights);
    assert!(!workspace.preview_bounds);
    assert!(!workspace.show_play_debug_overlays);
}

#[test]
fn the_grid_master_toggle_flips_the_overlay() {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("grid-master"),
        ProjectDocument::new("grid master"),
    );
    workspace.show_grid = true;

    workspace.toggle_grid_overlays();
    assert!(!workspace.show_grid);

    workspace.toggle_grid_overlays();
    assert!(workspace.show_grid);
}

#[test]
fn debug_snapshot_omits_retired_portal_runtime_log() {
    let workspace = EditorWorkspace::with_project(
        test_temp_dir("debug-snapshot"),
        ProjectDocument::new("debug snapshot"),
    );

    let metrics = EditorPlaytestMetrics {
        fixed_update_task_ms: 0.0,
        fixed_update_task_max_ms: 0.0,
        visual_render_task_ms: 0.0,
        visual_render_task_max_ms: 0.0,
        vram_caps_full: [0, 0, 0, 0],
    };
    let path = workspace.debug_log_path();

    workspace
        .write_debug_snapshot(&path, Some(metrics))
        .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("scheduler_tasks:"));
    assert!(!content.contains("runtime_player:"));
    assert!(!content.contains("runtime_camera:"));
    assert!(!content.contains("connected_portals:"));
    assert!(!content.contains("portal #"));

    let _ = std::fs::remove_dir_all(workspace.project_dir);
}

#[test]
fn play_performance_panel_toggle_persists_in_the_view_sidecar() {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("perf-panel-toggle"),
        ProjectDocument::new("perf-panel-toggle"),
    );
    let shown = workspace.play_performance_panel_visible();
    workspace.toggle_play_performance_panel();
    assert_eq!(workspace.play_performance_panel_visible(), !shown);
    assert_eq!(
        workspace
            .editor_view_state()
            .visibility
            .show_play_debug_overlays,
        !shown
    );
    let _ = std::fs::remove_dir_all(workspace.project_dir);
}

#[test]
fn menu_labels_include_discoverable_shortcut_text() {
    assert_eq!(menu_label("Save", "Cmd+S"), "Save    Cmd+S");
}

#[test]
fn animation_source_catalogue_scans_synty_source_tree() {
    let dir = test_temp_dir("animation-source-catalogue");
    let anim_dir = dir.join("SourceFiles/Animations/Polygon/Dodge");
    let model_dir = dir.join("SourceFiles/Models");
    std::fs::create_dir_all(&anim_dir).unwrap();
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(anim_dir.join("A_DodgeRoll_F_RootMotion_Sword.fbx"), []).unwrap();
    std::fs::write(anim_dir.join("A_Block_Loop_Sword.fbx"), []).unwrap();
    std::fs::write(model_dir.join("POLYGONRig_01.fbx"), []).unwrap();

    let mut project = ProjectDocument::new("source catalogue");
    let report = catalogue_animation_sources_from_path(&mut project, &dir, &dir).unwrap();

    assert_eq!(report.source_candidates, 2);
    assert_eq!(report.sources_added, 2);
    assert_eq!(report.sources_updated, 0);
    let sources: Vec<_> = project
        .resources
        .iter()
        .filter_map(|resource| match &resource.data {
            ResourceData::AnimationSource(source) => Some((resource.name.as_str(), source)),
            _ => None,
        })
        .collect();
    assert_eq!(sources.len(), 2);
    let roll = sources
        .iter()
        .find(|(_, source)| source.clip_name == "A_DodgeRoll_F_RootMotion_Sword")
        .expect("roll source catalogued")
        .1;
    assert_eq!(roll.provider, psxed_project::AnimationSourceProvider::Synty);
    assert_eq!(roll.role, psxed_project::AnimationRole::Roll);
    assert!(!roll.looping);
    assert!(roll.tags.iter().any(|tag| tag == "dodge"));
    assert!(roll.tags.iter().any(|tag| tag == "root_motion"));
    assert_eq!(
        roll.source_path,
        "SourceFiles/Animations/Polygon/Dodge/A_DodgeRoll_F_RootMotion_Sword.fbx"
    );

    let second = catalogue_animation_sources_from_path(&mut project, &dir, &dir).unwrap();
    assert_eq!(second.source_candidates, 2);
    assert_eq!(second.sources_added, 0);
    assert_eq!(second.sources_updated, 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn materialize_authoring_source_path_extracts_deflated_zip_entry() {
    use std::io::Write;

    let dir = test_temp_dir("animation-source-zip");
    let temp_dir = dir.join("tmp");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let zip_path = dir.join("sources.zip");
    let file = std::fs::File::create(&zip_path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("SourceFiles/Animations/Test/test_clip.fbx", options)
        .unwrap();
    writer.write_all(b"fake-fbx-data").unwrap();
    writer.finish().unwrap();

    let source_path = format!(
        "{}::SourceFiles/Animations/Test/test_clip.fbx",
        zip_path.display()
    );
    let extracted = materialize_authoring_source_path(&source_path, &dir, &temp_dir).unwrap();

    assert_eq!(std::fs::read(extracted).unwrap(), b"fake-fbx-data");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_directory_saves_and_reloads_project() {
    let dir = std::env::temp_dir().join(format!(
        "psxed-ui-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let project_file = dir.join("project.ron");
    std::fs::write(
        &project_file,
        ProjectDocument::starter().to_ron_string().unwrap(),
    )
    .unwrap();

    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();
    assert!(!workspace.is_dirty());
    assert_eq!(workspace.project_root(), dir);
    workspace.save().unwrap();
    assert!(project_file.is_file());

    let loaded = EditorWorkspace::open_directory(&dir).unwrap();
    assert!(!loaded.is_dirty());
    assert_eq!(
        loaded.project().resources.len(),
        workspace.project().resources.len()
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_directory_does_not_resurrect_deleted_starter_characters() {
    let dir = test_temp_dir("no-resurrect-starter-catalogue");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut project = ProjectDocument::starter();
    project.resources.retain(|resource| {
        resource.name == "Crimson Cross Knight"
            || resource.name == "Crimson Cross Knight Player"
            || resource.name == "Crimson Cross Knight Player Set"
            || !STARTER_CHARACTER_PROFILE_NAMES.contains(&resource.name.as_str())
                && !STARTER_CHARACTER_MODEL_NAMES.contains(&resource.name.as_str())
                && !STARTER_ANIMATION_SET_NAMES.contains(&resource.name.as_str())
    });
    project.save_to_path(dir.join("project.ron")).unwrap();

    let workspace = EditorWorkspace::open_directory(&dir).unwrap();

    assert!(!workspace.is_dirty());
    for name in [
        "Obsidian Wraith Enemy",
        "Hooded Wretch Enemy",
        "Crowned Wraith Enemy",
        "Obsidian Wraith",
        "Hooded Wretch",
        "Crowned Wraith",
    ] {
        assert!(
            !workspace
                .project()
                .resources
                .iter()
                .any(|resource| resource.name == name),
            "{name} should stay deleted"
        );
    }
    assert!(!dir.join("assets/models/obsidian_wraith").exists());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_directory_errors_when_project_ron_missing() {
    let dir = std::env::temp_dir().join(format!(
        "psxed-ui-test-missing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let err = match EditorWorkspace::open_directory(&dir) {
        Ok(_) => panic!("expected open_directory to fail on missing project.ron"),
        Err(e) => e,
    };
    assert!(err.contains("project.ron"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn create_and_open_project_validates_non_empty_name() {
    let mut ws = EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    assert!(ws.create_and_open_project("").is_err());
    // "default" is a real existing dir, so this hits the "already exists" branch.
    assert!(ws.create_and_open_project("default").is_err());
}

#[test]
fn create_and_open_project_sets_document_name_and_derived_directory() {
    let mut ws = EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let name = format!(
        "Project Rename {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let target = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(target.clone());
    let _ = std::fs::remove_dir_all(&target);

    ws.create_and_open_project(&name).unwrap();

    assert_eq!(ws.project().name, name);
    assert_eq!(ws.project_root(), target);
    assert!(!ws.is_dirty());
    assert!(!ws.view_2d);
    assert!(
        ws.project().active_scene().world.grid().is_some(),
        "new projects start as grid projects"
    );
    assert!(
        ProjectDocument::load_from_path(ws.project_root().join("project.ron")).is_ok(),
        "the saved new project loads without tripping the legacy grid-world check"
    );
    // The v0.4b starter carries three deliberately two-sided DP City overlays:
    // thin transparent geometry that has to draw from both sides. Everything
    // else must stay front-only, because two-sided defeats backface culling and
    // was measured at -40% render. Allow those three by name so accidental
    // two-sided materials still fail here.
    const TWO_SIDED_STARTER_OVERLAYS: &[&str] = &[
        "DP City / Cable Run (Average)",
        "DP City / Hanging Lattice (Average)",
        "DP City / Guardrail (Cutout)",
    ];
    assert!(ws
        .project()
        .resources
        .iter()
        .all(|resource| match &resource.data {
            ResourceData::Material(material) => {
                TWO_SIDED_STARTER_OVERLAYS.contains(&resource.name.as_str())
                    || (material.face_sidedness == MaterialFaceSidedness::Front
                        && !material.double_sided)
            }
            _ => true,
        }));
    let saved = ProjectDocument::load_from_path(target.join("project.ron")).unwrap();
    assert_eq!(saved.name, ws.project().name);
    let _ = std::fs::remove_dir_all(target);
}

fn action_bar_text_center(
    shapes: &[egui::epaint::ClippedShape],
    label: &str,
) -> Option<egui::Pos2> {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    shapes.iter().find_map(|shape| find(&shape.shape, label))
}

fn click_real_egui_play_control(
    workspace: &mut EditorWorkspace,
    status: EditorPlaytestStatus,
    label: &str,
) {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    let proportional = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .expect("default proportional font family");
    fonts
        .families
        .insert(egui::FontFamily::Name("lucide".into()), proportional);
    ctx.set_fonts(fonts);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 120.0));
    let input = |time, events| egui::RawInput {
        screen_rect: Some(screen),
        time: Some(time),
        events,
        ..egui::RawInput::default()
    };
    let draw = |ctx: &egui::Context, workspace: &mut EditorWorkspace| {
        egui::CentralPanel::default().show(ctx, |ui| {
            workspace.draw_build_play_controls(ui, status, None);
        });
    };
    let output = ctx.run(input(0.0, vec![]), |ctx| draw(ctx, workspace));
    let rendered = crate::icons::label(crate::icons::PLAY, label);
    let point = action_bar_text_center(&output.shapes, &rendered)
        .unwrap_or_else(|| panic!("action bar did not render {label:?}"));
    let _ = ctx.run(
        input(
            1.0 / 60.0,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
        |ctx| draw(ctx, workspace),
    );
    let _ = ctx.run(
        input(
            2.0 / 60.0,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        |ctx| draw(ctx, workspace),
    );
}

#[test]
fn play_and_rebuild_buttons_emit_requests_through_real_egui_input() {
    let mut workspace = EditorWorkspace::with_project(
        test_temp_dir("real-egui-play-control"),
        ProjectDocument::new("Real egui Play control"),
    );

    click_real_egui_play_control(&mut workspace, EditorPlaytestStatus::Idle, "Play");
    assert_eq!(
        workspace.take_playtest_request(),
        Some(EditorPlaytestRequest::Play)
    );

    click_real_egui_play_control(
        &mut workspace,
        EditorPlaytestStatus::Running {
            input_captured: false,
        },
        "Rebuild & Play",
    );
    assert_eq!(
        workspace.take_playtest_request(),
        Some(EditorPlaytestRequest::Rebuild)
    );
}

#[test]
fn save_renames_project_directory_when_project_name_changes() {
    let parent = test_temp_dir("rename-project-parent");
    let source = parent.join("old_project");
    std::fs::create_dir_all(&source).unwrap();
    let project_file = source.join("project.ron");
    std::fs::write(
        &project_file,
        ProjectDocument::new("Old Project").to_ron_string().unwrap(),
    )
    .unwrap();
    let mut workspace = EditorWorkspace::open_directory(&source).unwrap();

    workspace.project.name = "New Project".to_string();
    workspace.mark_dirty();
    workspace.save().unwrap();

    let target = parent.join(psxed_project::project_file_stem("New Project"));
    assert_eq!(workspace.project_root(), target);
    assert!(!source.exists());
    let saved = ProjectDocument::load_from_path(target.join("project.ron")).unwrap();
    assert_eq!(saved.name, "New Project");
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn save_renaming_bsp_starter_creates_a_project_copy() {
    let source = psxed_project::new_project_template_dir();
    let source_project = source.join("project.ron");
    let source_bytes = std::fs::read(&source_project).unwrap();
    let name = format!(
        "BSP Starter Copy {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let target = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(target.clone());
    let _ = std::fs::remove_dir_all(&target);
    let mut workspace = EditorWorkspace::open_directory(&source).unwrap();

    workspace.project.name = name.clone();
    workspace.mark_dirty();
    workspace.save().unwrap();

    assert!(source_project.is_file());
    assert_eq!(std::fs::read(&source_project).unwrap(), source_bytes);
    assert!(target.join("project.ron").is_file());
    assert!(paths_equivalent(workspace.project_root(), &target));
    assert_eq!(
        ProjectDocument::load_from_path(target.join("project.ron"))
            .unwrap()
            .name,
        name
    );
    let _ = std::fs::remove_dir_all(target);
}

#[test]
fn save_rejects_project_rename_collision() {
    let parent = test_temp_dir("rename-project-collision");
    let source = parent.join("old_project");
    let target = parent.join(psxed_project::project_file_stem("New Project"));
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        source.join("project.ron"),
        ProjectDocument::new("Old Project").to_ron_string().unwrap(),
    )
    .unwrap();
    let mut workspace = EditorWorkspace::open_directory(&source).unwrap();

    workspace.project.name = "New Project".to_string();
    workspace.mark_dirty();
    let error = workspace.save().unwrap_err();

    assert!(error.contains("already exists"));
    assert_eq!(workspace.project_root(), source);
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn delete_current_project_refuses_bundled_projects() {
    for project_dir in [
        psxed_project::default_project_dir(),
        psxed_project::new_project_template_dir(),
    ] {
        let mut workspace = EditorWorkspace::open_directory(&project_dir).unwrap();

        let error = workspace.delete_current_project().unwrap_err();

        assert!(error.contains("Bundled starter"), "{error}");
        assert!(project_dir.join("project.ron").is_file());
    }
}

#[test]
fn delete_current_project_removes_directory_and_loads_bsp_starter() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let name = format!(
        "Delete Project {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let target = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(target.clone());
    let _ = std::fs::remove_dir_all(&target);

    workspace.create_and_open_project(&name).unwrap();
    assert!(target.join("project.ron").is_file());

    workspace.delete_current_project().unwrap();

    assert!(!target.exists());
    assert!(paths_equivalent(
        workspace.project_root(),
        &psxed_project::new_project_template_dir()
    ));
    assert!(!workspace.is_dirty());
}

#[test]
fn delete_current_project_refuses_directory_outside_projects_root() {
    let dir = test_temp_dir("delete-outside-project-root");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("project.ron"),
        ProjectDocument::new("External Project")
            .to_ron_string()
            .unwrap(),
    )
    .unwrap();
    let mut workspace = EditorWorkspace::open_directory(&dir).unwrap();

    let error = workspace.delete_current_project().unwrap_err();

    // The guard reports the root it actually enforced, and `projects_dir()`
    // builds that from CARGO_MANIFEST_DIR without normalising the `..`
    // segments, so match the resolved value rather than a literal path.
    assert!(
        error.contains(&psxed_project::projects_dir().display().to_string()),
        "{error}"
    );
    assert!(dir.join("project.ron").is_file());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn create_and_open_project_keeps_old_texture_handles_alive_temporarily() {
    let mut ws = EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let ctx = egui::Context::default();
    let texture_id = ws.project().resources[0].id;
    let handle = ctx.load_texture(
        "project-switch-thumb",
        ColorImage {
            size: [1, 1],
            pixels: vec![Color32::WHITE],
        },
        egui::TextureOptions::NEAREST,
    );
    ws.texture_thumbs.insert(
        texture_id,
        ThumbnailEntry {
            signature: "test.psxt".to_string(),
            handle,
            image: ColorImage {
                size: [1, 1],
                pixels: vec![Color32::WHITE],
            },
            stats: PsxtStats {
                width: 1,
                height: 1,
                depth_bits: 4,
                clut_entries: 16,
                index_zero_transparent: false,
                pixel_bytes: 1,
                clut_bytes: 32,
                file_bytes: 45,
            },
        },
    );
    ws.psoxide_logo_texture = Some(ctx.load_texture(
        "project-switch-logo",
        ColorImage {
            size: [1, 1],
            pixels: vec![Color32::WHITE],
        },
        egui::TextureOptions::NEAREST,
    ));
    ws.model_resource_preview_texture = Some(ctx.load_texture(
        "project-switch-model-preview",
        ColorImage {
            size: [1, 1],
            pixels: vec![Color32::WHITE],
        },
        egui::TextureOptions::NEAREST,
    ));
    ws.animation_viewer_preview_texture = Some(ctx.load_texture(
        "project-switch-animation-preview",
        ColorImage {
            size: [1, 1],
            pixels: vec![Color32::WHITE],
        },
        egui::TextureOptions::NEAREST,
    ));

    let name = format!(
        "texture-retire-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let target = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(target.clone());
    let _ = std::fs::remove_dir_all(&target);

    ws.create_and_open_project(&name).unwrap();

    assert!(ws.texture_thumbs.is_empty());
    assert_eq!(ws.import_retired_textures.len(), 4);
    assert!(ws
        .import_retired_textures
        .iter()
        .all(|(frames, _)| *frames == EGUI_TEXTURE_RETIRE_FRAMES));
    let _ = std::fs::remove_dir_all(target);
}

#[test]
fn switch_project_opens_target_and_retains_old_texture_handles() {
    let source_dir = test_temp_dir("switch-source");
    let target_dir = test_temp_dir("switch-target");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();
    let mut source_project = ProjectDocument::starter();
    source_project.name = "Source".to_string();
    let mut target_project = ProjectDocument::starter();
    target_project.name = "Target".to_string();
    std::fs::write(
        source_dir.join("project.ron"),
        source_project.to_ron_string().unwrap(),
    )
    .unwrap();
    std::fs::write(
        target_dir.join("project.ron"),
        target_project.to_ron_string().unwrap(),
    )
    .unwrap();

    let mut ws = EditorWorkspace::open_directory(&source_dir).unwrap();
    let ctx = egui::Context::default();
    let texture_id = ws.project().resources[0].id;
    let handle = ctx.load_texture(
        "switch-project-thumb",
        ColorImage {
            size: [1, 1],
            pixels: vec![Color32::WHITE],
        },
        egui::TextureOptions::NEAREST,
    );
    ws.texture_thumbs.insert(
        texture_id,
        ThumbnailEntry {
            signature: "test.psxt".to_string(),
            handle,
            image: ColorImage {
                size: [1, 1],
                pixels: vec![Color32::WHITE],
            },
            stats: PsxtStats {
                width: 1,
                height: 1,
                depth_bits: 4,
                clut_entries: 16,
                index_zero_transparent: false,
                pixel_bytes: 1,
                clut_bytes: 32,
                file_bytes: 45,
            },
        },
    );

    ws.switch_project(&target_dir).unwrap();

    assert_eq!(ws.project().name, "Target");
    assert_eq!(ws.project_root(), target_dir);
    assert!(ws.texture_thumbs.is_empty());
    assert_eq!(ws.import_retired_textures.len(), 1);
    assert_eq!(ws.import_retired_textures[0].0, EGUI_TEXTURE_RETIRE_FRAMES);

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}

#[test]
fn viewport_transform_roundtrips_world_and_screen_points() {
    let transform = ViewportTransform::new(
        Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, 200.0)),
        Vec2::new(12.0, -8.0),
        40.0,
    );

    let world = [1.25, -0.5];
    let screen = transform.world_to_screen(world);
    let roundtrip = transform.screen_to_world(screen);

    assert!((roundtrip[0] - world[0]).abs() < 0.001);
    assert!((roundtrip[1] - world[1]).abs() < 0.001);
}

#[test]
fn viewport_hits_rectangles_and_circles() {
    let rect = ViewportHit::rect(NodeId::ROOT, "Rect", [0.0, 0.0], [1.0, 0.5]);
    assert!(rect.contains([0.25, 0.25]));
    assert!(!rect.contains([1.25, 0.25]));

    let circle = ViewportHit::circle(NodeId::ROOT, "Circle", [2.0, 2.0], 0.5);
    assert!(circle.contains([2.25, 2.25]));
    assert!(!circle.contains([2.6, 2.0]));
}

#[test]
fn inspector_changes_are_undoable_as_discrete_steps() {
    let mut workspace =
        EditorWorkspace::with_project(std::env::temp_dir(), ProjectDocument::new("inspector-undo"));
    let root = workspace.project.active_scene().root;
    let original = workspace
        .project
        .active_scene()
        .node(root)
        .unwrap()
        .name
        .clone();

    for name in ["First inspector edit", "Second inspector edit"] {
        let before = workspace.project.clone();
        let history_epoch = workspace.history.epoch();
        workspace
            .project
            .active_scene_mut()
            .node_mut(root)
            .unwrap()
            .name = name.to_string();
        workspace.finish_inspector_undo(before, history_epoch, InspectorUndoInput::default());
    }

    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        "First inspector edit"
    );
    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        original
    );
}

#[test]
fn global_ctrl_or_cmd_z_reverts_an_inspector_change() {
    let mut workspace = EditorWorkspace::with_project(
        std::env::temp_dir(),
        ProjectDocument::new("inspector-shortcut-undo"),
    );
    let root = workspace.project.active_scene().root;
    let original = workspace
        .project
        .active_scene()
        .node(root)
        .unwrap()
        .name
        .clone();
    let before = workspace.project.clone();
    let history_epoch = workspace.history.epoch();
    workspace
        .project
        .active_scene_mut()
        .node_mut(root)
        .unwrap()
        .name = "Changed in Inspector".to_string();
    workspace.finish_inspector_undo(before, history_epoch, InspectorUndoInput::default());

    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            ..egui::RawInput::default()
        },
        |ctx| workspace.handle_global_shortcuts(ctx, EditorPlaytestStatus::Idle),
    );

    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        original
    );
    assert_eq!(workspace.status, "Undo");
}

#[test]
fn physical_ctrl_s_saves_without_moving_the_free_camera() {
    let dir = test_temp_dir("physical-ctrl-save");
    let project = ProjectDocument::new("physical ctrl save");
    save_watch_project(&dir, &project);
    let mut workspace = EditorWorkspace::with_project(dir.clone(), project);
    let root = workspace.project.active_scene().root;
    workspace
        .project
        .active_scene_mut()
        .node_mut(root)
        .unwrap()
        .name = "Saved with physical Ctrl+S".to_string();
    workspace.mark_dirty();
    workspace.camera_rig.mode = ViewportCameraMode::Free;
    workspace.camera_rig.free_initialized = true;
    workspace.camera_rig.free_position = [320, 640, 960];
    let camera_before = workspace.camera_rig.free_position;

    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            modifiers: egui::Modifiers::CTRL,
            events: vec![egui::Event::Key {
                key: egui::Key::S,
                physical_key: Some(egui::Key::S),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
            ..egui::RawInput::default()
        },
        |ctx| {
            workspace.handle_global_shortcuts(ctx, EditorPlaytestStatus::Idle);
            egui::CentralPanel::default().show(ctx, |ui| {
                workspace.update_free_camera_keyboard(ui);
            });
        },
    );

    assert!(!workspace.is_dirty());
    assert_eq!(workspace.camera_rig.free_position, camera_before);
    assert_eq!(
        ProjectDocument::load_from_path(dir.join("project.ron"))
            .unwrap()
            .active_scene()
            .node(root)
            .unwrap()
            .name,
        "Saved with physical Ctrl+S"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn inspector_pointer_drag_coalesces_to_one_undo_step() {
    let mut workspace = EditorWorkspace::with_project(
        std::env::temp_dir(),
        ProjectDocument::new("inspector-drag-undo"),
    );
    let root = workspace.project.active_scene().root;
    let original = workspace
        .project
        .active_scene()
        .node(root)
        .unwrap()
        .name
        .clone();
    let drag = InspectorUndoInput {
        pointer_down: true,
        ..InspectorUndoInput::default()
    };

    for name in ["Drag frame one", "Drag frame two", "Drag frame three"] {
        let before = workspace.project.clone();
        let history_epoch = workspace.history.epoch();
        workspace
            .project
            .active_scene_mut()
            .node_mut(root)
            .unwrap()
            .name = name.to_string();
        workspace.finish_inspector_undo(before, history_epoch, drag);
    }
    workspace.prepare_inspector_undo(InspectorUndoInput::default());

    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        original
    );
    workspace.do_redo();
    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        "Drag frame three"
    );
}

#[test]
fn inspector_respects_explicit_history_and_deliberate_clears() {
    let mut workspace = EditorWorkspace::with_project(
        std::env::temp_dir(),
        ProjectDocument::new("inspector-explicit-history"),
    );
    let root = workspace.project.active_scene().root;
    let original = workspace
        .project
        .active_scene()
        .node(root)
        .unwrap()
        .name
        .clone();

    let before = workspace.project.clone();
    let history_epoch = workspace.history.epoch();
    workspace.push_undo();
    workspace
        .project
        .active_scene_mut()
        .node_mut(root)
        .unwrap()
        .name = "Explicit".to_string();
    workspace.finish_inspector_undo(before, history_epoch, InspectorUndoInput::default());
    workspace.do_undo();
    assert_eq!(
        workspace.project.active_scene().node(root).unwrap().name,
        original
    );
    workspace.do_undo();
    assert_eq!(workspace.status, "Nothing to undo");

    let before = workspace.project.clone();
    let history_epoch = workspace.history.epoch();
    workspace.history.clear();
    workspace
        .project
        .active_scene_mut()
        .node_mut(root)
        .unwrap()
        .name = "Filesystem edit".to_string();
    workspace.finish_inspector_undo(before, history_epoch, InspectorUndoInput::default());
    workspace.do_undo();
    assert_eq!(workspace.status, "Nothing to undo");
}

/// A genuinely fresh New Project must already carry the production content
/// catalogue: Aletha, the Light Enemy, the Heavy Enemy and the saved material
/// library, with every reference between them resolving. Its asset files are
/// shared with the base project through the `assets` link.
#[test]
fn new_project_starts_with_verified_character_and_material_content() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let name = format!(
        "Starter Combat Sync {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let project_dir = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(project_dir.clone());
    let _ = std::fs::remove_dir_all(&project_dir);

    workspace.create_and_open_project(&name).unwrap();

    let project = workspace.project();
    let resource_by_name = |name: &str, wants: fn(&ResourceData) -> bool| {
        project
            .resources
            .iter()
            .find(|resource| resource.name == name && wants(&resource.data))
            .unwrap_or_else(|| panic!("synced project is missing '{name}'"))
    };

    // Full referential integrity: after syncing into a genuinely fresh
    // project every cook-relevant resource reference must resolve. Only
    // `AnimationClip::source` / `AnimationSource` provenance is exempt: the
    // catalogue deliberately does not carry authoring sources, and the cook
    // never reads them.
    for resource in &project.resources {
        let check = |label: &str, reference: Option<psxed_project::ResourceId>| {
            if let Some(id) = reference {
                assert!(
                    project.resource(id).is_some(),
                    "synced '{}' has a dangling {label} reference #{}",
                    resource.name,
                    id.raw()
                );
            }
        };
        match &resource.data {
            ResourceData::Character(character) => {
                check("model", character.model);
                check("material", character.material);
                check("animation set", character.animation_set);
            }
            ResourceData::Model(model) => check("skeleton", model.skeleton),
            ResourceData::Weapon(weapon) => check("weapon model", weapon.model),
            ResourceData::AnimationClip(clip) => {
                check("skeleton", clip.skeleton);
                check("target model", clip.target_model);
            }
            ResourceData::AnimationSet(set) => {
                check("skeleton", set.skeleton);
                for binding in &set.action_clips {
                    check("action clip", Some(binding.clip));
                }
                for clip in &set.clips {
                    check("library clip", Some(*clip));
                }
            }
            _ => {}
        }
    }

    let aletha = resource_by_name("Aletha", |data| matches!(data, ResourceData::Character(_)));
    let player = project
        .active_scene()
        .nodes()
        .iter()
        .find(|node| {
            matches!(
                node.kind,
                NodeKind::CharacterController { player: true, .. }
            )
        })
        .expect("fresh project has a player controller");
    assert!(matches!(
        player.kind,
        NodeKind::CharacterController {
            character: Some(character),
            ..
        } if character == aletha.id
    ));
    let _ = std::fs::remove_dir_all(project_dir);
}

/// Drive one Inspector ComboBox through real egui: click the closed combo
/// (whose selected text is `closed`), assert the popup it opens offers every
/// label in `expected`, then click `option`.
///
/// Both label lookups demand a unique match. The Inspector paints plenty of
/// same-named text, and a helper that silently took the first hit could drive
/// a control in another section and still report success.
fn choose_inspector_combo_option(
    workspace: &mut EditorWorkspace,
    closed: &str,
    expected: &[&str],
    option: &str,
) {
    use super::egui_driver::{
        press_release, real_egui_workspace_ctx, real_egui_workspace_frame, text_shape_centers,
    };

    let (ctx, viewport) = real_egui_workspace_ctx("inspector-combo");
    let closed_frame = real_egui_workspace_frame(&ctx, workspace, &viewport, 0.0, vec![]);
    let anchors = text_shape_centers(&closed_frame.shapes, closed);
    assert_eq!(
        anchors.len(),
        1,
        "the closed combo {closed:?} must be visible exactly once, saw {anchors:?}"
    );
    let (press, release) = press_release(anchors[0]);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 1.0 / 60.0, press);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 2.0 / 60.0, release);

    let open_frame = real_egui_workspace_frame(&ctx, workspace, &viewport, 3.0 / 60.0, vec![]);
    for label in expected {
        assert!(
            !text_shape_centers(&open_frame.shapes, label).is_empty(),
            "the open {closed:?} combo omitted {label:?}"
        );
    }
    let choices = text_shape_centers(&open_frame.shapes, option);
    assert_eq!(
        choices.len(),
        1,
        "the open {closed:?} combo must offer {option:?} exactly once, saw {choices:?}"
    );
    let (press, release) = press_release(choices[0]);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 4.0 / 60.0, press);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 5.0 / 60.0, release);
    // One quiet frame so the Inspector undo transaction closes.
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 6.0 / 60.0, vec![]);
}

/// The native-Inspector route to the two souls entities the author has to
/// switch by hand on a fresh project: the lift Door and the Checkpoint.
///
/// `souls_slice_project_is_authored_through_production_commands` writes both
/// node kinds straight into the document, and the souls gate proves the
/// cooked door opens and the cooked checkpoint respawns. Neither proves the
/// Inspector still OFFERS the switches: both live inside a `ComboBox` popup,
/// so a dropped or renamed entry would leave every gate green while the
/// authoring step became impossible at the window. Both combos are driven
/// here through real egui, on a project created the way the author creates
/// one.
#[test]
fn fresh_project_inspector_switches_logic_to_door_and_interactable_to_checkpoint() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let name = format!(
        "Inspector Kind Switch {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let project_dir = psxed_project::projects_dir().join(psxed_project::project_file_stem(&name));
    // Removes the created project even if an assertion below panics.
    let _scratch = ScratchProjectDir::new(project_dir.clone());
    workspace.create_and_open_project(&name).unwrap();
    let root = workspace.project().active_scene().root;

    // A Logic node from the Place lane starts as a Trigger Volume, which is
    // also the node's default NAME; rename it first so the combo's selected
    // text is the only "Trigger Volume" on screen. The name is not what this
    // test is about, so it is written the way the souls authoring test does.
    workspace.set_active_tool_cycle_value((ViewTool::Place, Some(PlaceKind::Logic)));
    place_at(&mut workspace, [1024.0, 1024.0]);
    let door_node = workspace.selected_node_id();
    workspace.push_undo();
    workspace
        .project
        .active_scene_mut()
        .node_mut(door_node)
        .expect("placed Logic node")
        .name = "Lift Door".to_string();
    workspace.mark_dirty();

    choose_inspector_combo_option(
        &mut workspace,
        "Trigger Volume",
        &["Trigger Volume", "Relay", "Multisource", "Door"],
        "Door",
    );
    let NodeKind::Logic { kind, .. } = &workspace
        .project()
        .active_scene()
        .node(door_node)
        .expect("Logic node")
        .kind
    else {
        panic!("the placed node must still be a Logic node");
    };
    assert_eq!(
        *kind,
        psxed_project::LogicNodeKind::Door {
            box_prop: String::new(),
            start_open: false,
            open_offset: psxed_project::default_brush_door_open_offset(),
            travel_ticks: psxed_project::default_brush_door_travel_ticks(),
        },
        "picking Door must seat the shared brush-door defaults"
    );
    assert!(workspace.is_dirty());
    workspace.do_undo();
    assert!(
        matches!(
            workspace
                .project()
                .active_scene()
                .node(door_node)
                .expect("Logic node")
                .kind,
            NodeKind::Logic {
                kind: psxed_project::LogicNodeKind::TriggerVolume { .. },
                ..
            }
        ),
        "the kind switch must cost exactly one undo step"
    );
    workspace.do_redo();

    // The checkpoint half: an Entity plus the Interactable component, both
    // through the scene tree's own commands, then the Inspector kind switch.
    workspace.replace_node_selection(root);
    workspace.add_child(NodeKind::Entity, "Entity");
    let checkpoint_entity = workspace.selected_node_id();
    let (label, template) = crate::scene_tree::component_templates_for_host(&NodeKind::Entity)
        .into_iter()
        .find(|(label, _)| *label == "Interactable")
        .expect("the Entity component menu offers an Interactable");
    workspace.add_component_to_host(checkpoint_entity, label, template);
    let interactable = workspace.selected_node_id();

    choose_inspector_combo_option(
        &mut workspace,
        "Message",
        &["Message", "Checkpoint"],
        "Checkpoint",
    );
    let NodeKind::Interactable { kind, prompt, .. } = &workspace
        .project()
        .active_scene()
        .node(interactable)
        .expect("Interactable component")
        .kind
    else {
        panic!("the added component must still be an Interactable");
    };
    assert!(
        matches!(kind, psxed_project::InteractableKind::Checkpoint { .. }),
        "picking Checkpoint must switch the interactable kind"
    );
    assert_eq!(
        prompt, "SYNCHRONIZE",
        "switching to Checkpoint must retire the default message prompt"
    );

    let _ = std::fs::remove_dir_all(project_dir);
}

/// A project file that still holds a retired grid-world node as raw RON.
fn legacy_grid_world_project_dir(label: &str) -> PathBuf {
    let dir = test_temp_dir(label);
    std::fs::create_dir_all(&dir).unwrap();
    let mut project = ProjectDocument::new(label);
    let root = project.active_scene().root;
    project
        .active_scene_mut()
        .add_node(root, "Legacy Room", NodeKind::Node3D);
    let ron =
        project
            .to_ron_string()
            .unwrap()
            .replacen("kind: Node3D", "kind: Section(grid: ())", 1);
    assert!(ron.contains("kind: Section("), "scaffold node rewritten");
    std::fs::write(dir.join("project.ron"), ron).unwrap();
    dir
}

#[test]
fn opening_a_project_with_a_legacy_grid_world_fails_with_the_plain_message() {
    let dir = legacy_grid_world_project_dir("legacy-grid-open");
    let before = std::fs::read_to_string(dir.join("project.ron")).unwrap();

    let error = match EditorWorkspace::open_directory(&dir) {
        Ok(_) => panic!("a project holding a Section node must not open"),
        Err(error) => error,
    };

    assert!(error.contains("legacy grid-world node(s)"), "{error}");
    assert!(error.contains("Grid worlds were removed"), "{error}");
    assert_eq!(
        std::fs::read_to_string(dir.join("project.ron")).unwrap(),
        before,
        "a refused load must not touch the file"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reloading_a_project_that_gained_a_legacy_grid_world_reports_it_in_the_status() {
    let dir = legacy_grid_world_project_dir("legacy-grid-reload");
    let mut workspace =
        EditorWorkspace::with_project(dir.clone(), ProjectDocument::new("legacy-grid-reload"));

    workspace.reload();

    assert!(
        workspace.status_text().starts_with("Reload failed: "),
        "{}",
        workspace.status_text()
    );
    assert!(
        workspace
            .status_text()
            .contains("legacy grid-world node(s)"),
        "{}",
        workspace.status_text()
    );
    let _ = std::fs::remove_dir_all(dir);
}
