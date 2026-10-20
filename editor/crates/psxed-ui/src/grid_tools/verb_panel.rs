//! The option row of the level verbs and the Inspector's Region context.

use super::extrude_tool::ExtrudeSurface;
use super::verb_state::RegionSource;
use super::*;

use psxed_project::grid::edit::verbs::break_verb::BreakStyle;
use psxed_project::grid::edit::verbs::raise::RaiseMode;

impl EditorWorkspace {
    /// Options of the active verb, in the strip's option row.
    pub(super) fn draw_verb_options(&mut self, ui: &mut egui::Ui) {
        if self.grid.tool == GridTool::Extrude {
            self.draw_extrude_surfaces(ui);
            ui.separator();
            if self.grid.verb.surface == ExtrudeSurface::Wall {
                return self.draw_wall_options(ui);
            }
        }
        ui.label(RichText::new("Region").color(STUDIO_TEXT_WEAK).size(11.0));
        // A flight is a rectangle or a path; the other sources make no order
        // of treads.
        let stairs = self.grid.tool == GridTool::Stairs;
        let sources: &[RegionSource] = if stairs {
            &[RegionSource::Marquee, RegionSource::Path]
        } else {
            &RegionSource::ALL
        };
        for &source in sources {
            let on = self.grid.verb.source == source;
            let (label, hint) = if stairs && source == RegionSource::Marquee {
                (
                    "Rectangle",
                    "Drag a rectangle: the treads run from its bottom side across its depth",
                )
            } else if stairs {
                (
                    "Path",
                    "Click the waypoints of a winding flight, Enter finishes; [ and ] change the width",
                )
            } else {
                (source.label(), source.hint())
            };
            if ui.selectable_label(on, label).on_hover_text(hint).clicked() {
                self.grid.verb.source = source;
                self.grid.verb.path.clear();
                if stairs {
                    self.grid.verb.stairs_source = source;
                    // The other way of making a flight starts afresh.
                    self.grid.verb.region = None;
                    self.grid.verb.stairs_path.clear();
                    self.grid.verb.stairs_refusal = None;
                }
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
            RegionSource::Marquee | RegionSource::Lasso => {}
        }
        ui.separator();
        match self.grid.tool {
            GridTool::Extrude => self.draw_raise_options(ui),
            GridTool::Ramp => self.draw_ramp_options(ui),
            GridTool::Stairs => self.draw_stairs_options(ui),
            GridTool::Spiral => self.draw_spiral_options(ui),
            GridTool::Break => return self.draw_break_options(ui),
            GridTool::Rough => return self.draw_rough_options(ui),
            GridTool::Slab => return self.draw_slab_options(ui),
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

    /// Upper floor: the mode and a typed height over the level.
    fn draw_slab_options(&mut self, ui: &mut egui::Ui) {
        use super::slab_tool::SlabMode;
        ui.label(
            RichText::new("Upper floor")
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
        );
        for mode in SlabMode::ALL {
            if ui
                .selectable_label(self.grid.verb.slab.mode == mode, mode.label())
                .on_hover_text(format!("{} (Tab cycles)", mode.hint()))
                .clicked()
            {
                self.grid.verb.slab.mode = mode;
            }
        }
        ui.separator();
        ui.label(RichText::new("Height").color(STUDIO_TEXT_WEAK).size(11.0));
        ui.add(
            egui::DragValue::new(&mut self.grid.verb.slab.typed_clicks)
                .speed(0.05)
                .suffix(" clicks")
                .max_decimals(2),
        )
        .on_hover_text("A typed height of the slab's top over the level, in 256-unit clicks");
        let has_region = self.grid.verb.region.is_some();
        if ui
            .add_enabled(has_region, egui::Button::new("Apply"))
            .on_hover_text("Apply the mode to the region as one undo step")
            .clicked()
        {
            super::slab_tool::apply_typed(self);
        }
    }

    /// Break: targets, keep walkable, snap, amplitude, material mix, rough
    /// level, the seed and a typed intensity.
    fn draw_break_options(&mut self, ui: &mut egui::Ui) {
        use psxed_project::grid::edit::verbs::break_verb::BreakSnap;
        ui.label(RichText::new("Break").color(STUDIO_TEXT_WEAK).size(11.0));
        for (index, (label, _)) in super::break_tool::TARGET_SETS.iter().enumerate() {
            if ui
                .selectable_label(self.grid.verb.brk.targets == index, *label)
                .on_hover_text("What the break touches (Tab cycles)")
                .clicked()
            {
                self.grid.verb.brk.targets = index;
            }
        }
        ui.separator();
        let brk = &mut self.grid.verb.brk;
        ui.checkbox(&mut brk.keep_walkable, "Keep walkable")
            .on_hover_text(
                "Refuse any change that would add a floor too steep to stand on, a step the character cannot climb, too little headroom, or cut a route off",
            );
        for (snap, label, hint) in [
            (BreakSnap::Fine, "Fine", "Vertical changes in 64 unit steps"),
            (
                BreakSnap::Click,
                "Click",
                "Vertical changes in 256 unit clicks",
            ),
        ] {
            if ui
                .selectable_label(brk.snap == snap, label)
                .on_hover_text(hint)
                .clicked()
            {
                brk.snap = snap;
            }
        }
        ui.add(
            egui::DragValue::new(&mut brk.amplitude)
                .range(1..=32)
                .prefix("amplitude ")
                .suffix(" fine"),
        )
        .on_hover_text("The largest vertical change at intensity 100, in 64 unit fine steps");
        ui.checkbox(&mut brk.mix_materials, "Mix materials")
            .on_hover_text("A cracked triangle takes the material of the floor it now borders");
        ui.checkbox(&mut brk.gaps, "Gaps").on_hover_text(
            "Let a high intensity (70 and up) turn whole floor cells into see-through holes. Off by default: a break never leaves holes unless you ask",
        );
        ui.add(
            egui::DragValue::new(&mut brk.jitter)
                .range(0..=3)
                .prefix("rough "),
        )
        .on_hover_text("Give the broken sectors this rough level (0 leaves them smooth)");
        ui.separator();
        ui.add(egui::DragValue::new(&mut brk.seed).speed(1).prefix("seed "))
            .on_hover_text("Same seed, same result. Shift+X draws the next one");
        if ui
            .button("Re-roll")
            .on_hover_text("The next seed (Shift+X), live while dragging")
            .clicked()
        {
            super::break_tool::reroll(self);
        }
        ui.separator();
        let has_region = self.grid.verb.region.is_some();
        let brk = &mut self.grid.verb.brk;
        if ui
            .button("Cathedral style")
            .on_hover_text(
                "The Cathedral's sparse hand-made look: walls and floor, one click of amplitude, intensity 15",
            )
            .clicked()
        {
            brk.apply_style(BreakStyle::Cathedral);
        }
        ui.add(
            egui::DragValue::new(&mut brk.typed_intensity)
                .range(0..=100)
                .suffix(" percent"),
        )
        .on_hover_text("A typed intensity");
        if ui
            .add_enabled(has_region, egui::Button::new("Apply"))
            .on_hover_text("Break the region at the typed intensity as one undo step")
            .clicked()
        {
            super::verb_tool::apply_typed(self);
        }
    }

    /// Rough: the room's seed, amplitudes and tint, and a typed level.
    fn draw_rough_options(&mut self, ui: &mut egui::Ui) {
        let room = self
            .grid
            .verb
            .region
            .as_ref()
            .map_or_else(|| self.grid_target_room(), |r| r.room);
        ui.label(RichText::new("Room").color(STUDIO_TEXT_WEAK).size(11.0));
        self.draw_rough_params(ui, room, true);
        ui.separator();
        let has_region = self.grid.verb.region.is_some();
        ui.add(
            egui::DragValue::new(&mut self.grid.verb.rough.typed_level)
                .range(0..=3)
                .prefix("level "),
        )
        .on_hover_text("A typed rough level, 0 to 3");
        if ui
            .add_enabled(has_region, egui::Button::new("Apply"))
            .on_hover_text("Give the region the typed level as one undo step")
            .clicked()
        {
            super::verb_tool::apply_typed(self);
        }
    }

    fn draw_spiral_options(&mut self, ui: &mut egui::Ui) {
        use psxed_project::grid::edit::verbs::stairs::RiserOption;
        ui.label(RichText::new("Storeys").color(STUDIO_TEXT_WEAK).size(11.0));
        ui.add(
            egui::DragValue::new(&mut self.grid.verb.spiral.storeys)
                .speed(0.1)
                .range(1..=8),
        )
        .on_hover_text("How many storeys the stairs climb, a drag of the climb of one");
        ui.label(RichText::new("Riser").color(STUDIO_TEXT_WEAK).size(11.0));
        let current = self.grid.verb.stairs.riser;
        for option in [RiserOption::Click, RiserOption::Max] {
            if ui
                .selectable_label(current == option, super::stairs_tool::riser_label(option))
                .clicked()
            {
                self.grid.verb.stairs.riser = option;
            }
        }
    }

    fn draw_stairs_options(&mut self, ui: &mut egui::Ui) {
        use psxed_project::grid::edit::verbs::stairs::RiserOption;
        ui.label(RichText::new("Riser").color(STUDIO_TEXT_WEAK).size(11.0));
        let current = self.grid.verb.stairs.riser;
        for (option, hint) in [
            (
                RiserOption::Click,
                "One click (256 units), half the climb limit: comfortable",
            ),
            (
                RiserOption::Max,
                "The tallest riser the character steps up (40 runtime units)",
            ),
        ] {
            if ui
                .selectable_label(current == option, super::stairs_tool::riser_label(option))
                .on_hover_text(hint)
                .clicked()
            {
                self.grid.verb.stairs.riser = option;
            }
        }
        let mut typed = match current {
            RiserOption::Typed(v) => v as f32 / psxed_project::grid::CLICK_UNITS as f32,
            _ => 1.0,
        };
        if ui
            .add(
                egui::DragValue::new(&mut typed)
                    .speed(0.05)
                    .range(0.25..=8.0)
                    .suffix(" clicks"),
            )
            .on_hover_text("A typed riser, held to the climb limit")
            .changed()
        {
            self.grid.verb.stairs.riser = RiserOption::Typed(psxed_project::grid::units::snap_to(
                (typed * psxed_project::grid::CLICK_UNITS as f32).round() as i32,
                psxed_project::grid::FINE_UNITS,
            ));
        }
        ui.checkbox(&mut self.grid.verb.stairs.ramp, "Ramp")
            .on_hover_text("Slope every tread instead of stepping (Tab)");
        ui.checkbox(&mut self.grid.verb.follow_ceiling, "Follow ceiling")
            .on_hover_text("Ceilings move with the floors (headroom stays)");
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

    /// Extrude: what it moves.
    fn draw_extrude_surfaces(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Moves").color(STUDIO_TEXT_WEAK).size(11.0));
        for surface in ExtrudeSurface::ALL {
            if ui
                .selectable_label(self.grid.verb.surface == surface, surface.label())
                .on_hover_text(surface.hint())
                .clicked()
            {
                self.grid.verb.surface = surface;
                self.grid.verb.region = None;
                self.grid.verb.path.clear();
                self.grid.selection.clear();
            }
        }
    }

    /// Extrude on walls: the step and the Out and In buttons.
    fn draw_wall_options(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Push").color(STUDIO_TEXT_WEAK).size(11.0));
        let picked = !self.grid.selection.is_empty();
        let steps = self.grid.verb.wall_steps.max(1);
        if ui
            .add_enabled(picked, egui::Button::new("Out"))
            .on_hover_text(
                "Push the picked pieces out: an alcove (a niche for a band) a sector deeper (=)",
            )
            .clicked()
        {
            super::extrude_tool::step(self, steps);
        }
        if ui
            .add_enabled(picked, egui::Button::new("In"))
            .on_hover_text("Push the picked pieces in: fill the sector in front of them (-)")
            .clicked()
        {
            super::extrude_tool::step(self, -steps);
        }
        ui.add(
            egui::DragValue::new(&mut self.grid.verb.wall_steps)
                .range(1..=super::extrude_tool::MAX_STEPS)
                .suffix(if steps == 1 { " sector" } else { " sectors" }),
        )
        .on_hover_text("Sectors a button or key pushes");
        ui.label(
            RichText::new(format!("{} picked", self.grid.selection.len()))
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
        );
    }

    fn draw_raise_options(&mut self, ui: &mut egui::Ui) {
        if self.grid.verb.surface == ExtrudeSurface::Ceiling {
            // A ceiling moves by the amount; Offset and Level, Follow ceiling
            // and the corner cut belong to a floor.
            return;
        }
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
        let has_region = self.grid.verb.region.is_some();
        if ui
            .add_enabled(has_region, egui::Button::new("Cut corners"))
            .on_hover_text(
                "Crack the outer triangle of every convex corner of the region down by the last raise (a click when there is none): a plateau with 45 degree corners",
            )
            .clicked()
        {
            super::verb_tool::cut_corners(self);
        }
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
                    let hint = match self.grid.tool {
                        GridTool::Stairs => {
                            "Drag a rectangle where the stairs go, then press on it and drag up for the height"
                        }
                        GridTool::Slab => {
                            "Adds a floor above this one, with open space underneath (needs a ceiling or open sky above). Drag a rectangle, then press on it and drag up"
                        }
                        _ => "Drag a marquee (or use the brush or a path) to pick the sectors, then press on them and drag up or down",
                    };
                    ui.label(RichText::new(hint).color(STUDIO_TEXT_WEAK));
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
                if self.grid.tool == GridTool::Stairs {
                    if self.grid.verb.source == RegionSource::Path {
                        ui.label(format!(
                            "Path: {} waypoints, width {}",
                            self.grid.verb.stairs_path.len(),
                            self.grid.verb.path_width
                        ));
                    } else {
                        ui.label(format!(
                            "Bottom: {}",
                            self.grid
                                .verb
                                .ramp_from
                                .map_or("the side nearest the press", |d| d.label())
                        ));
                    }
                    ui.label(format!(
                        "Riser: {}{}",
                        super::stairs_tool::riser_label(self.grid.verb.stairs.riser),
                        if self.grid.verb.stairs.ramp { ", ramp" } else { "" }
                    ));
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
                if self.grid.tool == GridTool::Extrude {
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
                if self.grid.tool == GridTool::Break {
                    let brk = &self.grid.verb.brk;
                    ui.label(format!(
                        "Targets: {}, seed {}, amplitude {} fine",
                        brk.targets_label(),
                        brk.seed,
                        brk.amplitude
                    ));
                    ui.label(format!(
                        "Route: {}",
                        if brk.keep_walkable {
                            "kept walkable"
                        } else {
                            "not checked"
                        }
                    ));
                }
                if self.grid.tool == GridTool::Rough {
                    ui.label("Drag up for rougher: levels 0 to 3, render only");
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
