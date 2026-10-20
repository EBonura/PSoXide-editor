//! The Storeys panel at the top of the left dock: the storeys the document
//! holds, lowest first, with a name each. A click goes to a storey (the storey
//! cursor), a double-click renames it.
//!
//! A storey is derived from the floors, never declared, so its name is kept
//! in the world against the click height of its floor
//! (`GridWorld::level_names`). A name whose floor moved away stays in the
//! document and comes back if a storey stands at that height again.

use super::*;

use psxed_project::grid::storeys::{Storey, STOREY_MIN_CELLS, STOREY_PITCH};
use psxed_project::grid::{GridWorld, CLICK_UNITS};

/// Maximum height of the storey list before it scrolls.
const LIST_MAX_HEIGHT: f32 = 150.0;

/// The click index a storey's name is kept under.
pub(crate) fn name_key(height: i32) -> i32 {
    height.div_euclid(CLICK_UNITS)
}

/// The name of the storey at `height`: the author's, else `Storey N` by its
/// place in the list.
pub(crate) fn level_name(world: &GridWorld, list: &[Storey], height: i32) -> String {
    if let Some(name) = world.level_names.get(&name_key(height)) {
        return name.clone();
    }
    list.iter()
        .position(|s| s.height == height)
        .map_or_else(|| "Storey -".to_string(), |i| format!("Storey {i}"))
}

impl EditorWorkspace {
    /// The Storeys panel. Does nothing without a grid world.
    pub(crate) fn draw_grid_levels_panel(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let list = self.level_storeys();
        let rows: Vec<(i32, String, usize)> = list
            .iter()
            .map(|s| (s.height, level_name(world, &list, s.height), s.cells))
            .collect();
        let cursor = self.grid.level.y;
        let mut go: Option<Option<i32>> = None;
        let mut rename_start: Option<i32> = None;
        let mut rename_commit = false;
        let mut add = false;
        let mut cutaway = self.grid.level.cutaway;
        structure_section_header(ui, icons::LAYERS, "Storeys", |ui| {
            add = ui
                .button(icons::text(icons::PLUS, 14.0))
                .on_hover_text(format!(
                    "New storey: the Upper floor tool, set to one storey ({} clicks) above this one. Pick the sectors, then press on them and drag up.",
                    STOREY_PITCH / CLICK_UNITS
                ))
                .clicked();
            let on = cutaway == level::Cutaway::Cut;
            if ui
                .add(
                    egui::Button::new(icons::text(icons::SCAN, 13.0))
                        .selected(on)
                        .frame(on),
                )
                .on_hover_text("Cutaway (Z): hide what lies above the storey in the 3D view")
                .clicked()
            {
                cutaway = if on {
                    level::Cutaway::Off
                } else {
                    level::Cutaway::Cut
                };
            }
        });
        if rows.is_empty() {
            ui.label(
                RichText::new(format!(
                    "No storeys yet: a floor height needs {STOREY_MIN_CELLS} cells"
                ))
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
            );
        } else {
            egui::ScrollArea::vertical()
                .id_salt("grid_levels_scroll")
                .max_height(LIST_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let all = cursor.is_none();
                    if ui
                        .selectable_label(all, "All storeys")
                        .on_hover_text("No storey: tools edit as they always did, nothing is cut away")
                        .clicked()
                    {
                        go = Some(None);
                    }
                    for (height, name, cells) in rows.iter().rev() {
                        ui.horizontal(|ui| {
                            if let Some((editing, buffer)) = &mut self.grid.level.renaming {
                                if *editing == *height {
                                    let response = ui.add(
                                        egui::TextEdit::singleline(buffer).desired_width(130.0),
                                    );
                                    response.request_focus();
                                    if response.lost_focus() {
                                        rename_commit = true;
                                    }
                                    return;
                                }
                            }
                            let on = cursor == Some(*height);
                            let clicks = *height as f32 / CLICK_UNITS as f32;
                            let response = ui
                                .selectable_label(on, name)
                                .on_hover_text(
                                    "Click to work on this storey. Double-click to rename. Alt+Up and Alt+Down step through the storeys, G picks one from a surface.",
                                );
                            if response.clicked() {
                                go = Some(Some(*height));
                            }
                            if response.double_clicked() {
                                rename_start = Some(*height);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("y {clicks:+.1}  {cells} cells"))
                                            .color(STUDIO_TEXT_WEAK)
                                            .size(10.5),
                                    );
                                },
                            );
                        });
                    }
                });
        }
        ui.separator();

        if let Some(height) = rename_start {
            let current = rows
                .iter()
                .find(|(h, _, _)| *h == height)
                .map(|(_, name, _)| name.clone())
                .unwrap_or_default();
            self.grid.level.renaming = Some((height, current));
        }
        if rename_commit {
            self.commit_level_rename();
        }
        match go {
            Some(Some(height)) => self.set_level(height),
            Some(None) => {
                self.grid.level.y = None;
                self.status = "All storeys".to_string();
            }
            None => {}
        }
        if cutaway != self.grid.level.cutaway {
            self.grid.level.cutaway = cutaway;
            self.status = format!("Cutaway {}", cutaway.label());
        }
        if add {
            self.add_level();
        }
    }

    /// Store the name typed in the Levels panel (an empty one restores the
    /// default) as one undo step.
    pub(crate) fn commit_level_rename(&mut self) {
        let Some((height, typed)) = self.grid.level.renaming.take() else {
            return;
        };
        let name = typed.trim().to_string();
        let key = name_key(height);
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        let wanted = (!name.is_empty()).then_some(name);
        let changed = world.level_names.get(&key) != wanted.as_ref();
        if changed {
            match wanted {
                Some(name) => world.level_names.insert(key, name),
                None => world.level_names.remove(&key),
            };
        }
        if changed {
            self.history.record(before);
            self.grid_changed();
        }
    }

    /// The panel's `+`: the Upper floor tool in Add floor above mode, set to one storey
    /// above the level, with a status that says what to do next.
    fn add_level(&mut self) {
        self.set_grid_tool(GridTool::Slab);
        self.grid.verb.slab.mode = slab_tool::SlabMode::Insert;
        self.grid.verb.slab.typed_clicks = (STOREY_PITCH / CLICK_UNITS) as f32;
        self.status = format!(
            "New storey: pick the sectors it covers, then press on them and drag up (or type {} clicks and Apply)",
            STOREY_PITCH / CLICK_UNITS
        );
    }
}

impl EditorWorkspace {
    /// The left dock of a grid world: Levels and Rooms (scrolling when the
    /// room settings are open), then the scene graph for the rest.
    /// The left dock of a grid world: Structure (the levels and the rooms),
    /// then the Scene Graph.
    pub(crate) fn draw_grid_left_dock(&mut self, ui: &mut egui::Ui) {
        let content_width = ui.available_width().max(1.0);
        let total = ui.available_height().max(1.0);
        let top = (total * 0.58).max(160.0).min(total - 120.0).max(60.0);
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, top),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_max_width(content_width);
                egui::ScrollArea::vertical()
                    .id_salt("grid_left_dock_top")
                    .auto_shrink([false, true])
                    .max_height(top)
                    .show(ui, |ui| {
                        tool_panel_header(ui, icons::LAYERS_2, "Structure", |_| {});
                        self.draw_world_entry(ui);
                        self.draw_grid_levels_panel(ui);
                        self.draw_grid_rooms_panel(ui);
                    });
            },
        );
        let rest = ui.available_height().max(1.0);
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, rest),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_size(Vec2::new(content_width, rest));
                ui.set_max_width(content_width);
                self.draw_objects_panel(ui);
            },
        );
    }
}

/// The head of a section inside the Structure panel: the panel's own header
/// is the heavy one, so a section is a quiet line with its buttons at the
/// right.
pub(crate) fn structure_section_header(
    ui: &mut egui::Ui,
    icon: char,
    label: &str,
    add_actions: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(icons::text(icon, 13.0).color(STUDIO_TEXT_WEAK));
        ui.label(RichText::new(label).strong().size(11.5).color(STUDIO_TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add_actions(ui);
        });
    });
}
