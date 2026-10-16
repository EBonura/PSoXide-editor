//! The material palette and the inspector for selected grid faces, drawn
//! at the top of the Inspector column while a grid world is open.

use super::*;

use psxed_project::grid::edit::{
    edit_uv, material_of, set_material, set_solid, set_wall_facing, toggle_split, uv_of, FaceRef,
    FaceTarget, UvOp,
};
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::GridSplit;

const SWATCH: f32 = 44.0;

impl EditorWorkspace {
    /// Palette, then the properties of the selected faces.
    pub(crate) fn draw_grid_panels(&mut self, ui: &mut egui::Ui) {
        self.draw_grid_palette(ui);
        ui.add_space(6.0);
        self.draw_grid_face_inspector(ui);
        ui.add_space(6.0);
        self.draw_grid_room_inspector(ui);
        ui.add_space(8.0);
        ui.separator();
    }

    /// Apply the palette material to every selected face.
    pub(crate) fn apply_grid_material_to_selection(&mut self, material: ResourceId) -> usize {
        let Some(world) = self.project.active_scene().world.grid() else {
            return 0;
        };
        let faces = self.grid.selection.faces(world);
        if faces.is_empty() {
            return 0;
        }
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return 0;
        };
        let mut changed = 0;
        for face in &faces {
            changed += usize::from(set_material(world, *face, None, Some(material)));
        }
        if changed > 0 {
            self.history.record(before);
            self.grid_changed();
        }
        changed
    }

    fn draw_grid_palette(&mut self, ui: &mut egui::Ui) {
        self.refresh_texture_thumbs(ui.ctx());
        let materials = self.project.material_options();
        egui::CollapsingHeader::new(RichText::new("Materials").strong())
            .default_open(true)
            .show(ui, |ui| {
                if materials.is_empty() {
                    ui.label(RichText::new("No materials in this project").color(STUDIO_TEXT_WEAK));
                    return;
                }
                let scroll_to = self.grid.palette_scroll_to.take();
                egui::ScrollArea::vertical()
                    .id_salt("grid_palette_scroll")
                    .max_height(236.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                            for (id, name) in &materials {
                                let selected = self.grid.material == Some(*id);
                                let response = self.draw_swatch(ui, *id, name, selected);
                                if scroll_to == Some(*id) {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    self.grid.material = Some(*id);
                                    let applied = self.apply_grid_material_to_selection(*id);
                                    self.status = if applied > 0 {
                                        format!("Applied {name} to {applied} faces")
                                    } else {
                                        format!("Material: {name}")
                                    };
                                }
                            }
                        });
                    });
            });
    }

    fn draw_swatch(
        &self,
        ui: &mut egui::Ui,
        id: ResourceId,
        name: &str,
        selected: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(SWATCH), egui::Sense::click());
        let thumb = self.texture_thumbs.get(&id);
        let painter = ui.painter_at(rect);
        match thumb {
            Some(entry) => {
                painter.image(
                    entry.handle.id(),
                    rect.shrink(1.0),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                painter.rect_filled(rect, 2.0, Color32::from_rgb(52, 56, 66));
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    name.chars().next().unwrap_or('?').to_string(),
                    FontId::proportional(16.0),
                    STUDIO_TEXT_WEAK,
                );
            }
        }
        let stroke = if selected {
            Stroke::new(2.5, STUDIO_ACCENT)
        } else if response.hovered() {
            Stroke::new(1.5, Color32::from_rgb(200, 200, 210))
        } else {
            Stroke::new(1.0, STUDIO_BORDER)
        };
        painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
        response.on_hover_text(name)
    }

    fn draw_grid_face_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let faces = self.grid.selection.faces(world);
        egui::CollapsingHeader::new(RichText::new("Selection").strong())
            .default_open(true)
            .show(ui, |ui| {
                let Some(&primary) = faces.first() else {
                    ui.label(
                        RichText::new("Select faces with the Select tool").color(STUDIO_TEXT_WEAK),
                    );
                    return;
                };
                let (mut floors, mut ceilings, mut walls) = (0, 0, 0);
                for f in &faces {
                    match f.target {
                        FaceTarget::Surface(psxed_project::grid::edit::SurfaceKind::Floor) => {
                            floors += 1
                        }
                        FaceTarget::Surface(_) => ceilings += 1,
                        FaceTarget::Wall { .. } => walls += 1,
                    }
                }
                let plural = |n: usize, one: &str| {
                    if n == 1 {
                        format!("{n} {one}")
                    } else {
                        format!("{n} {one}s")
                    }
                };
                ui.label(format!(
                    "{}: {}, {}, {}",
                    plural(faces.len(), "face"),
                    plural(floors, "floor"),
                    plural(ceilings, "ceiling"),
                    plural(walls, "wall"),
                ));
                self.draw_primary_face_controls(ui, primary, &faces);
            });
    }

    fn draw_primary_face_controls(
        &mut self,
        ui: &mut egui::Ui,
        primary: FaceRef,
        faces: &[FaceRef],
    ) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let name_of = |ws: &Self, id: Option<ResourceId>| {
            id.and_then(|id| ws.project.resource_name(id))
                .unwrap_or("none")
                .to_string()
        };
        let is_surface = matches!(primary.target, FaceTarget::Surface(_));
        let split = primary.surface(world).map(|s| {
            (
                s.split,
                s.tri_b.is_some() || s.heights_b.is_some() || !s.is_planar(),
            )
        });
        let mat_a = material_of(world, primary, Some(0));
        let mat_b = material_of(world, primary, Some(1));
        let uv = uv_of(world, primary, None);
        let heights = primary.heights(world, 0);
        ui.separator();
        ui.label(
            RichText::new(match primary.target {
                FaceTarget::Surface(k) => {
                    format!("{} at {},{}", k.label(), primary.cell[0], primary.cell[1])
                }
                FaceTarget::Wall { edge, index } => format!(
                    "{} wall {index} at {},{}",
                    edge.label(),
                    primary.cell[0],
                    primary.cell[1]
                ),
            })
            .strong(),
        );
        if let Some(h) = heights {
            ui.label(
                RichText::new(format!("heights {} {} {} {}", h[0], h[1], h[2], h[3]))
                    .size(11.5)
                    .color(STUDIO_TEXT_WEAK),
            );
        }
        let mut ops: Vec<UvOp> = Vec::new();
        let mut toggle = false;
        let mut facing: Option<WallFacing> = None;
        let mut solid: Option<bool> = None;
        ui.horizontal(|ui| {
            ui.label("Material");
            ui.label(RichText::new(name_of(self, mat_a)).color(STUDIO_ACCENT));
        });
        if is_surface {
            if let Some((s, bent)) = split {
                ui.horizontal(|ui| {
                    ui.label("Split");
                    if ui
                        .button(match s {
                            GridSplit::NwSe => "NW to SE",
                            GridSplit::NeSw => "NE to SW",
                        })
                        .on_hover_text("Flip the diagonal")
                        .clicked()
                    {
                        toggle = true;
                    }
                    if !bent {
                        ui.label(
                            RichText::new("(flat: no visible change)")
                                .color(STUDIO_TEXT_WEAK)
                                .size(10.0),
                        );
                    }
                });
                if mat_a != mat_b {
                    ui.label(
                        RichText::new(format!(
                            "Triangles: {} / {}",
                            name_of(self, mat_a),
                            name_of(self, mat_b)
                        ))
                        .size(11.0)
                        .color(STUDIO_TEXT_WEAK),
                    );
                }
            }
        }
        if let Some(mut uv) = uv {
            ui.label(RichText::new("Texture").color(STUDIO_TEXT_WEAK).size(11.0));
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Offset");
                changed |= ui
                    .add(egui::DragValue::new(&mut uv.offset_texels[0]).speed(0.25))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut uv.offset_texels[1]).speed(0.25))
                    .changed();
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
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Angle");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut uv.rotation_deg)
                            .speed(1.0)
                            .suffix(" deg"),
                    )
                    .changed();
            });
            if changed {
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
        }
        match primary.target {
            FaceTarget::Wall { .. } => {
                if let Some(w) = primary.wall_of(world) {
                    let mut current = w.facing;
                    let mut solid_now = w.solid;
                    ui.horizontal(|ui| {
                        ui.label("Faces");
                        egui::ComboBox::from_id_salt("grid_wall_facing")
                            .selected_text(match current {
                                WallFacing::Front => "inside",
                                WallFacing::Back => "outside",
                                WallFacing::Both => "both sides",
                            })
                            .show_ui(ui, |ui| {
                                for (value, label) in [
                                    (WallFacing::Front, "inside"),
                                    (WallFacing::Back, "outside"),
                                    (WallFacing::Both, "both sides"),
                                ] {
                                    if ui.selectable_label(current == value, label).clicked() {
                                        current = value;
                                    }
                                }
                            });
                    });
                    if current != w.facing {
                        facing = Some(current);
                    }
                    if ui.checkbox(&mut solid_now, "Solid").changed() {
                        solid = Some(solid_now);
                    }
                }
            }
            FaceTarget::Surface(_) => {
                if let Some(s) = primary.surface(world) {
                    let mut walkable = s.walkable;
                    if ui.checkbox(&mut walkable, "Walkable").changed() {
                        solid = Some(walkable);
                    }
                }
            }
        }
        // Apply what changed to every selected face.
        if toggle || !ops.is_empty() || facing.is_some() || solid.is_some() {
            let faces = faces.to_vec();
            let before = self.project.clone();
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                for face in &faces {
                    if toggle {
                        toggle_split(world, *face);
                    }
                    for op in &ops {
                        edit_uv(world, *face, None, *op);
                    }
                    if let Some(f) = facing {
                        set_wall_facing(world, *face, f);
                    }
                    if let Some(v) = solid {
                        set_solid(world, *face, v);
                    }
                }
            }
            // The Inspector records the undo step, coalescing a drag.
            if self.project != before {
                self.grid_changed();
            }
        }
    }

    fn draw_grid_room_inspector(&mut self, ui: &mut egui::Ui) {
        let room = self.grid_target_room();
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(r) = world.rooms.get(room) else {
            return;
        };
        let (mut y_clicks, mut ambient, mut name) = (
            r.y_offset as f32 / psxed_project::grid::CLICK_UNITS as f32,
            r.ambient,
            r.name.clone(),
        );
        let room_count = world.rooms.len();
        let (mut add_room, mut delete_room) = (false, false);
        let before = self.project.clone();
        let mut changed = false;
        egui::CollapsingHeader::new(RichText::new(format!("Room {room}")).strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    changed |= ui.text_edit_singleline(&mut name).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Raise");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut y_clicks)
                                .speed(0.1)
                                .suffix(" clicks")
                                .max_decimals(2),
                        )
                        .on_hover_text("Vertical offset of the whole room")
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Ambient");
                    for c in &mut ambient {
                        changed |= ui.add(egui::DragValue::new(c).range(0..=255)).changed();
                    }
                });
                ui.horizontal(|ui| {
                    add_room = ui
                        .button("Add room")
                        .on_hover_text("A new empty room, raised one storey above the highest")
                        .clicked();
                    delete_room = ui
                        .add_enabled(room_count > 1, egui::Button::new("Delete room"))
                        .on_hover_text("Remove this room and everything in it")
                        .clicked();
                });
            });
        if add_room {
            self.grid_add_room();
        }
        if delete_room {
            self.grid_delete_room(room);
        }
        if changed {
            if let Some(r) = self
                .project
                .active_scene_mut()
                .world
                .grid_mut()
                .and_then(|w| w.rooms.get_mut(room))
            {
                r.name = name;
                r.ambient = ambient;
                r.y_offset = psxed_project::grid::units::snap_to(
                    (y_clicks * psxed_project::grid::CLICK_UNITS as f32).round() as i32,
                    psxed_project::grid::FINE_UNITS,
                );
            }
            if self.project != before {
                self.grid_changed();
            }
        }
    }
}

impl EditorWorkspace {
    /// Add an empty room one storey above the highest room and make it the
    /// active one. One undo step.
    pub(crate) fn grid_add_room(&mut self) {
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        if world.rooms.len() >= psxed_project::grid::limits::MAX_ROOMS {
            self.status = "Too many rooms".to_string();
            return;
        }
        let top = world.rooms.iter().map(|r| r.y_offset).max().unwrap_or(0);
        let origin = world.rooms.last().map_or([0, 0], |r| r.origin);
        let mut room =
            psxed_project::grid::GridRoom::new(format!("Room {}", world.rooms.len()), 1, 1);
        room.origin = origin;
        room.y_offset = top + 3 * psxed_project::grid::SECTOR_UNITS;
        world.rooms.push(room);
        let index = world.rooms.len() - 1;
        self.grid_active_room = Some(index);
        self.grid.last_room = index;
        self.history.record(before);
        self.status = format!("Added room {index}; draw its floor with the Floor tool");
        self.grid_changed();
    }

    /// Remove room `index`, keeping the hidden set, the active room and the
    /// selection pointing at the right rooms. One undo step.
    pub(crate) fn grid_delete_room(&mut self, index: usize) {
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        if world.rooms.len() <= 1 || index >= world.rooms.len() {
            return;
        }
        world.rooms.remove(index);
        let shift = |i: usize| if i > index { i - 1 } else { i };
        self.grid_hidden_rooms = self
            .grid_hidden_rooms
            .iter()
            .filter(|i| **i != index)
            .map(|i| shift(*i))
            .collect();
        self.grid_active_room = self.grid_active_room.filter(|i| *i != index).map(shift);
        self.grid.last_room = shift(self.grid.last_room).min(self.grid_room_count() - 1);
        self.grid.selection.clear();
        self.history.record(before);
        self.status = format!("Deleted room {index}");
        self.grid_changed();
    }

    fn grid_room_count(&self) -> usize {
        self.project
            .active_scene()
            .world
            .grid()
            .map_or(1, |w| w.rooms.len().max(1))
    }
}
