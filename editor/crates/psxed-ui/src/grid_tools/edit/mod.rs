//! The Edit tool: one tool that does what the pointer is over.
//!
//! Hover names the element under the pointer and what a press-drag will do
//! to it, before any press. A press-drag acts on what was pressed (see
//! [`spec`] for the decision table): a floor or ceiling moves as one flat
//! block, an edge tilts, a corner lifts, a wall face pushes its room out and
//! in a sector at a time, a wall's top or bottom edge stretches it, and the
//! arrow over a wall slides it keeping its height. When the pressed element
//! belongs to the selection the gesture applies to the whole selection as
//! one shape; when it does not, it applies to that element alone and the
//! selection becomes it.
//!
//! * [`target`]: what is under the pointer (shared by hover, press, click).
//! * [`spec`]: what a press does, from the target and the selection.
//! * [`plan`]: the document at an amount, worked out at the press.
//! * [`wall_handle`]: the one seam over how walls are stored.
//! * [`amount`]: digits typed during a gesture.
//! * [`keys`]: the keyboard of the tool.
//! * [`pick_select`]: click, range, toggle, double-click and box selection.
//! * [`repeat_move`]: repeat the last action, and the Move mode.
//! * [`menu`]: the context menu of an element.
//! * [`overlay`]: arrows, handles and the amount chip.

use std::time::Instant;

use super::*;

use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

pub(super) mod amount;
pub(super) mod keys;
pub(crate) mod menu;
pub(super) mod overlay;
pub(super) mod pick_select;
pub(crate) mod plan;
pub(super) mod repeat_move;
pub(super) mod spec;
pub(crate) mod target;
pub(crate) mod wall_handle;

use amount::{clicks_text, Amount};
use plan::Plan;
use spec::{Kind, Spec};
use target::{Hit, Target};

/// The axis a gesture reads the pointer along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Axis {
    /// Straight up and down through the grab point.
    Vertical,
    /// Along a horizontal direction `[x, z]` (unit, editor space).
    Horizontal([f32; 2]),
}

/// A press-drag in flight.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    /// The document as it was at the press: Esc puts it back exactly.
    pub before: Box<ProjectDocument>,
    pub plan: Plan,
    pub kind: Kind,
    pub axis: Axis,
    /// Where the press landed, and the point of the element it grabbed
    /// (editor units).
    pub press: Pos2,
    pub grab: [f32; 3],
    /// How far along the axis, editor units, the press already was from
    /// the grab point (an arrow stands above its wall): the amount counts
    /// from here.
    pub travel0: f32,
    /// Pixels the pointer must travel up to change the grab point by one
    /// grid unit, from the press (the fallback for a camera that looks
    /// straight down).
    pub pixels_per_unit: f32,
    /// What it is called in the hint ("raise floor").
    pub label: String,
    /// What else it moves ("moves the floor of 2 cells").
    pub note: Option<String>,
    /// The amount applied now: grid units, or sectors for a push.
    pub amount: i32,
    /// Whether the undo step is recorded (at the first change).
    pub recorded: bool,
    /// The last result, for the hint.
    pub last: plan::Applied,
}

/// The state of the Edit tool. Not saved with the project.
#[derive(Default)]
pub(crate) struct EditState {
    /// What a press-drag would do now, for the hint bar.
    pub hint: String,
    /// The wall whose slide arrow shows: the last one hovered.
    pub arrow_wall: Option<FaceRef>,
    /// The gesture in flight.
    pub(super) drag: Option<Drag>,
    /// Digits typed while a gesture runs, or over a selection.
    pub(super) amount: Amount,
    /// The last action, for the repeat command.
    pub(super) last: Option<repeat_move::Repeat>,
    /// The last click, for the double click.
    pub(super) last_click: Option<(Instant, Pos2)>,
    /// What the context menu is open for.
    pub(super) menu: Option<menu::MenuTarget>,
    /// How far the backtick key has cycled the candidates under the pointer,
    /// and where.
    pub(super) cycle: (usize, Option<Pos2>),
    /// The Move handle drag in flight.
    pub(super) moving: Option<repeat_move::MoveDrag>,
    /// The amount of the options bar's "Raise by" field, in clicks.
    pub(crate) offset_clicks: f32,
}

impl EditState {
    /// Whether the context menu is open over an element.
    #[cfg(test)]
    pub(crate) fn menu_is_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Forget any gesture and typed number (a document swap, a tool change).
    pub(crate) fn abandon(&mut self) {
        self.drag = None;
        self.moving = None;
        self.amount.clear();
        self.hint.clear();
        self.arrow_wall = None;
    }
}

impl EditorWorkspace {
    /// The options of the Edit tool, for the options bar: Weld, what is
    /// selected, an exact amount to raise it by, and the last action again.
    pub(crate) fn draw_edit_options(&mut self, ui: &mut egui::Ui) {
        ui.toggle_value(&mut self.grid.link, "Weld").on_hover_text(
            "L: off, a floor moves as a block and the walls between heights follow; on, corners that meet move together",
        );
        let count = self
            .project
            .active_scene()
            .world
            .grid()
            .map_or(0, |w| self.grid.selection.faces(w).len());
        let text = match count {
            0 => "No faces selected".to_string(),
            1 => "1 face selected".to_string(),
            n => format!("{n} faces selected"),
        };
        ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(11.0));
        ui.separator();
        ui.label(RichText::new("Raise by").color(STUDIO_TEXT_WEAK).size(11.0));
        ui.add(
            egui::DragValue::new(&mut self.grid.edit.offset_clicks)
                .speed(0.05)
                .suffix(" clicks")
                .max_decimals(2),
        )
        .on_hover_text("An exact amount for the selection, in 256-unit clicks (negative lowers). Typing a number while you drag does the same.");
        if ui
            .add_enabled(count > 0, egui::Button::new("Apply"))
            .on_hover_text(
                "Raise or lower the selection by this amount (walls slide, keeping their height)",
            )
            .clicked()
        {
            let units = snap_to(
                (self.grid.edit.offset_clicks * CLICK_UNITS as f32).round() as i32,
                FINE_UNITS,
            );
            repeat_move::offset_selection(self, units);
        }
        if ui
            .add_enabled(self.grid.edit.last.is_some(), egui::Button::new("Repeat"))
            .on_hover_text("R: do the last action again on the selection")
            .clicked()
        {
            repeat_move::repeat(self);
        }
    }

    /// What the Edit tool says a press-drag would do, for the hint bar; empty
    /// when the tool is not active.
    pub(crate) fn edit_hint(&self) -> &str {
        if self.grid.tool == GridTool::Edit {
            &self.grid.edit.hint
        } else {
            ""
        }
    }
}

/// The amount, in grid units, the pointer asks for now.
fn pointer_amount(ws: &EditorWorkspace, drag: &Drag, ptr: &GridPointer) -> Option<i32> {
    let world = ws.project.active_scene().world.grid()?;
    let scale = world.scale;
    let per_editor = scale.den.max(1) as f32 / scale.num.max(1) as f32;
    let fine = ptr.alt();
    match (drag.axis, ptr.view) {
        (Axis::Vertical, GridView::Top(_)) => {
            let raw = (drag.press.y - ptr.pos.y) / view::TOP_PIXELS_PER_CLICK * CLICK_UNITS as f32;
            Some(snap_amount(raw, fine))
        }
        (Axis::Vertical, GridView::Persp(rect)) => {
            let units = vertical_travel(ws, rect, ptr.pos, drag.grab)
                .map(|editor| (editor - drag.travel0) * per_editor)
                .unwrap_or_else(|| (drag.press.y - ptr.pos.y) / drag.pixels_per_unit.max(1e-4));
            // One click a pixel at most, so a grazing camera cannot throw
            // the amount far for a small movement.
            let cap = drag.press.distance(ptr.pos).max(1.0) * CLICK_UNITS as f32;
            Some(snap_amount(units.clamp(-cap, cap), fine))
        }
        (Axis::Horizontal(n), GridView::Top(transform)) => {
            let a = transform.screen_to_world(drag.press);
            let b = transform.screen_to_world(ptr.pos);
            let travel = (b[0] - a[0]) * n[0] + (b[1] - a[1]) * n[1];
            Some((travel / scale.sector_editor_units() as f32).round() as i32)
        }
        (Axis::Horizontal(n), GridView::Persp(rect)) => {
            let (origin, dir) = ws.grid_ray(rect, ptr.pos)?;
            let hit = view::ray_plane_y(origin, dir, drag.grab[1])?;
            let travel = (hit[0] - drag.grab[0]) * n[0] + (hit[2] - drag.grab[2]) * n[1];
            Some((travel / scale.sector_editor_units() as f32).round() as i32)
        }
    }
}

/// Grid units snapped to a click, or to the fine step with Alt.
fn snap_amount(raw: f32, fine: bool) -> i32 {
    snap_to(
        raw.round() as i32,
        if fine { FINE_UNITS } else { CLICK_UNITS },
    )
}

/// How far, in editor units, the pointer's ray is above or below `grab` on
/// the vertical line through it. The ray is cut with the vertical plane that
/// contains the line and faces the camera, so the amount under the cursor is
/// the amount you get from any camera angle. `None` for a camera that looks
/// almost straight down or up.
fn vertical_travel(ws: &EditorWorkspace, rect: Rect, pos: Pos2, grab: [f32; 3]) -> Option<f32> {
    let (origin, dir) = ws.grid_ray(rect, pos)?;
    let flat = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt();
    if flat < 0.2 {
        return None;
    }
    let n = [dir[0] / flat, dir[2] / flat];
    let along = dir[0] * n[0] + dir[2] * n[1];
    let t = ((grab[0] - origin[0]) * n[0] + (grab[2] - origin[2]) * n[1]) / along;
    (t > 0.0).then(|| origin[1] + dir[1] * t - grab[1])
}

/// Hover: light up the element and publish what a press-drag would do.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if ws.grid.edit.drag.is_some() {
        return;
    }
    let hit = candidate(ws, ptr);
    set_hover(ws, ptr, &hit);
}

/// The pick, after the backtick key has cycled it.
fn candidate(ws: &mut EditorWorkspace, ptr: &GridPointer) -> Hit {
    let hit = target::at(ws, ptr);
    // The cycle starts over when the pointer moves on.
    if let (_, Some(at)) = ws.grid.edit.cycle {
        if at.distance(ptr.pos) > 6.0 {
            ws.grid.edit.cycle = (0, None);
        }
    }
    let level = ws.grid.edit.cycle.0;
    if level == 0 {
        return hit;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return hit;
    };
    let _ = world;
    pick_select::coarsen(hit, level)
}

fn set_hover(ws: &mut EditorWorkspace, ptr: &GridPointer, hit: &Hit) {
    ws.grid.edit.arrow_wall = match &hit.target {
        Target::Wall(face, _) | Target::WallEdge(face, _) | Target::WallArrow(face) => Some(*face),
        _ => {
            // The arrow stays while the pointer is on its way to it.
            None
        }
    };
    let world = ws.project.active_scene().world.grid();
    ws.grid.hover = match world.and_then(|w| hit.target.highlight(w)) {
        Some(element) => GridHover::Elements(vec![element]),
        None => GridHover::None,
    };
    ws.grid.edit.hint = hint_for(ws, ptr, hit);
}

/// The words for what a press-drag does on `hit`.
fn hint_for(ws: &EditorWorkspace, ptr: &GridPointer, hit: &Hit) -> String {
    if ptr.shift() {
        return "Drag: box select, adds to the selection".to_string();
    }
    if ws.project.active_scene().world.grid().is_none() {
        return String::new();
    }
    if hit.target == Target::Nothing {
        return "Drag: box select | Click: clear the selection".to_string();
    }
    let in_selection = spec::in_selection(ws, &hit.target);
    let Some(spec) = spec::for_press(ws, hit) else {
        return String::new();
    };
    let (label, note) = spec::describe(&spec);
    let scope = if in_selection && spec.count() > 1 {
        format!(" ({} selected)", spec.count())
    } else {
        String::new()
    };
    let note = note.map(|n| format!(", {n}")).unwrap_or_default();
    let backside = if ws.grid.edit.cycle.0 > 0 {
        " | ` cycles"
    } else {
        ""
    };
    format!("Drag: {label}{scope}{note} | Click: select | Shift+drag: box{backside} | Alt: fine")
}

/// A press that starts a drag (egui calls it once the pointer has moved).
pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.edit.drag = None;
    ws.grid.edit.amount.clear();
    let press = ptr.at_press();
    // Move handle, then everything else.
    if repeat_move::press_move(ws, &press) {
        return;
    }
    if ptr.shift() {
        // Shift never grabs: it boxes, adding.
        box_select(ws, ptr, true);
        return;
    }
    let hit = candidate(ws, &press);
    if hit.target == Target::Nothing {
        box_select(ws, ptr, ptr.command());
        return;
    }
    let Some(spec) = spec::for_press(ws, &hit) else {
        box_select(ws, ptr, false);
        return;
    };
    // An unselected element becomes the selection; a selected one acts for
    // the whole selection.
    if !spec::in_selection(ws, &hit.target) {
        ws.grid.selection.set(hit.target.elements());
        ws.after_grid_selection_changed();
    }
    begin(ws, ptr, &hit, spec);
}

fn box_select(ws: &mut EditorWorkspace, ptr: &GridPointer, additive: bool) {
    ws.grid.gesture = Gesture::Marquee {
        start: ptr.press.unwrap_or(ptr.pos),
        current: ptr.pos,
        additive,
    };
    ws.grid.edit.hint = "Box select: release to pick what the box holds".to_string();
}

/// Build the plan for `spec` and start the drag.
fn begin(ws: &mut EditorWorkspace, ptr: &GridPointer, hit: &Hit, spec: Spec) {
    let before = Box::new(ws.project.clone());
    let weld = ws.grid.link;
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let built = match spec::build(world, &spec, weld) {
        Ok(built) => built,
        Err(reason) => {
            ws.project = *before;
            ws.status = reason.clone();
            ws.grid.edit.hint = reason;
            return;
        }
    };
    let press = ptr.at_press();
    let grab = if hit.point == [0.0; 3] {
        grab_point(ws, &spec)
    } else {
        hit.point
    };
    let axis = built.axis;
    let label = built.label.clone();
    let ppu = ws.grid_pixels_per_unit(&press, grab);
    let travel0 = match press.view {
        GridView::Persp(rect) => vertical_travel(ws, rect, press.pos, grab).unwrap_or(0.0),
        GridView::Top(_) => 0.0,
    };
    ws.grid_changed();
    ws.grid.edit.drag = Some(Drag {
        before,
        plan: built.plan,
        kind: spec.kind,
        axis,
        press: press.pos,
        grab,
        travel0,
        pixels_per_unit: ppu,
        label: built.label,
        note: built.note,
        amount: 0,
        recorded: false,
        last: plan::Applied::default(),
    });
    ws.grid.gesture = Gesture::Edit;
    ws.grid.edit.hint = format!(
        "{} | type a number, Enter applies, Esc cancels",
        cap(&label)
    );
}

fn cap(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A point on the first element of the spec, editor units: where a drag in
/// the plan is anchored.
fn grab_point(ws: &EditorWorkspace, spec: &Spec) -> [f32; 3] {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return [0.0; 3];
    };
    let Some(face) = spec.faces.first() else {
        return [0.0; 3];
    };
    face.centre(world).map_or([0.0; 3], |g| {
        [
            world.scale.to_editor(g[0]) as f32,
            world.scale.to_editor(g[1]) as f32,
            world.scale.to_editor(g[2]) as f32,
        ]
    })
}

/// A drag step.
pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if repeat_move::drag_move(ws, ptr) {
        return;
    }
    match &mut ws.grid.gesture {
        Gesture::Marquee { current, .. } => {
            *current = ptr.pos;
            return;
        }
        Gesture::Edit => {}
        _ => return,
    }
    let Some(drag) = ws.grid.edit.drag.as_ref() else {
        return;
    };
    // A typed number holds the amount: the pointer no longer moves it.
    if !ws.grid.edit.amount.is_empty() {
        return;
    }
    let Some(amount) = pointer_amount(ws, drag, ptr) else {
        return;
    };
    set_amount(ws, amount);
}

/// Apply `amount` to the gesture in flight (grid units; sectors for a push).
pub(super) fn set_amount(ws: &mut EditorWorkspace, amount: i32) {
    let Some(mut drag) = ws.grid.edit.drag.take() else {
        return;
    };
    if amount != drag.amount {
        let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
            ws.grid.edit.drag = Some(drag);
            return;
        };
        let out = drag.plan.apply(world, amount);
        if out.changed && !drag.recorded {
            ws.history.record((*drag.before).clone());
            drag.recorded = true;
        }
        drag.amount = amount;
        drag.last = out;
        ws.grid_changed();
        let ruler = vertical_ruler(ws, &drag);
        ws.status = readout(&drag);
        ws.grid.edit.hint = format!(
            "{}{ruler} | type a number, Enter applies, Esc cancels",
            readout(&drag)
        );
    }
    ws.grid.edit.drag = Some(drag);
}

/// " | <size>" for a drag: a rise in sectors and player heights with whether
/// the player steps up it, a push in sectors and player heights.
fn vertical_ruler(ws: &EditorWorkspace, drag: &Drag) -> String {
    if drag.amount == 0 || drag.last.refused.is_some() {
        return String::new();
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return String::new();
    };
    match drag.axis {
        Axis::Vertical => format!(" | {}", super::ruler::rise_text(world, drag.amount)),
        // A push is counted in sectors.
        Axis::Horizontal(_) => format!(
            " | {}",
            super::ruler::length_text(world, drag.amount * psxed_project::grid::SECTOR_UNITS)
        ),
    }
}

/// The readout of a drag: what it does and by how much.
fn readout(drag: &Drag) -> String {
    let mut text = cap(&drag.label);
    if let Some(reason) = &drag.last.refused {
        return format!("{text}: {reason}");
    }
    if let Some(done) = &drag.last.text {
        return done.clone();
    }
    text += &format!(" {} clicks", clicks_text(drag.amount));
    if drag.plan.is_tilt() && drag.last.deviation > 0 {
        text += &format!(
            ", rounded to the 64 lattice (up to {} off)",
            drag.last.deviation
        );
    }
    if drag.last.steep > 0 {
        text += &format!(
            ", {} floor{} too steep to stand on",
            drag.last.steep,
            if drag.last.steep == 1 { "" } else { "s" }
        );
    }
    if let Some(note) = &drag.note {
        text += &format!(", {note}");
    }
    for warning in &drag.last.warnings {
        text += &format!(" | Warning: {warning}");
    }
    text
}

/// The button went up.
pub(super) fn release(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if repeat_move::release_move(ws) {
        return;
    }
    match ws.grid.gesture.clone() {
        Gesture::Marquee { .. } => select_tool::release(ws, ptr),
        Gesture::Edit => finish(ws),
        _ => {}
    }
}

/// End the gesture, keeping what it made. A gesture that changed nothing
/// leaves the document exactly as it was and no undo step.
pub(super) fn finish(ws: &mut EditorWorkspace) {
    ws.grid.gesture = Gesture::Idle;
    let Some(drag) = ws.grid.edit.drag.take() else {
        return;
    };
    ws.grid.edit.amount.clear();
    if drag.amount == 0 || !drag.last.changed {
        if drag.recorded {
            let _ = ws.history.pop_last();
        }
        ws.project = *drag.before;
        ws.grid_changed();
        if let Some(reason) = drag.last.refused {
            ws.status = reason;
        }
        ws.grid.edit.hint.clear();
        return;
    }
    ws.grid.edit.last = repeat_move::Repeat::of(drag.kind, drag.amount);
    ws.status = readout(&drag);
    if let Some(faces) = &drag.last.faces {
        // A wall that followed the cells is a wall of its own now: the
        // selection follows it to its new address.
        ws.grid
            .selection
            .set(faces.iter().copied().map(GridElement::Face));
    }
    ws.grid.edit.hint.clear();
    ws.after_grid_selection_changed();
}

/// Esc: the document is put back exactly as it was at the press.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    if let Some(moving) = ws.grid.edit.moving.take() {
        ws.project = *moving.before;
        ws.grid_changed();
        ws.grid.gesture = Gesture::Idle;
        ws.status = "Cancelled".to_string();
        return true;
    }
    let Some(drag) = ws.grid.edit.drag.take() else {
        return false;
    };
    if drag.recorded {
        let _ = ws.history.pop_last();
    }
    ws.project = *drag.before;
    ws.grid_changed();
    ws.grid.gesture = Gesture::Idle;
    ws.grid.edit.amount.clear();
    ws.grid.edit.hint.clear();
    ws.status = "Cancelled: the document is as it was".to_string();
    true
}

/// A click: select.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    pick_select::click(ws, ptr);
}
