//! Editor-only viewport overlay drawing: selection, paint ghosts, bounds, and gizmos.

use super::*;

#[derive(Copy, Clone)]
pub(super) struct FaceOutlineStyle {
    pub(super) rgb: (u8, u8, u8),
    pub(super) thickness_px: f32,
}

pub(super) const ENTITY_BOUND_HOVER: FaceOutlineStyle = FaceOutlineStyle {
    rgb: (0xFF, 0xE0, 0x60),
    thickness_px: EDITOR_PREVIEW_HOVER_STROKE_WIDTH,
};
pub(super) const ENTITY_BOUND_SELECTED: FaceOutlineStyle = FaceOutlineStyle {
    rgb: (0x60, 0xC8, 0xFF),
    thickness_px: EDITOR_PREVIEW_SELECTED_STROKE_WIDTH,
};

/// Ring + bulb gizmos for world-space lights. Positions are raw world
/// units; the radius scales by the World sector size, matching the runtime
/// light records.
pub(super) fn walk_light_gizmos(
    project: &ProjectDocument,
    camera: psx_engine::WorldCamera,
    hidden_scene_nodes: &HashSet<NodeId>,
    selected: psxed_project::NodeId,
    hovered: Option<psxed_project::NodeId>,
    scratch: &mut PreviewScratch,
) {
    let scene = project.active_scene();
    let radius_units = scene
        .world_sector_size_for_node(scene.root)
        .unwrap_or(1024)
        .max(1) as f32;
    {
        for light in preview_lights(scene, hidden_scene_nodes) {
            let center_world = light
                .transform
                .translation
                .map(|value| value.round() as i32);
            let center_world = [center_world[0], center_world[1], center_world[2]];

            let is_selected = preview_reference_selected(selected, light.host_id, None, None, None);
            let is_hovered = hovered
                .is_some_and(|id| preview_reference_selected(id, light.host_id, None, None, None));
            let style = if is_selected {
                FaceOutlineStyle {
                    rgb: (0xFF, 0xE0, 0x80),
                    thickness_px: EDITOR_PREVIEW_SELECTED_STROKE_WIDTH,
                }
            } else if is_hovered {
                FaceOutlineStyle {
                    rgb: (0xFF, 0xF0, 0x90),
                    thickness_px: EDITOR_PREVIEW_HOVER_STROKE_WIDTH,
                }
            } else {
                FaceOutlineStyle {
                    rgb: (
                        light.color[0].max(0x40),
                        light.color[1].max(0x40),
                        light.color[2].max(0x40),
                    ),
                    thickness_px: EDITOR_PREVIEW_HOVER_STROKE_WIDTH,
                }
            };
            if is_selected || is_hovered {
                if let Some(radius_engine) =
                    preview_light_radius_world_units(light.radius, radius_units)
                {
                    push_horizontal_ring(scratch, camera, center_world, radius_engine, 16, style);
                }
            }
            push_light_bulb_icon(
                scratch,
                center_world,
                (
                    light.color[0].max(0x40),
                    light.color[1].max(0x40),
                    light.color[2].max(0x40),
                ),
                is_selected || is_hovered,
            );
        }
    }
}

/// Wireframe AABB + facing arrow per selectable scene entity.
/// Bounds are gathered by `EditorWorkspace::collect_entity_bounds`
/// -- every entity-kind node carries an AABB the user can click to
/// select and drag to move. Image props, box props, and portals render
/// their own authored frames/seams, so this pass leaves those out and
/// draws simple manipulation boxes for the remaining object-like nodes.
///
/// Idle bounds draw thin and muted so they don't dominate the
/// viewport over the room they sit in. Hover and selected reuse
/// the room face palette for cross-tool consistency: yellow for
/// hover, cyan-bold for selected.
pub(super) fn walk_entity_bounds(
    bounds: &[psxed_ui::EntityBounds],
    selected: psxed_project::NodeId,
    hovered: Option<psxed_project::NodeId>,
    scratch: &mut PreviewScratch,
) {
    {
        for b in bounds {
            if matches!(
                b.kind,
                psxed_ui::EntityBoundKind::ImageProp
                    | psxed_ui::EntityBoundKind::BoxProp
                    | psxed_ui::EntityBoundKind::CylinderProp
                    | psxed_ui::EntityBoundKind::PointOfInterest
            ) {
                continue;
            }
            let is_selected = b.node == selected;
            let is_hovered = hovered == Some(b.node);
            let style = entity_bound_style(b.kind, is_selected, is_hovered);
            push_aabb_wireframe(scratch, b.center, b.half_extents, style);

            // Yaw arrow only for kinds with meaningful facing --
            // models and spawn points point at where they'll
            // render / face. Lights and portals are either
            // omnidirectional or carry their own
            // direction gizmo elsewhere (light radius ring).
            if matches!(
                b.kind,
                psxed_ui::EntityBoundKind::Model
                    | psxed_ui::EntityBoundKind::SpawnPoint
                    | psxed_ui::EntityBoundKind::MeshFallback
            ) {
                push_facing_arrow(scratch, b.center, b.half_extents, b.yaw_degrees, style);
            }
        }
    }
}

/// Pick the outline style for one bound. Selected wins over
/// hover; idle uses a muted kind-tinted thin line so multiple
/// boxes in a busy room read at a glance without dominating.
pub(super) fn entity_bound_style(
    kind: psxed_ui::EntityBoundKind,
    selected: bool,
    hovered: bool,
) -> FaceOutlineStyle {
    if selected {
        return ENTITY_BOUND_SELECTED;
    }
    if hovered {
        return ENTITY_BOUND_HOVER;
    }
    let rgb = match kind {
        psxed_ui::EntityBoundKind::Model => (0xC0, 0xC8, 0xD0),
        psxed_ui::EntityBoundKind::MeshFallback => (0x90, 0x98, 0xA0),
        psxed_ui::EntityBoundKind::ImageProp => (0xD0, 0xAA, 0x78),
        psxed_ui::EntityBoundKind::BoxProp => (0x87, 0xB4, 0xDC),
        psxed_ui::EntityBoundKind::CylinderProp => (0x78, 0xB8, 0xC8),
        psxed_ui::EntityBoundKind::ArchProp => (0xC0, 0xA0, 0x70),
        psxed_ui::EntityBoundKind::SpawnPoint => (0x60, 0xE0, 0x80),
        psxed_ui::EntityBoundKind::PointLight => (0xFF, 0xD8, 0x70),
        psxed_ui::EntityBoundKind::ParticleEmitter => (0x98, 0xD6, 0xE6),
        psxed_ui::EntityBoundKind::PointOfInterest => (0xE0, 0x48, 0x38),
        psxed_ui::EntityBoundKind::VitalityCircleHorizon => (0xFF, 0x58, 0x30),
        psxed_ui::EntityBoundKind::VitalityCircleZenith => (0x40, 0xE8, 0xC0),
        psxed_ui::EntityBoundKind::DestructibleHorizon => (0xFF, 0x64, 0x3A),
        psxed_ui::EntityBoundKind::DestructibleZenith => (0x52, 0xC2, 0xB2),
        psxed_ui::EntityBoundKind::DestructibleBoth => (0xF0, 0xB8, 0x48),
        psxed_ui::EntityBoundKind::Logic => (0xC8, 0x8C, 0xE8),
    };
    FaceOutlineStyle {
        rgb,
        thickness_px: EDITOR_PREVIEW_HOVER_STROKE_WIDTH,
    }
}

/// Project the 8 corners of a world-space AABB and emit the 12
/// edges as `push_screen_line` segments. Coordinates are stored
/// `f32` in the bound; rounded to `i32` here because the GTE
/// shim wants integer world coords.
pub(super) fn push_aabb_wireframe(
    scratch: &mut PreviewScratch,
    center: [f32; 3],
    half_extents: [f32; 3],
    style: FaceOutlineStyle,
) {
    let cx = center[0].round() as i32;
    let cy = center[1].round() as i32;
    let cz = center[2].round() as i32;
    let hx = half_extents[0].round() as i32;
    let hy = half_extents[1].round() as i32;
    let hz = half_extents[2].round() as i32;
    if hx <= 0 || hy <= 0 || hz <= 0 {
        return;
    }
    let lo = [cx - hx, cy - hy, cz - hz];
    let hi = [cx + hx, cy + hy, cz + hz];
    // Corner index encoding: bit0 = X (lo/hi), bit1 = Y, bit2 = Z.
    let corner = |i: usize| -> [i32; 3] {
        [
            if i & 1 != 0 { hi[0] } else { lo[0] },
            if i & 2 != 0 { hi[1] } else { lo[1] },
            if i & 4 != 0 { hi[2] } else { lo[2] },
        ]
    };
    let corners: [_; 8] = std::array::from_fn(corner);
    push_world_box_wireframe(scratch, corners, style);
}

pub(super) fn push_world_box_wireframe(
    scratch: &mut PreviewScratch,
    corners: [[i32; 3]; 8],
    style: FaceOutlineStyle,
) {
    let p = corners.map(|corner| gte_scene::project_vertex(world_to_view(corner)));
    // 12 edges of a box: 4 along X, 4 along Y, 4 along Z. Pairs
    // of corners that differ in exactly one bit.
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7), // along X
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7), // along Y
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // along Z
    ];
    for (a, b) in EDGES {
        if p[a].sz == 0 || p[b].sz == 0 {
            continue;
        }
        push_screen_line(scratch, p[a], p[b], style);
    }
}

pub(super) fn selected_node_is_image_prop(project: &ProjectDocument, selected: NodeId) -> bool {
    project.active_scene().node(selected).is_some_and(|node| {
        matches!(
            node.kind,
            NodeKind::ImageProp { .. } | NodeKind::BoxProp { .. } | NodeKind::CylinderProp { .. }
        )
    })
}

/// Draw a forward-pointing arrow from the bound centre out
/// past the front face, indicating where the entity faces.
/// Length scales with the bound's horizontal extent so big
/// models get a visible arrow and tiny markers don't grow
/// horns.
pub(super) fn push_facing_arrow(
    scratch: &mut PreviewScratch,
    center: [f32; 3],
    half_extents: [f32; 3],
    yaw_degrees: f32,
    style: FaceOutlineStyle,
) {
    let yaw_q12 = yaw_to_q12(yaw_degrees);
    let s = sin_q12_turn(yaw_q12);
    let c = cos_q12_turn(yaw_q12);
    // Arrow length = bound's horizontal half-extent + a small
    // overshoot so the head sits clearly outside the box.
    let reach = (half_extents[0].max(half_extents[2]) * 1.5).max(96.0) as i32;
    let cx = center[0].round() as i32;
    let cy = center[1].round() as i32;
    let cz = center[2].round() as i32;
    let tip = [cx + ((s * reach) >> 12), cy, cz + ((c * reach) >> 12)];
    let p_origin = gte_scene::project_vertex(world_to_view([cx, cy, cz]));
    let p_tip = gte_scene::project_vertex(world_to_view(tip));
    if p_origin.sz != 0 && p_tip.sz != 0 {
        push_screen_line(scratch, p_origin, p_tip, style);
    }
}

/// Project a horizontal `segments`-sided polygon at world
/// `center` with `radius` into screen space and emit one
/// `push_screen_line` per edge. Used for light radius gizmos
/// and any future ground-plane affordances.
pub(super) fn push_horizontal_ring(
    scratch: &mut PreviewScratch,
    camera: psx_engine::WorldCamera,
    center: [i32; 3],
    radius: i32,
    segments: u16,
    style: FaceOutlineStyle,
) {
    if segments < 3 || radius <= 0 {
        return;
    }
    let mut prev_world = [center[0].saturating_add(radius), center[1], center[2]];
    for i in 1..=segments {
        // Authored editor angles use 4096 units per turn; sample
        // the unit circle around the light origin once per segment.
        let angle_q12 = ((i as u32 * 4096) / segments as u32) as u16;
        let s = sin_q12_turn(angle_q12);
        let c = cos_q12_turn(angle_q12);
        let next_world = [
            center[0].saturating_add(q12_scale_radius(c, radius)),
            center[1],
            center[2].saturating_add(q12_scale_radius(s, radius)),
        ];
        if let Some([prev_proj, next_proj]) =
            clip_preview_world_segment(camera, prev_world, next_world)
        {
            push_screen_line(scratch, prev_proj, next_proj, style);
        }
        prev_world = next_world;
    }
}

fn q12_scale_radius(q12: i32, radius: i32) -> i32 {
    ((i64::from(q12) * i64::from(radius)) >> 12).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
        as i32
}

pub(super) fn push_light_bulb_icon(
    scratch: &mut PreviewScratch,
    center_world: [i32; 3],
    rgb: (u8, u8, u8),
    emphasized: bool,
) {
    let projected = gte_scene::project_vertex(world_to_view(center_world));
    if projected.sz == 0 {
        return;
    }
    let cx = projected.sx as f32;
    let cy = projected.sy as f32;
    let radius = if emphasized { 8.5 } else { 7.0 };
    let icon = FaceOutlineStyle {
        rgb,
        thickness_px: if emphasized { 1.0 } else { 0.75 },
    };
    let glass = egui::pos2(cx, cy - radius * 0.28);
    let segments = 16;
    for i in 0..segments {
        let a0 = i as f32 * std::f32::consts::TAU / segments as f32;
        let a1 = (i + 1) as f32 * std::f32::consts::TAU / segments as f32;
        push_overlay_line(
            scratch,
            egui::pos2(glass.x + a0.cos() * radius, glass.y + a0.sin() * radius),
            egui::pos2(glass.x + a1.cos() * radius, glass.y + a1.sin() * radius),
            icon,
        );
    }

    let neck_y = glass.y + radius * 0.72;
    let base_y = neck_y + radius * 0.62;
    let neck_half = radius * 0.46;
    let base_half = radius * 0.34;
    push_overlay_line(
        scratch,
        egui::pos2(cx - neck_half, neck_y),
        egui::pos2(cx - base_half, base_y),
        icon,
    );
    push_overlay_line(
        scratch,
        egui::pos2(cx + neck_half, neck_y),
        egui::pos2(cx + base_half, base_y),
        icon,
    );
    for step in [0.0, 0.28, 0.56] {
        let y = neck_y + radius * step;
        push_overlay_line(
            scratch,
            egui::pos2(cx - base_half, y),
            egui::pos2(cx + base_half, y),
            icon,
        );
    }

    let filament_y = glass.y + radius * 0.18;
    push_overlay_line(
        scratch,
        egui::pos2(cx - radius * 0.36, filament_y),
        egui::pos2(cx - radius * 0.12, filament_y + radius * 0.2),
        icon,
    );
    push_overlay_line(
        scratch,
        egui::pos2(cx - radius * 0.12, filament_y + radius * 0.2),
        egui::pos2(cx + radius * 0.12, filament_y + radius * 0.2),
        icon,
    );
    push_overlay_line(
        scratch,
        egui::pos2(cx + radius * 0.12, filament_y + radius * 0.2),
        egui::pos2(cx + radius * 0.36, filament_y),
        icon,
    );

    if emphasized {
        let halo = FaceOutlineStyle {
            rgb: (0xFF, 0xFF, 0xFF),
            thickness_px: 0.45,
        };
        let halo_radius = radius + 2.0;
        for i in 0..segments {
            let a0 = i as f32 * std::f32::consts::TAU / segments as f32;
            let a1 = (i + 1) as f32 * std::f32::consts::TAU / segments as f32;
            push_overlay_line(
                scratch,
                egui::pos2(
                    glass.x + a0.cos() * halo_radius,
                    glass.y + a0.sin() * halo_radius,
                ),
                egui::pos2(
                    glass.x + a1.cos() * halo_radius,
                    glass.y + a1.sin() * halo_radius,
                ),
                halo,
            );
        }
    }
}

pub(super) fn push_screen_line(
    scratch: &mut PreviewScratch,
    a: psx_gte::scene::Projected,
    b: psx_gte::scene::Projected,
    style: FaceOutlineStyle,
) {
    push_overlay_line(
        scratch,
        egui::pos2(a.sx as f32, a.sy as f32),
        egui::pos2(b.sx as f32, b.sy as f32),
        style,
    );
}

pub(super) fn push_overlay_line(
    scratch: &mut PreviewScratch,
    a: egui::Pos2,
    b: egui::Pos2,
    style: FaceOutlineStyle,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.5 {
        return;
    }
    scratch
        .overlay_lines
        .push(psxed_ui::EditorViewportOverlayLine::new(
            a,
            b,
            bright_overlay_color(style.rgb),
            style.thickness_px,
        ));
}

pub(super) fn bright_overlay_color(rgb: (u8, u8, u8)) -> egui::Color32 {
    let lift = |channel: u8| -> u8 {
        let c = channel as u16;
        (c + ((255 - c) * 3 / 5)).min(255) as u8
    };
    egui::Color32::from_rgb(lift(rgb.0), lift(rgb.1), lift(rgb.2))
}
