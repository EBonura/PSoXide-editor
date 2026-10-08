use super::*;

#[test]
fn node_gizmo_axes_appear_for_selected_entity_and_light() {
    let mut project = ProjectDocument::new("node-gizmo-axes");
    let room = NodeId::ROOT;
    let entity = project
        .active_scene_mut()
        .add_node(room, "Entity", NodeKind::Entity);
    let light = project.active_scene_mut().add_node(
        room,
        "Light",
        NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("node-gizmo-axes"), project);
    set_gizmo_test_camera(&mut workspace);
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));

    workspace.replace_node_selection(entity);
    let entity_axes: HashSet<_> = workspace
        .node_gizmo_screen_axes(viewport)
        .into_iter()
        .map(|axis| axis.axis)
        .collect();
    assert!(entity_axes.contains(&PrimitiveGizmoAxis::X));
    assert!(entity_axes.contains(&PrimitiveGizmoAxis::Y));
    assert!(entity_axes.contains(&PrimitiveGizmoAxis::Z));

    workspace.replace_node_selection(light);
    let light_axes: HashSet<_> = workspace
        .node_gizmo_screen_axes(viewport)
        .into_iter()
        .map(|axis| axis.axis)
        .collect();
    assert!(light_axes.contains(&PrimitiveGizmoAxis::X));
    assert!(light_axes.contains(&PrimitiveGizmoAxis::Y));
    assert!(light_axes.contains(&PrimitiveGizmoAxis::Z));
}

#[test]
fn node_gizmo_move_planes_appear_for_selected_entity() {
    let mut project = ProjectDocument::new("node-gizmo-planes");
    let room = NodeId::ROOT;
    let entity = project
        .active_scene_mut()
        .add_node(room, "Entity", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("node-gizmo-planes"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.camera_rig.free_position = [2048, 1024, -2048];
    let (yaw, pitch) = camera_angles_to_look_at(
        workspace.camera_rig.free_position,
        [
            DEFAULT_WORLD_SECTOR_SIZE / 2,
            DEFAULT_WORLD_SECTOR_SIZE / 4,
            DEFAULT_WORLD_SECTOR_SIZE / 2,
        ],
    )
    .expect("oblique gizmo test camera can face the entity");
    workspace.camera_rig.free_yaw = yaw;
    workspace.camera_rig.free_pitch = pitch;
    workspace.replace_node_selection(entity);

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let planes: HashSet<_> = workspace
        .node_gizmo_screen_planes(viewport)
        .into_iter()
        .map(|plane| plane.plane)
        .collect();

    assert!(planes.contains(&NodeGizmoPlane::XY));
    assert!(planes.contains(&NodeGizmoPlane::XZ));
    assert!(planes.contains(&NodeGizmoPlane::YZ));
}

#[test]
fn node_gizmo_xy_plane_moves_entity_on_two_axes() {
    let mut project = ProjectDocument::new("entity-gizmo-xy");
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Entity", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("entity-gizmo-xy"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(entity);

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let screen_plane = projected_node_gizmo_plane(&workspace, viewport, NodeGizmoPlane::XY);
    let start = screen_plane_center(screen_plane);
    assert_eq!(
        workspace.pick_node_gizmo_handle(viewport, start),
        Some(NodeGizmoHandle::Plane(NodeGizmoPlane::XY))
    );
    assert!(workspace.begin_node_gizmo_handle_drag(
        NodeGizmoHandle::Plane(NodeGizmoPlane::XY),
        viewport,
        start
    ));
    let start_hit = workspace
        .interaction
        .node_gizmo_drag()
        .and_then(|drag| drag.start_plane_hit)
        .expect("plane drag stores start hit");
    let target_hit = [
        start_hit[0] + HEIGHT_QUANTUM as f32,
        start_hit[1] + HEIGHT_QUANTUM as f32,
        start_hit[2],
    ];
    let target_pointer =
        project_world_to_viewport_screen(workspace.viewport_3d_camera(), viewport, target_hit)
            .expect("target hit projects");

    workspace.update_node_gizmo_drag(viewport, target_pointer, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_vec3_approx(
        node.transform.translation,
        [HEIGHT_QUANTUM as f32, HEIGHT_QUANTUM as f32, 0.0],
    );
    assert_eq!(workspace.status, "Moved 1 node on XY");
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(node.transform.translation, [0.0, 0.0, 0.0]);
}

#[test]
fn node_gizmo_moves_bsp_entity_in_world_units() {
    // BSP scenes hang entities off the root in raw world units; a gizmo
    // step must move snap_units world units (the 1/1024-speed regression),
    // and Shift (free) must drop to single-unit precision.
    let mut project = ProjectDocument::new("entity-gizmo-bsp");
    project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, 0, 0],
            [256, 256, 256],
        ));
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Entity", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("entity-gizmo-bsp"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(entity);

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    // The axis handle follows the pointer ray, like the plane handle: the
    // node moves to the axis point under the pointer, on the grid.
    let (pivot, _) = workspace.node_gizmo_bounds_3d(&[entity]).unwrap();
    let screen_at = |workspace: &EditorWorkspace, dx: f64| {
        workspace
            .project_brush_point_3d(
                viewport,
                [
                    f64::from(pivot[0]) + dx,
                    f64::from(pivot[1]),
                    f64::from(pivot[2]),
                ],
            )
            .unwrap()
    };
    let start = screen_at(&workspace, 0.0);
    let target = screen_at(&workspace, 100.0);
    workspace.snap_units = 64;
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, start));
    workspace.update_node_gizmo_drag(viewport, target, false);
    workspace.end_node_gizmo_drag();
    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(
        node.transform.translation[0], 128.0,
        "100 units along the axis lands on the nearest Grid 64 line"
    );
    workspace.do_undo();

    // Shift: one engine unit (16 authored units), still under the pointer.
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, start));
    workspace.update_node_gizmo_drag(viewport, target, true);
    workspace.end_node_gizmo_drag();
    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(
        node.transform.translation[0], 96.0,
        "free drag follows the pointer to the nearest engine unit"
    );
    workspace.do_undo();

    // Zoomed out, the same world point is fewer pixels away; the node still
    // lands under the pointer (the old handle moved one step per 4 px).
    workspace.camera_rig.free_position = [512 * 3, 768 * 3, -2048 * 3];
    let start = screen_at(&workspace, 0.0);
    let target = screen_at(&workspace, 100.0);
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, start));
    workspace.update_node_gizmo_drag(viewport, target, true);
    workspace.end_node_gizmo_drag();
    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(
        node.transform.translation[0], 96.0,
        "zoomed-out drag still follows the pointer"
    );
}

#[test]
fn node_gizmo_moves_entity_on_selected_axis() {
    let mut project = ProjectDocument::new("entity-gizmo-x");
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Entity", NodeKind::Entity);
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("entity-gizmo-x"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(entity);

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let x_axis = projected_node_gizmo_axis(&workspace, viewport, PrimitiveGizmoAxis::X);
    let unit = (x_axis.end - x_axis.start).normalized();
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, x_axis.start));
    workspace.update_node_gizmo_drag(viewport, x_axis.start + unit * 4.0, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(entity).unwrap();
    assert!((node.transform.translation[0] - workspace.snap_units.max(1) as f32).abs() < 0.001);
    assert_eq!(node.transform.translation[1], 0.0);
    assert_eq!(node.transform.translation[2], 0.0);
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(entity).unwrap();
    assert_eq!(node.transform.translation, [0.0, 0.0, 0.0]);
}

#[test]
fn node_gizmo_moves_point_light_on_y_axis() {
    let mut project = ProjectDocument::new("light-gizmo-y");
    let light = project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Light",
        NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("light-gizmo-y"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(light);

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let y_axis = projected_node_gizmo_axis(&workspace, viewport, PrimitiveGizmoAxis::Y);
    let unit = (y_axis.end - y_axis.start).normalized();
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::Y, viewport, y_axis.start));
    workspace.update_node_gizmo_drag(viewport, y_axis.start + unit * 4.0, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(light).unwrap();
    assert_vec3_approx(
        node.transform.translation,
        [0.0, workspace.snap_units.max(1) as f32, 0.0],
    );
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(light).unwrap();
    assert_eq!(node.transform.translation, [0.0, 0.0, 0.0]);
}

#[test]
fn node_gizmo_rotates_image_prop_around_y() {
    let mut project = ProjectDocument::new("image-prop-gizmo-rotate");
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Banner",
        NodeKind::ImageProp {
            material: None,
            width: 1024,
            height: 1024,
            cylindrical_billboard: false,
            collision_enabled: false,
            collision_size: [1024, 1024, 1024],
            destructible: None,
        },
    );
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir("image-prop-gizmo-rotate"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(prop);
    workspace.transform_gizmo_mode = TransformGizmoMode::Rotate;

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let ring = workspace
        .node_rotation_gizmo_screen_ring_for_axis(viewport, PrimitiveGizmoAxis::Y)
        .expect("rotation ring projects");
    // Sweep the pointer along the ring in its own point order (ring
    // points advance with a positive world rotation), so the drag must
    // rotate by the swept screen angle, positively, no matter where
    // the camera sits. Radial pointer motion sweeps no angle and must
    // change nothing.
    let start = ring.points[0];
    let target = ring.points[8];
    let start_angle = (start - ring.center).angle();
    let target_angle = (target - ring.center).angle();
    let mut swept = (target_angle - start_angle).to_degrees();
    while swept > 180.0 {
        swept -= 360.0;
    }
    while swept <= -180.0 {
        swept += 360.0;
    }
    let expected_yaw = swept.abs().round();
    assert!(expected_yaw >= 10.0, "test sweep too small: {swept}");

    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::Y, viewport, start));
    let radial = (start - ring.center).normalized();
    workspace.update_node_gizmo_drag(viewport, start + radial * 24.0, false);
    let node = workspace.project.active_scene().node(prop).unwrap();
    assert_eq!(
        node.transform.rotation_degrees[1], 0.0,
        "radial motion must not rotate"
    );
    workspace.update_node_gizmo_drag(viewport, target, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(prop).unwrap();
    assert_eq!(node.transform.rotation_degrees, [0.0, expected_yaw, 0.0]);
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(prop).unwrap();
    assert_eq!(node.transform.rotation_degrees, [0.0, 0.0, 0.0]);
}

#[test]
fn cortex_group_rotates_around_every_gizmo_axis() {
    let project_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../projects/default");
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0));

    for axis in [
        PrimitiveGizmoAxis::X,
        PrimitiveGizmoAxis::Y,
        PrimitiveGizmoAxis::Z,
    ] {
        let mut workspace = EditorWorkspace::open_directory(&project_dir).unwrap();
        workspace.snap_units = 64;
        let group = workspace
            .project
            .active_scene()
            .nodes()
            .iter()
            .find(|node| node.name == "GroupS")
            .expect("v0.4b group node")
            .id;
        workspace.replace_node_selection(group);
        workspace.transform_gizmo_mode = TransformGizmoMode::Rotate;
        workspace.frame_viewport();
        let before: Vec<_> = workspace
            .project
            .active_scene()
            .brushes
            .iter()
            .filter(|brush| brush.group == Some(group))
            .cloned()
            .collect();
        let ring = workspace
            .node_rotation_gizmo_screen_ring_for_axis(viewport, axis)
            .unwrap_or_else(|| panic!("{axis:?} ring projects"));
        assert!(
            workspace.begin_node_gizmo_drag(axis, viewport, ring.points[0]),
            "{axis:?} ring begins a drag"
        );
        workspace
            .interaction
            .node_gizmo_drag_mut()
            .expect("active group drag")
            .current_steps = 90;
        workspace.apply_node_gizmo_drag();
        workspace.end_node_gizmo_drag();
        let after: Vec<_> = workspace
            .project
            .active_scene()
            .brushes
            .iter()
            .filter(|brush| brush.group == Some(group))
            .cloned()
            .collect();
        assert_ne!(after, before, "{axis:?} rotation changes group brushes");
        assert!(
            after
                .iter()
                .all(|brush| brush.solved_vertices_on_grid(64, 0.01)),
            "{axis:?} rotation keeps every group corner on Grid 64"
        );
    }
}

#[test]
fn node_gizmo_local_space_rotates_about_node_axis() {
    let mut project = ProjectDocument::new("image-prop-gizmo-local");
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Banner",
        NodeKind::ImageProp {
            material: None,
            width: 1024,
            height: 1024,
            cylindrical_billboard: false,
            collision_enabled: false,
            collision_size: [1024, 1024, 1024],
            destructible: None,
        },
    );
    let start_rotation = [90.0f32, 0.0, 0.0];
    project
        .active_scene_mut()
        .node_mut(prop)
        .unwrap()
        .transform
        .rotation_degrees = start_rotation;
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir("image-prop-gizmo-local"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(prop);
    workspace.transform_gizmo_mode = TransformGizmoMode::Rotate;
    workspace.gizmo_space = GizmoSpace::Local;

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let ring = workspace
        .node_rotation_gizmo_screen_ring_for_axis(viewport, PrimitiveGizmoAxis::Y)
        .expect("local rotation ring projects");
    let start = ring.points[0];
    let target = ring.points[8];
    let start_angle = (start - ring.center).angle();
    let target_angle = (target - ring.center).angle();
    let mut swept = (target_angle - start_angle).to_degrees();
    while swept > 180.0 {
        swept -= 360.0;
    }
    while swept <= -180.0 {
        swept += 360.0;
    }
    let steps = swept.abs().round();
    assert!(steps >= 10.0, "test sweep too small: {swept}");

    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::Y, viewport, start));
    workspace.update_node_gizmo_drag(viewport, target, false);
    workspace.end_node_gizmo_drag();

    // A local-space drag must equal composing the delta in the node's
    // own frame; compare rotation matrices since Euler triples alias.
    let expected = psxed_project::spatial::rotate_euler_degrees(
        start_rotation,
        1,
        steps,
        psxed_project::spatial::RotationSpace::Local,
    );
    let node = workspace.project.active_scene().node(prop).unwrap();
    let actual_m = psxed_project::spatial::euler_degrees_to_matrix(node.transform.rotation_degrees);
    let expected_m = psxed_project::spatial::euler_degrees_to_matrix(expected);
    for row in 0..3 {
        for col in 0..3 {
            assert!(
                (actual_m[row][col] - expected_m[row][col]).abs() < 1e-3,
                "actual {:?} expected {expected:?}",
                node.transform.rotation_degrees
            );
        }
    }
    // And it must differ from the global-space composition, proving
    // the toggle reached the apply path.
    let global = psxed_project::spatial::rotate_euler_degrees(
        start_rotation,
        1,
        steps,
        psxed_project::spatial::RotationSpace::Global,
    );
    let global_m = psxed_project::spatial::euler_degrees_to_matrix(global);
    let mut differs = false;
    for row in 0..3 {
        for col in 0..3 {
            differs |= (actual_m[row][col] - global_m[row][col]).abs() > 1e-3;
        }
    }
    assert!(differs, "local and global must diverge for a pitched prop");
}

#[test]
fn arch_prop_exposes_move_rotate_and_quantized_scale_gizmos() {
    let mut project = ProjectDocument::new("arch-prop-gizmos");
    let room = NodeId::ROOT;
    let arch = project.active_scene_mut().add_node(
        room,
        "Arch",
        NodeKind::ArchProp {
            materials: [None; psxed_project::ARCH_PROP_MATERIAL_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::ARCH_PROP_MATERIAL_COUNT],
            geometry: psxed_project::ArchPropGeometry::default(),
            collision_enabled: false,
        },
    );
    let mut workspace = EditorWorkspace::with_project(test_temp_dir("arch-prop-gizmos"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(arch);
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));

    workspace.transform_gizmo_mode = TransformGizmoMode::Move;
    assert_eq!(workspace.selected_node_gizmo_targets(), vec![arch]);
    assert_eq!(workspace.node_gizmo_screen_axes(viewport).len(), 3);

    workspace.transform_gizmo_mode = TransformGizmoMode::Rotate;
    assert_eq!(
        workspace.selected_node_rotation_axes(),
        vec![PrimitiveGizmoAxis::Y]
    );
    assert!(!workspace
        .node_rotation_gizmo_screen_rings(viewport)
        .is_empty());

    workspace.transform_gizmo_mode = TransformGizmoMode::Scale;
    let x_axis = projected_node_gizmo_axis(&workspace, viewport, PrimitiveGizmoAxis::X);
    let unit = (x_axis.end - x_axis.start).normalized();
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, x_axis.start));
    workspace.update_node_gizmo_drag(viewport, x_axis.start + unit * 8.0, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(arch).unwrap();
    let NodeKind::ArchProp { geometry, .. } = &node.kind else {
        panic!("expected arch prop");
    };
    assert_eq!(geometry.span_tiles, 3);
    assert_eq!(node.transform.scale, [1.0, 1.0, 1.0]);
    assert!(workspace.is_dirty());
}

#[test]
fn node_gizmo_scales_image_prop_width() {
    let mut project = ProjectDocument::new("image-prop-gizmo-scale");
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Banner",
        NodeKind::ImageProp {
            material: None,
            width: 1024,
            height: 1024,
            cylindrical_billboard: false,
            collision_enabled: false,
            collision_size: [1024, 1024, 1024],
            destructible: None,
        },
    );
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir("image-prop-gizmo-scale"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(prop);
    workspace.transform_gizmo_mode = TransformGizmoMode::Scale;

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let x_axis = projected_node_gizmo_axis(&workspace, viewport, PrimitiveGizmoAxis::X);
    let unit = (x_axis.end - x_axis.start).normalized();
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, x_axis.start));
    workspace.update_node_gizmo_drag(viewport, x_axis.start + unit * 8.0, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::ImageProp { width, height, .. } = &node.kind else {
        panic!("expected image prop");
    };
    assert_eq!(*width, 1024 + HEIGHT_QUANTUM as u16);
    assert_eq!(*height, 1024);
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::ImageProp { width, height, .. } = &node.kind else {
        panic!("expected image prop");
    };
    assert_eq!(*width, 1024);
    assert_eq!(*height, 1024);
}

#[test]
fn node_gizmo_scales_box_prop_width() {
    let mut project = ProjectDocument::new("box-prop-gizmo-scale");
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Crate",
        NodeKind::BoxProp {
            materials: [None; psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
            vertices: psxed_project::box_prop_vertices_for_size(1024),
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    let mut workspace =
        EditorWorkspace::with_project(test_temp_dir("box-prop-gizmo-scale"), project);
    set_gizmo_test_camera(&mut workspace);
    workspace.replace_node_selection(prop);
    workspace.transform_gizmo_mode = TransformGizmoMode::Scale;

    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let x_axis = projected_node_gizmo_axis(&workspace, viewport, PrimitiveGizmoAxis::X);
    let unit = (x_axis.end - x_axis.start).normalized();
    assert!(workspace.begin_node_gizmo_drag(PrimitiveGizmoAxis::X, viewport, x_axis.start));
    workspace.update_node_gizmo_drag(viewport, x_axis.start + unit * 8.0, false);
    workspace.end_node_gizmo_drag();

    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::BoxProp { vertices, .. } = &node.kind else {
        panic!("expected box prop");
    };
    let min_x = vertices.iter().map(|v| v[0]).min().unwrap();
    let max_x = vertices.iter().map(|v| v[0]).max().unwrap();
    let min_y = vertices.iter().map(|v| v[1]).min().unwrap();
    let max_y = vertices.iter().map(|v| v[1]).max().unwrap();
    assert_eq!(min_x, -544);
    assert_eq!(max_x, 544);
    assert_eq!(min_y, 0);
    assert_eq!(max_y, 1024);
    assert!(workspace.is_dirty());

    workspace.do_undo();
    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::BoxProp { vertices, .. } = &node.kind else {
        panic!("expected box prop");
    };
    assert_eq!(*vertices, psxed_project::box_prop_vertices_for_size(1024));
}

#[test]
fn box_prop_one_to_one_uv_span_tracks_face_size_and_native_texture() {
    let mut vertices = psxed_project::box_prop_vertices_for_size(1024);
    assert_eq!(
        box_prop_face_native_texel_span(vertices, 0, 1024, [64, 32]),
        [63, 31]
    );

    for vertex in &mut vertices {
        vertex[0] *= 2;
    }
    assert_eq!(
        box_prop_face_native_texel_span(vertices, 0, 1024, [64, 32]),
        [127, 31],
        "a two-sector face repeats a 64px texture twice without stretching it"
    );
}

#[test]
fn box_prop_face_resize_keeps_the_opposite_face_fixed() {
    let start_vertices = psxed_project::box_prop_vertices_for_size(1024);
    let mut project = ProjectDocument::new("anchored-box-resize");
    let node_id = project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Anchored Crate",
        NodeKind::BoxProp {
            materials: [None; psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
            vertices: start_vertices,
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    // Node translations are world units.
    let start_translation = [3072.0, 0.0, 2048.0];
    let node = project.active_scene_mut().node_mut(node_id).unwrap();
    node.transform.translation = start_translation;

    apply_box_prop_face_gizmo_resize(node, start_translation, Some(start_vertices), 1, 1);

    let NodeKind::BoxProp { vertices, .. } = &node.kind else {
        unreachable!();
    };
    assert_eq!(vertices.iter().map(|vertex| vertex[0]).min(), Some(-544));
    assert_eq!(vertices.iter().map(|vertex| vertex[0]).max(), Some(544));
    assert_eq!(node.transform.translation[0], 3104.0);
    let left_world = node.transform.translation[0] - 544.0;
    let right_world = node.transform.translation[0] + 544.0;
    assert_eq!(left_world, 2560.0, "the opposite (left) face stays fixed");
    assert_eq!(
        right_world, 3648.0,
        "the dragged face moves one 64-unit step"
    );
}

#[test]
fn material_click_assignment_updates_selected_box_prop_faces() {
    let mut project = ProjectDocument::new("box-prop-materials");
    let target = project.add_resource(
        "Target",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Crate",
        NodeKind::BoxProp {
            materials: [None; psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
            vertices: psxed_project::box_prop_vertices_for_size(1024),
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.replace_node_selection(prop);

    let assignment = workspace
        .assign_selected_box_props_resource(target)
        .expect("material applies to selected box prop");
    assert_eq!(assignment.updated, 1);
    assert_eq!(assignment.targets, 1);

    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::BoxProp { materials, .. } = &node.kind else {
        panic!("expected box prop");
    };
    assert!(materials.iter().all(|material| *material == Some(target)));
    assert!(workspace.is_dirty());
}

#[test]
fn material_click_assignment_applies_to_selected_box_prop() {
    let mut project = ProjectDocument::new("box-prop-texture");
    let material_id = project.add_resource(
        "Brick",
        ResourceData::Material(psxed_project::MaterialResource::opaque(Some(
            "assets/textures/brick.psxt".to_string(),
        ))),
    );
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Crate",
        NodeKind::BoxProp {
            materials: [None; psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
            vertices: psxed_project::box_prop_vertices_for_size(1024),
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.replace_node_selection(prop);

    let assignment = workspace
        .assign_selected_box_props_resource(material_id)
        .expect("material applies to selected box prop");
    assert_eq!(assignment.updated, 1);
    assert_eq!(assignment.material, material_id);

    let node = workspace.project.active_scene().node(prop).unwrap();
    let NodeKind::BoxProp { materials, .. } = &node.kind else {
        panic!("expected box prop");
    };
    assert!(materials
        .iter()
        .all(|material| *material == Some(assignment.material)));
    assert!(workspace.is_dirty());
}

#[test]
fn box_prop_resource_click_keeps_node_selection_active() {
    let mut project = ProjectDocument::new("box-prop-click-selection");
    let target = project.add_resource(
        "Target",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let room = NodeId::ROOT;
    let prop = project.active_scene_mut().add_node(
        room,
        "Crate",
        NodeKind::BoxProp {
            materials: [None; psxed_project::BOX_PROP_FACE_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
            vertices: psxed_project::box_prop_vertices_for_size(1024),
            collision_enabled: true,
            break_flags: 0,
            erosion: psxed_project::BoxPropErosion::default(),
        },
    );
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.replace_node_selection(prop);
    workspace.replace_resource_selection(target);

    assert!(
        workspace.apply_selected_box_prop_resource_click(ResourceClick {
            id: target,
            modifiers: egui::Modifiers::NONE,
        })
    );

    assert_eq!(workspace.selection.selected_node, prop);
    assert!(workspace.selection.selected_nodes.contains(&prop));
    assert_eq!(workspace.selection.selected_resource, None);
    assert!(workspace.selection.selected_resources.is_empty());
}

#[test]
fn place_image_prop_with_selected_material_creates_node() {
    let mut project = ProjectDocument::new("image-prop-material-place");
    let material = project.add_resource(
        "Banner",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    let root = project.active_scene().root;
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.place_kind = PlaceKind::ImageProp;
    workspace.replace_resource_selection(material);

    workspace.place_node_at_world_hit(root, [512.0, 384.0, 512.0]);

    let node = workspace
        .project
        .active_scene()
        .node(workspace.selected_node_id())
        .expect("placed image prop is selected");
    assert_eq!(node.name, "Banner Image");
    assert_eq!(workspace.active_tool, ViewTool::Select);
    assert_eq!(node.transform.translation[1], 384.0);
    let NodeKind::ImageProp {
        material: Some(actual),
        width,
        height,
        cylindrical_billboard,
        ..
    } = &node.kind
    else {
        panic!("expected image prop node");
    };
    assert_eq!(*actual, material);
    assert_eq!(*width, psxed_project::DEFAULT_IMAGE_PROP_SIZE);
    assert_eq!(*height, psxed_project::DEFAULT_IMAGE_PROP_SIZE);
    assert!(!*cylindrical_billboard);
    assert_eq!(workspace.status, "Placed Image Prop");
    assert!(workspace.is_dirty());
}

#[test]
fn snap_entity_to_floor_moves_the_complete_entity_and_is_undoable() {
    let mut project = ProjectDocument::new("snap-entity-floor");
    project
        .active_scene_mut()
        .brushes
        .push(psxed_project::brush::Brush::cuboid(
            [0, -64, 0],
            [512, 0, 512],
        ));
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Enemy", NodeKind::Entity);
    project
        .active_scene_mut()
        .node_mut(entity)
        .unwrap()
        .transform
        .translation = [128.0, 240.0, 128.0];
    let renderer = project.active_scene_mut().add_node(
        entity,
        "Model Renderer",
        NodeKind::ModelRenderer {
            model: None,
            material: None,
            visual_offset: [0; 3],
            visual_scale_q8: MODEL_SCALE_ONE_Q8,
        },
    );
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.replace_node_selection(renderer);

    assert!(workspace.snap_selected_entities_to_floor());
    assert_eq!(
        workspace
            .project
            .active_scene()
            .node(entity)
            .unwrap()
            .transform
            .translation,
        [128.0, 0.0, 128.0]
    );
    assert_eq!(
        workspace
            .project
            .active_scene()
            .node(renderer)
            .unwrap()
            .transform
            .translation,
        [0.0; 3],
        "the component stays local while its complete Entity moves"
    );
    assert_eq!(workspace.status, "Snapped Entity to floor");

    workspace.do_undo();
    assert_eq!(
        workspace
            .project
            .active_scene()
            .node(entity)
            .unwrap()
            .transform
            .translation,
        [128.0, 240.0, 128.0]
    );
}

#[test]
fn snap_entities_to_floor_preserves_exact_ramp_height_and_deduplicates_components() {
    let mut project = ProjectDocument::new("snap-entities-ramp");
    let ramp = psxed_project::brush::Brush::convex_prism(
        &[[0, -64], [256, -64], [256, 128], [0, 0]],
        [0, 1],
        2,
        [0, 256],
    )
    .expect("convex ramp");
    project.active_scene_mut().brushes.push(ramp);
    let entity = project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Enemy", NodeKind::Entity);
    project
        .active_scene_mut()
        .node_mut(entity)
        .unwrap()
        .transform
        .translation = [128.0, 240.0, 128.0];
    let controller = project.active_scene_mut().add_node(
        entity,
        "Character Controller",
        NodeKind::CharacterController {
            loadout: None,
            character: None,
            settings: None,
            player: false,
        },
    );
    let mut workspace = EditorWorkspace::with_project(std::env::temp_dir(), project);
    workspace.selection.selected_node = controller;
    workspace.selection.selected_nodes = [entity, controller].into_iter().collect();

    assert!(workspace.snap_selected_entities_to_floor());
    let y = workspace
        .project
        .active_scene()
        .node(entity)
        .unwrap()
        .transform
        .translation[1];
    assert!((y - 64.0).abs() < 0.001, "ramp height stays exact: {y}");
    assert_eq!(workspace.status, "Snapped Entity to floor");
}
