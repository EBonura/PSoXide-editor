//! The Materials panel at the top of the Inspector column while a grid world
//! is open: the one place a grid project's materials are browsed, made,
//! imported and chosen.
//!
//! The palette shows the materials the world uses, the images imported with
//! a level, the Graybox set and every material made or imported from here
//! (the world remembers those as `palette_pins`); a switch shows all the
//! project's materials. A click makes a material the paint material (the
//! Paint tool and new surfaces use it) and applies it to the selected faces.
//! The Resources dock does not list materials in a grid project, so there is
//! no second browser to keep in step.

use super::*;

use psxed_project::grid::edit::material_of;

const SWATCH: f32 = 44.0;

impl EditorWorkspace {
    /// The palette, its search box and the New and Import buttons.
    pub(super) fn draw_grid_palette(&mut self, ui: &mut egui::Ui) {
        self.refresh_texture_thumbs(ui.ctx());
        self.pin_imported_materials();
        let materials =
            psxed_project::grid::palette::palette_materials(&self.project, self.grid.palette_all);
        let total = materials.len();
        // A project opens with a paint material, so the first Paint click
        // paints (a material the user chose stays).
        if self.grid.material.is_none() {
            self.grid.material = materials.first().map(|(id, _)| *id);
        }
        let query = self.grid.palette_search.to_ascii_lowercase();
        let shown: Vec<(ResourceId, String)> = materials
            .into_iter()
            .filter(|(_, name)| query.is_empty() || name.to_ascii_lowercase().contains(&query))
            .collect();
        let on_face = self.selected_face_material();

        let mut new_material = false;
        let mut new_transition = false;
        let mut import = false;
        let mut edit = false;
        ui.horizontal(|ui| {
            ui.label(icons::text(icons::PALETTE, 14.0).color(STUDIO_ACCENT));
            ui.label(RichText::new("Materials").strong().size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(icons::text(icons::PLUS, 14.0), |ui| {
                    if ui
                        .button(icons::label(icons::BLEND, "New material"))
                        .on_hover_text("A flat material. Open it in the Material tab to give it an image and settings.")
                        .clicked()
                    {
                        new_material = true;
                        ui.close_menu();
                    }
                    if ui
                        .button(icons::label(icons::BLEND, "New transition material"))
                        .on_hover_text("A host-baked blend of two materials")
                        .clicked()
                    {
                        new_transition = true;
                        ui.close_menu();
                    }
                })
                .response
                .on_hover_text("Make a material");
                import = ui
                    .add(egui::Button::new(icons::text(icons::IMAGE_PLUS, 14.0)))
                    .on_hover_text("Import an image as a material (PNG, JPG, BMP)")
                    .clicked();
                edit = ui
                    .add_enabled(
                        self.grid.material.is_some(),
                        egui::Button::new(icons::text(icons::PENCIL, 14.0)),
                    )
                    .on_hover_text("Edit the paint material in the Material tab")
                    .clicked();
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.grid.palette_search)
                .hint_text("Search materials")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.grid.palette_all, "All project materials")
                .on_hover_text(
                    "Off: the wall, floor and ceiling materials. On: also HUD, menu, sky and character images.",
                );
        });
        if shown.is_empty() {
            let text = if total == 0 {
                "No materials in this project"
            } else {
                "No material matches"
            };
            ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK));
        } else {
            let scroll_to = self.grid.palette_scroll_to.take();
            egui::ScrollArea::vertical()
                .id_salt("grid_palette_scroll")
                .max_height(236.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                        for (id, name) in &shown {
                            let paint = self.grid.material == Some(*id);
                            let response =
                                self.draw_swatch(ui, *id, name, paint, on_face == Some(*id));
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
        }
        let name = self
            .grid
            .material
            .and_then(|id| self.project.resource_name(id))
            .unwrap_or("none")
            .to_string();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Paint").color(STUDIO_TEXT_WEAK).size(11.0));
            ui.label(RichText::new(name).color(STUDIO_ACCENT).size(11.5));
            ui.label(
                RichText::new(format!("{} of {total}", shown.len()))
                    .color(STUDIO_TEXT_WEAK)
                    .size(10.5),
            );
        });

        if new_material {
            let id = self.add_material_resource();
            self.pin_material(id);
        }
        if new_transition {
            let id = self.add_transition_material_resource();
            self.pin_material(id);
        }
        if import {
            self.grid.palette_import_watch = Some(
                self.project
                    .material_options()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect(),
            );
            self.open_texture_import_dialog();
        }
        if edit {
            self.open_paint_material_in_lab();
        }
    }

    /// The material of the first selected face, for the palette to mark.
    fn selected_face_material(&self) -> Option<ResourceId> {
        let world = self.project.active_scene().world.grid()?;
        let face = *self.grid.selection.faces(world).first()?;
        material_of(world, face, Some(0))
    }

    /// Put `id` on the palette, make it the paint material and scroll to it.
    pub(crate) fn pin_material(&mut self, id: ResourceId) {
        if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
            if !world.palette_pins.contains(&id) {
                world.palette_pins.push(id);
            }
        }
        self.grid.material = Some(id);
        self.grid.palette_scroll_to = Some(id);
        self.mark_dirty();
    }

    /// Materials an import made while the palette's Import dialog was open
    /// join the palette.
    fn pin_imported_materials(&mut self) {
        if self.texture_import_dialog.open {
            return;
        }
        let Some(known) = self.grid.palette_import_watch.take() else {
            return;
        };
        let fresh: Vec<ResourceId> = self
            .project
            .material_options()
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| !known.contains(id))
            .collect();
        for id in fresh {
            self.pin_material(id);
            self.status = "Imported material added to the palette".to_string();
        }
    }

    /// The pencil: the paint material in the Material tab.
    fn open_paint_material_in_lab(&mut self) {
        let Some(id) = self.grid.material else {
            return;
        };
        self.material_lab.focused_material = Some(id);
        self.active_workspace = WorkspaceView::Material;
        self.status = "Workspace: Material".to_string();
    }

    fn draw_swatch(
        &self,
        ui: &mut egui::Ui,
        id: ResourceId,
        name: &str,
        paint: bool,
        on_face: bool,
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
        // The accent ring is the paint material; a white corner mark says
        // the selected face wears this one.
        let stroke = if paint {
            Stroke::new(2.5, STUDIO_ACCENT)
        } else if response.hovered() {
            Stroke::new(1.5, Color32::from_rgb(200, 200, 210))
        } else {
            Stroke::new(1.0, STUDIO_BORDER)
        };
        painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
        if on_face {
            let corner = Rect::from_min_size(rect.left_top() + Vec2::splat(2.0), Vec2::splat(9.0));
            painter.rect_filled(corner, 2.0, Color32::from_rgb(250, 250, 250));
            painter.text(
                corner.center(),
                Align2::CENTER_CENTER,
                "F",
                FontId::proportional(8.0),
                Color32::from_rgb(30, 30, 36),
            );
        }
        response.on_hover_text(if on_face {
            format!("{name} (the selected face wears this)")
        } else {
            name.to_string()
        })
    }
}
