//! The grid tool strip, its option row and the status bar.

use super::*;

use psxed_project::grid::limits::MAX_WORLD_FACES;
use psxed_project::grid::{GridBudget, CLICK_UNITS, FINE_UNITS};

impl EditorWorkspace {
    /// The eight tool buttons.
    pub(crate) fn draw_grid_tool_strip(&mut self, ui: &mut egui::Ui) {
        for tool in GridTool::ALL {
            let selected = self.grid.tool == tool;
            let response = ui
                .add(
                    egui::Button::new(icons::label(tool.icon(), tool.label()))
                        .selected(selected)
                        .min_size(Vec2::new(28.0, 23.0)),
                )
                .on_hover_text(format!(
                    "{} ({})\n{}",
                    tool.label(),
                    tool.key_label(),
                    tool.tooltip()
                ));
            if response.clicked() {
                self.set_grid_tool(tool);
            }
        }
    }

    /// Options of the active tool, next to the strip.
    pub(crate) fn draw_grid_context_controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        match self.grid.tool {
            GridTool::Floor | GridTool::Ceiling => {
                let ceiling = self.grid.tool == GridTool::Ceiling;
                let value = if ceiling {
                    &mut self.grid.ceiling_height
                } else {
                    &mut self.grid.floor_height
                };
                let mut clicks = *value as f32 / CLICK_UNITS as f32;
                ui.label(RichText::new("Height").color(STUDIO_TEXT_WEAK).size(11.0));
                if ui
                    .add(
                        egui::DragValue::new(&mut clicks)
                            .speed(0.05)
                            .suffix(" clicks")
                            .max_decimals(2),
                    )
                    .on_hover_text("Height of the next surface, in 256-unit clicks. Shift-drag sets it while dragging.")
                    .changed()
                {
                    *value = psxed_project::grid::units::snap_to(
                        (clicks * CLICK_UNITS as f32).round() as i32,
                        FINE_UNITS,
                    );
                }
            }
            GridTool::Wall => {
                let label = format!("Direction: {}", self.grid.wall_dir.label());
                if ui
                    .button(label)
                    .on_hover_text(
                        "R turns the wall direction: auto, N, E, S, W, then the two diagonals",
                    )
                    .clicked()
                {
                    self.grid.wall_dir = self.grid.wall_dir.next();
                }
            }
            GridTool::Select | GridTool::Height => {
                ui.toggle_value(&mut self.grid.link, "Link")
                    .on_hover_text("L: coincident corners move together");
            }
            GridTool::Place => {
                self.draw_bsp_add_menu(ui);
                ui.label(
                    RichText::new(self.place_kind.label())
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
            }
            GridTool::Paint => {
                let name = self
                    .grid
                    .material
                    .and_then(|id| self.project.resource_name(id))
                    .unwrap_or("none")
                    .to_string();
                ui.label(
                    RichText::new(format!("Material: {name}"))
                        .color(STUDIO_TEXT_WEAK)
                        .size(11.0),
                );
            }
            GridTool::Split => {}
        }
        ui.separator();
        ui.label(
            RichText::new(format!("Snap {CLICK_UNITS} (Alt {FINE_UNITS})"))
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
        );
    }

    /// Key hints for the active tool.
    fn grid_key_hints(&self) -> &'static str {
        if self.grid.paste.is_some() {
            return "Click place | Shift keep stamping | H/V flip | R turn | Esc cancel";
        }
        match self.grid.tool {
            GridTool::Select => {
                "Click pick | Shift range | Ctrl toggle | Drag box | Shift-drag height | Del delete | Ctrl+C/V copy"
            }
            GridTool::Floor => "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
            GridTool::Ceiling => "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
            GridTool::Wall => "Drag a run | R direction | Ctrl remove | Esc cancel",
            GridTool::Height => "Drag up/down | Alt fine | L link | PgUp/PgDn nudge | Esc cancel",
            GridTool::Paint => "Click or drag paint | Ctrl sample | pick in the palette",
            GridTool::Split => "Click a floor or ceiling | Alt ceiling in Top",
            GridTool::Place => "Click to place at the sector centre | Alt free",
        }
    }

    fn grid_budget(&self) -> Option<GridBudget> {
        let world = self.project.active_scene().world.grid()?;
        let mut slot = self.grid.budget.borrow_mut();
        if slot.map(|(rev, _)| rev) != Some(self.grid.rev) {
            *slot = Some((self.grid.rev, GridBudget::of(world)));
        }
        slot.map(|(_, budget)| budget)
    }

    /// The bottom bar: last message, key hints, budget.
    pub(crate) fn draw_grid_status_bar(&mut self, ctx: &egui::Context) {
        let Some(budget) = self.grid_budget() else {
            return;
        };
        let room = self.grid_target_room();
        let (room_sectors, room_name, room_count) = self
            .project
            .active_scene()
            .world
            .grid()
            .map(|w| {
                let r = w.rooms.get(room);
                (
                    r.map_or(0, |r| r.sector_count()),
                    r.map_or(String::new(), |r| r.name.clone()),
                    w.rooms.len(),
                )
            })
            .unwrap_or_default();
        let percent = budget.faces_percent();
        let hot = percent >= 85;
        egui::TopBottomPanel::bottom("psxed_grid_status_bar")
            .exact_height(24.0)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.status)
                            .color(Color32::from_rgb(120, 220, 140))
                            .monospace()
                            .size(11.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let budget_text = format!(
                            "room {room}{}: {room_sectors} sectors   world: {} rooms, {} faces / {MAX_WORLD_FACES} ({percent}%)",
                            if room_name.is_empty() { String::new() } else { format!(" {room_name}") },
                            room_count,
                            budget.faces,
                        );
                        ui.label(
                            RichText::new(budget_text)
                                .color(if hot { Color32::from_rgb(240, 150, 90) } else { STUDIO_TEXT_WEAK })
                                .monospace()
                                .size(11.0),
                        );
                        ui.label(RichText::new("|").color(STUDIO_TEXT_WEAK).size(11.0));
                        ui.label(
                            RichText::new(self.grid_key_hints())
                                .color(STUDIO_TEXT_WEAK)
                                .monospace()
                                .size(11.0),
                        );
                    });
                });
            });
    }
}
