use super::*;

impl EditorWorkspace {
    /// World directions of the gizmo handle axes, as matrix columns.
    ///
    /// Identity in Global space; the active (first) target's authored
    /// rotation in Local space. Scale handles always follow the object
    /// regardless of the toggle: they edit object dimensions, so
    /// world-aligned handles on a rotated prop would point away from
    /// the extent they resize.
    pub(crate) fn node_gizmo_basis(&self, targets: &[NodeId]) -> [[f32; 3]; 3] {
        const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let object_aligned = match self.transform_gizmo_mode {
            TransformGizmoMode::Scale => true,
            TransformGizmoMode::Move | TransformGizmoMode::Rotate => {
                self.gizmo_space == GizmoSpace::Local
            }
        };
        if !object_aligned {
            return IDENTITY;
        }
        let scene = self.project.active_scene();
        targets
            .iter()
            .find_map(|id| scene.node(*id))
            .map(|node| euler_degrees_to_matrix(node.transform.rotation_degrees))
            .unwrap_or(IDENTITY)
    }

    pub(crate) fn node_gizmo_screen_axes(&self, rect: Rect) -> Vec<PrimitiveGizmoScreenAxis> {
        let targets = self.selected_node_gizmo_targets();
        if targets.is_empty() {
            return Vec::new();
        }
        let Some((pivot, _)) = self.node_gizmo_bounds_3d(&targets) else {
            return Vec::new();
        };
        let camera = self.viewport_3d_camera();
        let Some(start) = project_world_to_viewport_screen(camera, rect, pivot) else {
            return Vec::new();
        };
        let axis_len = self.node_gizmo_axis_world_length(&targets) as f32;
        let basis = self.node_gizmo_basis(&targets);
        [
            PrimitiveGizmoAxis::X,
            PrimitiveGizmoAxis::Y,
            PrimitiveGizmoAxis::Z,
        ]
        .into_iter()
        .filter_map(|axis| {
            let dir = basis_column(&basis, axis.index());
            let end_world = [
                pivot[0] + dir[0] * axis_len,
                pivot[1] + dir[1] * axis_len,
                pivot[2] + dir[2] * axis_len,
            ];
            let end = project_world_to_viewport_screen(camera, rect, end_world)?;
            ((end - start).length_sq() >= 64.0).then_some(PrimitiveGizmoScreenAxis {
                axis,
                start,
                end,
            })
        })
        .collect()
    }

    /// Six object-aligned resize anchors, one at the centre of each Box
    /// Prop face. The short stem points outwards and supplies an
    /// unambiguous drag direction even when a face is seen obliquely.
    pub(crate) fn box_prop_face_screen_handles(&self, rect: Rect) -> Vec<BoxPropFaceScreenHandle> {
        if self.transform_gizmo_mode != TransformGizmoMode::Scale {
            return Vec::new();
        }
        let targets = self.selected_node_gizmo_targets();
        let [node_id] = targets.as_slice() else {
            return Vec::new();
        };
        let scene = self.project.active_scene();
        let Some(node) = scene.node(*node_id) else {
            return Vec::new();
        };
        let NodeKind::BoxProp { vertices, .. } = &node.kind else {
            return Vec::new();
        };
        let origin = node.transform.translation;
        let basis = euler_degrees_to_matrix(node.transform.rotation_degrees);
        let camera = self.viewport_3d_camera();
        let sector_size = self.project.world_sector_size_for_node(*node_id);
        let stem_length = (sector_size.max(1) as f32 * 0.18).max(64.0);
        const FACE_NORMALS: [[f32; 3]; psxed_project::BOX_PROP_FACE_COUNT] = [
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ];

        psxed_project::BOX_PROP_FACE_VERTEX_INDICES
            .iter()
            .enumerate()
            .filter_map(|(face, indices)| {
                let mut local_center = [0.0; 3];
                for index in indices {
                    for axis in 0..3 {
                        local_center[axis] += f32::from(vertices[*index][axis]) * 0.25;
                    }
                }
                let rotated_center = rotate_vector_by_matrix(&basis, local_center);
                let center_world = [
                    origin[0] + rotated_center[0],
                    origin[1] + rotated_center[1],
                    origin[2] + rotated_center[2],
                ];
                let normal = rotate_vector_by_matrix(&basis, FACE_NORMALS[face]);
                let end_world = [
                    center_world[0] + normal[0] * stem_length,
                    center_world[1] + normal[1] * stem_length,
                    center_world[2] + normal[2] * stem_length,
                ];
                let center = project_world_to_viewport_screen(camera, rect, center_world)?;
                let end = project_world_to_viewport_screen(camera, rect, end_world)?;
                ((end - center).length_sq() >= 16.0).then_some(BoxPropFaceScreenHandle {
                    face: face as u8,
                    center,
                    end,
                })
            })
            .collect()
    }

    pub(crate) fn node_gizmo_screen_planes(&self, rect: Rect) -> Vec<NodeGizmoScreenPlane> {
        if self.transform_gizmo_mode != TransformGizmoMode::Move {
            return Vec::new();
        }
        // Plane handles drag along axis-aligned world planes and snap
        // per world component; in Local space the axis handles carry
        // the rotated directions and the planes are hidden rather than
        // drawn misaligned with them.
        if self.gizmo_space == GizmoSpace::Local {
            return Vec::new();
        }
        let targets = self.selected_node_gizmo_targets();
        if targets.is_empty() {
            return Vec::new();
        }
        let Some((pivot, _)) = self.node_gizmo_bounds_3d(&targets) else {
            return Vec::new();
        };
        let camera = self.viewport_3d_camera();
        let axis_len = self.node_gizmo_axis_world_length(&targets) as f32;
        let near = axis_len * 0.18;
        let far = axis_len * 0.44;

        NodeGizmoPlane::ALL
            .into_iter()
            .filter_map(|plane| {
                let [a, b] = plane.axes();
                let a_delta = a.world_delta(1);
                let b_delta = b.world_delta(1);
                let corner_world = |a_scale: f32, b_scale: f32| {
                    [
                        pivot[0] + a_delta[0] * a_scale + b_delta[0] * b_scale,
                        pivot[1] + a_delta[1] * a_scale + b_delta[1] * b_scale,
                        pivot[2] + a_delta[2] * a_scale + b_delta[2] * b_scale,
                    ]
                };
                let corners = [
                    project_world_to_viewport_screen(camera, rect, corner_world(near, near))?,
                    project_world_to_viewport_screen(camera, rect, corner_world(far, near))?,
                    project_world_to_viewport_screen(camera, rect, corner_world(far, far))?,
                    project_world_to_viewport_screen(camera, rect, corner_world(near, far))?,
                ];
                (polygon_area_2d(&corners).abs() >= 24.0)
                    .then_some(NodeGizmoScreenPlane { plane, corners })
            })
            .collect()
    }

    pub(crate) fn selected_node_gizmo_targets(&self) -> Vec<NodeId> {
        self.selected_node_ids_in_hierarchy()
            .into_iter()
            .filter(|id| self.node_supports_transform_gizmo(*id, self.transform_gizmo_mode))
            .collect()
    }

    pub(crate) fn node_supports_transform_gizmo(
        &self,
        id: NodeId,
        mode: TransformGizmoMode,
    ) -> bool {
        if self.scene_node_effectively_hidden(id) {
            return false;
        }
        self.project
            .active_scene()
            .node(id)
            .is_some_and(|node| node_kind_supports_transform_gizmo(&node.kind, mode))
    }

    pub(crate) fn node_gizmo_bounds_3d(&self, targets: &[NodeId]) -> Option<([f32; 3], [f32; 3])> {
        let mut bounds = None;
        for id in targets {
            if let Some((center, half)) = self.node_frame_bounds_3d(*id) {
                merge_bounds_3d(&mut bounds, center, half);
            }
        }
        bounds.map(bounds_3d_to_center_half)
    }

    pub(crate) fn node_gizmo_axis_world_length(&self, targets: &[NodeId]) -> i32 {
        targets.first().map_or(DEFAULT_WORLD_SECTOR_SIZE, |id| {
            self.project.world_sector_size_for_node(*id).max(1)
        })
    }

    pub(crate) fn node_rotation_gizmo_screen_ring_for_axis(
        &self,
        rect: Rect,
        axis: PrimitiveGizmoAxis,
    ) -> Option<NodeRotationGizmoScreenRing> {
        let targets = self.selected_node_gizmo_targets();
        if targets.is_empty() {
            return None;
        }
        let (pivot, half) = self.node_gizmo_bounds_3d(&targets)?;
        let camera = self.viewport_3d_camera();
        let center = project_world_to_viewport_screen(camera, rect, pivot)?;
        let base_radius = self.node_gizmo_axis_world_length(&targets) as f32 * 0.65;
        let bound_radius = half[0].max(half[1]).max(half[2]) * 1.35;
        let radius = base_radius.max(bound_radius).max(128.0);
        // Ring lies in the plane perpendicular to `axis`, spanned by
        // the other two basis columns in the cyclic order that makes a
        // positive rotation about `axis` carry `u` toward `v`. Ring
        // points are therefore ordered so a positive world rotation
        // advances them, which is what the drag's winding test reads.
        let basis = self.node_gizmo_basis(&targets);
        let u = basis_column(&basis, (axis.index() + 1) % 3);
        let v = basis_column(&basis, (axis.index() + 2) % 3);
        let mut points = Vec::with_capacity(49);
        for step in 0..=48 {
            let angle = step as f32 / 48.0 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let world = [
                pivot[0] + (u[0] * cos + v[0] * sin) * radius,
                pivot[1] + (u[1] * cos + v[1] * sin) * radius,
                pivot[2] + (u[2] * cos + v[2] * sin) * radius,
            ];
            if let Some(screen) = project_world_to_viewport_screen(camera, rect, world) {
                points.push(screen);
            }
        }
        (points.len() >= 8).then_some(NodeRotationGizmoScreenRing {
            axis,
            center,
            points,
        })
    }

    pub(crate) fn node_rotation_gizmo_screen_rings(
        &self,
        rect: Rect,
    ) -> Vec<NodeRotationGizmoScreenRing> {
        self.selected_node_rotation_axes()
            .into_iter()
            .filter_map(|axis| self.node_rotation_gizmo_screen_ring_for_axis(rect, axis))
            .collect()
    }

    pub(crate) fn selected_node_rotation_axes(&self) -> Vec<PrimitiveGizmoAxis> {
        let scene = self.project.active_scene();
        let targets = self.selected_node_gizmo_targets();
        if targets.is_empty() {
            return Vec::new();
        }
        let all_axes = [
            PrimitiveGizmoAxis::X,
            PrimitiveGizmoAxis::Y,
            PrimitiveGizmoAxis::Z,
        ];
        let mut axes: Vec<PrimitiveGizmoAxis> = all_axes.to_vec();
        for id in &targets {
            let Some(node) = scene.node(*id) else {
                continue;
            };
            let supported = node_rotation_axes(&node.kind);
            axes.retain(|axis| supported.contains(axis));
        }
        axes
    }

    pub(crate) fn pick_node_gizmo_handle(
        &self,
        rect: Rect,
        pointer: Pos2,
    ) -> Option<NodeGizmoHandle> {
        if self.transform_gizmo_mode == TransformGizmoMode::Rotate {
            return self
                .node_rotation_gizmo_screen_rings(rect)
                .into_iter()
                .filter_map(|ring| {
                    ring.points
                        .windows(2)
                        .map(|pair| distance_to_segment_2d(pointer, pair[0], pair[1]))
                        .min_by(|a, b| a.total_cmp(b))
                        .map(|distance| (distance, ring.axis))
                })
                .filter(|(distance, _)| *distance <= GIZMO_ROTATION_PICK_RADIUS)
                .min_by(|(a, _), (b, _)| a.total_cmp(b))
                .map(|(_, axis)| NodeGizmoHandle::Axis(axis));
        }
        if self.transform_gizmo_mode == TransformGizmoMode::Scale {
            if let Some((_, handle)) = self
                .box_prop_face_screen_handles(rect)
                .into_iter()
                .map(|handle| ((pointer - handle.end).length(), handle))
                .filter(|(distance, _)| *distance <= GIZMO_AXIS_PICK_RADIUS)
                .min_by(|(a, _), (b, _)| a.total_cmp(b))
            {
                return Some(NodeGizmoHandle::BoxFace(handle.face));
            }
        }
        // Axes and move planes overlap on screen: each plane handle is an
        // inner quad spanning two axes, so a cursor inside a plane is also
        // within the axis pick radius of both of that plane's axes.
        // Gather every in-tolerance handle and choose the globally closest
        // instead of returning the first kind checked. The old code
        // early-returned on axes, so a click inside the plane grabbed an
        // axis -- and when the plane was foreshortened while zoomed out,
        // that miss read as "grabbed the tile behind it". Ties go to the
        // plane: a plane hit reports distance 0 by polygon containment, so
        // an equal distance means the cursor is inside the quad, where the
        // user is aiming at the plane rather than its bounding axes.
        let mut best: Option<(f32, u8, NodeGizmoHandle)> = None;
        let mut consider = |distance: f32, plane_tiebreak: u8, handle: NodeGizmoHandle| {
            let better = match best {
                Some((best_distance, best_tiebreak, _)) => {
                    distance < best_distance
                        || (distance == best_distance && plane_tiebreak > best_tiebreak)
                }
                None => true,
            };
            if better {
                best = Some((distance, plane_tiebreak, handle));
            }
        };
        for screen_axis in self.node_gizmo_screen_axes(rect) {
            let distance = distance_to_segment_2d(pointer, screen_axis.start, screen_axis.end)
                .min((pointer - screen_axis.end).length());
            if distance <= GIZMO_AXIS_PICK_RADIUS {
                consider(distance, 0, NodeGizmoHandle::Axis(screen_axis.axis));
            }
        }
        if self.transform_gizmo_mode == TransformGizmoMode::Move {
            let axes = self.node_gizmo_screen_axes(rect);
            let pivot = axes.first().map(|axis| axis.start);
            for screen_plane in self.node_gizmo_screen_planes(rect) {
                // The plane is drawn as a small square inset into the
                // corner between its two axes, but the user reads the whole
                // corner as the handle. So the plane is grabbable in two
                // tiers, both reported at distance-to-quad so a genuinely
                // nearby axis (added above within its 10px radius) still
                // wins and the plane only claims interior the axes leave:
                //   1. inside the quad (distance 0) or within the usual
                //      few-px tolerance of it -- the tight, primary target;
                //   2. anywhere inside the footprint triangle (pivot + the
                //      two axis endpoints) -- fills the wedge of bare tile
                //      that used to sit between the inset quad and the axes
                //      and caused zoomed-out clicks to fall through to the
                //      floor.
                let quad_distance = if point_in_polygon_2d(pointer, &screen_plane.corners) {
                    0.0
                } else {
                    distance_to_polygon_edges_2d(pointer, &screen_plane.corners)
                };
                if quad_distance <= GIZMO_PLANE_PICK_RADIUS {
                    consider(quad_distance, 1, NodeGizmoHandle::Plane(screen_plane.plane));
                    continue;
                }
                if let Some(pivot) = pivot {
                    let [axis_a, axis_b] = screen_plane.plane.axes();
                    let end_a = axes.iter().find(|axis| axis.axis == axis_a).map(|a| a.end);
                    let end_b = axes.iter().find(|axis| axis.axis == axis_b).map(|a| a.end);
                    if let (Some(end_a), Some(end_b)) = (end_a, end_b) {
                        if point_in_polygon_2d(pointer, &[pivot, end_a, end_b]) {
                            consider(quad_distance, 1, NodeGizmoHandle::Plane(screen_plane.plane));
                        }
                    }
                }
            }
        }
        best.map(|(_, _, handle)| handle)
    }

    pub(crate) fn resolve_viewport_3d_pointer_target(
        &self,
        rect: Rect,
        pointer: Pos2,
        select_pick_enabled: bool,
    ) -> Option<Viewport3dPointerTarget> {
        if !select_pick_enabled {
            return None;
        }
        if let Some(handle) = self.pick_node_gizmo_handle(rect, pointer) {
            return Some(Viewport3dPointerTarget::NodeGizmo(handle));
        }

        let brush = self
            .pick_brush_face_nearest_for_selection_3d(rect, pointer)
            .and_then(|(brush, face, hit)| {
                let (origin, _) = self.camera_ray_for_pointer(rect, pointer)?;
                Some((
                    Viewport3dPointerTarget::Brush { brush, face },
                    distance3_f32(origin, hit),
                ))
            });

        // Compare all authored geometry in one distance domain. Preserve the
        // legacy tie rule where entities consume a click exactly on a surface.
        let mut best = brush;
        if let Some(entity) = self.pick_entity_bound(rect, pointer) {
            if best.is_none_or(|(_, best_distance)| entity.distance <= best_distance) {
                best = Some((Viewport3dPointerTarget::Entity(entity), entity.distance));
            }
        }
        best.map(|(target, _)| target)
    }

    pub(crate) fn draw_node_gizmo(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        hovered_handle: Option<NodeGizmoHandle>,
    ) {
        if self.transform_gizmo_mode == TransformGizmoMode::Rotate {
            let rings = self.node_rotation_gizmo_screen_rings(rect);
            if rings.is_empty() {
                return;
            }
            let active_axis = self
                .interaction
                .node_gizmo_drag()
                .and_then(|drag| drag.handle.axis());
            painter.circle_filled(rings[0].center, 4.0, Color32::from_rgb(235, 242, 248));
            for ring in &rings {
                let highlighted = active_axis == Some(ring.axis)
                    || hovered_handle == Some(NodeGizmoHandle::Axis(ring.axis));
                let color = gizmo_axis_color(ring.axis, highlighted);
                let stroke_width = gizmo_axis_stroke_width(highlighted);
                for pair in ring.points.windows(2) {
                    painter.line_segment([pair[0], pair[1]], Stroke::new(stroke_width, color));
                }
                if let Some(label_pos) = ring.points.first().copied() {
                    painter.circle_filled(label_pos, gizmo_axis_handle_radius(highlighted), color);
                    painter.text(
                        label_pos + Vec2::new(14.0, 0.0),
                        Align2::CENTER_CENTER,
                        ring.axis.label(),
                        FontId::monospace(12.0),
                        color,
                    );
                }
            }
            if let Some(drag) = self
                .interaction
                .node_gizmo_drag()
                .filter(|drag| drag.mode == TransformGizmoMode::Rotate)
            {
                let snap = if drag.group_brushes.is_empty() {
                    1
                } else {
                    BRUSH_ROTATION_SNAP_DEGREES
                };
                paint_rotation_readout(painter, rings[0].center, drag.current_steps, snap);
            }
            return;
        }
        if self.transform_gizmo_mode == TransformGizmoMode::Scale {
            let handles = self.box_prop_face_screen_handles(rect);
            if !handles.is_empty() {
                let active_handle = self.interaction.node_gizmo_drag().map(|drag| drag.handle);
                for handle in handles {
                    let face_handle = NodeGizmoHandle::BoxFace(handle.face);
                    let highlighted =
                        active_handle == Some(face_handle) || hovered_handle == Some(face_handle);
                    let axis = box_prop_face_axis(handle.face);
                    let color = gizmo_axis_color(axis, highlighted);
                    painter.line_segment(
                        [handle.center, handle.end],
                        Stroke::new(gizmo_axis_stroke_width(highlighted), color),
                    );
                    painter.circle_filled(
                        handle.end,
                        gizmo_axis_handle_radius(highlighted) + 1.5,
                        color,
                    );
                    painter.circle_stroke(
                        handle.end,
                        gizmo_axis_handle_radius(highlighted) + 3.5,
                        Stroke::new(1.0, Color32::from_white_alpha(150)),
                    );
                }
                return;
            }
        }
        let axes = self.node_gizmo_screen_axes(rect);
        if axes.is_empty() {
            return;
        }
        let active_handle = self.interaction.node_gizmo_drag().map(|drag| drag.handle);
        painter.circle_filled(axes[0].start, 4.0, Color32::from_rgb(235, 242, 248));
        if self.transform_gizmo_mode == TransformGizmoMode::Move {
            for screen_plane in self.node_gizmo_screen_planes(rect) {
                let highlighted = active_handle == Some(NodeGizmoHandle::Plane(screen_plane.plane))
                    || hovered_handle == Some(NodeGizmoHandle::Plane(screen_plane.plane));
                let color = gizmo_highlight_color(screen_plane.plane.color(), highlighted);
                let fill_alpha = if highlighted { 128 } else { 58 };
                let stroke_width = if highlighted { 3.0 } else { 1.5 };
                painter.add(egui::Shape::convex_polygon(
                    screen_plane.corners.to_vec(),
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), fill_alpha),
                    Stroke::new(stroke_width, color),
                ));
            }
        }
        for screen_axis in axes {
            let highlighted = active_handle == Some(NodeGizmoHandle::Axis(screen_axis.axis))
                || hovered_handle == Some(NodeGizmoHandle::Axis(screen_axis.axis));
            let color = gizmo_axis_color(screen_axis.axis, highlighted);
            let stroke_width = gizmo_axis_stroke_width(highlighted);
            painter.line_segment(
                [screen_axis.start, screen_axis.end],
                Stroke::new(stroke_width, color),
            );
            painter.circle_filled(
                screen_axis.end,
                gizmo_axis_handle_radius(highlighted),
                color,
            );
            let label_offset = (screen_axis.end - screen_axis.start).normalized() * 12.0;
            painter.text(
                screen_axis.end + label_offset,
                Align2::CENTER_CENTER,
                screen_axis.axis.label(),
                FontId::monospace(12.0),
                color,
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn begin_node_gizmo_drag(
        &mut self,
        axis: PrimitiveGizmoAxis,
        rect: Rect,
        pointer: Pos2,
    ) -> bool {
        self.begin_node_gizmo_handle_drag(NodeGizmoHandle::Axis(axis), rect, pointer)
    }

    pub(crate) fn begin_node_gizmo_handle_drag(
        &mut self,
        handle: NodeGizmoHandle,
        rect: Rect,
        pointer: Pos2,
    ) -> bool {
        let mode = self.transform_gizmo_mode;
        let ids = self.selected_node_gizmo_targets();
        if ids.is_empty() {
            return false;
        }
        if mode != TransformGizmoMode::Move
            && !matches!(
                (mode, handle),
                (TransformGizmoMode::Rotate, NodeGizmoHandle::Axis(_))
                    | (TransformGizmoMode::Scale, NodeGizmoHandle::Axis(_))
                    | (TransformGizmoMode::Scale, NodeGizmoHandle::BoxFace(_))
            )
        {
            return false;
        }

        let start_plane_hit =
            if let (TransformGizmoMode::Move, NodeGizmoHandle::Plane(plane)) = (mode, handle) {
                let Some((pivot, _)) = self.node_gizmo_bounds_3d(&ids) else {
                    return false;
                };
                let Some((origin, dir)) = self.camera_ray_for_pointer(rect, pointer) else {
                    return false;
                };
                let normal = plane.normal_axis();
                ray_intersects_axis_aligned_plane(origin, dir, normal, pivot[normal.index()])
            } else {
                None
            };
        if matches!(
            (mode, handle),
            (TransformGizmoMode::Move, NodeGizmoHandle::Plane(_))
        ) && start_plane_hit.is_none()
        {
            return false;
        }

        let mut rotate_state: Option<NodeGizmoRotateDrag> = None;
        let screen_axis_delta = match (mode, handle) {
            (TransformGizmoMode::Rotate, NodeGizmoHandle::Axis(axis)) => {
                // Rotation tracks the angle the pointer sweeps around
                // the projected pivot rather than linear motion along
                // a fixed screen direction, so the ring can be grabbed
                // anywhere and circled from any camera angle.
                let Some(ring) = self.node_rotation_gizmo_screen_ring_for_axis(rect, axis) else {
                    return false;
                };
                let grab = pointer - ring.center;
                if grab.length_sq() < 64.0 {
                    return false;
                }
                let winding = ring_screen_winding(&ring);
                if winding == 0.0 {
                    return false;
                }
                rotate_state = Some(NodeGizmoRotateDrag {
                    center: ring.center,
                    winding,
                    last_angle: grab.y.atan2(grab.x),
                    accumulated: 0.0,
                    space: self.gizmo_space.rotation_space(),
                });
                // Unused by the angular path; non-degenerate so the
                // shared length check below stays inert.
                Vec2::new(64.0, 0.0)
            }
            (_, NodeGizmoHandle::Axis(axis)) => {
                let Some(screen_axis) = self
                    .node_gizmo_screen_axes(rect)
                    .into_iter()
                    .find(|candidate| candidate.axis == axis)
                else {
                    return false;
                };
                screen_axis.end - screen_axis.start
            }
            (TransformGizmoMode::Move, NodeGizmoHandle::Plane(plane)) => {
                let Some(screen_plane) = self
                    .node_gizmo_screen_planes(rect)
                    .into_iter()
                    .find(|candidate| candidate.plane == plane)
                else {
                    return false;
                };
                screen_plane.corners[2] - screen_plane.corners[0]
            }
            (TransformGizmoMode::Scale, NodeGizmoHandle::BoxFace(face)) => {
                let Some(handle) = self
                    .box_prop_face_screen_handles(rect)
                    .into_iter()
                    .find(|candidate| candidate.face == face)
                else {
                    return false;
                };
                handle.end - handle.center
            }
            (_, NodeGizmoHandle::Plane(_) | NodeGizmoHandle::BoxFace(_)) => return false,
        };
        if screen_axis_delta.length_sq() < 64.0 {
            return false;
        }
        let move_axis_world = match (mode, handle) {
            (TransformGizmoMode::Move, NodeGizmoHandle::Axis(axis)) => {
                basis_column(&self.node_gizmo_basis(&ids), axis.index())
            }
            _ => [0.0; 3],
        };
        let move_axis_ray_start = match (mode, handle) {
            (TransformGizmoMode::Move, NodeGizmoHandle::Axis(_)) => self
                .node_gizmo_bounds_3d(&ids)
                .zip(self.camera_ray_for_pointer(rect, pointer))
                .and_then(|((pivot, _), (origin, dir))| {
                    gizmo_axis_param_under_ray(pivot, move_axis_world, origin, dir)
                        .map(|t| (pivot, t))
                }),
            _ => None,
        };

        let scene = self.project.active_scene();
        let group_roots: Vec<NodeId> = ids
            .iter()
            .copied()
            .filter(|id| {
                scene
                    .node(*id)
                    .is_some_and(|node| matches!(node.kind, NodeKind::Group))
            })
            .collect();
        let group_pivot = (!group_roots.is_empty())
            .then(|| {
                self.node_gizmo_bounds_3d(&ids)
                    .map(|(pivot, _)| pivot.map(f64::from))
            })
            .flatten();
        let group_count = group_roots.len();
        let mut target_ids = ids;
        if mode == TransformGizmoMode::Move {
            for node in scene.nodes() {
                if !matches!(node.kind, NodeKind::Group)
                    && group_roots
                        .iter()
                        .any(|group| scene.is_descendant_of(node.id, *group))
                    && node_kind_supports_transform_gizmo(&node.kind, mode)
                    && !target_ids.contains(&node.id)
                {
                    target_ids.push(node.id);
                }
            }
        }
        let targets: Vec<NodeGizmoTarget> = target_ids
            .into_iter()
            .filter(|id| {
                !scene
                    .node(*id)
                    .is_some_and(|node| matches!(node.kind, NodeKind::Group))
            })
            .filter_map(|id| {
                scene.node(id).map(|node| NodeGizmoTarget {
                    node: id,
                    start_translation: node.transform.translation,
                    start_rotation_degrees: node.transform.rotation_degrees,
                    start_image_prop_size: match &node.kind {
                        NodeKind::ImageProp { width, height, .. } => Some([*width, *height]),
                        _ => None,
                    },
                    start_box_prop_vertices: match &node.kind {
                        NodeKind::BoxProp { vertices, .. } => Some(*vertices),
                        _ => None,
                    },
                    start_cylinder_prop_geometry: match &node.kind {
                        NodeKind::CylinderProp { geometry, .. } => Some(*geometry),
                        _ => None,
                    },
                    start_arch_prop_geometry: match &node.kind {
                        NodeKind::ArchProp { geometry, .. } => Some(*geometry),
                        _ => None,
                    },
                })
            })
            .collect();
        let mut group_brush_indices = Vec::new();
        for group in &group_roots {
            for index in scene.brush_indices_in_group(*group, true) {
                if !group_brush_indices.contains(&index) {
                    group_brush_indices.push(index);
                }
            }
        }
        let group_brushes: Vec<GroupBrushGizmoTarget> = group_brush_indices
            .into_iter()
            .filter_map(|index| {
                scene
                    .brushes
                    .get(index)
                    .cloned()
                    .map(|start| GroupBrushGizmoTarget { index, start })
            })
            .collect();
        if targets.is_empty() && group_brushes.is_empty() {
            return false;
        }

        self.interaction = Interaction::NodeGizmo(NodeGizmoDrag {
            mode,
            handle,
            start_pointer: pointer,
            screen_axis: screen_axis_delta,
            start_plane_hit,
            current_plane_delta_world: [0.0, 0.0, 0.0],
            move_axis_world,
            move_axis_ray_start,
            rotate: rotate_state,
            targets,
            group_brushes,
            group_pivot,
            group_count,
            current_steps: 0,
            snapshot_pushed: false,
            free: false,
        });
        true
    }

    pub(crate) fn update_node_gizmo_drag(&mut self, rect: Rect, pointer: Pos2, free: bool) {
        if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
            drag.free = free;
        }
        let Some(drag) = self.interaction.node_gizmo_drag() else {
            return;
        };
        if let (TransformGizmoMode::Move, NodeGizmoHandle::Plane(plane)) = (drag.mode, drag.handle)
        {
            let Some(start_hit) = drag.start_plane_hit else {
                return;
            };
            let Some((origin, dir)) = self.camera_ray_for_pointer(rect, pointer) else {
                return;
            };
            let normal = plane.normal_axis();
            let Some(hit) =
                ray_intersects_axis_aligned_plane(origin, dir, normal, start_hit[normal.index()])
            else {
                return;
            };
            let [a, b] = plane.axes();
            let mut delta = [0.0, 0.0, 0.0];
            delta[a.index()] = hit[a.index()] - start_hit[a.index()];
            delta[b.index()] = hit[b.index()] - start_hit[b.index()];
            if vec3_nearly_equal(delta, drag.current_plane_delta_world) {
                return;
            }
            if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
                drag.current_plane_delta_world = delta;
            }
            self.apply_node_gizmo_drag();
            return;
        }

        if let Some(rotate) = drag.rotate {
            // Angular tracking: one step per degree the pointer sweeps
            // around the projected pivot. Unwrapping per update lets a
            // drag wind through multiple revolutions.
            let current_steps = drag.current_steps;
            let rotates_brush_group = !drag.group_brushes.is_empty();
            let offset = pointer - rotate.center;
            if offset.length_sq() < 16.0 {
                return;
            }
            let angle = offset.y.atan2(offset.x);
            let accumulated = rotate.accumulated + wrap_angle_radians(angle - rotate.last_angle);
            if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
                if let Some(rotate) = drag.rotate.as_mut() {
                    rotate.last_angle = angle;
                    rotate.accumulated = accumulated;
                }
            }
            let raw_degrees = f64::from(rotate.winding * accumulated.to_degrees());
            let steps = if rotates_brush_group {
                snap_brush_rotation_degrees(raw_degrees)
            } else {
                raw_degrees.round() as i32
            };
            if steps == current_steps {
                return;
            }
            if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
                drag.current_steps = steps;
            }
            self.apply_node_gizmo_drag();
            return;
        }

        if let (TransformGizmoMode::Move, Some((pivot, start_t))) =
            (drag.mode, drag.move_axis_ray_start)
        {
            // Follow the pointer ray along the axis, in grid steps (one
            // engine unit when free), like the plane handle follows its plane.
            let Some((origin, dir)) = self.camera_ray_for_pointer(rect, pointer) else {
                return;
            };
            let Some(t) = gizmo_axis_param_under_ray(pivot, drag.move_axis_world, origin, dir)
            else {
                return;
            };
            let quantum = if free {
                f32::from(ENGINE_UNIT)
            } else {
                f32::from(self.snap_units.max(1))
            };
            let steps = ((t - start_t) / quantum).round() as i32;
            if steps == drag.current_steps {
                return;
            }
            if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
                drag.current_steps = steps;
            }
            self.apply_node_gizmo_drag();
            return;
        }

        let axis_len_sq = drag.screen_axis.length_sq();
        if axis_len_sq < f32::EPSILON {
            return;
        }
        let pixels_per_step = match drag.mode {
            TransformGizmoMode::Move => 4.0,
            TransformGizmoMode::Scale => 8.0,
            // Rotate never reaches the linear path; it tracks the
            // swept pointer angle above.
            TransformGizmoMode::Rotate => return,
        };
        let pointer_delta = pointer - drag.start_pointer;
        let unit = drag.screen_axis / axis_len_sq.sqrt();
        let steps = (pointer_delta.dot(unit) / pixels_per_step).round() as i32;
        if steps == drag.current_steps {
            return;
        }
        if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
            drag.current_steps = steps;
        }
        self.apply_node_gizmo_drag();
    }

    pub(crate) fn apply_node_gizmo_drag(&mut self) {
        let Some(drag) = self.interaction.node_gizmo_drag() else {
            return;
        };
        if !node_gizmo_drag_has_motion(drag) && !drag.snapshot_pushed {
            return;
        }
        let snapshot_pushed = drag.snapshot_pushed;
        let handle = drag.handle;
        let steps = drag.current_steps;
        let plane_delta_world = drag.current_plane_delta_world;
        // Nodes move on the brush grid; Shift (free) drops to one engine
        // unit, the finest step that survives the cook.
        let free = drag.free;
        let world_quantum = if free {
            i32::from(ENGINE_UNIT)
        } else {
            i32::from(self.snap_units.max(1))
        };
        let move_axis_world = drag.move_axis_world;
        let rotation_space = drag
            .rotate
            .map(|rotate| rotate.space)
            .unwrap_or(RotationSpace::Global);
        let mode = drag.mode;
        let targets = drag.targets.clone();
        let group_brushes = drag.group_brushes.clone();
        let group_has_brushes = !group_brushes.is_empty();
        let group_pivot = drag.group_pivot;
        let brush_delta = if mode == TransformGizmoMode::Move {
            match handle {
                NodeGizmoHandle::Axis(_) => std::array::from_fn(|axis| {
                    (move_axis_world[axis] * steps as f32 * world_quantum as f32).round() as i32
                }),
                NodeGizmoHandle::Plane(plane) => {
                    let mut delta = [0; 3];
                    for axis in plane.axes() {
                        let index = axis.index();
                        delta[index] = (plane_delta_world[index] / world_quantum as f32).round()
                            as i32
                            * world_quantum;
                    }
                    delta
                }
                NodeGizmoHandle::BoxFace(_) => [0; 3],
            }
        } else {
            [0; 3]
        };
        let group_brush_previews = if mode == TransformGizmoMode::Move {
            None
        } else if let (Some(pivot), NodeGizmoHandle::Axis(axis)) = (group_pivot, handle) {
            if mode == TransformGizmoMode::Rotate
                && steps.rem_euclid(BRUSH_ROTATION_SNAP_DEGREES) != 0
            {
                self.status = format!(
                    "Rotate {steps}° rejected: brush groups snap every {}°",
                    BRUSH_ROTATION_SNAP_DEGREES
                );
                return;
            }
            let map = group_brush_transform_map(mode, axis, steps);
            let snap_step = i32::from(self.snap_units.max(1));
            let mut previews = Vec::with_capacity(group_brushes.len());
            for target in &group_brushes {
                let mut brush = target.start.clone();
                let faces: Vec<usize> = (0..brush.faces.len()).collect();
                if brush.transform_selected_snapped(&faces, &[], pivot, map, 0.5, snap_step) == 0
                    || !brush.is_pickable()
                {
                    self.status = if mode == TransformGizmoMode::Rotate {
                        format!(
                            "Rotate {steps}° rejected: result would not form a valid solid on Grid {}",
                            self.snap_units
                        )
                    } else {
                        format!("Scale rejected on Grid {}", self.snap_units)
                    };
                    return;
                }
                if mode == TransformGizmoMode::Rotate {
                    let Some(snapped) = brush.snapped_solved_to_grid(snap_step) else {
                        self.status = format!(
                            "Rotate {steps}° rejected: result cannot be re-snapped as a valid solid on Grid {}",
                            self.snap_units
                        );
                        return;
                    };
                    brush = snapped;
                }
                previews.push((target.index, brush));
            }
            Some(previews)
        } else {
            None
        };
        if !snapshot_pushed {
            if let Some(drag) = self.interaction.node_gizmo_drag_mut() {
                drag.snapshot_pushed = true;
            }
            self.push_undo();
        }
        let texture_lock = self.brush_texture_lock;
        // Arch props span whole World sectors (the cook's tile size).
        let arch_tile_size = self
            .project
            .world_sector_size_for_node(self.project.active_scene().root);
        let scene = self.project.active_scene_mut();
        for target in targets {
            let Some(node) = scene.node_mut(target.node) else {
                continue;
            };
            match mode {
                TransformGizmoMode::Move => match handle {
                    NodeGizmoHandle::Axis(_) => {
                        node.transform.translation = node_gizmo_translation(
                            node,
                            target.start_translation,
                            move_axis_world,
                            steps,
                            world_quantum,
                        );
                    }
                    NodeGizmoHandle::Plane(plane) => {
                        node.transform.translation = node_gizmo_plane_translation(
                            node,
                            target.start_translation,
                            plane,
                            plane_delta_world,
                            world_quantum,
                        );
                    }
                    NodeGizmoHandle::BoxFace(_) => {}
                },
                TransformGizmoMode::Rotate => {
                    if let NodeGizmoHandle::Axis(axis) = handle {
                        node.transform.rotation_degrees = node_gizmo_rotation(
                            node,
                            target.start_rotation_degrees,
                            axis,
                            steps,
                            rotation_space,
                        );
                    }
                }
                TransformGizmoMode::Scale => match handle {
                    NodeGizmoHandle::Axis(axis) => apply_node_gizmo_scale(
                        node,
                        target.start_image_prop_size,
                        target.start_box_prop_vertices,
                        target.start_cylinder_prop_geometry,
                        target.start_arch_prop_geometry,
                        axis,
                        steps,
                    ),
                    NodeGizmoHandle::BoxFace(face) => apply_box_prop_face_gizmo_resize(
                        node,
                        target.start_translation,
                        target.start_box_prop_vertices,
                        face,
                        steps,
                    ),
                    NodeGizmoHandle::Plane(_) => {}
                },
            }
            if let NodeKind::ArchProp { geometry, .. } = &node.kind {
                snap_arch_prop_transform(&mut node.transform, *geometry, arch_tile_size);
            }
        }
        if mode == TransformGizmoMode::Move {
            for target in group_brushes {
                let mut brush = target.start;
                if texture_lock {
                    brush.translate_with_uv_lock(
                        brush_delta,
                        psxed_project::brush::BRUSH_UV_UNITS_PER_TEXEL,
                    );
                } else {
                    brush.translate(brush_delta);
                }
                if let Some(destination) = scene.brushes.get_mut(target.index) {
                    *destination = brush;
                }
            }
        } else if let Some(previews) = group_brush_previews {
            for (index, brush) in previews {
                if let Some(destination) = scene.brushes.get_mut(index) {
                    *destination = brush;
                }
            }
        }
        self.mark_dirty();
        if mode == TransformGizmoMode::Rotate {
            let snap = if group_has_brushes {
                BRUSH_ROTATION_SNAP_DEGREES
            } else {
                1
            };
            self.status = format!("Rotate {steps:+}° on {} (snap {snap}°)", handle.label());
        }
    }

    pub(crate) fn end_node_gizmo_drag(&mut self) {
        let Some(drag) = self.interaction.take_node_gizmo_drag() else {
            return;
        };
        if !drag.snapshot_pushed {
            return;
        }
        let handle = drag.handle.label();
        let moved = drag.targets.len() + drag.group_count;
        let action = match drag.mode {
            TransformGizmoMode::Move => "Moved",
            TransformGizmoMode::Rotate => "Rotated",
            TransformGizmoMode::Scale => "Scaled",
        };
        let amount = if drag.mode == TransformGizmoMode::Rotate {
            let snap = if drag.group_brushes.is_empty() {
                1
            } else {
                BRUSH_ROTATION_SNAP_DEGREES
            };
            format!(" {}° (snap {snap}°)", drag.current_steps)
        } else {
            String::new()
        };
        self.status = if moved == 1 {
            format!("{action} 1 node{amount} on {handle}")
        } else {
            format!("{action} {moved} nodes{amount} on {handle}")
        };
    }

    /// Promote `node` to the active selected node, keeping the
    /// inspector and scene tree in sync with the viewport click.
    pub(crate) fn commit_node_selection(&mut self, node: NodeId) {
        self.replace_node_selection(node);
        self.clear_resource_selection_state();
        self.clear_brush_selection();
        let scene = self.project.active_scene();
        if let Some(n) = scene.node(node) {
            self.status = format!("Selected {} '{}'", n.kind.label(), n.name);
        } else {
            self.status = format!("Selected node #{}", node.raw());
        }
    }

    fn selected_floor_snap_entities(&self) -> Vec<NodeId> {
        let scene = self.project.active_scene();
        let mut entities = Vec::new();
        for selected in self.selected_node_ids_in_hierarchy() {
            let Some(entity) = owning_entity_id(scene, selected) else {
                continue;
            };
            if !entities.contains(&entity) {
                entities.push(entity);
            }
        }
        entities
    }

    pub(crate) fn can_snap_selected_entities_to_floor(&self) -> bool {
        !self.selected_floor_snap_entities().is_empty()
    }

    /// Exact supporting surface beneath an Entity floor anchor.
    ///
    /// Authored Entity transforms are raw world units and already represent
    /// the character/controller foot point. A short upward probe allowance
    /// also recovers an entity that is intersecting its floor by less than one
    /// height quantum, while remaining below any usable character ceiling.
    fn entity_floor_height(&self, entity: NodeId) -> Option<f32> {
        let scene = self.project.active_scene();
        let node = scene.node(entity)?;
        let [x, y, z] = node.transform.translation;
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return None;
        }

        let probe_y = f64::from(y) + f64::from(HEIGHT_QUANTUM);
        let origin = [f64::from(x), probe_y, f64::from(z)];
        let mut best: Option<f64> = None;
        for brush in &scene.brushes {
            if !brush.contents.is_solid() || brush.mover == Some(entity) {
                continue;
            }
            let Some((distance, face_index)) = brush.raycast(origin, [0.0, -1.0, 0.0]) else {
                continue;
            };
            let Some(face) = brush.faces.get(face_index) else {
                continue;
            };
            let Some(plane) = psxed_project::brush::Plane::from_points(face.points) else {
                continue;
            };
            // A downward ray can only use an outward-upward face as a floor.
            // This rejects vertical walls and the underside of solid ceilings.
            if plane.normal[1] <= 0 {
                continue;
            }
            let floor_y = probe_y - distance;
            if floor_y.is_finite() && best.is_none_or(|current| floor_y > current) {
                best = Some(floor_y);
            }
        }

        best.map(|height| height as f32)
    }

    /// Move every selected Entity root to the exact supporting surface below.
    /// Child-component selections are promoted to their owning Entity; each
    /// Entity is moved once and the whole operation is one undo step.
    pub(crate) fn snap_selected_entities_to_floor(&mut self) -> bool {
        let entities = self.selected_floor_snap_entities();
        if entities.is_empty() {
            self.status = "Select an Entity or one of its components to snap to floor".to_string();
            return false;
        }

        let mut placements = Vec::new();
        let mut missing = 0usize;
        let mut already_grounded = 0usize;
        for entity in entities {
            let Some(floor_y) = self.entity_floor_height(entity) else {
                missing += 1;
                continue;
            };
            let Some(node) = self.project.active_scene().node(entity) else {
                missing += 1;
                continue;
            };
            if (node.transform.translation[1] - floor_y).abs() <= 0.001 {
                already_grounded += 1;
                continue;
            }
            placements.push((entity, floor_y));
        }

        if placements.is_empty() {
            self.status = if already_grounded > 0 && missing == 0 {
                if already_grounded == 1 {
                    "Entity is already on the floor".to_string()
                } else {
                    format!("All {already_grounded} entities are already on the floor")
                }
            } else {
                "No floor found beneath the selected entity".to_string()
            };
            return false;
        }

        self.push_undo();
        let moved = placements.len();
        for (entity, floor_y) in placements {
            if let Some(node) = self.project.active_scene_mut().node_mut(entity) {
                node.transform.translation[1] = floor_y;
            }
        }
        self.status = match (moved, missing) {
            (1, 0) => "Snapped Entity to floor".to_string(),
            (1, missing) => {
                format!("Snapped Entity to floor; {missing} had no floor beneath it")
            }
            (moved, 0) => format!("Snapped {moved} entities to floor"),
            (moved, missing) => {
                format!("Snapped {moved} entities to floor; {missing} had no floor beneath them")
            }
        };
        self.mark_dirty();
        true
    }
}

/// Total transform applied to every brush in a selected Group. Rebuilding
/// from each drag-start brush prevents incremental rounding drift.
fn group_brush_transform_map(
    mode: TransformGizmoMode,
    axis: PrimitiveGizmoAxis,
    steps: i32,
) -> [[f64; 3]; 3] {
    let mut map = [[0.0; 3]; 3];
    match mode {
        TransformGizmoMode::Rotate => {
            let (sin, cos) = f64::from(steps).to_radians().sin_cos();
            let a = axis.index();
            let (u, v) = ((a + 1) % 3, (a + 2) % 3);
            map[a][a] = 1.0;
            map[u][u] = cos;
            map[u][v] = -sin;
            map[v][u] = sin;
            map[v][v] = cos;
        }
        TransformGizmoMode::Scale => {
            let factor = (1.0 + f64::from(steps) * 0.05).clamp(0.05, 16.0);
            for (index, row) in map.iter_mut().enumerate() {
                row[index] = if index == axis.index() { factor } else { 1.0 };
            }
        }
        TransformGizmoMode::Move => {
            for (index, row) in map.iter_mut().enumerate() {
                row[index] = 1.0;
            }
        }
    }
    map
}

/// World direction of basis column `index` (the gizmo handle axis).
fn basis_column(basis: &[[f32; 3]; 3], index: usize) -> [f32; 3] {
    [basis[0][index], basis[1][index], basis[2][index]]
}

/// Screen winding of a projected rotation ring: `+1.0` when the ring's
/// increasing-angle order (a positive world rotation) advances the
/// pointer polar angle `atan2(dy, dx)` in viewport coordinates, `-1.0`
/// when it runs the other way, `0.0` for a degenerate (edge-on) ring.
fn ring_screen_winding(ring: &NodeRotationGizmoScreenRing) -> f32 {
    let mut sum = 0.0f32;
    for pair in ring.points.windows(2) {
        let a = pair[0] - ring.center;
        let b = pair[1] - ring.center;
        sum += a.x * b.y - a.y * b.x;
    }
    if sum.abs() < 1.0 {
        0.0
    } else {
        sum.signum()
    }
}

/// Wrap an angle difference into `(-PI, PI]` so per-frame pointer
/// deltas accumulate across the atan2 seam without 2*PI jumps.
pub(crate) fn wrap_angle_radians(delta: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut delta = delta;
    while delta > PI {
        delta -= TAU;
    }
    while delta <= -PI {
        delta += TAU;
    }
    delta
}
