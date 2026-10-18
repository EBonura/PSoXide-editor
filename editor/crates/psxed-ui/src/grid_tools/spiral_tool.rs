//! The Spiral tool: stairs that wind round a ring of sectors
//! (`docs/grid/multi-storey.md`, 5.7).
//!
//! Click the corners of the ring with Path, press Enter to close it (the last
//! corner joins back to the first), then press on the ring and drag up or
//! down for the climb of one storey. A straight run of the ring is treads, a
//! turn is a landing, and the stairs wind round as many times as the storeys
//! of the option row need. Each tread is a slab laid with the storey
//! primitive, so a sector the stairs pass twice holds two treads a storey
//! apart. Like every verb the gesture is live and exact: Esc puts the
//! document back byte for byte.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::focus::Focus;
use psxed_project::grid::edit::verbs::spiral::{
    base_height, ring_cells, SpiralDrag, SpiralParams, SpiralRefusal, SpiralReport,
};
use psxed_project::grid::edit::Region;
use psxed_project::grid::CLICK_UNITS;

/// What the option row keeps for the Spiral tool (the riser comes from the
/// Stairs options).
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpiralOptions {
    /// Storeys to climb.
    pub storeys: usize,
}

impl Default for SpiralOptions {
    fn default() -> Self {
        Self { storeys: 1 }
    }
}

/// The spiral's parameters from the options.
pub(super) fn params(ws: &EditorWorkspace) -> SpiralParams {
    SpiralParams {
        storeys: ws.grid.verb.spiral.storeys.max(1),
        riser: ws.grid.verb.stairs.riser,
        material: ws.grid.material,
    }
}

/// Enter: the waypoints become a closed ring, which is also the region the
/// drag presses on.
pub(super) fn keep_ring(ws: &mut EditorWorkspace) {
    let path = ws.grid.verb.path.clone();
    if path.is_empty() {
        return;
    }
    let Some(ring) = ring_cells(&path) else {
        ws.status = SpiralRefusal::NotARing.describe();
        return;
    };
    let room = ws
        .grid
        .verb
        .region
        .as_ref()
        .map_or_else(|| ws.grid_target_room(), |r| r.room);
    ws.status = format!(
        "Ring of {} sectors. Press on it and drag up for the climb of a storey",
        ring.len()
    );
    ws.grid.verb.region = Some(Region::from_cells(room, ring.iter().copied()));
    ws.grid.verb.stairs_path = ring;
}

/// The ring and its room, or why there is none.
fn ring_of(ws: &EditorWorkspace) -> Result<(usize, Vec<[i32; 2]>), String> {
    let region = ws
        .grid
        .verb
        .region
        .as_ref()
        .ok_or("Click the corners of the ring with Path, then press Enter")?;
    let ring = ws.grid.verb.stairs_path.clone();
    if ring.is_empty() {
        return Err("Click the corners of the ring with Path, then press Enter".to_string());
    }
    Ok((region.room, ring))
}

/// A drag on the finished ring. The climb starts from the floor the storey
/// cursor names under the ring's first sector (the ground floor of a column
/// of one span); the stairs themselves are slabs laid into whatever span
/// holds each tread's height, so they need no aim of their own and the
/// gesture runs without a focus.
pub(super) fn start(ws: &mut EditorWorkspace) -> Result<(VerbDrag, Focus), String> {
    let (room, ring) = ring_of(ws)?;
    let focus = ws.capture_focus(room, ring.iter().copied())?;
    let base = ws.with_focused_world(&focus, |ws| {
        ws.project
            .active_scene()
            .world
            .grid()
            .and_then(|w| base_height(w, room, &ring))
    });
    let world = ws
        .project
        .active_scene()
        .world
        .grid()
        .ok_or("The project is not a grid world")?;
    let drag = SpiralDrag::begin_at(world, room, &ring, params(ws), base)
        .ok_or_else(|| SpiralRefusal::NotARing.describe())?;
    Ok((VerbDrag::Spiral(drag), Focus::none(room)))
}

/// Press on the ring: start the amount drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
) -> super::verb_tool::Begin {
    let (drag, focus) = match start(ws) {
        Ok(started) => started,
        Err(why) => {
            ws.status = why;
            return super::verb_tool::Begin::Refused;
        }
    };
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

/// What a spiral came to, for the status line.
pub(super) fn readout(
    world: &psxed_project::grid::GridWorld,
    report: &SpiralReport,
    amount: i32,
) -> String {
    let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let ru = |v: i32| units.as_ref().map_or(0, |u| u.rt(v));
    match report.refused {
        Some(SpiralRefusal::NoClimb) => {
            return format!(
                "Spiral {:+.2} clicks: drag up for the climb of one storey",
                amount as f32 / CLICK_UNITS as f32
            )
        }
        Some(other) => return format!("Spiral refused: {}", other.describe()),
        None => {}
    }
    let mut text = format!(
        "{} treads, riser {:+.2} clicks ({:+} ru), {:.1} clicks a storey, {} laps, total {:+} ru",
        report.treads,
        report.riser as f32 / CLICK_UNITS as f32,
        ru(report.riser),
        report.per_storey as f32 / CLICK_UNITS as f32,
        report.laps,
        ru(report.total),
    );
    if report.slabs.skipped.total() > 0 {
        text += &format!("; skipped: {}", report.slabs.skipped.describe());
    }
    text
}

/// The short text over a spiral being dragged.
pub(super) fn chip(drag: &SpiralDrag, amount: i32) -> String {
    let plan = drag.plan(amount);
    match plan.refused {
        Some(SpiralRefusal::NoClimb) => "drag up".to_string(),
        Some(other) => other.describe(),
        None => format!(
            "{} treads in {} laps, {:.1} clicks a storey",
            plan.treads,
            plan.laps,
            plan.per_storey as f32 / CLICK_UNITS as f32
        ),
    }
}
