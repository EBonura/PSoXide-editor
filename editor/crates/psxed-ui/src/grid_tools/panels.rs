//! The material palette and the inspector for selected grid faces, drawn
//! at the top of the Inspector column while a grid world is open.

use super::*;

use psxed_project::grid::edit::{
    edit_uv, material_of, reset_derived, set_derived_hidden, set_material, set_solid,
    set_wall_facing, toggle_split, uv_of, FaceRef, FaceTarget, UvOp,
};
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::GridSplit;

const SWATCH: f32 = 44.0;

impl EditorWorkspace {
    /// Palette, then the properties of the selected faces.
    pub(crate) fn draw_grid_panels(&mut self, ui: &mut egui::Ui) {
        self.draw_grid_palette(ui);
        ui.add_space(6.0);
        if self.plane_primary().is_some() {
            self.draw_plane_inspector(ui);
        } else if self.grid.tool == GridTool::Mesh || self.mesh_primary().is_some() {
            self.draw_mesh_panels(ui);
        } else if self.grid.tool.is_verb() {
            self.draw_verb_inspector(ui);
        } else {
            self.draw_grid_face_inspector(ui);
        }
        ui.add_space(8.0);
        ui.separator();
    }

    /// Apply the palette material to every selected face.
    pub(crate) fn apply_grid_material_to_selection(&mut self, material: ResourceId) -> usize {
        let planes = self.apply_material_to_selected_planes(material);
        if planes > 0 {
            return planes;
        }
        let meshes = self.apply_material_to_selected_mesh(material);
        if meshes > 0 {
            return meshes;
        }
        // A band picked with Shift+click in the Bands tool takes the
        // material instead of the whole wall.
        if let Some((face, band)) = self.grid.band_sel {
            let before = self.project.clone();
            let scope = self.grid.band_scope;
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                if psxed_project::grid::edit::bands::paint_band(
                    world,
                    face,
                    band,
                    Some(material),
                    scope,
                ) {
                    self.history.record(before);
                    self.grid_changed();
                    return 1;
                }
            }
        }
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
        let materials =
            psxed_project::grid::palette::palette_materials(&self.project, self.grid.palette_all);
        egui::CollapsingHeader::new(RichText::new("Materials").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(&mut self.grid.palette_all, "Show all project materials")
                    .on_hover_text(
                        "Off: only wall, floor and ceiling images. On: also HUD, menu, sky and character images.",
                    );
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
        // A small badge when the material carries its own grid tiling.
        if let Some(psxed_project::ResourceData::Material(m)) =
            self.project.resource(id).map(|r| &r.data)
        {
            if !m.grid_uv.is_inherit() {
                let label = match m.grid_uv {
                    psxed_project::face_uv::UvMode::Fit { repeats_q4 } => format!(
                        "{}x{}",
                        repeats_q4[0] as f32 / 16.0,
                        repeats_q4[1] as f32 / 16.0
                    ),
                    _ => "W".to_string(),
                };
                painter.text(
                    rect.right_bottom() - Vec2::new(3.0, 2.0),
                    Align2::RIGHT_BOTTOM,
                    label,
                    FontId::proportional(9.5),
                    Color32::from_rgb(255, 214, 120),
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
                        FaceTarget::Wall { .. } | FaceTarget::Derived { .. } => walls += 1,
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
                    format!(
                        "{} at {},{}",
                        k.label(),
                        primary.cell()[0],
                        primary.cell()[1]
                    )
                }
                FaceTarget::Wall { edge, index } => format!(
                    "{} wall {index} at {},{}",
                    edge.label(),
                    primary.cell()[0],
                    primary.cell()[1]
                ),
                FaceTarget::Derived { edge, kind } => format!(
                    "{} {} (derived) at {},{}",
                    edge.label(),
                    kind.label(),
                    primary.cell()[0],
                    primary.cell()[1]
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
        let mut hidden: Option<bool> = None;
        let mut reset_override = false;
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
        if let Some(uv) = uv {
            let report = psxed_project::grid::uv::report_for(
                world,
                &psxed_project::grid::uv::ProjectUvDefaults(&self.project),
                primary,
                None,
            );
            ops.extend(super::tiling_panel::draw_tiling(ui, uv, report, None));
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
            FaceTarget::Derived { .. } => {
                if let Some(d) = primary.derived_wall(world) {
                    ui.label(
                        RichText::new(format!(
                            "from the height difference with {},{}",
                            d.owner[0], d.owner[1]
                        ))
                        .size(11.0)
                        .color(STUDIO_TEXT_WEAK),
                    );
                    let mut solid_now = d.blocks();
                    if ui
                        .checkbox(&mut solid_now, "Solid")
                        .on_hover_text(
                            "A wall record that blocks movement. Closure walls are solid by default; risers are left to the step rule.",
                        )
                        .changed()
                    {
                        solid = Some(solid_now);
                    }
                    let mut hidden_now = d.hidden;
                    if ui
                        .checkbox(&mut hidden_now, "Hidden")
                        .on_hover_text("Do not draw this face (it keeps blocking if it is solid)")
                        .changed()
                    {
                        hidden = Some(hidden_now);
                    }
                    if ui
                        .button("Back to automatic")
                        .on_hover_text("Forget the material, texture and flags set on this face")
                        .clicked()
                    {
                        reset_override = true;
                    }
                }
            }
        }
        if matches!(
            primary.target,
            FaceTarget::Wall { .. } | FaceTarget::Derived { .. }
        ) {
            self.draw_wall_run_bands(ui, primary);
        }
        // Apply what changed to every selected face.
        if toggle
            || !ops.is_empty()
            || facing.is_some()
            || solid.is_some()
            || hidden.is_some()
            || reset_override
        {
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
                    if let Some(v) = hidden {
                        set_derived_hidden(world, *face, v);
                    }
                    if reset_override {
                        reset_derived(world, *face);
                    }
                }
            }
            // The Inspector records the undo step, coalescing a drag.
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
            psxed_project::grid::GridRoom::new_derived(format!("Room {}", world.rooms.len()), 1, 1);
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
