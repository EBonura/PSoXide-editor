//! The Tiling section of the Inspector: how the selected face's texture is
//! laid over it. One panel for floors, ceilings, authored walls, derived
//! walls and (through the planes inspector) planes, always printing what the
//! setting comes to on this face and what the cook does with it.

use super::*;

use psxed_project::face_uv::{FaceUv, UvMode};
use psxed_project::grid::edit::UvOp;
use psxed_project::grid::uv::UvReport;
use psxed_project::grid::PlaneUvMode;

/// Most tiles across a face the Inspector accepts without a second thought.
const MAX_FIT_TILES: f32 = 16.0;

/// Draw the Tiling controls for `uv` and return the edits the user made.
/// `report` describes the current setting on the selected face.
///
/// A plane passes its legacy mapping as `plane`: its two stock mappings (Tile
/// and Stretch, the same as World at the default density and Fit 1x1) stand
/// in for "Default".
pub(super) fn draw_tiling(
    ui: &mut egui::Ui,
    uv: FaceUv,
    report: Option<UvReport>,
    mut plane: Option<&mut PlaneUvMode>,
) -> Vec<UvOp> {
    let mut ops = Vec::new();
    ui.label(RichText::new("Tiling").color(STUDIO_TEXT_WEAK).size(11.0));
    // Which mapping.
    let mode = uv.mode;
    ui.horizontal(|ui| {
        if let Some(mapping) = plane.as_deref_mut() {
            for (value, label, hint) in [
                (
                    PlaneUvMode::Tile,
                    "Tile",
                    "Repeat every sector from the top-left corner",
                ),
                (
                    PlaneUvMode::Stretch,
                    "Stretch",
                    "One whole texture across the plane",
                ),
            ] {
                let on = mode == UvMode::Inherit && *mapping == value;
                if ui.selectable_label(on, label).on_hover_text(hint).clicked() && !on {
                    *mapping = value;
                    ops.push(UvOp::SetMode(UvMode::Inherit));
                }
            }
        }
        let choices = [
            ("Default", UvMode::Inherit),
            ("World", UvMode::LEGACY),
            ("Fit", UvMode::fit(1, 1)),
        ];
        for (label, target) in choices {
            if label == "Default" && plane.is_some() {
                continue;
            }
            let on = std::mem::discriminant(&mode) == std::mem::discriminant(&target);
            if ui
                .selectable_label(on, label)
                .on_hover_text(match label {
                    "Default" => {
                        "The material's tiling, else world aligned at 32 texels per sector"
                    }
                    "World" => "Texels follow the world, so neighbouring faces line up",
                    _ => "Put a fixed number of tiles across this whole face",
                })
                .clicked()
                && !on
            {
                ops.push(UvOp::SetMode(target));
            }
        }
    });
    match mode {
        UvMode::World { texels_per_sector } => {
            let mut texels = texels_per_sector;
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Texels per sector");
                for t in &mut texels {
                    changed |= ui
                        .add(egui::DragValue::new(t).range(1..=1024).speed(0.5))
                        .changed();
                }
                ui.label(
                    RichText::new(format!(
                        "= {} x {} tile per sector",
                        trim(f32::from(texels[0]) / 64.0),
                        trim(f32::from(texels[1]) / 64.0)
                    ))
                    .size(10.5)
                    .color(STUDIO_TEXT_WEAK),
                );
            });
            if changed {
                ops.push(UvOp::SetMode(UvMode::World {
                    texels_per_sector: texels,
                }));
            }
        }
        UvMode::Fit { repeats_q4 } => {
            let mut tiles = repeats_q4.map(|q| f32::from(q) / 16.0);
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Tiles across");
                for t in &mut tiles {
                    changed |= ui
                        .add(
                            egui::DragValue::new(t)
                                .range(0.0625..=MAX_FIT_TILES)
                                .speed(0.05)
                                .max_decimals(2),
                        )
                        .changed();
                }
            });
            if changed {
                ops.push(UvOp::SetMode(UvMode::Fit {
                    repeats_q4: tiles.map(|t| (t * 16.0).round().clamp(1.0, 256.0) as u16),
                }));
            }
            ui.horizontal(|ui| {
                for n in [1u16, 2, 4] {
                    if ui.button(format!("{n}x{n}")).clicked() {
                        ops.push(UvOp::SetMode(UvMode::fit(n, n)));
                    }
                }
            });
        }
        UvMode::Inherit => {}
    }
    // Turn and flip.
    ui.horizontal(|ui| {
        ui.label("Turn");
        if ui
            .button("<")
            .on_hover_text("Turn the texture a quarter counter-clockwise")
            .clicked()
        {
            ops.push(UvOp::Turn(false));
        }
        if ui
            .button(">")
            .on_hover_text("Turn the texture a quarter clockwise")
            .clicked()
        {
            ops.push(UvOp::Turn(true));
        }
        ui.label(RichText::new(format!("{} deg", u32::from(uv.turns) * 90)).size(10.5));
        ui.label("Flip");
        for (axis, label) in [(0usize, "H"), (1, "V")] {
            if ui
                .selectable_label(uv.flip[axis], label)
                .on_hover_text("Mirror the texture on the face")
                .clicked()
            {
                ops.push(UvOp::ToggleFlip(axis));
            }
        }
    });
    // Fine adjustment.
    let mut fine = uv;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Offset");
        for o in &mut fine.offset_texels {
            changed |= ui.add(egui::DragValue::new(o).speed(0.25)).changed();
        }
        ui.label("Angle");
        changed |= ui
            .add(
                egui::DragValue::new(&mut fine.rotation_deg)
                    .speed(1.0)
                    .suffix(" deg"),
            )
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Scale");
        let mut sx = f32::from(fine.scale_q8[0]) / 256.0;
        let mut sy = f32::from(fine.scale_q8[1]) / 256.0;
        let a = ui
            .add(egui::DragValue::new(&mut sx).speed(0.01).max_decimals(2))
            .changed();
        let b = ui
            .add(egui::DragValue::new(&mut sy).speed(0.01).max_decimals(2))
            .changed();
        if a || b {
            fine.scale_q8 = [(sx * 256.0).round() as i16, (sy * 256.0).round() as i16];
            changed = true;
        }
        if ui
            .button("Reset")
            .on_hover_text("Back to the default tiling")
            .clicked()
        {
            ops.push(UvOp::Reset);
        }
    });
    if changed {
        ops.push(UvOp::SetOffset(fine.offset_texels));
        ops.push(UvOp::SetScale(fine.scale_q8));
        ops.push(UvOp::SetRotation(fine.rotation_deg));
    }
    // What it comes to.
    if let Some(report) = report {
        let source = if report.from_material {
            "  (from the material)"
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("effective: {}{source}", report.effective_text()))
                .size(11.0)
                .color(STUDIO_ACCENT),
        );
        if report.approximate {
            ui.label(
                RichText::new("free rotation: the cook counts the cuts exactly")
                    .size(10.5)
                    .color(STUDIO_TEXT_WEAK),
            );
        }
        if let Some(warning) = report.split_warning() {
            ui.label(
                RichText::new(format!("! {warning}"))
                    .size(10.5)
                    .color(Color32::from_rgb(232, 168, 64)),
            );
        }
    }
    ops
}

/// A tile count without trailing zeros.
fn trim(value: f32) -> String {
    format!("{value:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}
