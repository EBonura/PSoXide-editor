//! The wall policy of the active room, in the Rooms panel: how walls come
//! to exist, the Convert command for legacy rooms, and the defaults derived
//! faces fall back on.

use super::*;

use psxed_project::grid::edit::{clean_overrides, convert_to_derived, ConvertMode};
use psxed_project::grid::WallMode;
use psxed_project::grid::CLICK_UNITS;

impl EditorWorkspace {
    /// Wall mode, Convert and the derived-wall defaults of room `index`.
    pub(super) fn draw_grid_room_walls(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(room) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(index))
        else {
            return;
        };
        let mode = room.wall_mode;
        let default_wall = room.defaults.wall_material;
        let overrides = room.edge_overrides.len();
        let mut open_clicks = room.defaults.open_wall as f32 / CLICK_UNITS as f32;
        let default_name = default_wall
            .and_then(|id| self.project.resource_name(id))
            .unwrap_or("none")
            .to_string();
        let palette = self.grid.material;
        let mut convert: Option<ConvertMode> = None;
        let mut set_wall_material: Option<Option<ResourceId>> = None;
        let mut clean = false;
        let mut open_changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Walls").color(STUDIO_TEXT_WEAK).size(11.0));
            match mode {
                WallMode::Derived => {
                    ui.label(RichText::new("derived from heights").color(STUDIO_ACCENT));
                }
                WallMode::Authored => {
                    ui.label("authored (legacy)");
                }
            }
        });
        match mode {
            WallMode::Authored => {
                ui.horizontal(|ui| {
                    if ui
                        .button("Convert to derived")
                        .on_hover_text(
                            "Walls come from height differences from now on. Walls that are exactly a derived face become overrides; everything else keeps looking and playing as it does.",
                        )
                        .clicked()
                    {
                        convert = Some(ConvertMode::PreserveLook);
                    }
                    if ui
                        .button("Convert and fill")
                        .on_hover_text(
                            "Same, but also show the walls the room never had: closed cliffs and a closed outline.",
                        )
                        .clicked()
                    {
                        convert = Some(ConvertMode::FillNewWalls);
                    }
                });
            }
            WallMode::Derived => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Wall").color(STUDIO_TEXT_WEAK).size(11.0));
                    ui.label(RichText::new(default_name).color(STUDIO_ACCENT));
                    if ui
                        .add_enabled(palette.is_some(), egui::Button::new("Use palette"))
                        .on_hover_text("Make the chosen palette material the room's wall material")
                        .clicked()
                    {
                        set_wall_material = Some(palette);
                    }
                    if ui
                        .add_enabled(default_wall.is_some(), egui::Button::new("Clear"))
                        .clicked()
                    {
                        set_wall_material = Some(None);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Open wall").color(STUDIO_TEXT_WEAK).size(11.0));
                    open_changed = ui
                        .add(
                            egui::DragValue::new(&mut open_clicks)
                                .speed(0.1)
                                .range(0.0..=32.0)
                                .suffix(" clicks")
                                .max_decimals(2),
                        )
                        .on_hover_text(
                            "Height of the wall around a floor with no ceiling. 0 leaves such edges open.",
                        )
                        .changed();
                });
                if overrides > 0 {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{overrides} wall overrides"))
                                .color(STUDIO_TEXT_WEAK)
                                .size(11.0),
                        );
                        if ui
                            .button("Clean up")
                            .on_hover_text("Drop the overrides whose wall no longer exists")
                            .clicked()
                        {
                            clean = true;
                        }
                    });
                }
            }
        }
        if let Some(convert_mode) = convert {
            self.grid_convert_room(index, convert_mode);
        }
        if let Some(material) = set_wall_material {
            self.edit_room(index, |r| r.defaults.wall_material = material);
        }
        if open_changed {
            let units = (open_clicks.max(0.0) * CLICK_UNITS as f32).round() as i32;
            self.edit_room(index, |r| r.defaults.open_wall = units);
        }
        if clean {
            let before = self.project.clone();
            let removed = self
                .project
                .active_scene_mut()
                .world
                .grid_mut()
                .map_or(0, |w| clean_overrides(w, index));
            if removed > 0 {
                self.history.record(before);
                self.status = format!("Removed {removed} unused wall overrides");
                self.grid_changed();
            } else {
                self.status = "No unused wall overrides".to_string();
            }
        }
    }

    /// Convert room `index` to derived walls. One undo step.
    pub(crate) fn grid_convert_room(&mut self, index: usize, mode: ConvertMode) {
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        let Some(report) = convert_to_derived(world, index, mode) else {
            return;
        };
        self.history.record(before);
        self.grid.selection.clear();
        self.status = match mode {
            ConvertMode::PreserveLook => format!(
                "Room {index} is derived: {} walls converted, {} kept authored, {} faces hidden to keep the look",
                report.converted, report.kept_authored, report.hidden
            ),
            ConvertMode::FillNewWalls => format!(
                "Room {index} is derived: {} walls converted, {} kept authored, {} new wall faces shown",
                report.converted, report.kept_authored, report.filled
            ),
        };
        self.grid_changed();
    }
}
