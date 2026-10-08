use super::*;

#[test]
fn dragging_selected_node_moves_it_in_xz_space() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let spawn = starter_player_entity(workspace.project.active_scene()).id;
    let snap = f32::from(workspace.snap_units.max(1));
    let start = workspace
        .project
        .active_scene()
        .node(spawn)
        .unwrap()
        .transform
        .translation;

    workspace.selection.selected_node = spawn;
    workspace.drag_selected_node(Vec2::new(
        workspace.viewport_zoom * snap,
        -workspace.viewport_zoom * snap,
    ));

    // Right is +X and up is -Z: the Top view is the 3D view (yaw 0 looks
    // down -Z) seen from above.
    let node = workspace.project.active_scene().node(spawn).unwrap();
    assert!((node.transform.translation[0] - (start[0] + snap)).abs() < 0.001);
    assert!((node.transform.translation[2] - (start[2] - snap)).abs() < 0.001);
    assert!(workspace.is_dirty());
}

#[test]
fn light_transform_normalises_hidden_rotation_and_scale_and_keeps_y_exact() {
    let mut transform = psxed_project::Transform3 {
        translation: [10.0, 37.0, 20.0],
        rotation_degrees: [10.0, 90.0, 5.0],
        scale: [2.0, 3.0, 4.0],
    };

    assert!(normalise_light_transform(&mut transform));

    // World-unit lights keep their authored height.
    assert_eq!(transform.translation, [10.0, 37.0, 20.0]);
    assert_eq!(transform.rotation_degrees, [0.0, 0.0, 0.0]);
    assert_eq!(transform.scale, [1.0, 1.0, 1.0]);
    assert!(!normalise_light_transform(&mut transform));
}

#[test]
fn rotate_selected_yaw_ignores_light_nodes() {
    let mut project = ProjectDocument::new("light-rotate");
    let light = project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Point Light",
        NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("light-rotate"), project);
    workspace.selection.selected_node = light;

    workspace.rotate_selected_yaw_90();

    let node = workspace.project.active_scene().node(light).unwrap();
    assert_eq!(node.transform.rotation_degrees, [0.0, 0.0, 0.0]);
    assert!(!workspace.is_dirty());
}

#[test]
fn rotate_selected_yaw_rotates_entity_hosts() {
    let mut project = ProjectDocument::new("entity-rotate");
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Prop", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("entity-rotate"), project);
    workspace.selection.selected_node = entity;

    workspace.rotate_selected_yaw_90();

    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(node.transform.rotation_degrees, [0.0, 90.0, 0.0]);
    assert!(workspace.is_dirty());
}

#[test]
fn duplicating_point_of_interest_component_copies_its_complete_host() {
    let mut project = ProjectDocument::new("poi-duplicate");
    let host =
        project
            .active_scene_mut()
            .add_node(NodeId::ROOT, "Point of Interest", NodeKind::Entity);
    {
        let host = project.active_scene_mut().node_mut(host).unwrap();
        host.transform.translation = [128.0, 64.0, 256.0];
    }
    let component = project.active_scene_mut().add_node(
        host,
        "Point of Interest",
        NodeKind::PointOfInterest {
            pages: vec!["FIRST PAGE".to_string(), "SECOND PAGE".to_string()],
            pages_it: vec!["PRIMA PAGINA".to_string()],
            prompt: "READ".to_string(),
            radius: 640,
            marker_height: 160,
            repeatable: false,
            persistence_id: "authored-poi-id".to_string(),
            reward: None,
            enabled: true,
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("poi-duplicate"), project);

    // Placement leaves the component selected, while viewport picking selects
    // its host. Both routes must duplicate the same complete authored object.
    workspace.replace_node_selection(component);
    workspace.duplicate_current_selection();

    let copied_host_id = workspace.selection.selected_node;
    let scene = workspace.project.active_scene();
    let source_host = scene.node(host).expect("source host remains");
    let copied_host = scene.node(copied_host_id).expect("copied host");
    assert_ne!(copied_host_id, host);
    assert_eq!(copied_host.name, "Point of Interest Copy");
    assert!(matches!(copied_host.kind, NodeKind::Entity));
    assert_eq!(copied_host.parent, source_host.parent);
    assert_eq!(copied_host.transform, source_host.transform);
    assert_eq!(copied_host.children.len(), 1);

    let copied_component = scene
        .node(copied_host.children[0])
        .expect("copied POI component");
    assert_eq!(copied_component.parent, Some(copied_host_id));
    let NodeKind::PointOfInterest {
        pages,
        pages_it,
        prompt,
        radius,
        marker_height,
        repeatable,
        persistence_id,
        reward,
        enabled,
        ..
    } = &copied_component.kind
    else {
        panic!("copied child must remain a POI component");
    };
    assert_eq!(pages, &["FIRST PAGE", "SECOND PAGE"]);
    assert_eq!(pages_it, &["PRIMA PAGINA"]);
    assert_eq!(prompt, "READ");
    assert_eq!(*radius, 640);
    assert_eq!(*marker_height, 160);
    assert!(!repeatable);
    assert!(persistence_id.is_empty());
    assert!(reward.is_none());
    assert!(*enabled);
}

#[test]
fn node_transform_inspector_hides_unused_transform_fields() {
    assert_eq!(
        node_transform_inspector(&NodeKind::Node),
        NodeTransformInspector::Hidden
    );
    assert_eq!(
        node_transform_inspector(&NodeKind::Group),
        NodeTransformInspector::Hidden
    );
    assert_eq!(
        node_transform_inspector(&NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        }),
        NodeTransformInspector::PositionOnly
    );
    assert_eq!(
        node_transform_inspector(&NodeKind::SpawnPoint {
            player: false,
            character: None,
        }),
        NodeTransformInspector::PositionYaw
    );
    assert_eq!(
        node_transform_inspector(&NodeKind::ImageProp {
            material: None,
            width: 256,
            height: 256,
            cylindrical_billboard: false,
            collision_enabled: false,
            collision_size: [256; 3],
            destructible: None,
        }),
        NodeTransformInspector::PositionFullRotation
    );
    assert_eq!(
        node_transform_inspector(&NodeKind::Node3D),
        NodeTransformInspector::FullTransform
    );
}

#[test]
fn tree_row_drop_zone_uses_top_band_without_extra_layout() {
    let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(200.0, 24.0));

    assert_eq!(
        tree_row_drop_zone(rect, Some(Pos2::new(40.0, 22.0)), true),
        TreeRowDropZone::Before
    );
    assert_eq!(
        tree_row_drop_zone(rect, Some(Pos2::new(40.0, 32.0)), true),
        TreeRowDropZone::Inside
    );
    assert_eq!(
        tree_row_drop_zone(rect, Some(Pos2::new(40.0, 22.0)), false),
        TreeRowDropZone::Inside
    );
}

#[test]
fn tree_drag_autoscroll_delta_tracks_edge_bands() {
    let viewport = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(240.0, 200.0));

    assert!(tree_drag_autoscroll_delta(viewport, Pos2::new(120.0, 106.0)) > 0.0);
    assert_eq!(
        tree_drag_autoscroll_delta(viewport, Pos2::new(120.0, 200.0)),
        0.0
    );
    assert!(tree_drag_autoscroll_delta(viewport, Pos2::new(120.0, 294.0)) < 0.0);
    assert_eq!(
        tree_drag_autoscroll_delta(viewport, Pos2::new(-4.0, 106.0)),
        0.0
    );
}

#[test]
fn scene_tree_select_clears_inspector_shadow_selection() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let scene = workspace.project.active_scene();
    let spawn = starter_player_entity(scene).id;
    let resource = workspace
        .project
        .resources
        .first()
        .expect("starter project has resources")
        .id;
    workspace
        .project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, 0, 0],
            [128, 128, 128],
        ));

    workspace.selection.selected_node = NodeId::ROOT;
    workspace.selection.selected_resource = Some(resource);
    workspace.replace_brush_selection(workspace.project.active_scene().brushes.len() - 1, None);

    workspace.apply_tree_action(
        TreeAction::Select {
            id: spawn,
            modifiers: egui::Modifiers::NONE,
        },
        &[NodeId::ROOT, spawn],
    );

    assert_eq!(workspace.selection.selected_node, spawn);
    assert_eq!(workspace.selected_brush, None);
    assert_eq!(workspace.selection.selected_resource, None);
}

#[test]
fn scene_tree_ctrl_toggles_node_multi_selection() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let order = workspace.scene_node_order();
    let ids: Vec<NodeId> = order
        .iter()
        .copied()
        .filter(|id| *id != NodeId::ROOT)
        .take(2)
        .collect();
    assert!(ids.len() >= 2, "starter scene has at least two nodes");

    let mut ctrl = egui::Modifiers::NONE;
    ctrl.ctrl = true;
    workspace.apply_tree_action(
        TreeAction::Select {
            id: ids[0],
            modifiers: egui::Modifiers::NONE,
        },
        &order,
    );
    workspace.apply_tree_action(
        TreeAction::Select {
            id: ids[1],
            modifiers: ctrl,
        },
        &order,
    );

    assert!(workspace.selection.selected_nodes.contains(&ids[0]));
    assert!(workspace.selection.selected_nodes.contains(&ids[1]));
    assert_eq!(workspace.selection.selected_nodes.len(), 2);

    workspace.apply_tree_action(
        TreeAction::Select {
            id: ids[0],
            modifiers: ctrl,
        },
        &order,
    );
    assert!(!workspace.selection.selected_nodes.contains(&ids[0]));
    assert!(workspace.selection.selected_nodes.contains(&ids[1]));
    assert_eq!(workspace.selection.selected_node, ids[1]);
}

#[test]
fn scene_tree_shift_selects_visible_node_range() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let order = workspace.scene_node_order();
    let ids: Vec<NodeId> = order
        .iter()
        .copied()
        .filter(|id| *id != NodeId::ROOT)
        .take(3)
        .collect();
    assert!(ids.len() >= 3, "starter scene has at least three nodes");

    let mut shift = egui::Modifiers::NONE;
    shift.shift = true;
    workspace.apply_tree_action(
        TreeAction::Select {
            id: ids[0],
            modifiers: egui::Modifiers::NONE,
        },
        &order,
    );
    workspace.apply_tree_action(
        TreeAction::Select {
            id: ids[2],
            modifiers: shift,
        },
        &order,
    );

    for id in &ids {
        assert!(workspace.selection.selected_nodes.contains(id));
    }
    assert_eq!(workspace.selection.selected_nodes.len(), 3);
}

#[test]
fn scene_tree_dragging_selected_group_moves_all_into_folder() {
    let mut project = ProjectDocument::new("multi-drag-folder");
    let scene = project.active_scene_mut();
    let folder = scene.add_node(NodeId::ROOT, "Folder", NodeKind::Node);
    let a = scene.add_node(NodeId::ROOT, "A", NodeKind::Entity);
    let b = scene.add_node(NodeId::ROOT, "B", NodeKind::Entity);
    let c = scene.add_node(NodeId::ROOT, "C", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("multi-drag-folder"), project);
    workspace.selection.selected_node = a;
    workspace.selection.selected_nodes = [a, b].into_iter().collect();

    let order = workspace.scene_node_order();
    workspace.apply_tree_action(
        TreeAction::Reparent {
            source: a,
            target_parent: folder,
            position: 0,
        },
        &order,
    );

    let scene = workspace.project.active_scene();
    assert_eq!(scene.node(folder).unwrap().children, vec![a, b]);
    assert_eq!(scene.node(NodeId::ROOT).unwrap().children, vec![folder, c]);
    assert_eq!(scene.node(a).unwrap().parent, Some(folder));
    assert_eq!(scene.node(b).unwrap().parent, Some(folder));
    assert_eq!(workspace.selection.selected_node, a);
    assert_eq!(workspace.selection.selected_nodes.len(), 2);
    assert!(workspace.selection.selected_nodes.contains(&a));
    assert!(workspace.selection.selected_nodes.contains(&b));
}

#[test]
fn scene_tree_dragging_selected_siblings_reorders_as_group() {
    let mut project = ProjectDocument::new("multi-drag-reorder");
    let scene = project.active_scene_mut();
    let a = scene.add_node(NodeId::ROOT, "A", NodeKind::Entity);
    let b = scene.add_node(NodeId::ROOT, "B", NodeKind::Entity);
    let c = scene.add_node(NodeId::ROOT, "C", NodeKind::Entity);
    let d = scene.add_node(NodeId::ROOT, "D", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("multi-drag-reorder"), project);
    workspace.selection.selected_node = b;
    workspace.selection.selected_nodes = [b, c].into_iter().collect();

    let order = workspace.scene_node_order();
    workspace.apply_tree_action(
        TreeAction::Reparent {
            source: b,
            target_parent: NodeId::ROOT,
            position: 4,
        },
        &order,
    );

    let scene = workspace.project.active_scene();
    assert_eq!(scene.node(NodeId::ROOT).unwrap().children, vec![a, d, b, c]);
    assert_eq!(workspace.selection.selected_node, b);
    assert_eq!(workspace.selection.selected_nodes.len(), 2);
    assert!(workspace.selection.selected_nodes.contains(&b));
    assert!(workspace.selection.selected_nodes.contains(&c));
}

#[test]
fn scene_tree_toggle_expanded_hides_descendants_from_display_rows() {
    let mut project = ProjectDocument::new("tree-collapse");
    let parent = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Parent", NodeKind::Node);
    let child = project
        .active_scene_mut()
        .add_node(parent, "Child", NodeKind::Node3D);
    let grandchild = project
        .active_scene_mut()
        .add_node(child, "Grandchild", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("tree-collapse"), project);

    workspace.apply_tree_action(TreeAction::ToggleExpanded(parent), &[]);

    let rows = workspace.project.active_scene().hierarchy_rows();
    let visible = scene_tree_display_rows(&rows, "", &workspace.collapsed_scene_nodes)
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    assert!(visible.contains(&parent));
    assert!(!visible.contains(&child));
    assert!(!visible.contains(&grandchild));

    workspace.apply_tree_action(TreeAction::ToggleExpanded(parent), &[]);
    let rows = workspace.project.active_scene().hierarchy_rows();
    let visible = scene_tree_display_rows(&rows, "", &workspace.collapsed_scene_nodes)
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    assert!(visible.contains(&child));
    assert!(visible.contains(&grandchild));
}

#[test]
fn scene_tree_toggle_visibility_hides_entity_bounds() {
    let mut project = ProjectDocument::new("tree-visibility");
    let actor = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Actor", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("tree-visibility"), project);

    assert!(workspace
        .collect_entity_bounds()
        .iter()
        .any(|bound| bound.node == actor));

    workspace.apply_tree_action(TreeAction::ToggleVisibility(actor), &[]);

    assert!(workspace.hidden_scene_nodes.contains(&actor));
    assert!(!workspace
        .collect_entity_bounds()
        .iter()
        .any(|bound| bound.node == actor));

    workspace.apply_tree_action(TreeAction::ToggleVisibility(actor), &[]);
    assert!(!workspace.hidden_scene_nodes.contains(&actor));
    assert!(workspace
        .collect_entity_bounds()
        .iter()
        .any(|bound| bound.node == actor));
}

#[test]
fn ui_tree_visibility_is_scene_local_and_hides_canvas_hits() {
    let mut project = ProjectDocument::new("ui-tree-visibility");
    project.ui_scenes = vec![UiScene::empty_canvas("Main", UiSceneId::FIRST)];
    let first_scene_id = project.ui_scenes[0].id;
    let group = project.ui_scenes[0].add_node(
        UiNodeId::ROOT,
        "Panel",
        UiNodeKind::Group {
            rect: UiRect::new(0, 0, 96, 64),
        },
    );
    let label = project.ui_scenes[0].add_node(
        group,
        "Label",
        UiNodeKind::Group {
            rect: UiRect::new(4, 4, 48, 16),
        },
    );
    let second_scene_id = project.add_ui_scene("Settings");
    let second_group = project.ui_scene_mut(second_scene_id).unwrap().add_node(
        UiNodeId::ROOT,
        "Panel",
        UiNodeKind::Group {
            rect: UiRect::new(0, 0, 96, 64),
        },
    );
    assert_eq!(group.raw(), second_group.raw());

    let mut workspace = EditorWorkspace::with_project(test_temp_dir("ui-tree-visibility"), project);
    workspace.apply_ui_tree_action(UiTreeAction::ToggleVisibility(group));

    let first_scene = workspace.current_ui_scene().unwrap();
    assert!(workspace.hidden_ui_nodes.contains(&(first_scene_id, group)));
    assert!(ui_node_hidden(
        first_scene,
        &workspace.hidden_ui_nodes,
        group
    ));
    assert!(ui_node_hidden(
        first_scene,
        &workspace.hidden_ui_nodes,
        label
    ));
    let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 240.0));
    assert_eq!(
        ui_scene_hit_test(
            first_scene,
            &workspace.hidden_ui_nodes,
            canvas,
            [320, 240],
            Pos2::new(8.0, 8.0)
        ),
        None
    );

    let second_scene = workspace.project.ui_scene(second_scene_id).unwrap();
    assert!(!ui_node_hidden(
        second_scene,
        &workspace.hidden_ui_nodes,
        second_group
    ));
    assert_eq!(
        ui_scene_hit_test(
            second_scene,
            &workspace.hidden_ui_nodes,
            canvas,
            [320, 240],
            Pos2::new(8.0, 8.0)
        ),
        Some(second_group)
    );
}

#[test]
fn ui_node_clipboard_pastes_subtree_into_another_ui_scene() {
    let mut project = ProjectDocument::new("ui-node-clipboard");
    project.ui_scenes = vec![UiScene::empty_canvas("Main", UiSceneId::FIRST)];
    let source_root = project.ui_scenes[0].root;
    let panel = project.ui_scenes[0].add_node(
        source_root,
        "Panel",
        UiNodeKind::Group {
            rect: UiRect::new(10, 12, 80, 40),
        },
    );
    project.ui_scenes[0].add_node(
        panel,
        "Child",
        UiNodeKind::Group {
            rect: UiRect::new(3, 4, 16, 8),
        },
    );
    let target_scene_id = project.add_ui_scene("Settings");
    let target_root = project.ui_scene(target_scene_id).unwrap().root;
    let target_parent = project.ui_scene_mut(target_scene_id).unwrap().add_node(
        target_root,
        "Destination",
        UiNodeKind::Group {
            rect: UiRect::new(20, 30, 100, 50),
        },
    );

    let mut workspace = EditorWorkspace::with_project(test_temp_dir("ui-node-clipboard"), project);
    assert!(workspace.copy_ui_node(panel));
    workspace.active_ui_scene_index = 1;
    workspace.selection.selected_ui_node = target_parent;
    assert!(workspace.paste_ui_node());

    let pasted = workspace.selection.selected_ui_node;
    let scene = workspace.current_ui_scene().unwrap();
    let pasted_node = scene.node(pasted).unwrap();
    assert_eq!(pasted_node.name, "Panel");
    assert_eq!(pasted_node.parent, Some(target_parent));
    assert_eq!(pasted_node.children.len(), 1);
    let pasted_child = pasted_node.children[0];
    assert_eq!(scene.node(pasted_child).unwrap().name, "Child");
    assert_eq!(scene.node(pasted_child).unwrap().parent, Some(pasted));
    assert_eq!(
        scene.absolute_rect(pasted_child),
        Some(UiRect::new(33, 46, 16, 8))
    );
}

#[test]
fn resource_browser_supports_ctrl_and_shift_multi_selection() {
    let mut workspace =
        EditorWorkspace::open_directory(psxed_project::default_project_dir()).unwrap();
    let order: Vec<ResourceId> = workspace
        .project
        .resources
        .iter()
        .map(|resource| resource.id)
        .take(3)
        .collect();
    assert!(
        order.len() >= 3,
        "starter project has at least three resources"
    );

    let mut ctrl = egui::Modifiers::NONE;
    ctrl.ctrl = true;
    workspace.apply_resource_selection_modifiers(order[0], egui::Modifiers::NONE, &order);
    workspace.apply_resource_selection_modifiers(order[1], ctrl, &order);

    assert!(workspace.selection.selected_resources.contains(&order[0]));
    assert!(workspace.selection.selected_resources.contains(&order[1]));

    let mut shift = egui::Modifiers::NONE;
    shift.shift = true;
    workspace.apply_resource_selection_modifiers(order[2], shift, &order);

    assert!(workspace.selection.selected_resources.contains(&order[1]));
    assert!(workspace.selection.selected_resources.contains(&order[2]));
    assert!(!workspace.selection.selected_resources.contains(&order[0]));
    assert_eq!(workspace.selection.selected_resources.len(), 2);
}

#[test]
fn select_all_current_scope_selects_all_resources_from_resource_context() {
    let mut project = ProjectDocument::new("select-all-resources");
    let first = project.add_resource("A", ResourceData::Material(MaterialResource::opaque(None)));
    project.add_resource("B", ResourceData::Material(MaterialResource::opaque(None)));
    project.add_resource("C", ResourceData::Material(MaterialResource::opaque(None)));
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.replace_resource_selection(first);

    workspace.select_all_current_scope();

    assert_eq!(workspace.selection.selected_resources.len(), 3);
    assert_eq!(workspace.selection.selected_resource, Some(first));
    assert_eq!(workspace.selection.selected_node, NodeId::ROOT);
}

#[test]
fn select_all_current_scope_selects_scene_nodes_outside_select_tool() {
    let mut project = ProjectDocument::new("select-all-nodes");
    let first = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "First", NodeKind::Entity);
    let second = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Second", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.active_tool = ViewTool::Place;

    workspace.select_all_current_scope();

    assert!(workspace.selection.selected_nodes.contains(&first));
    assert!(workspace.selection.selected_nodes.contains(&second));
    assert!(!workspace.selection.selected_nodes.contains(&NodeId::ROOT));
    assert_eq!(workspace.selection.selected_nodes.len(), 2);
    assert_eq!(workspace.selected_brush, None);
}
