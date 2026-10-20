//! The world's global light, in the World panel: ambient occlusion, sky
//! light and bounced light, baked into the corner colours.

use super::*;

impl EditorWorkspace {
    /// The switch and the knobs of the world's global light.
    pub(super) fn draw_grid_global_light(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let current = world.global_light;
        let mut next = current;
        ui.checkbox(&mut next.enabled, "Gather light").on_hover_text(
            "Bake ambient occlusion, sky light and bounced light into the corner colours. Corners near walls get darker, an open sky lights them with the sky colour and a lit wall lights its neighbours. Costs nothing on the console. The preview refines in the background; Play shows the same.",
        );
        if next.enabled {
            let weak = |text: &str| {
                RichText::new(text.to_owned())
                    .color(STUDIO_TEXT_WEAK)
                    .size(11.0)
            };
            egui::Grid::new("grid_global_light")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(weak("Occlusion"));
                    ui.add(egui::Slider::new(&mut next.occlusion, 0..=100).suffix("%"))
                        .on_hover_text("How much of the ambient a blocked ray takes away.");
                    ui.end_row();
                    ui.label(weak("Reach"));
                    ui.add(egui::Slider::new(&mut next.reach, 1..=16).suffix(" sectors"))
                        .on_hover_text("A wall further off than this does not darken a corner.");
                    ui.end_row();
                    ui.label(weak("Bounces"));
                    ui.add(egui::Slider::new(&mut next.bounces, 0..=3))
                        .on_hover_text("Passes of light bounced off lit surfaces.");
                    ui.end_row();
                    ui.label(weak("Bounce gain"));
                    ui.add(egui::Slider::new(&mut next.bounce_gain, 0..=200).suffix("%"))
                        .on_hover_text(
                            "How much of the light a surface passes on. 100% keeps its texture's average colour.",
                        );
                    ui.end_row();
                    ui.label(weak("Rays"));
                    ui.add(egui::Slider::new(&mut next.rays, 8..=128))
                        .on_hover_text("Rays per corner. More is smoother and slower to bake.");
                    ui.end_row();
                    ui.label(weak("Sky"));
                    ui.horizontal(|ui| {
                        let mut own = next.sky.is_some();
                        if ui.checkbox(&mut own, "own colour").changed() {
                            next.sky = own.then_some(next.sky.unwrap_or([96, 120, 168]));
                        }
                        if let Some(sky) = &mut next.sky {
                            ui.color_edit_button_srgb(sky);
                        }
                    })
                    .response
                    .on_hover_text(
                        "The colour rays that leave the world bring in. Without one the room's ambient is used.",
                    );
                    ui.end_row();
                });
        }
        if next != current {
            // Recorded by the World panel's inspector transaction, like every
            // other edit made there.
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                world.global_light = next.clamped();
            }
            self.grid_changed();
        }
    }
}
