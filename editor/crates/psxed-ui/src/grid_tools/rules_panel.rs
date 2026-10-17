//! Smart paint rules of the active room, in the Rooms panel: what the level
//! verbs paint on the floors and walls they shape.

use super::*;

use psxed_project::grid::paint_rules::HeightRule;
use psxed_project::grid::SlopeClass;

/// Which rule a row edits.
#[derive(Clone, Copy)]
enum Rule {
    Floor(SlopeClass),
    Wall,
    Trim,
}

/// Which height rule a section edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HeightKind {
    Floor,
    Wall,
}

/// Grid units in one step of a threshold.
const THRESHOLD_STEP: i32 = 64;

/// `rule` with threshold `which` set to `grid_units` (rounded to a step) and
/// the others kept ascending.
fn with_threshold(mut rule: HeightRule, which: usize, grid_units: i32) -> HeightRule {
    let steps = (grid_units as f32 / THRESHOLD_STEP as f32).round() as i32;
    rule.at[which] = steps.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    for j in which + 1..3 {
        rule.at[j] = rule.at[j].max(rule.at[j - 1]);
    }
    for j in (0..which).rev() {
        rule.at[j] = rule.at[j].min(rule.at[j + 1]);
    }
    rule
}

impl EditorWorkspace {
    /// One height rule: up to three thresholds (grid units, relative to the
    /// room's base) and a material per band. Returns the edited rule.
    fn draw_height_rule(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        kind: HeightKind,
        title: &str,
        hint: &str,
        rule: HeightRule,
    ) -> Option<HeightRule> {
        let palette = self.grid.material;
        let mut edited: Option<HeightRule> = None;
        let id = ("room_height_rule", index, matches!(kind, HeightKind::Wall));
        egui::CollapsingHeader::new(RichText::new(title).size(11.0))
            .id_salt(id)
            .default_open(!rule.is_empty())
            .show(ui, |ui| {
                ui.label(RichText::new(hint).size(10.5).color(STUDIO_TEXT_WEAK));
                for band in 0..4usize {
                    ui.horizontal(|ui| {
                        if band == 0 {
                            ui.label(RichText::new("below").color(STUDIO_TEXT_WEAK).size(11.0));
                        } else {
                            let mut gu = i32::from(rule.at[band - 1]) * THRESHOLD_STEP;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut gu)
                                        .speed(8.0)
                                        .prefix("from ")
                                        .suffix(" gu"),
                                )
                                .on_hover_text(
                                    "Where this band begins, grid units from the room's base, in steps of 64",
                                )
                                .changed()
                            {
                                edited = Some(with_threshold(rule, band - 1, gu));
                            }
                        }
                        let material = rule.materials[band];
                        ui.label(
                            RichText::new(
                                material
                                    .and_then(|id| self.project.resource_name(id))
                                    .unwrap_or("none")
                                    .to_string(),
                            )
                            .color(STUDIO_ACCENT),
                        );
                        if ui
                            .add_enabled(palette.is_some(), egui::Button::new("Use palette"))
                            .clicked()
                        {
                            let mut next = rule;
                            next.materials[band] = palette;
                            edited = Some(next);
                        }
                        if ui
                            .add_enabled(material.is_some(), egui::Button::new("Clear"))
                            .clicked()
                        {
                            let mut next = rule;
                            next.materials[band] = None;
                            edited = Some(next);
                        }
                    });
                }
                ui.label(
                    RichText::new(match rule.bands() {
                        0 => "No band names a material: the rule does nothing".to_string(),
                        n => format!("{n} bands in use (the last one with a material is the top)"),
                    })
                    .size(10.5)
                    .color(STUDIO_TEXT_WEAK),
                );
            });
        edited
    }

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
        let mut height_change: Option<(HeightKind, HeightRule)> = None;
        egui::CollapsingHeader::new(RichText::new("Paint rules").size(11.5))
            .id_salt(("room_paint_rules", index))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("What Raise, Ramp, Stairs and Break paint. A face you painted by hand is never changed.")
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
                ui.separator();
                for (kind, title, hint, rule) in [
                    (
                        HeightKind::Floor,
                        "Floor by height",
                        "Floor material by the height of the floor: scree low, rock high",
                        rules.floor_by_height,
                    ),
                    (
                        HeightKind::Wall,
                        "Wall by height",
                        "Wall strata: a material per band, cut by a line at each threshold",
                        rules.wall_by_height,
                    ),
                ] {
                    if let Some(edited) = self.draw_height_rule(ui, index, kind, title, hint, rule)
                    {
                        height_change = Some((kind, edited));
                    }
                }
            });
        if let Some((kind, rule)) = height_change {
            self.edit_room(index, |r| match kind {
                HeightKind::Floor => r.defaults.rules.floor_by_height = rule,
                HeightKind::Wall => r.defaults.rules.wall_by_height = rule,
            });
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_snap_to_64_and_stay_ascending() {
        let rule = HeightRule {
            at: [4, 8, 12],
            ..HeightRule::default()
        };
        // 700 gu is 10.9 steps: 11. The ones above it are lifted to match.
        let up = with_threshold(rule, 1, 700);
        assert_eq!(up.at, [4, 11, 12]);
        let way_up = with_threshold(rule, 0, 2000);
        assert_eq!(way_up.at, [31, 31, 31]);
        // Dragging a high one down pulls the lower ones with it.
        let down = with_threshold(rule, 2, 0);
        assert_eq!(down.at, [0, 0, 0]);
    }
}
