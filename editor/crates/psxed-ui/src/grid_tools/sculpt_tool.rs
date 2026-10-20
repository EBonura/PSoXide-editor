//! The Sculpt tool (S): terraforming with a brush, in the Top and 3D views.
//!
//! Press and drag over the ground: the brush (a circle that follows the
//! pointer on the surface) raises, smooths, flattens, ramps or ripples it,
//! live, as one undo step per stroke. Shift inverts (Raise digs, a Ramp's
//! rise goes down, Dunes start with a trough). `[` and `]` change the radius,
//! Esc puts the document back exactly. The data operations are
//! `psxed_project::grid::edit::sculpt`; this file is the gesture, the option
//! row and the ghost.
//!
//! While the tool is active the walkability overlay is on: orange floors are
//! too steep to stand on, red edges are steps the body cannot climb, so the
//! ground is shaped with the character's limits in sight. The option row
//! shows the room's face count and what the stroke changed, in orange from
//! 85 percent of the per room cap (the strip's budget chip turns at the same
//! point) and red at the cap.

use super::pick::Cam;
use super::view::{cell_at_editor, editor_y};
use super::*;

use psxed_project::grid::edit::sculpt::{
    height_under, Detail, Falloff, Sculpt, SculptMode, SculptParams, SculptReport, Target,
    MAX_RADIUS, MAX_WAVELENGTH, MIN_RADIUS, MIN_WAVELENGTH,
};
use psxed_project::grid::faces::room_face_count;
use psxed_project::grid::limits::MAX_ROOM_FACES;
use psxed_project::grid::units::CLICK_UNITS;
use psxed_project::grid::GridScale;

/// What the options row keeps for the tool.
#[derive(Clone, Debug, Default)]
pub(crate) struct SculptOptions {
    pub params: SculptParams,
    /// The brush ring under the pointer, for the overlays.
    pub ghost: Option<Ghost>,
    /// The last stroke's readout, kept after release.
    pub last: Option<Readout>,
}

/// The brush ring: where, in click coordinates, and how big.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ghost {
    pub room: usize,
    pub at: [f64; 2],
    pub radius: f64,
}

/// What the option row says about the room being sculpted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Readout {
    pub room: usize,
    /// Faces of the room before the stroke and now.
    pub faces: [usize; 2],
    pub report: SculptReport,
}

impl Readout {
    /// Orange from 85 percent of the cap, red past it.
    pub(crate) fn warning(&self) -> Option<bool> {
        let now = self.faces[1];
        if now >= MAX_ROOM_FACES {
            Some(true)
        } else if now * 100 / MAX_ROOM_FACES >= 85 {
            Some(false)
        } else {
            None
        }
    }

    /// "Faces 4210 (+312)".
    pub(crate) fn faces_text(&self) -> String {
        let delta = self.faces[1] as i64 - self.faces[0] as i64;
        format!("Faces {} of {MAX_ROOM_FACES} ({delta:+})", self.faces[1])
    }

    /// The stroke in words, for the status line.
    pub(crate) fn status(&self, mode: SculptMode) -> String {
        let r = &self.report;
        let mut text = format!("Sculpt {}: {}", mode.label(), self.faces_text());
        if r.leaves[1] > r.leaves[0] {
            text += &format!(", {} cells divided", r.leaves[1] - r.leaves[0]);
        }
        if r.moved == 0 && r.vertices == 0 {
            text += ", nothing moved (the brush is smaller than the cells: raise the radius or the detail)";
        }
        if r.storeys > 0 {
            text += &format!(", {} cell(s) with storeys stay whole sectors", r.storeys);
        }
        if r.clamped > 0 {
            text += &format!(", {} corner(s) held a body under the ceiling", r.clamped);
        }
        text
    }
}

/// A stroke in flight.
#[derive(Clone, Debug)]
pub(crate) struct SculptLive {
    stroke: Sculpt,
    /// The document before the stroke: the exact cancel, and the undo step.
    before: Box<ProjectDocument>,
    faces_before: usize,
    /// Pointer events since the last dab (a held pointer keeps dabbing).
    idle: u32,
    readout: Option<Readout>,
}

/// Events a held, still pointer waits between dabs.
const HOLD_EVERY: u32 = 6;

/// Editor-space position as click coordinates (a click is a quarter sector).
pub(crate) fn clicks_at_editor(scale: GridScale, x: f32, z: f32) -> [f64; 2] {
    let per_click = f64::from(scale.num) * f64::from(CLICK_UNITS) / f64::from(scale.den.max(1));
    [f64::from(x) / per_click, f64::from(z) / per_click]
}

/// Click coordinates as editor-space `(x, z)`.
fn editor_at_clicks(scale: GridScale, at: [f64; 2]) -> [f32; 2] {
    let per_click = f64::from(scale.num) * f64::from(CLICK_UNITS) / f64::from(scale.den.max(1));
    [(at[0] * per_click) as f32, (at[1] * per_click) as f32]
}

/// The room and click position under the pointer.
fn under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(usize, [f64; 2])> {
    let world = ws.project.active_scene().world.grid()?;
    let scale = world.scale;
    let (room, x, z) = match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            let cell = cell_at_editor(scale, x, z);
            let room = ws
                .top_room_at(cell)
                .unwrap_or_else(|| ws.grid_target_room());
            (room, x, z)
        }
        GridView::Persp(rect) => {
            let hit = ws.pick_face_3d(rect, ptr.pos)?;
            (hit.face.room, hit.point[0], hit.point[2])
        }
    };
    Some((room, clicks_at_editor(scale, x, z)))
}

/// The pointer position as click coordinates, whatever room it is over (the
/// stroke stays in the room it began in).
fn clicks_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<[f64; 2]> {
    under(ws, ptr).map(|(_, at)| at)
}

fn params_for(ws: &EditorWorkspace, ptr: &GridPointer) -> SculptParams {
    SculptParams {
        invert: ptr.shift(),
        ..ws.grid.sculpt.params.clone()
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = GridHover::None;
    let radius = ws.grid.sculpt.params.radius;
    ws.grid.sculpt.ghost = under(ws, ptr).map(|(room, at)| Ghost { room, at, radius });
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((room, at)) = under(ws, &ptr.at_press()) else {
        ws.status = "Point at the ground to sculpt it".to_string();
        return;
    };
    let params = params_for(ws, ptr);
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let Some(stroke) = Sculpt::begin(world, room, at, params) else {
        return;
    };
    let live = SculptLive {
        faces_before: room_face_count(world, room),
        stroke,
        before: Box::new(ws.project.clone()),
        idle: 0,
        readout: None,
    };
    ws.grid.gesture = Gesture::Sculpt(Box::new(live));
    refresh(ws);
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let at = clicks_under(ws, ptr);
    let Gesture::Sculpt(live) = &mut ws.grid.gesture else {
        return;
    };
    let room = live.stroke.room();
    let params = params_for_stroke(&live.stroke, &ws.grid.sculpt.params, ptr);
    let mut changed = &params != live.stroke.params();
    if changed {
        live.stroke.set_params(params);
    }
    if let Some(at) = at {
        let moved = live.stroke.add_dab(at, false);
        live.idle = if moved { 0 } else { live.idle + 1 };
        if live.idle >= HOLD_EVERY {
            live.idle = 0;
            live.stroke.add_dab(at, true);
            changed = true;
        }
        changed |= moved;
        let radius = live.stroke.params().radius;
        ws.grid.sculpt.ghost = Some(Ghost { room, at, radius });
    }
    if changed {
        refresh(ws);
    }
}

/// The options now, with Shift read from the pointer.
fn params_for_stroke(stroke: &Sculpt, options: &SculptParams, ptr: &GridPointer) -> SculptParams {
    let _ = stroke;
    SculptParams {
        invert: ptr.shift(),
        ..options.clone()
    }
}

/// Replay the stroke into the document and read it back.
fn refresh(ws: &mut EditorWorkspace) {
    let Gesture::Sculpt(mut live) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    let room = live.stroke.room();
    let mode = live.stroke.params().mode;
    if let Some(world) = ws.project.active_scene_mut().world.grid_mut() {
        let report = live.stroke.apply(world);
        let faces = room_face_count(world, room);
        let readout = Readout {
            room,
            faces: [live.faces_before, faces],
            report,
        };
        ws.status = readout.status(mode);
        live.readout = Some(readout);
    }
    ws.grid_changed();
    ws.grid.sculpt.last = live.readout;
    ws.grid.gesture = Gesture::Sculpt(live);
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    let Gesture::Sculpt(live) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    let changed = live.readout.is_some_and(|r| r.report.moved > 0);
    if changed {
        ws.history.record(*live.before);
    } else {
        // A stroke that moved nothing leaves no step; put the room back (the
        // replay already did) and say so.
        ws.project = *live.before;
        ws.grid_changed();
        ws.status = live.readout.map_or_else(
            || "Nothing sculpted".to_string(),
            |r| r.status(live.stroke.params().mode),
        );
    }
}

/// Esc: put the document back as the stroke found it. Returns whether
/// there was a stroke.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    if !matches!(ws.grid.gesture, Gesture::Sculpt(_)) {
        return false;
    }
    let Gesture::Sculpt(live) = std::mem::take(&mut ws.grid.gesture) else {
        return false;
    };
    ws.project = *live.before;
    ws.grid_changed();
    ws.status = "Sculpt cancelled".to_string();
    true
}

/// `[` and `]`: the brush radius, a click at a time.
pub(super) fn adjust_radius(ws: &mut EditorWorkspace, delta: f64) {
    let radius = (ws.grid.sculpt.params.radius + delta).clamp(MIN_RADIUS, MAX_RADIUS);
    ws.grid.sculpt.params.radius = radius;
    if let Some(ghost) = &mut ws.grid.sculpt.ghost {
        ghost.radius = radius;
    }
    ws.status = format!(
        "Sculpt brush radius: {radius} clicks ({} sectors)",
        radius / 4.0
    );
}

/// Tab: the next mode.
pub(super) fn cycle_mode(ws: &mut EditorWorkspace) {
    let all = SculptMode::ALL;
    let at = all
        .iter()
        .position(|m| *m == ws.grid.sculpt.params.mode)
        .unwrap_or(0);
    let next = all[(at + 1) % all.len()];
    ws.grid.sculpt.params.mode = next;
    ws.status = format!("Sculpt mode: {}", next.label());
    if matches!(ws.grid.gesture, Gesture::Sculpt(_)) {
        refresh_params(ws);
    }
}

/// Re-run the stroke in flight with the option row's values.
pub(super) fn refresh_params(ws: &mut EditorWorkspace) {
    let options = ws.grid.sculpt.params.clone();
    let Gesture::Sculpt(live) = &mut ws.grid.gesture else {
        return;
    };
    let invert = live.stroke.params().invert;
    live.stroke.set_params(SculptParams { invert, ..options });
    refresh(ws);
}

impl EditorWorkspace {
    /// Options of the Sculpt tool, in the strip's option row.
    pub(super) fn draw_sculpt_options(&mut self, ui: &mut egui::Ui) {
        let before = self.grid.sculpt.params.clone();
        let p = &mut self.grid.sculpt.params;
        for mode in SculptMode::ALL {
            if ui
                .selectable_label(p.mode == mode, mode.label())
                .on_hover_text(match mode {
                    SculptMode::Raise => {
                        "Raise the ground; hold Shift to lower it (Tab cycles modes)"
                    }
                    SculptMode::Smooth => "Blend the ground toward the mean of its neighbours",
                    SculptMode::Flatten => "Level the ground to the height the stroke started at",
                    SculptMode::Ramp => {
                        "Drag from A to B: a straight slope between the two heights, plus the rise"
                    }
                    SculptMode::Dunes => {
                        "Drag across the ground: ripples with crests perpendicular to the drag"
                    }
                })
                .clicked()
            {
                p.mode = mode;
            }
        }
        ui.separator();
        let mut radius = (p.radius / 4.0) as f32;
        if ui
            .add(
                egui::DragValue::new(&mut radius)
                    .range((MIN_RADIUS / 4.0) as f32..=(MAX_RADIUS / 4.0) as f32)
                    .speed(0.05)
                    .fixed_decimals(2)
                    .prefix("radius ")
                    .suffix(" sectors"),
            )
            .on_hover_text("Brush radius ([ and ] change it a click at a time)")
            .changed()
        {
            p.radius = (f64::from(radius) * 4.0).clamp(MIN_RADIUS, MAX_RADIUS);
        }
        if p.mode.accumulates() {
            let mut strength = p.strength as f32;
            if ui
                .add(
                    egui::DragValue::new(&mut strength)
                        .range(0.05..=1.0)
                        .speed(0.01)
                        .fixed_decimals(2)
                        .prefix("strength "),
                )
                .on_hover_text("How much one dab does")
                .changed()
            {
                p.strength = f64::from(strength);
            }
        }
        egui::ComboBox::from_id_salt("sculpt-falloff")
            .selected_text(format!("falloff {}", p.falloff.label()))
            .width(104.0)
            .show_ui(ui, |ui| {
                for falloff in Falloff::ALL {
                    ui.selectable_value(&mut p.falloff, falloff, falloff.label());
                }
            })
            .response
            .on_hover_text("How the strength fades from the centre to the rim");
        match p.mode {
            SculptMode::Dunes => {
                ui.separator();
                let mut wave = (p.wavelength / 4.0) as f32;
                if ui
                    .add(
                        egui::DragValue::new(&mut wave)
                            .range((MIN_WAVELENGTH / 4.0) as f32..=(MAX_WAVELENGTH / 4.0) as f32)
                            .speed(0.05)
                            .fixed_decimals(2)
                            .prefix("wavelength ")
                            .suffix(" sectors"),
                    )
                    .on_hover_text("Crest to crest")
                    .changed()
                {
                    p.wavelength = (f64::from(wave) * 4.0).clamp(MIN_WAVELENGTH, MAX_WAVELENGTH);
                }
                let mut clicks = p.dune_height as f32 / CLICK_UNITS as f32;
                if ui
                    .add(
                        egui::DragValue::new(&mut clicks)
                            .range(0.25..=8.0)
                            .speed(0.05)
                            .fixed_decimals(2)
                            .prefix("height ")
                            .suffix(" clicks"),
                    )
                    .on_hover_text(
                        "A crest above the mean, in clicks (256 units; the fine step is a quarter)",
                    )
                    .changed()
                {
                    p.dune_height = ((clicks * 4.0).round() as i32).max(1) * 64;
                }
            }
            SculptMode::Ramp => {
                ui.separator();
                let mut clicks = p.rise as f32 / CLICK_UNITS as f32;
                if ui
                    .add(
                        egui::DragValue::new(&mut clicks)
                            .range(-16.0..=16.0)
                            .speed(0.05)
                            .fixed_decimals(2)
                            .prefix("rise ")
                            .suffix(" clicks"),
                    )
                    .on_hover_text(
                        "Extra height at B over the ground there: 0 grades between the two heights",
                    )
                    .changed()
                {
                    p.rise = (clicks * 4.0).round() as i32 * 64;
                }
            }
            _ => {}
        }
        ui.separator();
        egui::ComboBox::from_id_salt("sculpt-detail")
            .selected_text(format!("Detail {}", p.detail.label()))
            .width(104.0)
            .show_ui(ui, |ui| {
                for detail in Detail::ALL {
                    ui.selectable_value(&mut p.detail, detail, detail.label())
                        .on_hover_text(match detail {
                            Detail::Sector => "Whole sectors: the cheapest, the coarsest",
                            Detail::Half => "Half sectors under the brush",
                            Detail::Quarter => "Quarter sectors under the brush",
                            Detail::Auto => "Sectors, divided only where the new ground needs it. A column of storeys always stays a whole sector",
                        });
                }
            })
            .response
            .on_hover_text("How fine the cells under the brush may become");
        for (target, label) in [(Target::Floor, "Floor"), (Target::Ceiling, "Ceiling")] {
            if ui
                .selectable_label(p.target == target, label)
                .on_hover_text("Which surface of the cell is sculpted")
                .clicked()
            {
                p.target = target;
            }
        }
        let changed = *p != before;
        ui.separator();
        if let Some(readout) = self.grid.sculpt.last {
            let color = match readout.warning() {
                Some(true) => Color32::from_rgb(244, 96, 84),
                Some(false) => Color32::from_rgb(240, 150, 90),
                None => STUDIO_TEXT,
            };
            ui.label(RichText::new(readout.faces_text()).color(color).size(12.0))
                .on_hover_text(format!(
                    "Faces of room {} against the cap of {MAX_ROOM_FACES} (orange from 85 percent). The delta is this stroke's.",
                    readout.room
                ));
        }
        if changed {
            refresh_params(self);
        }
    }

    /// The brush ring over the Top view.
    pub(super) fn draw_sculpt_ghost_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some(ghost) = self.sculpt_ghost() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let scale = world.scale;
        let ring: Vec<Pos2> = ring_points(ghost.at, ghost.radius, 48)
            .into_iter()
            .map(|p| {
                let [x, z] = editor_at_clicks(scale, p);
                transform.world_to_screen([x, z])
            })
            .collect();
        draw_ring(painter, ring, self.sculpt_inverted());
        let [cx, cz] = editor_at_clicks(scale, ghost.at);
        painter.circle_filled(
            transform.world_to_screen([cx, cz]),
            2.5,
            ring_color(self.sculpt_inverted()),
        );
    }

    /// The brush ring over the 3D view, resting on the ground.
    pub(super) fn draw_sculpt_ghost_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(ghost) = self.sculpt_ghost() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(room) = world.rooms.get(ghost.room) else {
            return;
        };
        let scale = world.scale;
        let cam = Cam::new(self.viewport_3d_camera(), rect);
        let target = self.grid.sculpt.params.target;
        let lift = 24.0;
        let points: Vec<Pos2> = ring_points(ghost.at, ghost.radius, 64)
            .into_iter()
            .filter_map(|p| {
                let h = height_under(world, ghost.room, target, p)
                    .or_else(|| height_under(world, ghost.room, target, ghost.at))?;
                let [x, z] = editor_at_clicks(scale, p);
                let y = editor_y(world, room.y_offset, h.round() as i32) + lift;
                cam.project([x, y, z]).map(|(s, _)| s)
            })
            .collect();
        draw_ring(painter, points, self.sculpt_inverted());
    }

    /// The ring to draw: the hover, or the stroke's last dab.
    fn sculpt_ghost(&self) -> Option<Ghost> {
        if self.grid.tool != GridTool::Sculpt {
            return None;
        }
        self.grid.sculpt.ghost
    }

    fn sculpt_inverted(&self) -> bool {
        match &self.grid.gesture {
            Gesture::Sculpt(live) => live.stroke.params().invert,
            _ => false,
        }
    }
}

fn ring_color(invert: bool) -> Color32 {
    if invert {
        Color32::from_rgb(244, 96, 84)
    } else {
        Color32::from_rgb(110, 220, 255)
    }
}

fn ring_points(centre: [f64; 2], radius: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n)
        .map(|i| {
            let a = core::f64::consts::TAU * i as f64 / n as f64;
            [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()]
        })
        .collect()
}

fn draw_ring(painter: &egui::Painter, points: Vec<Pos2>, invert: bool) {
    if points.len() < 3 {
        return;
    }
    let color = ring_color(invert);
    painter.add(egui::Shape::closed_line(points, Stroke::new(2.0, color)));
}
