use super::*;

#[test]
fn action_bar_height_stays_compact_for_build_output() {
    assert_eq!(
        action_bar_height_for_status("Ready"),
        ACTION_BAR_COMPACT_HEIGHT
    );
    assert_eq!(
            action_bar_height_for_status(
                "Embedded Play failed while cooking assets: playtest validation failed: No player source. Place one Player Spawn, or select a Character Controller and enable Player controlled."
            ),
            ACTION_BAR_COMPACT_HEIGHT
        );
    assert_eq!(
        action_bar_height_for_status("First line\nSecond line"),
        ACTION_BAR_COMPACT_HEIGHT
    );
}

#[test]
fn long_build_output_does_not_take_space_from_the_editor_viewport() {
    fn viewport_top(status: &str) -> f32 {
        let mut workspace = EditorWorkspace::with_project(
            test_temp_dir("fixed-action-bar"),
            ProjectDocument::new("fixed action bar"),
        );
        workspace.status = status.to_string();
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
        let mut top = 0.0;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                ..egui::RawInput::default()
            },
            |ctx| {
                workspace.draw_action_bar(ctx, EditorPlaytestStatus::Building, None);
                egui::CentralPanel::default().show(ctx, |ui| {
                    top = ui.max_rect().top();
                });
            },
        );
        top
    }

    let ready_top = viewport_top("Ready");
    let build_top = viewport_top(
        "Embedded Play failed while cooking assets:\nthis deliberately long build diagnostic belongs in the bottom Console and must never resize the top action bar or the editor viewport.",
    );
    assert_eq!(build_top, ready_top);
    assert!(build_top <= ACTION_BAR_COMPACT_HEIGHT + 10.0);
}

/// End-to-end of the multi-scene UX on the editor side: create a
/// scene, switch to it, confirm edits land only in the selected
/// scene, then delete it and confirm the active index clamps and the
/// scene list never empties.
#[test]
fn ui_scene_create_switch_edit_isolated_delete_clamps() {
    let mut project = ProjectDocument::new("ui-scene-crud");
    project.normalize_loaded();
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("ui-scene-crud"), project);

    // One default scene to start; index points at it.
    assert_eq!(workspace.project.ui_scenes.len(), 1);
    assert_eq!(workspace.current_ui_scene_index(), 0);
    let first_id = workspace.current_ui_scene().unwrap().id;
    let first_node_count = workspace.current_ui_scene().unwrap().nodes().len();

    // Create -> the new scene becomes active and selection resets to
    // its root canvas (no stale node id from scene 0).
    workspace.add_ui_scene_action();
    assert_eq!(workspace.project.ui_scenes.len(), 2);
    assert_eq!(workspace.current_ui_scene_index(), 1);
    let second_id = workspace.current_ui_scene().unwrap().id;
    assert_ne!(first_id, second_id, "new scene gets a fresh stable id");
    let second_root = workspace.current_ui_scene().unwrap().root;
    assert_eq!(workspace.selection.selected_ui_node, second_root);

    // Edit isolation: add a node into the active (second) scene.
    workspace.add_ui_child(
        UiNodeKind::Rect {
            rect: UiRect::new(8, 8, 32, 16),
            color: [10, 20, 30],
            gradient: None,
            transparent: false,
            shape: None,
        },
        "Probe",
    );
    let added = workspace.selection.selected_ui_node;
    assert!(workspace.current_ui_scene().unwrap().node(added).is_some());
    let second_node_count = workspace.current_ui_scene().unwrap().nodes().len();

    // Switch back to scene 0: its structure is untouched, and the
    // selection snaps to scene 0's root rather than carrying the
    // second scene's node over. Node ids are per-scene, so isolation
    // is asserted structurally (count + the absence of "Probe")
    // rather than by id, which can legitimately repeat across scenes.
    workspace.switch_ui_scene(0);
    assert_eq!(workspace.current_ui_scene_index(), 0);
    let first_scene = workspace.current_ui_scene().unwrap();
    assert_eq!(first_scene.id, first_id);
    assert_eq!(
        first_scene.nodes().len(),
        first_node_count,
        "edit must not change the other scene's node count"
    );
    assert!(
        first_scene.nodes().iter().all(|node| node.name != "Probe"),
        "edit must not leak into the other scene"
    );
    assert_eq!(
        workspace.selection.selected_ui_node, first_scene.root,
        "selection resets on scene switch"
    );

    // The second scene still holds its extra node.
    assert_eq!(
        workspace.project.ui_scene(second_id).unwrap().nodes().len(),
        second_node_count
    );

    // Point the active index at the last scene, then delete it:
    // the index must clamp back into range and the list stays
    // non-empty.
    workspace.switch_ui_scene(1);
    assert_eq!(workspace.current_ui_scene_index(), 1);
    workspace.delete_ui_scene_action(1);
    assert_eq!(workspace.project.ui_scenes.len(), 1);
    assert_eq!(
        workspace.current_ui_scene_index(),
        0,
        "active index clamps after deleting the last scene"
    );
    assert_eq!(workspace.current_ui_scene().unwrap().id, first_id);

    // Deleting the final remaining scene is forbidden (never empty).
    workspace.delete_ui_scene_action(0);
    assert_eq!(
        workspace.project.ui_scenes.len(),
        1,
        "the last UI scene cannot be deleted"
    );
}

#[test]
fn button_and_slider_are_addable_and_options_crud_round_trips() {
    // Both new interactive kinds appear in the add-node menu.
    let addable = default_addable_ui_kinds();
    assert!(
        addable
            .iter()
            .any(|(label, kind)| *label == "Button" && matches!(kind, UiNodeKind::Button { .. })),
        "Button must be addable"
    );
    assert!(
        addable
            .iter()
            .any(|(label, kind)| *label == "Slider" && matches!(kind, UiNodeKind::Slider { .. })),
        "Slider must be addable"
    );

    let mut project = ProjectDocument::new("ui-button-slider");
    project.normalize_loaded();
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("ui-button-slider"), project);

    // Options CRUD: add two, remove the first, ids stay distinct and
    // a slider can bind to a surviving option.
    let first = workspace.project.add_option("Volume");
    let second = workspace.project.add_option("Brightness");
    assert_ne!(first, second);
    assert_eq!(workspace.project.options.len(), 2);
    assert!(workspace.project.remove_option(0));
    assert_eq!(workspace.project.options.len(), 1);
    assert_eq!(workspace.project.options[0].id, second);
    // A newly added option after a removal must not collide with a
    // surviving id (so a slider bound to `second` is never shadowed).
    let third = workspace.project.add_option("Contrast");
    assert_ne!(third, second);

    // Add a Slider bound to the surviving option and confirm the
    // authored binding round-trips through the scene tree.
    workspace.add_ui_child(
        UiNodeKind::Slider {
            rect: UiRect::new(8, 8, 96, 8),
            option: second,
            track: [11, 12, 13],
            track_gradient: None,
            fill: [21, 22, 23],
            fill_gradient: None,
            knob: [31, 32, 33],
            knob_gradient: None,
            sfx: UiSfxBindings::default(),
        },
        "Brightness",
    );
    let added = workspace.selection.selected_ui_node;
    let node = workspace
        .current_ui_scene()
        .unwrap()
        .node(added)
        .expect("slider node added");
    match &node.kind {
        UiNodeKind::Slider { option, knob, .. } => {
            assert_eq!(*option, second);
            assert_eq!(*knob, [31, 32, 33]);
        }
        other => panic!("expected slider, got {other:?}"),
    }
    // The bound option still resolves to a name in the project.
    assert_eq!(
        workspace.project.option(second).map(|o| o.name.as_str()),
        Some("Brightness")
    );
}
