//! The World: one entry at the top of the Structure panel, and its settings
//! in the Inspector (shading, streaming radii, the default camera, then
//! sky, physics and the rest).

use super::*;

impl EditorWorkspace {
    /// Whether the World entry is what is selected (not "nothing selected",
    /// which is the root with an empty selection set).
    pub(crate) fn world_is_selected(&self) -> bool {
        self.selection.selected_node == NodeId::ROOT
            && self.selection.selected_nodes.contains(&NodeId::ROOT)
    }

    /// Choose the World: the Inspector shows its settings and nothing else
    /// is picked.
    pub(crate) fn select_world(&mut self) {
        self.grid.selection.clear();
        self.grid.plane_sel.clear();
        self.clear_resource_selection_state();
        self.replace_node_selection(NodeId::ROOT);
        self.status = "Selected the World".to_string();
    }

    /// The one entry of the World in the Structure panel.
    pub(crate) fn draw_world_entry(&mut self, ui: &mut egui::Ui) {
        let selected = self.world_is_selected();
        ui.add_space(2.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
        let painter = ui.painter_at(rect);
        paint_list_row(&painter, rect, selected, response.hovered());
        painter.text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            Align2::CENTER_CENTER,
            icons::HOUSE.to_string(),
            icons::font(13.0),
            STUDIO_ACCENT,
        );
        painter.text(
            Pos2::new(rect.left() + 26.0, rect.center().y),
            Align2::LEFT_CENTER,
            "World",
            FontId::proportional(13.0),
            if selected {
                Color32::WHITE
            } else {
                STUDIO_TEXT
            },
        );
        painter.text(
            Pos2::new(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            "sky, physics, camera",
            FontId::proportional(10.5),
            STUDIO_TEXT_WEAK,
        );
        let clicked = response
            .on_hover_text("Sky, physics, shading, streaming and the default camera")
            .clicked();
        ui.separator();
        if clicked {
            self.select_world();
        }
    }

    /// "slots used N of 832": the count from the last cook when there was one
    /// (it counts every record that keeps its state), else the numbers the
    /// project holds so far.
    pub(crate) fn save_slots_text(&self) -> String {
        let used = self
            .last_slots_used
            .map_or(self.project.persistent_slots.len(), usize::from);
        format!(
            "slots used {used} of {}",
            psx_level::POI_PERSISTENT_FLAG_CAPACITY
        )
    }

    /// The Inspector of the World.
    pub(crate) fn draw_world_inspector(&mut self, ui: &mut egui::Ui) {
        let material_options = self.project.material_options();
        let mut nav_target: Option<ResourceId> = None;
        let mut changed = false;

        {
            let scene = self.project.active_scene();
            if let Some(node) = scene.node(NodeId::ROOT) {
                inspector_identity_header(
                    ui,
                    icons::HOUSE,
                    STUDIO_ACCENT,
                    &node.name,
                    "World",
                    node.id.raw(),
                );
            }
        }
        {
            let scene = self.project.active_scene_mut();
            if let Some(node) = scene.node_mut(NodeId::ROOT) {
                changed |= inspector_property_row(ui, "Name", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut node.name).desired_width(f32::INFINITY))
                        .changed()
                });
            }
        }

        inspector_section(
            ui,
            "world-density",
            icons::GRID,
            "Texture density",
            true,
            |ui| {
                changed |= self.draw_texel_density(ui);
            },
        );
        inspector_section(ui, "world-look", icons::SUN, "Look", true, |ui| {
            // Sky first; the rest of the mood is below it.
            let mut sky_nav = None;
            if let Some(node) = self.project.active_scene_mut().node_mut(NodeId::ROOT) {
                if let NodeKind::World { sky, .. } = &mut node.kind {
                    changed |= draw_world_sky(ui, false, sky, &material_options, &mut sky_nav);
                }
            }
            if sky_nav.is_some() {
                nav_target = sky_nav;
            }
            inspector_section(ui, "world-shading", icons::SUN, "Shading", true, |ui| {
                self.draw_grid_shading(ui);
            });
            inspector_section(
                ui,
                "world-global-light",
                icons::LIGHTBULB,
                "Global light",
                true,
                |ui| {
                    self.draw_grid_global_light(ui);
                },
            );
            ui.label(
                RichText::new("Ambient and fog: per room.")
                    .small()
                    .color(STUDIO_TEXT_WEAK),
            );
        });
        inspector_section(
            ui,
            "world-camera",
            icons::CAMERA,
            "Default camera",
            false,
            |ui| {
                ui.label(
                RichText::new(
                    "The follow camera when the player actor has no Camera part. A Camera part on the player replaces it.",
                )
                .small()
                .color(STUDIO_TEXT_WEAK),
            );
                if let Some(node) = self.project.active_scene_mut().node_mut(NodeId::ROOT) {
                    if let NodeKind::World { camera, .. } = &mut node.kind {
                        changed |= draw_gameplay_camera_settings(ui, camera);
                    }
                }
            },
        );

        inspector_section(
            ui,
            "world-streaming",
            icons::LAYERS,
            "Streaming and far view",
            false,
            |ui| {
                changed |= self.draw_stream_radii(ui);
            },
        );

        inspector_section(
            ui,
            "world-save-slots",
            icons::SAVE,
            "Save slots",
            false,
            |ui| {
                ui.label(RichText::new(self.save_slots_text()).color(STUDIO_TEXT));
                ui.label(
                    RichText::new(
                        "Each door and lever that keeps its state between visits takes one. The editor numbers them when you Play and keeps the numbers, so adding a door never changes the others.",
                    )
                    .small()
                    .color(STUDIO_TEXT_WEAK),
                );
            },
        );

        // Message and physics.
        if let Some(node) = self.project.active_scene_mut().node_mut(NodeId::ROOT) {
            if let NodeKind::World {
                physics,
                world_message,
                start_abilities,
                ..
            } = &mut node.kind
            {
                changed |=
                    draw_world_message_and_physics(ui, physics, world_message, start_abilities);
            }
        }
        if changed {
            self.mark_dirty();
            ui.ctx().request_repaint();
        }
        if let Some(target) = nav_target {
            self.replace_resource_selection(target);
            self.clear_node_selection_state();
        }
    }

    /// The "Streaming and far view" group: how far the level draws and keeps
    /// rooms, whether the far view is on and where it starts, in sectors.
    fn draw_stream_radii(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(world) = self.project.active_scene().world.grid() else {
            return false;
        };
        let mut next = world.stream;
        let sectors = |ui: &mut egui::Ui, label: &str, value: &mut u8, hint: &str| {
            inspector_property_row(ui, label, |ui| {
                let mut v = i32::from(*value);
                let changed = ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .range(0..=255)
                            .suffix(" sectors"),
                    )
                    .on_hover_text(hint)
                    .changed();
                if changed {
                    *value = v.clamp(0, 255) as u8;
                }
            });
        };
        sectors(
            ui,
            "Draw distance",
            &mut next.draw_far,
            "A room further than this many sectors from the camera, walking through the doors, is neither drawn nor kept in memory. 0 draws every room that can be reached.",
        );
        sectors(
            ui,
            "Actors think within",
            &mut next.active_radius,
            "Actors in rooms within this many sectors of the player think.",
        );
        sectors(
            ui,
            "Prefetch margin",
            &mut next.lead_radius,
            "Rooms between the draw distance and the draw distance plus this are loaded ahead and dropped first.",
        );
        inspector_property_row(ui, "Neighbours only", |ui| {
            ui.checkbox(&mut next.ring, "")
                .on_hover_text(
                    "Draw and stream only the room you are in and the rooms one door away, whatever the distances say. For a world cut into even chunks.",
                )
                .changed()
        });
        inspector_property_row(ui, "Far view", |ui| {
            ui.checkbox(&mut next.flood.on, "On")
                .on_hover_text(
                    "Stream the level by distance and draw what is beyond the near rooms as a far view.",
                )
                .changed()
        });
        if next.flood.on {
            sectors(
                ui,
                "Far view starts at",
                &mut next.flood.near_radius,
                "Cells within this many sectors of the camera are drawn whole, collide and run actors. Beyond it the far view shows.",
            );
            sectors(
                ui,
                "Kept in memory to",
                &mut next.flood.res_radius,
                "Cells within this many sectors of the player stay loaded so the drawn set can grow into them without a wait. At least where the far view starts.",
            );
            next.flood.res_radius = next.flood.res_radius.max(next.flood.near_radius);
            sectors(
                ui,
                "Far panel spacing",
                &mut next.flood.panels,
                "Everything beyond the near rooms is drawn as panels baked at a lattice of viewpoints this many sectors apart. 0 for none. Needs a cube sky.",
            );
        }
        if ui
            .button("Far view preset")
            .on_hover_text(
                "The values the Citadel uses: draw 12, actors 8, prefetch 4, neighbours only, far view on from 12 sectors, kept to 13, panels every 10",
            )
            .clicked()
        {
            apply_far_view_preset(&mut next);
        }
        if next != world.stream {
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                world.stream = next;
            }
            self.grid_changed();
            return true;
        }
        false
    }
}

/// The values the Citadel streams with (`projects/zz-far-s10`): draw 12, actors
/// 8, prefetch 4, neighbours only, far view on from 12 sectors, kept to 13,
/// panels every 10. Every other setting is left as it is.
pub(crate) fn apply_far_view_preset(stream: &mut psxed_project::grid::StreamRadii) {
    stream.draw_far = 12;
    stream.active_radius = 8;
    stream.lead_radius = 4;
    stream.ring = true;
    stream.flood.on = true;
    stream.flood.near_radius = 12;
    stream.flood.res_radius = 13;
    stream.flood.panels = 10;
}
