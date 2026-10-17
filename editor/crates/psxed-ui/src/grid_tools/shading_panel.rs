//! The world's facing shade, in the Rooms panel: how much darker a face is
//! by the way it looks, so a step or a platform reads under one lamp.

use super::*;

use psxed_project::grid::FacingShade;

/// Percent of a Q8 factor, for the number fields.
fn percent(q8: u16) -> u16 {
    ((u32::from(q8) * 100 + 128) / 256) as u16
}

fn from_percent(percent: u16) -> u16 {
    ((u32::from(percent) * 256 + 50) / 100).min(256) as u16
}

/// Which preset a shade is, for the combo's label.
fn label(shade: FacingShade) -> &'static str {
    if shade == FacingShade::OFF {
        "Off"
    } else if shade == FacingShade::SUBTLE {
        "Subtle"
    } else if shade == FacingShade::STRONG {
        "Strong"
    } else {
        "Custom"
    }
}

impl EditorWorkspace {
    /// Preset buttons and the four factors of the world's facing shade.
    pub(super) fn draw_grid_shading(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let current = world.shading;
        let mut next = current;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Shading").color(STUDIO_TEXT_WEAK).size(11.0));
            egui::ComboBox::from_id_salt("grid_facing_shade")
                .selected_text(label(current))
                .show_ui(ui, |ui| {
                    for (name, preset) in [
                        ("Off", FacingShade::OFF),
                        ("Subtle", FacingShade::SUBTLE),
                        ("Strong", FacingShade::STRONG),
                    ] {
                        if ui.selectable_label(current == preset, name).clicked() {
                            next = preset;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Darken faces by the way they look: ceilings and walls against floors, and the two wall directions against each other. The preview and the cook agree.",
                );
        });
        if current != FacingShade::OFF {
            ui.horizontal(|ui| {
                let mut fields = [
                    ("Floor", percent(current.up)),
                    ("Ceil", percent(current.down)),
                    ("E-W", percent(current.side_x)),
                    ("N-S", percent(current.side_z)),
                ];
                let mut edited = false;
                for (name, value) in &mut fields {
                    ui.label(RichText::new(*name).color(STUDIO_TEXT_WEAK).size(10.0));
                    edited |= ui
                        .add(egui::DragValue::new(value).range(0..=100).suffix("%"))
                        .changed();
                }
                if edited {
                    next = FacingShade {
                        up: from_percent(fields[0].1),
                        down: from_percent(fields[1].1),
                        side_x: from_percent(fields[2].1),
                        side_z: from_percent(fields[3].1),
                    };
                }
            });
        }
        if next != current {
            // Recorded by the Rooms panel's inspector transaction, like
            // every other edit made there.
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                world.shading = next;
            }
            self.grid_changed();
        }
    }
}
