//! The option row of the level verbs and the Inspector's Region context.

use super::verb_state::RegionSource;
use super::*;

use psxed_project::grid::edit::verbs::raise::RaiseMode;

impl EditorWorkspace {
    /// Options of the active verb, in the strip's option row.
    pub(super) fn draw_verb_options(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Region").color(STUDIO_TEXT_WEAK).size(11.0));
        for source in RegionSource::ALL {
            let on = self.grid.verb.source == source;
            if ui
                .selectable_label(on, source.label())
                .on_hover_text(source.hint())
                .clicked()
            {
                self.grid.verb.source = source;
                self.grid.verb.path.clear();
            }
        }
        match self.grid.verb.source {
            RegionSource::Brush => {
                ui.add(
                    egui::DragValue::new(&mut self.grid.verb.brush_radius)
                        .range(0..=6)
                        .prefix("radius "),
                )
                .on_hover_text("Brush radius in sectors ([ and ])");
            }
            RegionSource::Path => {
                ui.add(
                    egui::DragValue::new(&mut self.grid.verb.path_width)
                        .range(1..=13)
                        .prefix("width "),
                )
                .on_hover_text("Path width in sectors ([ and ])");
            }
            RegionSource::Marquee => {}
        }
        ui.separator();
        match self.grid.tool {
            GridTool::Raise => self.draw_raise_options(ui),
            GridTool::Ramp => self.draw_ramp_options(ui),
            _ => {}
        }
        ui.separator();
        ui.label(RichText::new("Amount").color(STUDIO_TEXT_WEAK).size(11.0));
        ui.add(
            egui::DragValue::new(&mut self.grid.verb.typed_clicks)
                .speed(0.05)
                .suffix(" clicks")
                .max_decimals(2),
        )
        .on_hover_text("A typed amount, in 256-unit clicks");
        let has_region = self.grid.verb.region.is_some();
        if ui
            .add_enabled(has_region, egui::Button::new("Apply"))
            .on_hover_text("Apply the typed amount to the region as one undo step")
            .clicked()
        {
            super::verb_tool::apply_typed(self);
        }
    }

    fn draw_ramp_options(&mut self, ui: &mut egui::Ui) {
        use psxed_project::grid::edit::verbs::ramp::RampDir;
        ui.label(RichText::new("Keeps").color(STUDIO_TEXT_WEAK).size(11.0));
        let from = self.grid.verb.ramp_from;
        if ui
            .selectable_label(from.is_none(), "Nearest side")
            .on_hover_text("The side of the region nearest where you press keeps its height")
            .clicked()
        {
            self.grid.verb.ramp_from = None;
        }
        for dir in RampDir::ALL {
            if ui
                .selectable_label(from == Some(dir), dir.label())
                .on_hover_text("This side keeps its height; the opposite one rises (R turns)")
                .clicked()
            {
                self.grid.verb.ramp_from = Some(dir);
            }
        }
        ui.checkbox(&mut self.grid.verb.ramp_clamp, "Walkable")
            .on_hover_text(
                "Hold the rise to the steepest slope the character motor stands on (Tab)",
            );
        ui.checkbox(&mut self.grid.verb.follow_ceiling, "Follow ceiling")
            .on_hover_text("Ceilings move with the floors (headroom stays)");
    }

    fn draw_raise_options(&mut self, ui: &mut egui::Ui) {
        let mode = self.grid.verb.raise_mode;
        for (label, value, hint) in [
            ("Auto", None, "Level on a flat region, Offset otherwise"),
            (
                "Offset",
                Some(RaiseMode::Offset),
                "Every corner moves by the amount; the shape is kept",
            ),
            (
                "Level",
                Some(RaiseMode::Level),
                "Every corner is set to the height under the pointer plus the amount",
            ),
        ] {
            if ui
                .selectable_label(mode == value, label)
                .on_hover_text(hint)
                .clicked()
            {
                self.grid.verb.raise_mode = value;
            }
        }
        ui.checkbox(&mut self.grid.verb.follow_ceiling, "Follow ceiling")
            .on_hover_text("Ceilings move with the floors (headroom stays); off holds a floor one click under its ceiling");
    }

    /// The Inspector's Region context: what the region holds and what the
    /// active verb will do to it.
    pub(super) fn draw_verb_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        egui::CollapsingHeader::new(RichText::new("Region").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} tool", self.grid.tool.label()))
                        .color(STUDIO_ACCENT),
                );
                let Some(region) = &self.grid.verb.region else {
                    ui.label(
                        RichText::new("Drag a marquee (or use the brush or a path) to pick the sectors, then press on them and drag up or down")
                            .color(STUDIO_TEXT_WEAK),
                    );
                    return;
                };
                let (with_floor, skipped) = region.with_floors(world);
                ui.label(format!(
                    "{} sectors, {} with a floor{}",
                    region.len(),
                    with_floor.len(),
                    if skipped > 0 {
                        format!(" ({skipped} skipped)")
                    } else {
                        String::new()
                    }
                ));
                if let Some(b) = region.bounds() {
                    ui.label(
                        RichText::new(format!(
                            "from {},{} to {},{} in room {}",
                            b.min[0], b.min[1], b.max[0], b.max[1], region.room
                        ))
                        .size(11.5)
                        .color(STUDIO_TEXT_WEAK),
                    );
                }
                if self.grid.tool == GridTool::Ramp {
                    ui.label(format!(
                        "Keeps: {}",
                        self.grid
                            .verb
                            .ramp_from
                            .map_or("the side nearest the press", |d| d.label())
                    ));
                    ui.label(format!(
                        "Slope: {}",
                        if self.grid.verb.ramp_clamp {
                            "held to what the motor stands on"
                        } else {
                            "free (the readout says when it is too steep)"
                        }
                    ));
                }
                if self.grid.tool == GridTool::Raise {
                    let default = psxed_project::grid::edit::verbs::raise::default_mode(world, region);
                    let mode = self.grid.verb.raise_mode.unwrap_or(default);
                    ui.label(format!(
                        "Mode: {}{}",
                        mode.label(),
                        if self.grid.verb.raise_mode.is_none() {
                            " (automatic)"
                        } else {
                            ""
                        }
                    ));
                    ui.label(format!(
                        "Ceilings: {}",
                        if self.grid.verb.follow_ceiling {
                            "follow the floors"
                        } else {
                            "stay; floors stop a click under"
                        }
                    ));
                }
                let name = self
                    .grid
                    .material
                    .and_then(|id| self.project.resource_name(id))
                    .unwrap_or("none");
                ui.label(
                    RichText::new(format!("Wall material: {name} (the palette choice)"))
                        .size(11.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                if !self.grid.verb.readout.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(&self.grid.verb.readout).monospace().size(11.5));
                }
            });
    }
}
