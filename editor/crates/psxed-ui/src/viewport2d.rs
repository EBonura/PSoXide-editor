use super::*;

pub(crate) fn draw_viewport_overlay(
    painter: &egui::Painter,
    rect: Rect,
    project: &ProjectDocument,
    zoom: f32,
    snap_units: u16,
    view: OrthographicView,
    grid_world: bool,
) {
    let overlay = Rect::from_min_size(
        rect.left_top() + Vec2::new(12.0, 12.0),
        Vec2::new(132.0, 94.0),
    );
    painter.rect_filled(
        overlay,
        2.0,
        Color32::from_rgba_unmultiplied(14, 20, 26, 224),
    );
    painter.rect_stroke(
        overlay,
        2.0,
        Stroke::new(1.0, STUDIO_BORDER),
        StrokeKind::Inside,
    );
    let lines = [
        format!("{} Orthographic", view.label()),
        if grid_world {
            format!(
                "Snap {} (Alt {})",
                psxed_project::grid::CLICK_UNITS,
                psxed_project::grid::FINE_UNITS
            )
        } else {
            format!("Grid: {snap_units} units")
        },
        format!("{} nodes", project.active_scene().nodes().len()),
        format!("{} resources", project.resources.len()),
        format_viewport_zoom(zoom),
    ];
    for (idx, line) in lines.iter().enumerate() {
        painter.text(
            overlay.left_top() + Vec2::new(10.0, 10.0 + idx as f32 * 15.0),
            Align2::LEFT_TOP,
            line,
            FontId::monospace(11.0),
            STUDIO_TEXT,
        );
    }
}

pub(crate) fn format_viewport_zoom(zoom: f32) -> String {
    if zoom >= 10.0 {
        format!("{zoom:.0} px/unit")
    } else if zoom >= 1.0 {
        format!("{zoom:.1} px/unit")
    } else {
        format!("{zoom:.3} px/unit")
    }
}

pub(crate) fn draw_viewport_box_select_marquee(painter: &egui::Painter, rect: Option<Rect>) {
    let Some(rect) = rect else {
        return;
    };
    painter.rect_filled(
        rect,
        0.0,
        Color32::from_rgba_unmultiplied(255, 238, 150, 24),
    );
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(2.0, EDITOR_OUTLINE_GOLD),
        StrokeKind::Outside,
    );
}

pub(crate) fn draw_axes_gizmo(painter: &egui::Painter, rect: Rect, view: OrthographicView) {
    let origin = Pos2::new(rect.left() + 34.0, rect.bottom() - 38.0);
    let x_end = origin + Vec2::new(42.0, 0.0);
    let y_end = origin + Vec2::new(0.0, -42.0);
    let [horizontal_axis, vertical_axis] = view.plane_axes();
    let horizontal_color = axis_color(horizontal_axis);
    let vertical_color = axis_color(vertical_axis);
    let x_stroke = Stroke::new(2.0, horizontal_color);
    let y_stroke = Stroke::new(2.0, vertical_color);

    painter.circle_filled(origin, 3.0, STUDIO_ACCENT);
    painter.line_segment([origin, x_end], x_stroke);
    painter.line_segment([origin, y_end], y_stroke);
    painter.line_segment([x_end, x_end + Vec2::new(-7.0, -4.0)], x_stroke);
    painter.line_segment([x_end, x_end + Vec2::new(-7.0, 4.0)], x_stroke);
    painter.line_segment([y_end, y_end + Vec2::new(-4.0, 7.0)], y_stroke);
    painter.line_segment([y_end, y_end + Vec2::new(4.0, 7.0)], y_stroke);
    let [right_label, up_label] = view.screen_axis_labels();
    painter.text(
        x_end + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        right_label,
        FontId::monospace(12.0),
        horizontal_color,
    );
    painter.text(
        y_end + Vec2::new(0.0, -8.0),
        Align2::CENTER_BOTTOM,
        up_label,
        FontId::monospace(12.0),
        vertical_color,
    );
}

fn axis_label(axis: usize) -> &'static str {
    match axis {
        0 => "X",
        1 => "Y",
        2 => "Z",
        _ => unreachable!("world axis is always X, Y, or Z"),
    }
}

fn axis_color(axis: usize) -> Color32 {
    match axis {
        0 => Color32::from_rgb(255, 95, 88),
        1 => Color32::from_rgb(140, 255, 128),
        2 => Color32::from_rgb(96, 150, 255),
        _ => unreachable!("world axis is always X, Y, or Z"),
    }
}

/// The axis-aligned 2D authoring plane: the plan view of the sector grid,
/// using the editor's world-space focus, zoom and snap settings.
///
/// It is the right-handed, Y-up 3D view seen from above (yaw 0 looks down -Z
/// with +X to the right), not its mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrthographicView {
    /// Look down from +Y: +X right, +Z down (-Z, the 3D view's forward, up).
    Top,
}

impl OrthographicView {
    pub(crate) const ALL: [Self; 1] = [Self::Top];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
        }
    }

    /// `[horizontal, vertical]` world-axis indices.
    pub(crate) const fn plane_axes(self) -> [usize; 2] {
        match self {
            Self::Top => [0, 2],
        }
    }

    /// Screen direction of the plane axes: `[1, 1]` when the horizontal
    /// world axis grows to the right and the vertical one grows up; `-1`
    /// flips that axis. Z grows down the screen in the plan view.
    pub(crate) const fn screen_signs(self) -> [f32; 2] {
        match self {
            Self::Top => [1.0, -1.0],
        }
    }

    /// Axis label for the screen-right and screen-up arrows of the corner
    /// axes gizmo, with a minus where the world axis runs the other way.
    pub(crate) fn screen_axis_labels(self) -> [String; 2] {
        let [horizontal, vertical] = self.plane_axes();
        let [sign_h, sign_v] = self.screen_signs();
        let label = |axis: usize, sign: f32| {
            let name = axis_label(axis);
            if sign < 0.0 {
                format!("-{name}")
            } else {
                name.to_string()
            }
        };
        [label(horizontal, sign_h), label(vertical, sign_v)]
    }

    pub(crate) fn project_f32(self, world: [f32; 3]) -> [f32; 2] {
        let [horizontal, vertical] = self.plane_axes();
        [world[horizontal], world[vertical]]
    }

    /// Lift a panel-space world point into 3D while preserving the current
    /// focus coordinate along the hidden/depth axis.
    pub(crate) fn unproject(self, plane: [f32; 2], focus: [f32; 3]) -> [f32; 3] {
        let [horizontal, vertical] = self.plane_axes();
        let mut world = focus;
        world[horizontal] = plane[0];
        world[vertical] = plane[1];
        world
    }

    pub(crate) fn with_projected_focus(self, focus: [f32; 3], plane: [f32; 2]) -> [f32; 3] {
        self.unproject(plane, focus)
    }
}

/// Plane coordinates (true world values of the view's two axes) to panel
/// pixels. `signs` orients each axis on screen (`OrthographicView::screen_signs`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ViewportTransform {
    rect: Rect,
    pan: Vec2,
    zoom: f32,
    signs: [f32; 2],
}

impl ViewportTransform {
    /// Horizontal axis right, vertical axis up.
    #[cfg(test)]
    pub(crate) fn new(rect: Rect, pan: Vec2, zoom: f32) -> Self {
        Self {
            rect,
            pan,
            zoom,
            signs: [1.0, 1.0],
        }
    }

    /// The panel for `view`, centred on `focus` (plane coordinates).
    pub(crate) fn from_focus(
        rect: Rect,
        view: OrthographicView,
        focus: [f32; 2],
        zoom: f32,
    ) -> Self {
        let signs = view.screen_signs();
        Self {
            rect,
            pan: Vec2::new(-signs[0] * focus[0] * zoom, signs[1] * focus[1] * zoom),
            zoom,
            signs,
        }
    }

    pub(crate) fn world_to_screen(self, world: [f32; 2]) -> Pos2 {
        self.rect.center()
            + self.pan
            + Vec2::new(
                self.signs[0] * world[0] * self.zoom,
                -self.signs[1] * world[1] * self.zoom,
            )
    }

    pub(crate) fn screen_to_world(self, screen: Pos2) -> [f32; 2] {
        self.screen_delta_to_world(screen - self.rect.center() - self.pan)
    }

    /// A pointer movement as a plane-coordinate delta.
    pub(crate) fn screen_delta_to_world(self, delta: Vec2) -> [f32; 2] {
        [
            self.signs[0] * delta.x / self.zoom,
            -self.signs[1] * delta.y / self.zoom,
        ]
    }

    pub(crate) fn world_rect_to_screen(self, center: [f32; 2], half: [f32; 2]) -> Rect {
        let min = [center[0] - half[0], center[1] - half[1]];
        let max = [center[0] + half[0], center[1] + half[1]];
        let a = self.world_to_screen(min);
        let b = self.world_to_screen(max);
        Rect::from_min_max(
            Pos2::new(a.x.min(b.x), a.y.min(b.y)),
            Pos2::new(a.x.max(b.x), a.y.max(b.y)),
        )
    }

    pub(crate) fn screen_radius(self, radius: f32) -> f32 {
        radius * self.zoom
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ViewportHit {
    pub(crate) id: NodeId,
    shape: HitShape,
}

impl ViewportHit {
    pub(crate) fn rect(
        id: NodeId,
        _name: impl Into<String>,
        center: [f32; 2],
        half: [f32; 2],
    ) -> Self {
        Self {
            id,
            shape: HitShape::Rect { center, half },
        }
    }

    pub(crate) fn circle(
        id: NodeId,
        _name: impl Into<String>,
        center: [f32; 2],
        radius: f32,
    ) -> Self {
        Self {
            id,
            shape: HitShape::Circle { center, radius },
        }
    }

    pub(crate) fn contains(&self, world: [f32; 2]) -> bool {
        match self.shape {
            HitShape::Rect { center, half } => {
                world[0] >= center[0] - half[0]
                    && world[0] <= center[0] + half[0]
                    && world[1] >= center[1] - half[1]
                    && world[1] <= center[1] + half[1]
            }
            HitShape::Circle { center, radius } => {
                let dx = world[0] - center[0];
                let dz = world[1] - center[1];
                dx * dx + dz * dz <= radius * radius
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum HitShape {
    Rect { center: [f32; 2], half: [f32; 2] },
    Circle { center: [f32; 2], radius: f32 },
}

pub(crate) fn readable_grid_step(base_step: f32, zoom: f32) -> f32 {
    let mut step = base_step.max(f32::EPSILON);
    while step * zoom < 10.0 && step <= (i32::MAX as f32) * 0.5 {
        step *= 2.0;
    }
    step
}

pub(crate) fn draw_world_grid(
    painter: &egui::Painter,
    transform: ViewportTransform,
    base_step: f32,
) {
    let rect = transform.rect;
    let top_left = transform.screen_to_world(rect.left_top());
    let bottom_right = transform.screen_to_world(rect.right_bottom());
    let step = readable_grid_step(base_step, transform.zoom);
    let min_x = (top_left[0].min(bottom_right[0]) / step).floor() as i32 - 1;
    let max_x = (top_left[0].max(bottom_right[0]) / step).ceil() as i32 + 1;
    let min_z = (top_left[1].min(bottom_right[1]) / step).floor() as i32 - 1;
    let max_z = (top_left[1].max(bottom_right[1]) / step).ceil() as i32 + 1;

    for x in min_x..=max_x {
        let a = transform.world_to_screen([x as f32 * step, min_z as f32 * step]);
        let b = transform.world_to_screen([x as f32 * step, max_z as f32 * step]);
        painter.line_segment([a, b], world_grid_stroke(x));
    }

    for z in min_z..=max_z {
        let a = transform.world_to_screen([min_x as f32 * step, z as f32 * step]);
        let b = transform.world_to_screen([max_x as f32 * step, z as f32 * step]);
        painter.line_segment([a, b], world_grid_stroke(z));
    }
}

fn world_grid_stroke(index: i32) -> Stroke {
    let color = if index == 0 {
        Color32::from_rgb(58, 91, 103)
    } else if index % 4 == 0 {
        Color32::from_rgb(31, 63, 75)
    } else {
        Color32::from_rgb(20, 43, 52)
    };
    Stroke::new(1.0, color)
}

// Selection and visibility state for one 2D BSP scene-viewport pass.
pub(crate) struct SceneViewportContext<'a> {
    pub(crate) hidden_scene_nodes: &'a HashSet<NodeId>,
    pub(crate) selected: NodeId,
    pub(crate) selected_nodes: &'a HashSet<NodeId>,
    pub(crate) show_lights: bool,
}

pub(crate) fn draw_scene_viewport(
    painter: &egui::Painter,
    transform: ViewportTransform,
    project: &ProjectDocument,
    ctx: SceneViewportContext<'_>,
) -> Vec<ViewportHit> {
    let SceneViewportContext {
        hidden_scene_nodes,
        selected,
        selected_nodes,
        show_lights,
    } = ctx;
    let scene = project.active_scene();
    let mut hits = Vec::new();

    for node in scene.nodes() {
        if scene_node_hidden(scene, hidden_scene_nodes, node.id) {
            continue;
        }
        match &node.kind {
            NodeKind::Entity
                if node.children.iter().any(|child| {
                    scene
                        .node(*child)
                        .is_some_and(|child| matches!(child.kind, NodeKind::PointOfInterest { .. }))
                }) =>
            {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "X",
                    Color32::from_rgb(224, 72, 56),
                    &mut hits,
                );
            }
            NodeKind::MeshInstance { .. } => {
                draw_mesh_marker(
                    painter,
                    transform,
                    project,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    &mut hits,
                );
            }
            NodeKind::ImageProp { .. } => {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "I",
                    Color32::from_rgb(210, 170, 120),
                    &mut hits,
                );
            }
            NodeKind::BoxProp { .. } => {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "B",
                    Color32::from_rgb(135, 180, 220),
                    &mut hits,
                );
            }
            NodeKind::CylinderProp { .. } => {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "C",
                    Color32::from_rgb(120, 184, 200),
                    &mut hits,
                );
            }
            NodeKind::SpawnPoint { .. } => {
                draw_spawn_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    &mut hits,
                );
            }
            NodeKind::PointLight { color, radius, .. } if show_lights => {
                draw_light_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    *color,
                    *radius,
                    project.world_sector_size_for_node(node.id),
                    &mut hits,
                );
            }
            NodeKind::ParticleEmitter { .. } => {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "P",
                    Color32::from_rgb(152, 214, 230),
                    &mut hits,
                );
            }
            NodeKind::Node | NodeKind::Node3D if node.id != scene.root => {
                draw_simple_marker(
                    painter,
                    transform,
                    node,
                    selected_nodes.contains(&node.id)
                        || (selected_nodes.is_empty() && selected == node.id),
                    "N",
                    Color32::from_rgb(110, 124, 150),
                    &mut hits,
                );
            }
            _ => {}
        }
    }

    hits
}

pub(crate) fn draw_mesh_marker(
    painter: &egui::Painter,
    transform: ViewportTransform,
    project: &ProjectDocument,
    node: &psxed_project::SceneNode,
    selected: bool,
    hits: &mut Vec<ViewportHit>,
) {
    let NodeKind::MeshInstance { material, .. } = node.kind else {
        return;
    };
    let center = node_world(node);
    // Scale is a fraction of a World sector, like the preview's mesh
    // footprint; keep the marker clickable at any zoom.
    let sector = project.world_sector_size_for_node(node.id).max(1) as f32;
    let min_half = MARKER_RADIUS_PX / transform.zoom;
    let half = [
        (node.transform.scale[0].abs() * sector * 0.5).max(min_half),
        (node.transform.scale[2].abs() * sector * 0.5).max(min_half),
    ];
    let rect = transform.world_rect_to_screen(center, half);
    let color = material_color(project, material, SurfaceRole::Object);
    let translucent = material_is_translucent(project, material);
    painter.rect_filled(rect, 0.0, color);
    if translucent {
        draw_glass_marker(painter, rect);
    }
    painter.rect_stroke(rect, 0.0, selected_stroke(selected), StrokeKind::Outside);
    painter.text(
        rect.center_top() + Vec2::new(0.0, -6.0),
        Align2::CENTER_BOTTOM,
        &node.name,
        FontId::monospace(11.0),
        Color32::from_rgb(232, 238, 246),
    );
    hits.push(ViewportHit::rect(node.id, node.name.clone(), center, half));
}

pub(crate) fn draw_glass_marker(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(
        rect.shrink(1.0),
        0.0,
        Color32::from_rgba_unmultiplied(190, 230, 232, 34),
    );
    let sheen = Stroke::new(1.0, Color32::from_rgba_unmultiplied(222, 252, 255, 120));
    let step = 18.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom()),
                Pos2::new((x + rect.height()).min(rect.right()), rect.top()),
            ],
            sheen,
        );
        x += step;
    }
}

pub(crate) fn draw_spawn_marker(
    painter: &egui::Painter,
    transform: ViewportTransform,
    node: &psxed_project::SceneNode,
    selected: bool,
    hits: &mut Vec<ViewportHit>,
) {
    draw_simple_marker(
        painter,
        transform,
        node,
        selected,
        "P",
        Color32::from_rgb(82, 184, 118),
        hits,
    );
}

/// A point light: its reach (radius in World sectors, as the cook and the 3D
/// preview use it) as a faint disc, and a fixed-size bulb icon.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_light_marker(
    painter: &egui::Painter,
    transform: ViewportTransform,
    node: &psxed_project::SceneNode,
    selected: bool,
    color: [u8; 3],
    radius: f32,
    world_sector_size: i32,
    hits: &mut Vec<ViewportHit>,
) {
    let center = node_world(node);
    let screen_center = transform.world_to_screen(center);
    let reach = radius.max(0.0) * world_sector_size.max(1) as f32;
    if reach.is_finite() {
        painter.circle_filled(
            screen_center,
            transform.screen_radius(reach),
            Color32::from_rgba_unmultiplied(color[0], color[1], color[2], 28),
        );
    }
    let fill = Color32::from_rgb(color[0], color[1], color[2]);
    draw_light_bulb_marker(painter, screen_center, MARKER_RADIUS_PX, fill, selected);
    painter.text(
        screen_center + Vec2::new(0.0, 16.0),
        Align2::CENTER_TOP,
        &node.name,
        FontId::monospace(10.0),
        Color32::from_rgb(220, 228, 238),
    );
    hits.push(ViewportHit::circle(
        node.id,
        node.name.clone(),
        center,
        MARKER_RADIUS_PX / transform.zoom,
    ));
}

pub(crate) fn draw_light_bulb_marker(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    selected: bool,
) {
    let glass_center = center + Vec2::new(0.0, -radius * 0.25);
    let glass_radius = radius * 0.72;
    let glass_fill = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 224);
    let stroke = selected_stroke(selected);
    painter.circle_filled(glass_center, glass_radius, glass_fill);
    painter.circle_stroke(glass_center, glass_radius, stroke);

    let base = Rect::from_center_size(
        center + Vec2::new(0.0, radius * 0.52),
        Vec2::new(radius * 0.86, radius * 0.52),
    );
    painter.rect_filled(base, 2.0, darken(fill, 46));
    painter.rect_stroke(base, 2.0, stroke, StrokeKind::Outside);

    let filament = Stroke::new(1.0, Color32::from_rgba_unmultiplied(18, 20, 24, 190));
    let y = glass_center.y + glass_radius * 0.18;
    let left = glass_center.x - glass_radius * 0.38;
    let right = glass_center.x + glass_radius * 0.38;
    let mid = glass_center.x;
    painter.line_segment(
        [Pos2::new(left, y), Pos2::new(mid, y + glass_radius * 0.18)],
        filament,
    );
    painter.line_segment(
        [Pos2::new(mid, y + glass_radius * 0.18), Pos2::new(right, y)],
        filament,
    );
}

pub(crate) fn draw_simple_marker(
    painter: &egui::Painter,
    transform: ViewportTransform,
    node: &psxed_project::SceneNode,
    selected: bool,
    label: &str,
    fill: Color32,
    hits: &mut Vec<ViewportHit>,
) {
    let center = node_world(node);
    let screen = transform.world_to_screen(center);
    painter.circle_filled(screen, MARKER_RADIUS_PX, fill);
    painter.circle_stroke(screen, MARKER_RADIUS_PX, selected_stroke(selected));
    painter.text(
        screen,
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(12.0),
        Color32::from_rgb(8, 10, 13),
    );
    painter.text(
        screen + Vec2::new(0.0, 16.0),
        Align2::CENTER_TOP,
        &node.name,
        FontId::monospace(10.0),
        Color32::from_rgb(220, 228, 238),
    );
    hits.push(ViewportHit::circle(
        node.id,
        node.name.clone(),
        center,
        MARKER_RADIUS_PX / transform.zoom,
    ));
}

/// Screen radius of a node marker in the 2D views, independent of zoom.
const MARKER_RADIUS_PX: f32 = 8.0;

#[cfg(test)]
mod grid_step_tests {
    use super::*;

    #[test]
    fn the_overlay_uses_a_readable_interval_at_every_zoom() {
        assert_eq!(readable_grid_step(16.0, 1.0), 16.0);
        assert_eq!(readable_grid_step(16.0, 0.25), 64.0);
    }
}

#[cfg(test)]
mod view_orientation_tests {
    use super::*;

    /// Screen direction of a world axis in each orthographic view.
    fn screen_dir(view: OrthographicView, world_axis: usize) -> Vec2 {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0));
        let transform = ViewportTransform::from_focus(rect, view, [0.0, 0.0], 1.0);
        let mut world = [0.0f32; 3];
        world[world_axis] = 10.0;
        transform.world_to_screen(view.project_f32(world)) - rect.center()
    }

    #[test]
    fn the_plan_view_shows_the_3d_view_from_above_unmirrored() {
        // The 3D view at yaw 0 has +X right and looks down -Z, Y up.
        // From above that puts -Z up on screen.
        assert!(screen_dir(OrthographicView::Top, 0).x > 0.0);
        assert!(screen_dir(OrthographicView::Top, 2).y > 0.0, "+Z runs down");

        for view in OrthographicView::ALL {
            let rect = Rect::from_min_size(Pos2::new(5.0, 7.0), Vec2::new(300.0, 200.0));
            let transform = ViewportTransform::from_focus(rect, view, [40.0, -24.0], 2.5);
            let plane = [123.0, -45.0];
            let back = transform.screen_to_world(transform.world_to_screen(plane));
            assert!((back[0] - plane[0]).abs() < 1e-3 && (back[1] - plane[1]).abs() < 1e-3);
        }
        assert_eq!(
            OrthographicView::Top.screen_axis_labels(),
            ["X".to_string(), "-Z".to_string()]
        );
    }
}
