//! Smart paint rules of the active room, in the Rooms panel: what the level
//! verbs paint on the floors and walls they shape.

use super::*;

use psxed_project::grid::SlopeClass;

/// Which rule a row edits.
#[derive(Clone, Copy)]
enum Rule {
    Floor(SlopeClass),
    Wall,
    Trim,
}

impl EditorWorkspace {
    /// The rules of room `index`: a collapsed section of rows, each a
    /// material from the palette or none.
    pub(super) fn draw_grid_room_rules(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(room) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(index))
        else {
            return;
        };
        let rules = room.defaults.rules;
        let palette = self.grid.material;
        let name_of = |ws: &Self, id: Option<ResourceId>| {
            id.and_then(|id| ws.project.resource_name(id))
                .unwrap_or("none")
                .to_string()
        };
        let rows: [(&str, Rule, Option<ResourceId>, &str); 5] = [
            (
                "Flat floor",
                Rule::Floor(SlopeClass::Flat),
                rules.floor_by_class[0],
                "Painted on a floor a verb made level",
            ),
            (
                "Slope floor",
                Rule::Floor(SlopeClass::Slope),
                rules.floor_by_class[1],
                "Painted on a floor a verb tilted to a walkable slope",
            ),
            (
                "Steep floor",
                Rule::Floor(SlopeClass::Steep),
                rules.floor_by_class[2],
                "Painted on a floor a verb made too steep to stand on",
            ),
            (
                "Verb walls",
                Rule::Wall,
                rules.wall,
                "The wall material of the sectors a verb shapes (the palette choice wins)",
            ),
            (
                "Trim",
                Rule::Trim,
                rules.trim,
                "The foot band of every wall, written once when a verb makes a derived face in a room with no default band set",
            ),
        ];
        let mut change: Option<(Rule, Option<ResourceId>)> = None;
        egui::CollapsingHeader::new(RichText::new("Paint rules").size(11.5))
            .id_salt(("room_paint_rules", index))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("What Raise, Ramp and Stairs paint. A face you painted by hand is never changed.")
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                for (label, rule, current, hint) in rows {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).color(STUDIO_TEXT_WEAK).size(11.0))
                            .on_hover_text(hint);
                        ui.label(RichText::new(name_of(self, current)).color(STUDIO_ACCENT));
                        if ui
                            .add_enabled(palette.is_some(), egui::Button::new("Use palette"))
                            .clicked()
                        {
                            change = Some((rule, palette));
                        }
                        if ui
                            .add_enabled(current.is_some(), egui::Button::new("Clear"))
                            .clicked()
                        {
                            change = Some((rule, None));
                        }
                    });
                }
            });
        if let Some((rule, material)) = change {
            self.edit_room(index, |r| {
                let rules = &mut r.defaults.rules;
                match rule {
                    Rule::Floor(class) => rules.floor_by_class[class.index()] = material,
                    Rule::Wall => rules.wall = material,
                    Rule::Trim => rules.trim = material,
                }
            });
        }
    }
}
