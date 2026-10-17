//! The Inspector section for the selected free plane(s).
//!
//! One context at a time (the face section is not drawn while planes are
//! selected): a header that says what is selected, the transform as numbers,
//! material, texture placement, sidedness, light and the collision class,
//! each with the label of what it cooks to. Edits to the transform apply to
//! the primary plane; everything else applies to every selected plane. The
//! Inspector wraps the frame in an undo step, so a drag coalesces.

use super::*;

use psxed_project::grid::edit::{update_plane, UvOp};
use psxed_project::grid::plane::{MAX_PLANE_SIZE, MIN_PLANE_SIZE};
use psxed_project::grid::{PlaneShape, PlaneUvMode};

/// What a plane with this collision class does at runtime, for the label.
pub(super) fn collision_summary(plane: &GridPlane) -> String {
    match (plane.collision, plane.shape()) {
        (PlaneCollision::Decor, _) => "Drawn only, nothing collides with it.".to_string(),
        (_, PlaneShape::None) => "Hidden: no collision.".to_string(),
        (PlaneCollision::Walkable, PlaneShape::Platform) => {
            "Cooks to a platform: the character stands on it, up to the walk slope.".to_string()
        }
        (PlaneCollision::Walkable, _) => {
            "Too steep to stand on (limit 45 degrees): cooks to a blocker.".to_string()
        }
        (_, PlaneShape::Platform) => {
            "Cooks to a solid slab, which the character can also stand on.".to_string()
        }
        (_, PlaneShape::Wall) => "Cooks to a wall segment that blocks both sides.".to_string(),
        (_, PlaneShape::Blocker) => {
            "Leans between wall and floor: cooks to a box around it, which over-blocks.".to_string()
        }
    }
}

impl EditorWorkspace {
    /// Apply the palette material to every selected plane.
    pub(crate) fn apply_material_to_selected_planes(&mut self, material: ResourceId) -> usize {
        self.retain_valid_planes();
        if self.grid.plane_sel.is_empty() {
            return 0;
        }
        let ids = self.grid.plane_sel.clone();
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return 0;
        };
        let mut changed = 0;
        for id in ids {
            changed += usize::from(psxed_project::grid::edit::set_plane_material(
                world,
                id,
                Some(material),
            ));
        }
        if changed > 0 {
            self.history.record(before);
            self.grid_changed();
        }
        changed
    }

    /// The plane section of the Inspector.
    pub(crate) fn draw_plane_inspector(&mut self, ui: &mut egui::Ui) {
        self.retain_valid_planes();
        let Some(primary) = self.plane_primary() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(original) = primary.get(world).cloned() else {
            return;
        };
        let count = self.grid.plane_sel.len();
        let material_name = original
            .material
            .and_then(|id| self.project.resource_name(id))
            .unwrap_or("none")
            .to_string();
        let mut edited = original.clone();
        let mut ops: Vec<UvOp> = Vec::new();
        let mut delete = false;
        egui::CollapsingHeader::new(RichText::new("Plane").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(if count > 1 {
                        format!(
                            "{count} planes selected; room {} plane {} shown",
                            primary.room, primary.index
                        )
                    } else {
                        format!("Room {} plane {}", primary.room, primary.index)
                    })
                    .strong(),
                );
                ui.label(
                    RichText::new(collision_summary(&edited))
                        .size(11.0)
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.separator();
                ui.label(RichText::new("Transform").color(STUDIO_TEXT_WEAK).size(11.0));
                ui.horizontal(|ui| {
                    ui.label("Pos");
                    for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut edited.center[axis])
                                .speed(16.0)
                                .prefix(format!("{name} ")),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Size");
                    for (axis, name) in ["w", "h"].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut edited.size[axis])
                                .speed(16.0)
                                .range(MIN_PLANE_SIZE..=MAX_PLANE_SIZE)
                                .prefix(format!("{name} ")),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Turn");
                    for (axis, name) in ["yaw", "pitch", "roll"].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut edited.rotation[axis])
                                .speed(1.0)
                                .range(-180..=180)
                                .prefix(format!("{name} "))
                                .suffix("\u{b0}"),
                        );
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (label, pitch, hint) in [
                        ("Wall", 0, "Stand upright"),
                        ("Floor", 90, "Lay flat, facing up"),
                        ("Ceiling", -90, "Lay flat, facing down"),
                    ] {
                        if ui.button(label).on_hover_text(hint).clicked() {
                            edited.rotation[1] = pitch;
                            edited.rotation[2] = 0;
                        }
                    }
                    if ui
                        .button("Turn 90")
                        .on_hover_text("Quarter turn about the vertical axis (R)")
                        .clicked()
                    {
                        edited.rotation[0] += 90;
                    }
                });
                ui.label(
                    RichText::new("Grid units: a sector is 1024, a click 256. Yaw 0 faces +Z.")
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Material");
                    ui.label(RichText::new(&material_name).color(STUDIO_ACCENT));
                });
                ui.label(
                    RichText::new("Click a swatch above to apply. Blend and alpha cutout come from the material.")
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.horizontal(|ui| {
                    ui.label("Mapping");
                    for (mode, label) in [(PlaneUvMode::Tile, "Tile"), (PlaneUvMode::Stretch, "Stretch")] {
                        if ui.selectable_label(edited.uv_mode == mode, label).clicked() {
                            edited.uv_mode = mode;
                        }
                    }
                });
                let mut uv = edited.uv;
                ui.horizontal(|ui| {
                    ui.label("Offset");
                    ui.add(egui::DragValue::new(&mut uv.offset_texels[0]).speed(0.25));
                    ui.add(egui::DragValue::new(&mut uv.offset_texels[1]).speed(0.25));
                });
                ui.horizontal(|ui| {
                    ui.label("Scale ");
                    let mut su = f32::from(uv.scale_q8[0]) / 256.0;
                    let mut sv = f32::from(uv.scale_q8[1]) / 256.0;
                    let a = ui
                        .add(egui::DragValue::new(&mut su).speed(0.01).max_decimals(2))
                        .changed();
                    let b = ui
                        .add(egui::DragValue::new(&mut sv).speed(0.01).max_decimals(2))
                        .changed();
                    if a || b {
                        uv.scale_q8 = [(su * 256.0).round() as i16, (sv * 256.0).round() as i16];
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Angle");
                    ui.add(
                        egui::DragValue::new(&mut uv.rotation_deg)
                            .speed(1.0)
                            .suffix(" deg"),
                    );
                });
                if uv != edited.uv {
                    ops.push(UvOp::SetOffset(uv.offset_texels));
                    ops.push(UvOp::SetScale(uv.scale_q8));
                    ops.push(UvOp::SetRotation(uv.rotation_deg));
                }
                ui.horizontal_wrapped(|ui| {
                    for (label, op) in [
                        ("Flip U", UvOp::FlipU),
                        ("Flip V", UvOp::FlipV),
                        ("Rotate 90", UvOp::Rotate90),
                        ("Reset", UvOp::Reset),
                    ] {
                        if ui.button(label).clicked() {
                            ops.push(op);
                        }
                    }
                });
                ui.separator();
                ui.checkbox(&mut edited.double_sided, "Both sides")
                    .on_hover_text("Draw the back too (banners, grates, foliage)");
                ui.checkbox(&mut edited.fullbright, "Fullbright")
                    .on_hover_text("Show the material unlit instead of baking light into it");
                ui.checkbox(&mut edited.enabled, "Enabled")
                    .on_hover_text("A disabled plane is neither drawn nor collided with");
                ui.horizontal(|ui| {
                    ui.label("Collision");
                    egui::ComboBox::from_id_salt("plane_collision")
                        .selected_text(edited.collision.label())
                        .show_ui(ui, |ui| {
                            for class in PlaneCollision::ALL {
                                ui.selectable_value(&mut edited.collision, class, class.label());
                            }
                        });
                });
                ui.separator();
                if ui.button(icons::label(icons::TRASH, "Delete plane")).clicked() {
                    delete = true;
                }
            });
        if delete {
            plane_tool::delete_selected(self);
            return;
        }
        let ids = self.grid.plane_sel.clone();
        let transform_changed = edited.center != original.center
            || edited.size != original.size
            || edited.rotation != original.rotation;
        let shared_changed = edited.uv_mode != original.uv_mode
            || edited.double_sided != original.double_sided
            || edited.fullbright != original.fullbright
            || edited.enabled != original.enabled
            || edited.collision != original.collision;
        if !transform_changed && !shared_changed && ops.is_empty() {
            return;
        }
        if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
            if transform_changed {
                update_plane(world, primary, |p| {
                    p.center = edited.center;
                    p.size = edited.size;
                    p.rotation = edited.rotation;
                });
            }
            for id in ids {
                update_plane(world, id, |p| {
                    if shared_changed {
                        p.uv_mode = edited.uv_mode;
                        p.double_sided = edited.double_sided;
                        p.fullbright = edited.fullbright;
                        p.enabled = edited.enabled;
                        p.collision = edited.collision;
                    }
                    for op in &ops {
                        op.apply(&mut p.uv);
                    }
                });
            }
        }
        // The Inspector records the undo step, coalescing a drag.
        self.grid_changed();
    }
}
