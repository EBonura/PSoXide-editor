//! The Wall run section of the Inspector: the band set of the selected wall
//! run. Lines and bands read top to bottom, the way they sit on the wall;
//! every control is one edit of the set, applied to the run or the whole
//! room by the scope toggle, with a shared set copied first when only the
//! run changes.

use super::*;

use psxed_project::face_uv::UvMode;
use psxed_project::grid::bands::{Anchor, BandSet, BandV, MAX_BAND_LINES};
use psxed_project::grid::edit::bands::{
    clear_bands, edit_bands, effective_set, run_len, BandScope,
};
use psxed_project::grid::edit::{FaceRef, UvOp};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

/// One change the user asked for.
enum Edit {
    /// A new set: the wall's material and a trim line a click up.
    Add,
    /// Take the bands off.
    Clear,
    /// A line at the middle of the wall (an absolute datum).
    AddLine,
    RemoveLine(usize),
    SetLine(usize, Anchor),
    Material(usize, Option<ResourceId>),
    V(usize, BandV),
    Uv(usize, UvOp),
}

fn name_of(ws: &EditorWorkspace, id: Option<ResourceId>) -> String {
    id.and_then(|id| ws.project.resource_name(id))
        .unwrap_or("the wall's own")
        .to_string()
}

impl EditorWorkspace {
    /// Bands of the run `primary` belongs to.
    pub(super) fn draw_wall_run_bands(&mut self, ui: &mut egui::Ui, primary: FaceRef) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let segments = run_len(world, primary);
        let current = effective_set(world, primary).map(|(i, s)| (i, s.clone()));
        let palette = self.grid.material;
        let mut edits: Vec<Edit> = Vec::new();
        let mut scope = self.grid.band_scope;
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Bands").color(STUDIO_TEXT_WEAK).size(11.0));
            for s in [BandScope::Run, BandScope::Room] {
                if ui
                    .selectable_label(scope == s, super::bands_tool::scope_label(s))
                    .clicked()
                {
                    scope = s;
                }
            }
        });
        ui.label(
            RichText::new(format!(
                "{} wall segments in this run{}",
                segments,
                match scope {
                    BandScope::Run | BandScope::Piece => "",
                    BandScope::Room => ", edits reach every wall that uses the set",
                }
            ))
            .size(10.5)
            .color(STUDIO_TEXT_WEAK),
        );
        match &current {
            None => {
                ui.label(RichText::new("no bands").color(STUDIO_TEXT_WEAK));
                if ui
                    .button("Add bands")
                    .on_hover_text("Cut this run into a trim at the base and the wall above it")
                    .clicked()
                {
                    edits.push(Edit::Add);
                }
            }
            Some((index, set)) => {
                ui.label(
                    RichText::new(format!("set {} of {} lines", index, set.lines.len()))
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                self.draw_set(ui, set, palette, &mut edits);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(set.lines.len() < MAX_BAND_LINES, egui::Button::new("Add line"))
                        .on_hover_text("A line at the middle of the wall; drag it in the 3D view with the Bands tool (E)")
                        .clicked()
                    {
                        edits.push(Edit::AddLine);
                    }
                    if ui.button("Remove bands").clicked() {
                        edits.push(Edit::Clear);
                    }
                });
            }
        }
        self.grid.band_scope = scope;
        if edits.is_empty() {
            return;
        }
        if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
            for edit in edits {
                match edit {
                    Edit::Add => {
                        edit_bands(world, primary, scope, |set| {
                            set.insert_line(0, Anchor::Bottom(CLICK_UNITS))
                        });
                    }
                    Edit::Clear => {
                        clear_bands(world, primary, scope);
                    }
                    Edit::AddLine => {
                        edit_bands(world, primary, scope, |set| {
                            let at = set.lines.len();
                            set.insert_line(at, Anchor::Top(CLICK_UNITS))
                        });
                    }
                    Edit::RemoveLine(i) => {
                        edit_bands(world, primary, scope, |set| set.remove_line(i));
                    }
                    Edit::SetLine(i, anchor) => {
                        edit_bands(world, primary, scope, |set| set.set_line(i, anchor));
                    }
                    Edit::Material(i, material) => {
                        edit_bands(world, primary, scope, |set| {
                            set.set_band_material(i, material)
                        });
                    }
                    Edit::V(i, v) => {
                        edit_bands(world, primary, scope, |set| match set.bands.get_mut(i) {
                            Some(b) if b.v != v => {
                                b.v = v;
                                true
                            }
                            _ => false,
                        });
                    }
                    Edit::Uv(i, op) => {
                        edit_bands(world, primary, scope, |set| match set.bands.get_mut(i) {
                            Some(b) => {
                                let before = b.uv;
                                op.apply(&mut b.uv);
                                b.uv != before
                            }
                            None => false,
                        });
                    }
                }
            }
        }
        // The Inspector records the undo step, coalescing a drag.
        self.grid_changed();
    }

    /// Lines and bands of `set`, top to bottom.
    fn draw_set(
        &self,
        ui: &mut egui::Ui,
        set: &BandSet,
        palette: Option<ResourceId>,
        edits: &mut Vec<Edit>,
    ) {
        let n = set.bands.len();
        for band in (0..n).rev() {
            // The line above this band, if any.
            if band + 1 < n {
                self.draw_line_row(ui, set, band, edits);
            }
            self.draw_band_row(ui, set, band, palette, edits);
        }
    }

    fn draw_line_row(&self, ui: &mut egui::Ui, set: &BandSet, line: usize, edits: &mut Vec<Edit>) {
        let anchor = set.lines[line];
        ui.horizontal(|ui| {
            ui.label(RichText::new("line").color(STUDIO_TEXT_WEAK).size(10.5));
            let mut kind = anchor;
            egui::ComboBox::from_id_salt(("band_line_kind", line))
                .selected_text(match anchor {
                    Anchor::Bottom(_) => "above the bottom",
                    Anchor::Top(_) => "below the top",
                    Anchor::Abs(_) => "at height",
                })
                .width(112.0)
                .show_ui(ui, |ui| {
                    for (label, k) in [
                        ("above the bottom", Anchor::Bottom(anchor.value())),
                        ("below the top", Anchor::Top(anchor.value())),
                        ("at height", Anchor::Abs(anchor.value())),
                    ] {
                        if ui
                            .selectable_label(
                                std::mem::discriminant(&anchor) == std::mem::discriminant(&k),
                                label,
                            )
                            .clicked()
                        {
                            kind = k;
                        }
                    }
                });
            let mut clicks = kind.value() as f32 / CLICK_UNITS as f32;
            let moved = ui
                .add(
                    egui::DragValue::new(&mut clicks)
                        .speed(0.05)
                        .suffix(" clicks")
                        .max_decimals(2),
                )
                .on_hover_text("Bottom and top lines follow slopes; a height is a datum")
                .changed();
            if moved {
                kind = kind.with_value(snap_to(
                    (clicks * CLICK_UNITS as f32).round() as i32,
                    FINE_UNITS,
                ));
            }
            if kind != anchor {
                edits.push(Edit::SetLine(line, kind));
            }
            if ui
                .small_button(icons::text(icons::TRASH, 12.0))
                .on_hover_text("Remove the line; the bands around it join")
                .clicked()
            {
                edits.push(Edit::RemoveLine(line));
            }
        });
    }

    fn draw_band_row(
        &self,
        ui: &mut egui::Ui,
        set: &BandSet,
        band: usize,
        palette: Option<ResourceId>,
        edits: &mut Vec<Edit>,
    ) {
        let b = &set.bands[band];
        let title = match (band, set.bands.len()) {
            (0, _) => "base band".to_string(),
            (i, n) if i + 1 == n => "top band".to_string(),
            (i, _) => format!("band {}", i + 1),
        };
        egui::CollapsingHeader::new(
            RichText::new(format!("{title}: {}", name_of(self, b.material))).size(11.5),
        )
        .id_salt(("band_row", band))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(palette.is_some(), egui::Button::new("Use palette"))
                    .on_hover_text("Put the chosen palette material on this band")
                    .clicked()
                {
                    edits.push(Edit::Material(band, palette));
                }
                if ui
                    .add_enabled(b.material.is_some(), egui::Button::new("Wall's own"))
                    .clicked()
                {
                    edits.push(Edit::Material(band, None));
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("V").color(STUDIO_TEXT_WEAK).size(10.5));
                for v in [BandV::World, BandV::FromBottom, BandV::FromTop] {
                    if ui
                        .selectable_label(b.v == v, v.label())
                        .on_hover_text(match v {
                            BandV::World => "Texture follows the world height",
                            BandV::FromBottom => "Texture sits on the band's lower line",
                            BandV::FromTop => "Texture hangs from the band's upper line",
                        })
                        .clicked()
                    {
                        edits.push(Edit::V(band, v));
                    }
                }
            });
            let report = report_of(self, b.uv);
            for op in super::tiling_panel::draw_tiling(ui, b.uv, report, None) {
                edits.push(Edit::Uv(band, op));
            }
        });
    }
}

/// The tiling report of a band's mapping on a one sector wide strip, an
/// estimate: the run's real size is not known here.
fn report_of(
    ws: &EditorWorkspace,
    uv: psxed_project::face_uv::FaceUv,
) -> Option<psxed_project::grid::uv::UvReport> {
    let default: Option<UvMode> = None;
    let world_uv = ws
        .project
        .active_scene()
        .world
        .grid()
        .map_or(UvMode::LEGACY, |world| world.default_uv());
    Some(psxed_project::grid::uv::report(
        &uv,
        default,
        world_uv,
        [0.0, 0.0],
        [1024.0, 1024.0],
    ))
}
