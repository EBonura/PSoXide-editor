//! The Rooms section of the Structure panel while a grid world is open: one
//! row per room with its visibility, name and sector count, a pin (where new
//! geometry goes), and delete. A click selects the room: its settings are the
//! Inspector's (`draw_grid_room_inspector`).

use super::*;

use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

/// Maximum height of the room list before it scrolls.
const LIST_MAX_HEIGHT: f32 = 150.0;

impl EditorWorkspace {
    /// The Rooms panel. Does nothing without a grid world.
    pub(crate) fn draw_grid_rooms_panel(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let rows: Vec<(String, usize, i32)> = world
            .rooms
            .iter()
            .map(|r| (r.name.clone(), r.sector_count(), r.y_offset))
            .collect();
        let target = self.grid_target_room();
        let pinned = self.grid_active_room;
        let chosen = self.selection.selected_room;
        // A picked face puts its room in the list too.
        let of_face = self.grid.selection.primary().map(|element| element.room());
        let highlighted = chosen.or(of_face);
        // Name and rename edits coalesce into one undo step per typing burst,
        // like every other inspector.
        let ctx = ui.ctx().clone();
        self.prepare_inspector_undo_frame(&ctx);
        let undo_candidate =
            inspector_has_edit_input(&ctx).then(|| (self.project.clone(), self.history.epoch()));
        let mut add = false;
        let mut delete = None;
        let mut choose = None;
        let mut pin = None;
        let mut toggle = None;
        let mut rename_start = None;
        let mut rename_to: Option<(usize, String)> = None;
        super::levels_panel::structure_section_header(ui, icons::HOUSE, "Rooms", |ui| {
            add = ui
                .button(icons::text(icons::PLUS, 14.0))
                .on_hover_text("Add an empty room one storey above the highest")
                .clicked();
        });
        egui::ScrollArea::vertical()
            .id_salt("grid_rooms_scroll")
            .max_height(LIST_MAX_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (index, (name, sectors, y)) in rows.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let shown = !self.grid_hidden_rooms.contains(&index);
                        let eye = if shown { icons::EYE } else { icons::EYE_OFF };
                        if ui
                            .add(egui::Button::new(icons::text(eye, 13.0)).frame(false))
                            .on_hover_text(if shown { "Hide room" } else { "Show room" })
                            .clicked()
                        {
                            toggle = Some(index);
                        }
                        if self.grid.renaming_room == Some(index) {
                            let mut text = name.clone();
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut text).desired_width(120.0),
                            );
                            response.request_focus();
                            if response.changed() {
                                rename_to = Some((index, text));
                            }
                            if response.lost_focus() {
                                rename_start = Some(None);
                            }
                        } else {
                            let label = if *y == 0 {
                                format!("{name}  ({sectors})")
                            } else {
                                format!("{name}  ({sectors}, y {y:+})")
                            };
                            let response = ui
                                .selectable_label(highlighted == Some(index), label)
                                .on_hover_text(
                                    "Click to show this room's settings in the Inspector. Double-click to rename.",
                                );
                            if response.clicked() {
                                choose = Some(index);
                            }
                            if response.double_clicked() {
                                rename_start = Some(Some(index));
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let can_delete = rows.len() > 1;
                            if ui
                                .add_enabled(
                                    can_delete,
                                    egui::Button::new(icons::text(icons::TRASH, 13.0)).frame(false),
                                )
                                .on_hover_text("Delete this room and everything in it")
                                .clicked()
                            {
                                delete = Some(index);
                            }
                            let is_pinned = pinned == Some(index);
                            let is_target = target == index;
                            let color = if is_pinned {
                                STUDIO_ACCENT
                            } else if is_target {
                                STUDIO_TEXT_WEAK
                            } else {
                                STUDIO_BORDER
                            };
                            if ui
                                .add(
                                    egui::Button::new(icons::text(icons::MAP_PIN, 13.0).color(color))
                                        .frame(false),
                                )
                                .on_hover_text(if is_pinned {
                                    "New geometry goes into this room. Click to let it follow your edits again."
                                } else {
                                    "Click to pin this as the room new geometry goes into."
                                })
                                .clicked()
                            {
                                pin = Some(index);
                            }
                        });
                    });
                }
            });
        ui.separator();

        if let Some(next) = rename_start {
            self.grid.renaming_room = next;
        }
        if let Some((index, name)) = rename_to {
            self.edit_room(index, |room| room.name = name);
        }
        if let Some(index) = toggle {
            if !self.grid_hidden_rooms.remove(&index) {
                self.grid_hidden_rooms.insert(index);
            }
        }
        if let Some(index) = choose {
            self.select_room(index);
        }
        if let Some(index) = pin {
            self.toggle_room_pin(index);
        }
        if add {
            self.grid_add_room();
        }
        if let Some(index) = delete {
            self.grid_delete_room(index);
        }
        if let Some((before, epoch)) = undo_candidate {
            self.finish_inspector_undo_frame(before, epoch, &ctx);
        }
    }

    /// Choose room `index` in the Structure panel: the Inspector shows its
    /// settings, and nothing else is picked.
    pub(crate) fn select_room(&mut self, index: usize) {
        let count = self
            .project
            .active_scene()
            .world
            .grid()
            .map_or(0, |w| w.rooms.len());
        if index >= count {
            return;
        }
        self.grid.selection.clear();
        self.grid.plane_sel.clear();
        self.clear_node_selection_state();
        self.clear_resource_selection_state();
        self.selection.selected_room = Some(index);
        self.grid.last_room = index;
        self.status = format!("Selected room {index}");
    }

    /// The Inspector of a room: its name, light, fog, walls, paint rules and
    /// roughness.
    pub(crate) fn draw_grid_room_inspector(&mut self, ui: &mut egui::Ui, room: usize) {
        let Some(r) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
        else {
            ui.weak("No such room");
            return;
        };
        let mut name = r.name.clone();
        let fog = r.fog;
        inspector_identity_header(ui, icons::HOUSE, STUDIO_ACCENT, &name, "Room", room);
        if inspector_property_row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY))
                .changed()
        }) {
            self.edit_room(room, |r| r.name = name.clone());
        }
        inspector_section(ui, ("room-light", room), icons::SUN, "Light", true, |ui| {
            self.draw_grid_room_settings(ui, room);
        });
        inspector_section(
            ui,
            ("room-fog", room),
            icons::MOUNTAIN,
            "Fog",
            fog.enabled,
            |ui| {
                let mut next = fog;
                ui.checkbox(&mut next.enabled, "Fog on")
                    .on_hover_text("Distance fog in this room, in the preview and the cook.");
                ui.add_enabled_ui(next.enabled, |ui| {
                    color_editor(ui, "Color", &mut next.color);
                    let sector = psxed_project::grid::units::SECTOR_UNITS as f32;
                    for (label, value) in
                        [("Start", &mut next.start), ("Falloff", &mut next.falloff)]
                    {
                        let mut sectors = *value as f32 / sector;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(label).color(STUDIO_TEXT_WEAK).size(11.0));
                            if ui
                                .add(
                                    egui::DragValue::new(&mut sectors)
                                        .speed(0.05)
                                        .range(0.0..=64.0)
                                        .suffix(" sectors")
                                        .max_decimals(2),
                                )
                                .on_hover_text(if label == "Start" {
                                    "Distance where the fog begins."
                                } else {
                                    "Distance from the start to full fog."
                                })
                                .changed()
                            {
                                *value = snap_to((sectors * sector).round() as i32, FINE_UNITS);
                            }
                        });
                    }
                });
                if next != fog {
                    self.edit_room(room, |r| r.fog = next);
                }
            },
        );
        inspector_section(
            ui,
            ("room-walls", room),
            icons::BRICK_WALL,
            "Walls",
            true,
            |ui| {
                self.draw_grid_room_walls(ui, room);
            },
        );
        inspector_section(
            ui,
            ("room-rules", room),
            icons::PALETTE,
            "Paint rules",
            false,
            |ui| {
                self.draw_grid_room_rules(ui, room);
            },
        );
        inspector_section(
            ui,
            ("room-rough", room),
            icons::TRENDING_UP,
            "Rough",
            false,
            |ui| {
                self.draw_grid_room_rough(ui, room);
            },
        );
    }

    /// Pin room `index` as the room new geometry goes into; pinning the
    /// pinned room again lets the room follow your edits.
    pub(crate) fn toggle_room_pin(&mut self, index: usize) {
        if self.grid_active_room == Some(index) {
            self.grid_active_room = None;
            self.status = "Room follows your edits".to_string();
        } else {
            self.grid_active_room = Some(index);
            self.status = format!("New geometry goes into room {index}");
        }
        self.grid.last_room = index;
    }

    /// Raise and ambient of the active room.
    pub(super) fn draw_grid_room_settings(&mut self, ui: &mut egui::Ui, room: usize) {
        let Some(r) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
        else {
            return;
        };
        let (mut clicks, mut ambient) = (r.y_offset as f32 / CLICK_UNITS as f32, r.ambient);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Raise").color(STUDIO_TEXT_WEAK).size(11.0));
            changed |= ui
                .add(
                    egui::DragValue::new(&mut clicks)
                        .speed(0.1)
                        .suffix(" clicks")
                        .max_decimals(2),
                )
                .on_hover_text("Vertical offset of the whole room")
                .changed();
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Ambient").color(STUDIO_TEXT_WEAK).size(11.0));
            for c in &mut ambient {
                changed |= ui.add(egui::DragValue::new(c).range(0..=255)).changed();
            }
        });
        if changed {
            self.edit_room(room, |r| {
                r.ambient = ambient;
                r.y_offset = snap_to((clicks * CLICK_UNITS as f32).round() as i32, FINE_UNITS);
            });
        }
        self.draw_grid_shading(ui);
    }

    /// Apply `edit` to room `index`. The caller's inspector transaction
    /// records the undo step.
    pub(super) fn edit_room(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut psxed_project::grid::GridRoom),
    ) {
        let before = self.project.clone();
        let Some(room) = self
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .and_then(|w| w.rooms.get_mut(index))
        else {
            return;
        };
        edit(room);
        if self.project != before {
            self.grid_changed();
        }
    }
}
