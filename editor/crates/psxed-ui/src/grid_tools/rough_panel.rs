//! The room's Rough settings (seed, three amplitudes, tint): one editor used
//! by the Rough tool's option row and by the Rooms panel.
//!
//! Amplitudes are in grid units, the unit the data keeps, with the runtime
//! unit they come to beside them. Above the validator's cap (a third of the
//! character's radius) the number turns orange: it still saves, and the cook
//! warns, because inward jitter that large can clip the character and the
//! camera.

use super::*;

use psxed_project::grid::edit::rough::set_params;
use psxed_project::grid::rough::MAX_AMP_RU;
use psxed_project::grid::GridScale;

/// Colour of an amplitude over the cap.
const OVER_CAP: Color32 = Color32::from_rgb(240, 150, 90);

/// Runtime units an amplitude of `gu` grid units comes to, rounded up the
/// way the validator rounds.
pub(super) fn amp_ru(scale: GridScale, gu: u8) -> i32 {
    let n = i64::from(gu) * i64::from(scale.num);
    let d = 16 * i64::from(scale.den.max(1));
    ((n + d - 1) / d) as i32
}

impl EditorWorkspace {
    /// Edit the Rough settings of room `index`. `own_undo` wraps the edit in
    /// the Inspector's undo transaction (the Rooms panel already has one).
    pub(super) fn draw_rough_params(&mut self, ui: &mut egui::Ui, index: usize, own_undo: bool) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(room) = world.rooms.get(index) else {
            return;
        };
        let scale = world.scale;
        let rough = room.rough;
        let ctx = ui.ctx().clone();
        if own_undo {
            self.prepare_inspector_undo_frame(&ctx);
        }
        let undo_candidate = (own_undo && inspector_has_edit_input(&ctx))
            .then(|| (self.project.clone(), self.history.epoch()));
        let (mut seed, mut amp, mut tint) = (rough.seed, rough.amp, rough.tint);
        let mut changed = false;
        ui.label(RichText::new("Seed").color(STUDIO_TEXT_WEAK).size(11.0));
        changed |= ui
            .add(egui::DragValue::new(&mut seed).speed(1))
            .on_hover_text("A different seed is a different rock with the same amplitudes")
            .changed();
        ui.label(RichText::new("Amp").color(STUDIO_TEXT_WEAK).size(11.0))
            .on_hover_text("The largest sideways offset at rough levels 1, 2 and 3, in grid units");
        for (level, value) in amp.iter_mut().enumerate() {
            changed |= ui
                .add(egui::DragValue::new(value).range(0..=255).suffix(" gu"))
                .on_hover_text(format!("Amplitude at level {}", level + 1))
                .changed();
            let ru = amp_ru(scale, *value);
            let over = ru > MAX_AMP_RU;
            ui.label(
                RichText::new(format!("{ru} ru"))
                    .size(11.0)
                    .color(if over { OVER_CAP } else { STUDIO_TEXT_WEAK }),
            )
            .on_hover_text(if over {
                format!(
                    "Over {MAX_AMP_RU} runtime units, a third of the character's radius: the cook warns, the rock can clip the model and the camera"
                )
            } else {
                "Runtime units".to_string()
            });
        }
        ui.label(RichText::new("Tint").color(STUDIO_TEXT_WEAK).size(11.0));
        changed |= ui
            .add(egui::DragValue::new(&mut tint).range(0..=100).suffix("%"))
            .on_hover_text("Brightness noise on the faces of rough sectors, percent either way")
            .changed();
        if changed {
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                if set_params(world, index, Some(seed), Some(amp), Some(tint)) {
                    self.grid_changed();
                }
            }
        }
        if let Some((before, epoch)) = undo_candidate {
            self.finish_inspector_undo_frame(before, epoch, &ctx);
        }
    }

    /// The Rooms panel's Rough section.
    pub(super) fn draw_grid_room_rough(&mut self, ui: &mut egui::Ui, index: usize) {
        egui::CollapsingHeader::new(RichText::new("Rough").size(11.5))
            .id_salt(("room_rough", index))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Render-only jitter of the faces of rough sectors (the Rough tool, M). Collision stays on the grid.",
                    )
                    .size(10.5)
                    .color(STUDIO_TEXT_WEAK),
                );
                ui.horizontal_wrapped(|ui| self.draw_rough_params(ui, index, false));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitudes_round_up_to_runtime_units_like_the_validator() {
        let scale = GridScale { num: 5, den: 4 };
        assert_eq!(amp_ru(scale, 0), 0);
        assert_eq!(amp_ru(scale, 13), 2);
        // The default level 3 amplitude (4 ru) is exactly at the cap.
        let default = psxed_project::grid::rough::Rough::default_amp(scale);
        assert_eq!(amp_ru(scale, default[2]), MAX_AMP_RU);
        assert!(amp_ru(scale, default[2] + 13) > MAX_AMP_RU);
    }
}
