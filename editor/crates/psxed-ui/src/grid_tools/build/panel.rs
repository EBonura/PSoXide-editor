//! The options bar of each Build tool, and what the tools draw over the
//! views. Number fields read the same options a typed number sets during a
//! gesture.

use super::*;

use options::{ArcProfile, Lock, OpeningHead};
use psxed_project::{DoorDirection, DoorSide, SwitchBehavior};

fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(11.0));
}

/// A whole-number field.
fn num(
    ui: &mut egui::Ui,
    text: &str,
    v: &mut i32,
    range: std::ops::RangeInclusive<i32>,
    tip: &str,
) {
    label(ui, text);
    ui.add(egui::DragValue::new(v).range(range).speed(0.1))
        .on_hover_text(tip);
}

/// A field in clicks over a value in grid units.
fn clicks_field(ui: &mut egui::Ui, text: &str, v: &mut i32, min: i32, max: i32, tip: &str) {
    label(ui, text);
    let mut c = *v as f32 / CLICK_UNITS as f32;
    if ui
        .add(
            egui::DragValue::new(&mut c)
                .speed(0.05)
                .max_decimals(2)
                .suffix(" clicks"),
        )
        .on_hover_text(tip)
        .changed()
    {
        *v = snap_to((c * CLICK_UNITS as f32).round() as i32, FINE_UNITS).clamp(min, max);
    }
}

fn chip<T: PartialEq + Copy>(ui: &mut egui::Ui, v: &mut T, value: T, text: &str, tip: &str) {
    ui.selectable_value(v, value, text).on_hover_text(tip);
}

fn heads(ui: &mut egui::Ui, v: &mut OpeningHead) {
    chip(
        ui,
        v,
        OpeningHead::Rectangular,
        "Rectangular",
        "A flat lintel",
    );
    chip(ui, v, OpeningHead::Pointed, "Pointed", "A pointed arch");
    chip(ui, v, OpeningHead::Round, "Round", "A round arch");
}

fn seed(ui: &mut egui::Ui, v: &mut u32) {
    label(ui, "Seed");
    let mut n = *v as i32;
    if ui
        .add(egui::DragValue::new(&mut n).range(0..=9999))
        .changed()
    {
        *v = n.max(0) as u32;
    }
    if ui
        .button("\u{1F3B2}")
        .on_hover_text("Re-roll the seed (Shift+X)")
        .clicked()
    {
        *v = v.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 10_000;
    }
}

/// Draw the next seed for a terrain patch or a ruin.
pub(super) fn reroll(ws: &mut EditorWorkspace, kind: BuildKind) {
    let o = &mut ws.grid.build.opts;
    let v = if kind == BuildKind::Terrain {
        &mut o.terrain.seed
    } else {
        &mut o.ruin.seed
    };
    *v = v.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 10_000;
    ws.status = format!("Seed {}", *v);
    if ws.grid.build.drag.is_some() {
        refresh(ws);
    }
}

/// The style combo. `bays` offers the captured and shipped bays too: only a
/// corridor can lay one.
fn style_picker(
    ui: &mut egui::Ui,
    entries: &[StyleEntry],
    current: &mut options::StyleResolved,
    bays: bool,
) {
    label(ui, "Style");
    let shown = entries
        .iter()
        .find(|e| &e.what == current)
        .map_or("Plain", |e| e.name.as_str());
    egui::ComboBox::from_id_salt("build_style")
        .selected_text(shown)
        .width(120.0)
        .show_ui(ui, |ui| {
            for e in entries
                .iter()
                .filter(|e| bays || !matches!(e.what, options::StyleResolved::Bay(_)))
            {
                let text = if e.project {
                    format!("{}  (project)", e.name)
                } else {
                    e.name.clone()
                };
                if ui.selectable_label(&e.what == current, text).clicked() {
                    *current = e.what.clone();
                }
            }
        });
}

impl EditorWorkspace {
    /// The cells of the selection as a rectangle, for Capture bay.
    fn selected_bay(&self) -> Option<(usize, CellRect)> {
        use psxed_project::grid::edit::GridElement;
        let mut room = None;
        let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
        for e in self.grid.selection.items() {
            let (r, cell) = match e {
                GridElement::Cell { room, cell } => (*room, *cell),
                GridElement::Face(f) => (
                    f.room,
                    [
                        f.leaf.x.div_euclid(CLICKS_PER_SECTOR),
                        f.leaf.z.div_euclid(CLICKS_PER_SECTOR),
                    ],
                ),
                _ => continue,
            };
            room.get_or_insert(r);
            for i in 0..2 {
                lo[i] = lo[i].min(cell[i]);
                hi[i] = hi[i].max(cell[i]);
            }
        }
        Some((room?, CellRect { min: lo, max: hi }))
    }

    /// Capture the selected cells as a named style stored with the project.
    pub(crate) fn capture_bay_style(&mut self) {
        let Some((room, rect)) = self.selected_bay() else {
            self.status = "Select the cells of the bay first".to_string();
            return;
        };
        let name = self.grid.build.capture_name.clone();
        match styles::capture(&self.project_dir, &self.project, room, rect, &name) {
            Ok(stored) => {
                self.status = format!("Captured the bay as \"{stored}\", listed under Style");
                self.refresh_styles();
            }
            Err(e) => self.status = e,
        }
    }

    /// The options of the active Build tool.
    pub(crate) fn draw_build_options(&mut self, ui: &mut egui::Ui) {
        let Some(kind) = kind_of(self) else { return };
        if !self.grid.build.styles_loaded {
            self.refresh_styles();
        }
        let entries = self.grid.build.styles.clone();
        let mut capture = false;
        {
            let b = &mut self.grid.build;
            let o = &mut b.opts;
            match kind {
                BuildKind::Room => {
                    let r = &mut o.room;
                    chip(
                        ui,
                        &mut r.shape,
                        RoomShape::Rectangle,
                        "Rectangle",
                        "Drag a rectangle",
                    );
                    chip(
                        ui,
                        &mut r.shape,
                        RoomShape::Octagon,
                        "Octagon",
                        "A rectangle with cut corners",
                    );
                    chip(
                        ui,
                        &mut r.shape,
                        RoomShape::Lasso,
                        "Lasso",
                        "Click corners, Enter closes",
                    );
                    ui.selectable_value(&mut r.into_this, false, "New room");
                    ui.selectable_value(&mut r.into_this, true, "Into this room");
                    clicks_field(ui, "Height", &mut r.height, 1024, 65536, "Floor to ceiling");
                    if r.shape == RoomShape::Octagon {
                        num(ui, "Cut", &mut r.cut, 1..=8, "Sectors cut from each corner");
                    }
                    style_picker(ui, &entries, &mut r.style, false);
                    capture = true;
                }
                BuildKind::Corridor => {
                    let c = &mut o.corridor;
                    num(ui, "Width", &mut c.width, 1..=8, "Sectors across");
                    clicks_field(
                        ui,
                        "Headroom",
                        &mut c.headroom,
                        1024,
                        32768,
                        "Floor to ceiling",
                    );
                    chip(
                        ui,
                        &mut c.stairs,
                        false,
                        "Ramp",
                        "A slope where the height changes",
                    );
                    chip(
                        ui,
                        &mut c.stairs,
                        true,
                        "Stairs",
                        "Treads where the height changes",
                    );
                    ui.selectable_value(&mut c.into_this, false, "New room");
                    ui.selectable_value(&mut c.into_this, true, "Into this room");
                    let before = c.style.clone();
                    style_picker(ui, &entries, &mut c.style, true);
                    if c.style != before {
                        // A bay has one width: take it, so the next drag fits.
                        if let options::StyleResolved::Bay(bay) = &c.style {
                            c.width = bay.size[0];
                        }
                    }
                    capture = true;
                }
                BuildKind::Opening => {
                    let p = &mut o.opening;
                    chip(ui, &mut p.window, false, "Door", "From the floor up");
                    chip(ui, &mut p.window, true, "Window", "From a sill up");
                    heads(ui, &mut p.head);
                    clicks_field(ui, "Lintel", &mut p.top, 256, 32768, "Height of the top");
                    if p.window {
                        clicks_field(ui, "Sill", &mut p.sill, 0, 32768, "Height of the sill");
                    }
                }
                BuildKind::Arcade | BuildKind::Colonnade => {
                    let a = &mut o.arcade;
                    num(
                        ui,
                        "Bays",
                        &mut a.bays,
                        1..=32,
                        "Arches, or one less than the columns",
                    );
                    if kind == BuildKind::Arcade {
                        num(
                            ui,
                            "Span",
                            &mut a.span,
                            0..=8192,
                            "Clear opening, 0 fills the bay",
                        );
                        num(ui, "Pier", &mut a.pier, 64..=2048, "Pier width");
                        num(
                            ui,
                            "Rise",
                            &mut a.rise,
                            0..=8192,
                            "Crown rise, 0 for the curve's own",
                        );
                        chip(ui, &mut a.profile, ArcProfile::Round, "Round", "");
                        chip(ui, &mut a.profile, ArcProfile::Pointed, "Pointed", "");
                        chip(ui, &mut a.profile, ArcProfile::Segmental, "Flat arc", "");
                        chip(ui, &mut a.profile, ArcProfile::Flat, "Lintel", "");
                        ui.checkbox(&mut a.perch, "Hook on crowns");
                    }
                }
                BuildKind::Bridge => {
                    let g = &mut o.bridge;
                    num(ui, "Width", &mut g.width, 256..=8192, "Deck width");
                    num(
                        ui,
                        "Parapet",
                        &mut g.parapet,
                        0..=4096,
                        "Parapet height, 0 for none",
                    );
                    num(
                        ui,
                        "Piers every",
                        &mut g.pier_spacing,
                        0..=16384,
                        "Spacing, 0 for none",
                    );
                }
                BuildKind::Building => {
                    let g = &mut o.building;
                    num(ui, "Storeys", &mut g.storeys, 1..=4, "Floors");
                    clicks_field(
                        ui,
                        "Storey",
                        &mut g.storey_height,
                        2048,
                        16384,
                        "Floor to floor",
                    );
                    ui.checkbox(&mut g.door, "Door");
                    num(
                        ui,
                        "Window every",
                        &mut g.window_every,
                        0..=8,
                        "Sectors, 0 for none",
                    );
                    heads(ui, &mut g.window_head);
                    ui.checkbox(&mut g.stair, "Stair");
                    ui.checkbox(&mut g.parapet, "Parapet");
                }
                BuildKind::Terrain => {
                    let t = &mut o.terrain;
                    seed(ui, &mut t.seed);
                    num(
                        ui,
                        "Amplitude",
                        &mut t.amplitude,
                        0..=16,
                        "Clicks of relief",
                    );
                    num(ui, "Wavelength", &mut t.wavelength, 1..=32, "Sectors");
                    num(
                        ui,
                        "Slope X",
                        &mut t.slope_x,
                        -512..=512,
                        "Units of rise per sector",
                    );
                    num(
                        ui,
                        "Slope Z",
                        &mut t.slope_z,
                        -512..=512,
                        "Units of rise per sector",
                    );
                    ui.checkbox(&mut t.keep_walkable, "Keep walkable");
                    num(
                        ui,
                        "Edge blend",
                        &mut t.blend,
                        0..=8,
                        "Sectors, 0 for a cliff",
                    );
                }
                BuildKind::Ruin => {
                    let r = &mut o.ruin;
                    seed(ui, &mut r.seed);
                    label(ui, "Intensity");
                    let mut i = r.intensity as i32;
                    if ui
                        .add(egui::DragValue::new(&mut i).range(0..=100))
                        .changed()
                    {
                        r.intensity = i.clamp(0, 100) as u32;
                    }
                    ui.selectable_value(&mut r.whole_room, false, "Region");
                    ui.selectable_value(&mut r.whole_room, true, "Whole room");
                    ui.toggle_value(&mut r.painting, "Paint path")
                        .on_hover_text("Paint the cells to keep clear, Shift removes");
                    if ui
                        .add_enabled(
                            !r.keep.is_empty(),
                            egui::Button::new(format!("Clear path ({})", r.keep.len())),
                        )
                        .clicked()
                    {
                        r.keep.clear();
                    }
                    ui.checkbox(&mut r.crumble, "Crumble");
                    ui.checkbox(&mut r.sink, "Sink");
                    ui.checkbox(&mut r.floor_bays, "Floor holes");
                    ui.checkbox(&mut r.roof_bays, "Roof holes");
                    ui.checkbox(&mut r.breaches, "Breaches");
                    ui.checkbox(&mut r.fallen_arches, "Fallen arches");
                    ui.checkbox(&mut r.rubble, "Rubble");
                }
                BuildKind::Door => {
                    let d = &mut o.door;
                    heads(ui, &mut d.head);
                    clicks_field(ui, "Height", &mut d.height, 256, 32768, "Doorway height");
                    label(ui, "Slides");
                    egui::ComboBox::from_id_salt("door_dir")
                        .selected_text(format!("{:?}", d.direction))
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for dir in [
                                DoorDirection::Up,
                                DoorDirection::Down,
                                DoorDirection::East,
                                DoorDirection::West,
                                DoorDirection::South,
                                DoorDirection::North,
                            ] {
                                ui.selectable_value(&mut d.direction, dir, format!("{dir:?}"));
                            }
                        });
                    num(
                        ui,
                        "Distance",
                        &mut d.distance,
                        0..=8192,
                        "0 slides the whole doorway",
                    );
                    let mut t = i32::from(d.ticks);
                    num(ui, "Speed", &mut t, 1..=600, "Ticks from closed to open");
                    d.ticks = t.clamp(1, 600) as u16;
                    let mut kind_now = match d.lock {
                        Lock::Open => 0,
                        Lock::Locked => 1,
                        Lock::OneWay(_) => 2,
                    };
                    ui.selectable_value(&mut kind_now, 0, "Open");
                    ui.selectable_value(&mut kind_now, 1, "Locked");
                    ui.selectable_value(&mut kind_now, 2, "One way");
                    d.lock = match (kind_now, d.lock) {
                        (0, _) => Lock::Open,
                        (1, _) => Lock::Locked,
                        (_, Lock::OneWay(s)) => Lock::OneWay(s),
                        _ => Lock::OneWay(DoorSide::South),
                    };
                    if let Lock::OneWay(side) = &mut d.lock {
                        label(ui, "Refuses from");
                        for (s, arrow) in [
                            (DoorSide::North, "\u{2191} N"),
                            (DoorSide::East, "\u{2192} E"),
                            (DoorSide::South, "\u{2193} S"),
                            (DoorSide::West, "\u{2190} W"),
                        ] {
                            ui.selectable_value(side, s, arrow);
                        }
                    }
                    if d.lock != Lock::Open {
                        label(ui, "Refusal text");
                        ui.add(egui::TextEdit::singleline(&mut d.refusal).desired_width(140.0));
                    }
                    ui.checkbox(&mut d.persistent, "Remember across death and saves");
                }
                BuildKind::Lever => {
                    let l = &mut o.lever;
                    chip(
                        ui,
                        &mut l.behavior,
                        SwitchBehavior::Toggle,
                        "Toggle",
                        "Each pull flips it",
                    );
                    chip(
                        ui,
                        &mut l.behavior,
                        SwitchBehavior::OneShot,
                        "One shot",
                        "Pulled once, then done",
                    );
                    ui.checkbox(&mut l.unlocks, "Unlocks the door");
                    ui.checkbox(&mut l.persistent, "Remember across death and saves");
                }
                BuildKind::Checkpoint => {
                    label(ui, "Click a floor to place one");
                }
            }
        }
        if capture {
            ui.separator();
            label(ui, "Capture bay");
            ui.add(
                egui::TextEdit::singleline(&mut self.grid.build.capture_name)
                    .hint_text("name")
                    .desired_width(90.0),
            );
            if ui
                .button("Capture as style")
                .on_hover_text("Select the cells of one bay first; it is stored with the project")
                .clicked()
            {
                self.capture_bay_style();
            }
        }
    }
}

const GHOST: Color32 = Color32::from_rgb(96, 232, 132);
const REFUSED: Color32 = Color32::from_rgb(244, 96, 84);

impl EditorWorkspace {
    /// The outline of what a Build gesture has drawn, over the 3D view.
    pub(crate) fn draw_build_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some((lines, text)) = outline(self) else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let color = if text.is_empty() { GHOST } else { REFUSED };
        for line in &lines {
            for w in line.windows(2) {
                if let Some([a, b]) = view::project_segment(camera, rect, w[0], w[1]) {
                    painter.line_segment([a, b], Stroke::new(2.5, color));
                }
            }
        }
        if !text.is_empty() {
            super::super::overlay::label_chip(
                painter,
                rect.center_top() + Vec2::new(0.0, 28.0),
                Align2::CENTER_TOP,
                &text,
            );
        }
    }

    /// The same over the plan.
    pub(crate) fn draw_build_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some((lines, text)) = outline(self) else {
            return;
        };
        let color = if text.is_empty() { GHOST } else { REFUSED };
        for line in &lines {
            for w in line.windows(2) {
                painter.line_segment(
                    [
                        transform.world_to_screen([w[0][0], w[0][2]]),
                        transform.world_to_screen([w[1][0], w[1][2]]),
                    ],
                    Stroke::new(2.5, color),
                );
            }
        }
        if !text.is_empty() {
            super::super::overlay::label_chip(
                painter,
                painter.clip_rect().center_top() + Vec2::new(0.0, 28.0),
                Align2::CENTER_TOP,
                &text,
            );
        }
    }
}
