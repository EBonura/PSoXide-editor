//! The Rooms panel at the top of the left dock while a grid world is open:
//! one row per room with its visibility, name and sector count, the active
//! room (where new geometry goes), add and delete, and the selected room's
//! name, raise and ambient light.

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
        let active = self.grid_target_room();
        // Name, raise and ambient edits coalesce into one undo step per drag
        // or typing burst, like every other inspector.
        let ctx = ui.ctx().clone();
        self.prepare_inspector_undo_frame(&ctx);
        let undo_candidate =
            inspector_has_edit_input(&ctx).then(|| (self.project.clone(), self.history.epoch()));
        let mut add = false;
        let mut delete = None;
        let mut activate = None;
        let mut toggle = None;
        let mut rename_start = None;
        let mut rename_to: Option<(usize, String)> = None;
        tool_panel_header(ui, icons::HOUSE, "Rooms", |ui| {
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
                                .selectable_label(active == index, label)
                                .on_hover_text(
                                    "Click to pin this as the room new geometry goes into, click again to follow your edits. Double-click to rename.",
                                );
                            if response.clicked() {
                                activate = Some(index);
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
                        });
                    });
                }
            });
        let title = format!(
            "Settings of {}",
            rows.get(active).map_or("the room", |r| r.0.as_str())
        );
        let settings_open = self.grid.room_settings_open;
        let header = egui::CollapsingHeader::new(RichText::new(title).size(12.0))
            .id_salt("grid_room_settings")
            .open(Some(settings_open))
            .show(ui, |ui| {
                self.draw_grid_room_settings(ui, active);
                self.draw_grid_room_walls(ui, active);
                self.draw_grid_room_rules(ui, active);
                self.draw_grid_room_rough(ui, active);
            });
        if header.header_response.clicked() {
            self.grid.room_settings_open = !settings_open;
        }
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
        if let Some(index) = activate {
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
    fn draw_grid_room_settings(&mut self, ui: &mut egui::Ui, room: usize) {
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
