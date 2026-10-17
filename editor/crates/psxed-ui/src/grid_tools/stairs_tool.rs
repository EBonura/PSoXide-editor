//! The Stairs verb in the editor: click the waypoints of the flight's path
//! (Enter finishes), then press on it and drag up or down for the total rise.
//!
//! Each sector of the path is one tread and a turn is a landing. The riser is
//! one click, the tallest the character steps up, or a typed value; the
//! number of steps follows from the rise. A flight whose treads would need a
//! riser over the limit is refused (shown orange, nothing written): use the
//! ramp option or a longer path. Tab switches between steps and a ramp.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::verbs::stairs::{Flight, Refusal, RiserOption, StairsDrag};
use psxed_project::grid::units::{snap_to, FINE_UNITS};
use psxed_project::grid::CLICK_UNITS;

/// The Stairs options of the editor, as the verb's parameters.
pub(super) fn params(
    ws: &EditorWorkspace,
) -> psxed_project::grid::edit::verbs::stairs::StairsParams {
    let mut p = ws.grid.verb.stairs;
    p.width = ws.grid.verb.path_width;
    p.follow_ceiling = ws.grid.verb.follow_ceiling;
    p.wall_material = ws.grid.material;
    p
}

/// A drag on the finished path.
pub(super) fn drag_for(ws: &EditorWorkspace) -> Option<StairsDrag> {
    let world = ws.project.active_scene().world.grid()?;
    let room = ws.grid.verb.region.as_ref()?.room;
    if ws.grid.verb.stairs_path.is_empty() {
        return None;
    }
    StairsDrag::begin(world, room, &ws.grid.verb.stairs_path, params(ws))
}

/// Enter: the waypoints become the flight's path.
pub(super) fn keep_path(ws: &mut EditorWorkspace) {
    ws.grid.verb.stairs_path = ws.grid.verb.path.clone();
}

/// Press on the path: start the amount drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
) -> bool {
    let Some(drag) = drag_for(ws) else {
        ws.status = "Click the waypoints of the flight with Path, then press Enter".to_string();
        return false;
    };
    let ppu = super::verb_tool::drag_rate(ws, ptr, room, cell, 0);
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag: VerbDrag::Stairs(drag),
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: ppu,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
    }));
    true
}

/// The typed amount, as one undo step.
pub(super) fn apply_typed(ws: &mut EditorWorkspace, amount: i32) {
    let Some(drag) = drag_for(ws) else {
        ws.status = "Click the waypoints of the flight with Path, then press Enter".to_string();
        return;
    };
    let mut gesture = VerbGesture {
        drag: VerbDrag::Stairs(drag),
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount: snap_to(amount, FINE_UNITS),
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
}

/// Tab: steps and ramp, live when a drag is in flight.
pub(super) fn toggle_ramp(ws: &mut EditorWorkspace) {
    ws.grid.verb.stairs.ramp = !ws.grid.verb.stairs.ramp;
    if let Gesture::Verb(mut g) = std::mem::take(&mut ws.grid.gesture) {
        let fresh = params(ws);
        if let VerbDrag::Stairs(d) = &mut g.drag {
            d.set_params(fresh);
        }
        super::verb_tool::apply_amount(ws, &mut g);
        ws.grid.gesture = Gesture::Verb(g);
    }
    ws.status = if ws.grid.verb.stairs.ramp {
        "Stairs as a ramp: every tread slopes".to_string()
    } else {
        "Stairs as steps".to_string()
    };
}

/// What a flight came to, for the status line and the chip.
pub(super) fn readout(
    world: &psxed_project::grid::GridWorld,
    flight: &Flight,
    amount: i32,
) -> String {
    let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let ru = |v: i32| units.as_ref().map_or(0, |u| u.rt(v));
    if flight.refusal == Some(Refusal::NoFloor) {
        return "Stairs: no sector of the path is inside the room".to_string();
    }
    if let Some(Refusal::TooSteep { riser, limit }) = flight.refusal {
        return format!(
            "Stairs refused: {} ru risers are over the {} ru the character steps up. Use the ramp option or a longer path",
            ru(riser),
            ru(limit)
        );
    }
    if flight.steps == 0 {
        return format!(
            "Stairs {:+.2} clicks: drag up or down for the rise",
            amount as f32 / CLICK_UNITS as f32
        );
    }
    let clicks = flight.riser as f32 / CLICK_UNITS as f32;
    let mut text = format!(
        "{} steps, riser {clicks:+.2} clicks ({:+} ru), tread 1 sector, {} sectors, total {:+} ru",
        flight.steps,
        ru(flight.riser),
        flight.cells,
        ru(flight.total),
    );
    if flight.applied.created > 0 {
        text += &format!(", {} floors made", flight.applied.created);
    }
    if flight.applied.skipped > 0 {
        text += &format!(", {} off the room skipped", flight.applied.skipped);
    }
    text
}

/// The riser option as a label.
pub(super) fn riser_label(option: RiserOption) -> &'static str {
    match option {
        RiserOption::Click => "1 click",
        RiserOption::Max => "Max",
        RiserOption::Typed(_) => "Typed",
    }
}

/// The short text over a flight being dragged.
pub(super) fn chip(
    world: &psxed_project::grid::GridWorld,
    drag: &StairsDrag,
    amount: i32,
) -> String {
    let flight = drag.plan(amount);
    let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let ru = |v: i32| units.as_ref().map_or(0, |u| u.rt(v));
    match flight.refusal {
        Some(Refusal::TooSteep { riser, limit }) => format!(
            "too steep for treads: riser {} ru over the {} ru limit",
            ru(riser),
            ru(limit)
        ),
        Some(Refusal::NoFloor) => "the path is outside the room".to_string(),
        None if flight.steps == 0 => "drag up or down".to_string(),
        None => format!(
            "{} steps, riser {:+.2} clicks ({:+} ru), tread 1 sector   limit {} ru: ok",
            flight.steps,
            flight.riser as f32 / CLICK_UNITS as f32,
            ru(flight.riser),
            ru(drag.limit()),
        ),
    }
}
