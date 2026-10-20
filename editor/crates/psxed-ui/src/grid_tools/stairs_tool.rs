//! The Stairs verb in the editor: drag a rectangle where the stairs go, then
//! press on it and drag up for the total rise, as with Ramp. The side nearest
//! the pointer is the bottom of the flight (R turns it), the treads run from
//! there to the far side across the rectangle's depth, one sector each, and
//! the rectangle's breadth is the flight's width. The Path source keeps the
//! older way for a winding flight: click the waypoints, press Enter, then drag.
//!
//! Each sector of the flight is one tread and a turn is a landing. The riser
//! is one click, the tallest the character steps up, or a typed value; the
//! number of steps follows from the rise. A flight whose treads would need a
//! riser over the limit is refused (shown orange, nothing written) and the
//! options bar offers a ramp in its place: Tab, or the button, slopes every
//! tread instead of stepping.

use super::verb_state::{RegionSource, VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::verbs::fine::{FineDrag, FineVerb};
use psxed_project::grid::edit::verbs::ramp::RampDir;
use psxed_project::grid::edit::verbs::stairs::{
    Flight, Refusal, RiserOption, StairsDrag, StairsParams,
};
use psxed_project::grid::edit::Region;
use psxed_project::grid::CLICK_UNITS;

/// The Stairs options of the editor, as the verb's parameters.
pub(super) fn params(ws: &EditorWorkspace) -> StairsParams {
    let mut p = ws.grid.verb.stairs;
    p.width = ws.grid.verb.path_width;
    p.follow_ceiling = ws.grid.verb.follow_ceiling;
    p.wall_material = ws.grid.material;
    p
}

/// The waypoints and parameters of the flight over `region`, pressed at
/// `cell`: the finished path (Path source), or a straight run across the
/// rectangle from its bottom side to the far one, as wide as the rectangle.
/// `None` for a rectangle only one sector deep (no run to climb) or an empty
/// region.
pub(super) fn flight_for(
    ws: &EditorWorkspace,
    region: &Region,
    cell: [i32; 2],
) -> Option<(Vec<[i32; 2]>, StairsParams)> {
    if ws.grid.verb.source == RegionSource::Path {
        let path = ws.grid.verb.stairs_path.clone();
        return (!path.is_empty()).then(|| (path, params(ws)));
    }
    let b = region.bounds()?;
    let from = ws
        .grid
        .verb
        .ramp_from
        .or_else(|| super::ramp_tool::nearest_side(region, cell))
        .unwrap_or(RampDir::North);
    let (width, depth) = (b.max[0] - b.min[0] + 1, b.max[1] - b.min[1] + 1);
    // The run goes along the axis the bottom side is across from.
    let (breadth, length) = match from {
        RampDir::North | RampDir::South => (width, depth),
        RampDir::East | RampDir::West => (depth, width),
    };
    if length < 2 {
        return None;
    }
    let mid = (breadth - 1) / 2;
    let (start, end) = match from {
        RampDir::North => ([b.min[0] + mid, b.min[1]], [b.min[0] + mid, b.max[1]]),
        RampDir::South => ([b.min[0] + mid, b.max[1]], [b.min[0] + mid, b.min[1]]),
        RampDir::West => ([b.min[0], b.min[1] + mid], [b.max[0], b.min[1] + mid]),
        RampDir::East => ([b.max[0], b.min[1] + mid], [b.min[0], b.min[1] + mid]),
    };
    let mut p = params(ws);
    p.width = breadth;
    Some((vec![start, end], p))
}

/// A drag on the flight: the sector flight, or the library's fine flight
/// through the lens when the treads are smaller than a sector.
pub(super) fn drag_for(ws: &EditorWorkspace, cell: [i32; 2]) -> Option<VerbDrag> {
    let world = ws.project.active_scene().world.grid()?;
    let region = ws.grid.verb.region.as_ref()?;
    let room = region.room;
    let (path, params) = flight_for(ws, region, cell)?;
    if super::verb_tool::through_lens(ws, region) {
        let footprint: Vec<[i32; 2]> = Region::from_path(room, &path, params.width)
            .cells()
            .collect();
        let verb = FineVerb::Stairs { params, path };
        let drag = FineDrag::begin(world, room, region.level(), &footprint, verb)?;
        return Some(VerbDrag::Fine(Box::new(drag)));
    }
    StairsDrag::begin(world, room, &path, params).map(VerbDrag::Stairs)
}

/// What to say when there is no flight to drag.
fn no_flight_text(ws: &EditorWorkspace) -> &'static str {
    if ws.grid.verb.source == RegionSource::Path {
        "Click the waypoints of the flight with Path, then press Enter"
    } else {
        "Stairs need at least two sectors from the bottom to the top: drag a bigger rectangle"
    }
}

/// Enter: the waypoints become the flight's path.
pub(super) fn keep_path(ws: &mut EditorWorkspace) {
    ws.grid.verb.stairs_path = ws.grid.verb.path.clone();
}

/// R: the bottom of the flight turns a quarter turn and stays.
pub(super) fn turn(ws: &mut EditorWorkspace) {
    let current = ws
        .grid
        .verb
        .ramp_from
        .or(ws.grid.verb.ramp_hover)
        .unwrap_or(RampDir::North);
    let next = current.turned();
    ws.grid.verb.ramp_from = Some(next);
    ws.grid.verb.ramp_hover = Some(next);
    ws.status = format!("The stairs start at the {} side", next.label());
}

/// Press on the region: start the amount drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
) -> super::verb_tool::Begin {
    let Some(region) = ws.grid.verb.region.clone() else {
        ws.status = no_flight_text(ws).to_string();
        return super::verb_tool::Begin::Refused;
    };
    let focus = match super::verb_tool::region_focus(ws, &region) {
        Ok(focus) => focus,
        Err(why) => {
            ws.status = why;
            return super::verb_tool::Begin::Refused;
        }
    };
    let Some(drag) = ws.with_focused_world(&focus, |ws| drag_for(ws, cell)) else {
        ws.status = no_flight_text(ws).to_string();
        return super::verb_tool::Begin::Refused;
    };
    if let Some(from) = ws
        .grid
        .verb
        .ramp_from
        .or_else(|| super::ramp_tool::nearest_side(&region, cell))
        .filter(|_| ws.grid.verb.source != RegionSource::Path)
    {
        ws.grid.verb.ramp_hover = Some(from);
    }
    ws.grid.verb.stairs_press = cell;
    ws.grid.verb.stairs_refusal = None;
    let ppu = super::verb_tool::drag_rate(ws, ptr, room, cell, 0);
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag,
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: ppu,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus,
    }));
    super::verb_tool::Begin::Started
}

/// Hover: the side a press here would start the flight from.
pub(super) fn hover(ws: &mut EditorWorkspace, cell: [i32; 2]) {
    if ws.grid.tool != GridTool::Stairs || ws.grid.verb.source == RegionSource::Path {
        return;
    }
    let Some(region) = &ws.grid.verb.region else {
        return;
    };
    if !region.contains(cell) {
        return;
    }
    ws.grid.verb.ramp_hover = ws
        .grid
        .verb
        .ramp_from
        .or_else(|| super::ramp_tool::nearest_side(region, cell));
}

/// The flight's parameters with the options as they are now, keeping the
/// width the drag was started with.
fn refreshed(ws: &EditorWorkspace, width: i32) -> StairsParams {
    let mut p = params(ws);
    p.width = width;
    p
}

/// Tab: steps and ramp, live when a drag is in flight. After a flight that
/// was too steep for treads (and with no drag in flight) it lays the flight as
/// a ramp in its place, as one undo step.
pub(super) fn toggle_ramp(ws: &mut EditorWorkspace) {
    let in_flight = matches!(ws.grid.gesture, Gesture::Verb(_));
    if !in_flight && ws.grid.verb.stairs_refusal.is_some() {
        make_ramp(ws);
        return;
    }
    ws.grid.verb.stairs.ramp = !ws.grid.verb.stairs.ramp;
    if let Gesture::Verb(mut g) = std::mem::take(&mut ws.grid.gesture) {
        match &mut g.drag {
            VerbDrag::Stairs(d) => {
                let fresh = refreshed(ws, d.params().width);
                d.set_params(fresh);
            }
            VerbDrag::Fine(d) => {
                if let FineVerb::Stairs { params, path } = d.verb().clone() {
                    let fresh = refreshed(ws, params.width);
                    d.set_verb(FineVerb::Stairs {
                        params: fresh,
                        path,
                    });
                }
            }
            _ => {}
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

/// Lay the flight that was refused as a ramp, at the rise it was asked for.
pub(super) fn make_ramp(ws: &mut EditorWorkspace) {
    let Some(refusal) = ws.grid.verb.stairs_refusal.take() else {
        return;
    };
    let Some(region) = ws.grid.verb.region.clone() else {
        return;
    };
    ws.grid.verb.stairs.ramp = true;
    let cell = ws.grid.verb.stairs_press;
    let focus = match super::verb_tool::region_focus(ws, &region) {
        Ok(focus) => focus,
        Err(why) => {
            ws.status = why;
            return;
        }
    };
    let Some(drag) = ws.with_focused_world(&focus, |ws| drag_for(ws, cell)) else {
        ws.status = no_flight_text(ws).to_string();
        return;
    };
    let mut gesture = VerbGesture {
        drag,
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount: refusal.amount,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
}

/// What a flight came to, for the status line and the chip.
pub(super) fn readout(
    world: &psxed_project::grid::GridWorld,
    flight: &Flight,
    amount: i32,
    steep: usize,
) -> String {
    let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let ru = |v: i32| units.as_ref().map_or(0, |u| u.rt(v));
    if flight.refusal == Some(Refusal::NoFloor) {
        return "Stairs: no sector of the path is inside the room".to_string();
    }
    if let Some(Refusal::TooSteep { riser, limit }) = flight.refusal {
        return format!(
            "Stairs refused: {} ru risers are over the {} ru the character steps up. Tab makes it a ramp, or use a longer run",
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
    let walk = if steep == 0 {
        "walkable".to_string()
    } else {
        format!("{steep} sectors too steep to stand on")
    };
    let mut text = format!(
        "{} steps, riser {clicks:+.2} clicks ({:+} ru), tread 1 sector, {} sectors, total {:+} ru, {walk}",
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
            "{} steps, riser {:+.2} clicks ({:+} ru), limit {} ru: walkable",
            flight.steps,
            flight.riser as f32 / CLICK_UNITS as f32,
            ru(flight.riser),
            ru(drag.limit()),
        ),
    }
}
