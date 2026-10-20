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

    /// The Inspector of the World.
    pub(crate) fn draw_world_inspector(&mut self, ui: &mut egui::Ui) {
        let material_options = self.project.material_options();
        let world_sector_size = self.project.world_sector_size_for_node(NodeId::ROOT);
        let mut nav_target: Option<ResourceId> = None;
        let mut snap_to_floor = false;
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
        inspector_section(
            ui,
            "world-streaming",
            icons::LAYERS,
            "Streaming",
            false,
            |ui| {
                changed |= self.draw_stream_radii(ui);
            },
        );
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

        // Message, physics, sky and far vista.
        if let Some(node) = self.project.active_scene_mut().node_mut(NodeId::ROOT) {
            changed |= draw_transform_policy_editor(
                ui,
                node,
                world_sector_size,
                &material_options,
                &mut nav_target,
                &mut snap_to_floor,
            );
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

    /// The draw, active and lead radii of the level, in sectors.
    fn draw_stream_radii(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(world) = self.project.active_scene().world.grid() else {
            return false;
        };
        let mut next = world.stream;
        let fields: [(&str, &mut u8, &str); 3] = [
            (
                "Draw far",
                &mut next.draw_far,
                "A room further than this many sectors from the camera, walking through the doors, is neither drawn nor kept in memory. 0 draws every room that can be reached.",
            ),
            (
                "Active",
                &mut next.active_radius,
                "Actors in rooms within this many sectors of the player think.",
            ),
            (
                "Lead",
                &mut next.lead_radius,
                "Rooms between Draw far and Draw far plus this are prefetched and evicted first.",
            ),
        ];
        for (label, value, hint) in fields {
            inspector_property_row(ui, label, |ui| {
                let mut v = i32::from(*value);
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .range(0..=255)
                            .suffix(" sectors"),
                    )
                    .on_hover_text(hint)
                    .changed()
                {
                    *value = v.clamp(0, 255) as u8;
                }
            });
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
