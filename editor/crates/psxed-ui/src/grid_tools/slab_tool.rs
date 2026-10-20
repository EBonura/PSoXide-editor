//! The Upper floor tool in the editor (the Slab verb of `docs/grid/multi-storey.md`,
//! 5.6): pick a region, then press on it. An upper floor is a slab inserted
//! into the open volume of a column, with open space left underneath, so a
//! tall hall becomes two storeys without having been planned as two. It is not
//! Raise: Raise moves the floor itself up and leaves solid ground beneath.
//!
//! * **Add floor above** (Insert, the default): drag up for the slab's height above the storey
//!   cursor (a click of 256 units, Alt for 64). Cells with no span open at
//!   that height are skipped and counted.
//! * **Remove upper floor** (Cut): a click on the region takes the first slab above the cursor out
//!   of every column of it, merging the spans around it (a stairwell).
//! * **Fill opening** (Fill): a click on the region closes it up to the next storey above
//!   the cursor, the inverse of Cut.
//!
//! Tab cycles the mode. Every gesture is one undo step, and Esc puts the
//! document back exactly.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;
use psxed_project::grid::edit::focus::Focus;

use psxed_project::grid::edit::spans::{cut_hole, insert_slab, CutScope, SpanOutcome};
use psxed_project::grid::edit::verbs::Snapshot;
use psxed_project::grid::edit::CellRect;
use psxed_project::grid::storeys::{storey_above, MIN_HEADROOM, SLAB_THICKNESS};
use psxed_project::grid::{GridWorld, CLICK_UNITS, FINE_UNITS};

/// What a press on the region does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlabMode {
    /// Drag up for the height of a new slab.
    #[default]
    Insert,
    /// Take the first slab above the cursor out.
    Cut,
    /// Close the opening up to the next storey.
    Fill,
}

impl SlabMode {
    pub(crate) const ALL: [Self; 3] = [Self::Insert, Self::Cut, Self::Fill];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Insert => "Add floor above",
            Self::Cut => "Remove upper floor",
            Self::Fill => "Fill opening",
        }
    }

    pub(crate) const fn hint(self) -> &'static str {
        match self {
            Self::Insert => {
                "Press on the region and drag up: a floor at that height, open space underneath"
            }
            Self::Cut => "Click the region: the first upper floor above the level is removed",
            Self::Fill => "Click the region: the opening is closed up to the next level",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Insert => Self::Cut,
            Self::Cut => Self::Fill,
            Self::Fill => Self::Insert,
        }
    }
}

/// What the option row keeps for the Slab tool.
#[derive(Clone, Debug)]
pub(crate) struct SlabOptions {
    pub mode: SlabMode,
    /// The typed height of the option row's Apply, clicks above the level.
    pub typed_clicks: f32,
}

impl Default for SlabOptions {
    fn default() -> Self {
        Self {
            mode: SlabMode::Insert,
            typed_clicks: 4.0,
        }
    }
}

/// A slab gesture in flight.
#[derive(Clone, Debug)]
pub(crate) struct SlabLive {
    pub mode: SlabMode,
    room: usize,
    /// The sectors of the region.
    cells: Vec<[i32; 2]>,
    /// The sectors as they were: every change restores them first.
    snapshot: Snapshot,
    /// Room-relative height an Insert measures its amount from: the storey
    /// cursor, else the floor under the region.
    pub base: i32,
    /// Room-relative height a Fill closes up to, when a storey lies above.
    fill_to: Option<i32>,
    material: Option<ResourceId>,
}

/// The gesture over the current region, or why there is none.
fn start(ws: &EditorWorkspace) -> Result<SlabLive, String> {
    let region = ws.grid.verb.region.clone().ok_or("Pick a region first")?;
    if region.level() != 2 {
        return Err("The Upper floor tool works on whole sectors: pick Sector for it".to_string());
    }
    let world = ws
        .project
        .active_scene()
        .world
        .grid()
        .ok_or("The project is not a grid world")?;
    let room = world
        .rooms
        .get(region.room)
        .ok_or("The region's room is gone")?;
    let cells: Vec<[i32; 2]> = region.cells().collect();
    let first = cells.first().copied().ok_or("The region is empty")?;
    let base = ws.grid.level.y.map_or_else(
        || super::verb_tool::floor_height_at(world, region.room, first, 2),
        |y| y - room.y_offset,
    );
    let fill_to =
        storey_above(&ws.level_storeys(), base + room.y_offset).map(|above| above - room.y_offset);
    let snapshot =
        Snapshot::capture(world, region.room, &cells).ok_or("The region's room is gone")?;
    Ok(SlabLive {
        mode: ws.grid.verb.slab.mode,
        room: region.room,
        cells,
        snapshot,
        base,
        fill_to,
        material: ws.grid.material,
    })
}

/// Press on the region: Insert starts the height drag. Cut and Fill are
/// clicks (see [`click`]), so a press in those modes starts a new region.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
) -> super::verb_tool::Begin {
    if ws.grid.verb.slab.mode != SlabMode::Insert {
        return super::verb_tool::Begin::Refused;
    }
    let live = match start(ws) {
        Ok(live) => live,
        Err(why) => {
            ws.status = why;
            return super::verb_tool::Begin::Refused;
        }
    };
    let pixels_per_unit = super::verb_tool::drag_rate(ws, ptr, room, cell, live.base);
    ws.status = "Upper floor: drag up for its height".to_string();
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag: VerbDrag::Slab(Box::new(live)),
        press_y: ptr.at_press().pos.y,
        pixels_per_unit,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus: Focus::none(0),
    }));
    super::verb_tool::Begin::Started
}

/// A click on the region in Cut or Fill mode: act on it, as one undo step.
/// Returns whether the click was taken (it is not in Insert mode, nor
/// outside the region, where it picks sectors as for every verb).
pub(super) fn click(ws: &mut EditorWorkspace, room: usize, cell: [i32; 2]) -> bool {
    let inside = ws
        .grid
        .verb
        .region
        .as_ref()
        .is_some_and(|r| r.room == room && r.contains(cell));
    if ws.grid.verb.slab.mode == SlabMode::Insert || !inside {
        return false;
    }
    match start(ws) {
        Ok(live) => {
            let mut gesture = VerbGesture {
                drag: VerbDrag::Slab(Box::new(live)),
                press_y: 0.0,
                pixels_per_unit: 1.0,
                amount: 0,
                before: Some(Box::new(ws.project.clone())),
                max_rise: None,
                focus: Focus::none(0),
            };
            super::verb_tool::apply_amount(ws, &mut gesture);
        }
        Err(why) => ws.status = why,
    }
    true
}

/// Run `op` on every sector of the gesture's region, one rectangle each.
fn over_cells(
    world: &mut GridWorld,
    live: &SlabLive,
    op: impl Fn(&mut GridWorld, CellRect) -> SpanOutcome,
) -> SpanOutcome {
    let mut total = SpanOutcome::default();
    for &cell in &live.cells {
        total.merge(op(world, CellRect::from_corners(cell, cell)));
    }
    total
}

/// Apply the gesture at `amount` grid units above the base, from the start.
/// Returns the status line.
pub(super) fn apply(world: &mut GridWorld, live: &SlabLive, amount: i32) -> String {
    live.snapshot.restore(world);
    let (verb, height, outcome) = match live.mode {
        SlabMode::Insert if amount == 0 => {
            return "Upper floor: drag up for its height".to_string()
        }
        SlabMode::Insert => {
            let y = live.base + amount;
            let outcome = over_cells(world, live, |w, rect| {
                insert_slab(w, live.room, rect, y, SLAB_THICKNESS, live.material)
            });
            ("Upper floor added", y, outcome)
        }
        SlabMode::Cut => {
            let outcome = over_cells(world, live, |w, rect| {
                cut_hole(w, live.room, rect, live.base, CutScope::Next)
            });
            ("Upper floor removed", live.base, outcome)
        }
        SlabMode::Fill => {
            let Some(y) = live.fill_to else {
                return "Fill: no storey above the level to close up to".to_string();
            };
            let outcome = over_cells(world, live, |w, rect| {
                insert_slab(w, live.room, rect, y, SLAB_THICKNESS, live.material)
            });
            ("Opening filled", y, outcome)
        }
    };
    describe(verb, height, &outcome)
}

/// "Upper floor added at +6.00 clicks: 12 sector(s); skipped: 4 with no span
/// at that height".
fn describe(verb: &str, height: i32, outcome: &SpanOutcome) -> String {
    if let Some(reason) = outcome.refused {
        return format!("Upper floor refused: {}", reason.describe());
    }
    let mut line = format!(
        "{verb} at {:+.2} clicks: {} sector(s)",
        height as f32 / CLICK_UNITS as f32,
        outcome.changed
    );
    if outcome.skipped.total() > 0 {
        line += &format!("; skipped: {}", outcome.skipped.describe());
    }
    line
}

/// Clicks of open space an Insert at `amount` leaves under the new floor, or
/// `None` when there is too little for a gap and the floor simply rises (the
/// sliver rule: the same as Raise).
pub(super) fn open_space_clicks(amount: i32) -> Option<f32> {
    let gap = amount - SLAB_THICKNESS;
    (gap >= MIN_HEADROOM).then(|| gap as f32 / CLICK_UNITS as f32)
}

/// The room-relative bottom and top of the open space an Insert at `amount`
/// leaves under the new floor: from the floor under the region up to the
/// slab's underside. `None` while there is no gap (see [`open_space_clicks`]).
pub(super) fn gap_span(live: &SlabLive, amount: i32) -> Option<(i32, i32)> {
    open_space_clicks(amount)?;
    Some((live.base, live.base + amount - SLAB_THICKNESS))
}

/// The text over an upper floor drag.
pub(super) fn chip(live: &SlabLive, amount: i32) -> String {
    match live.mode {
        SlabMode::Insert => {
            let at = amount as f32 / CLICK_UNITS as f32;
            match open_space_clicks(amount) {
                Some(gap) => format!(
                    "Upper floor at {at:+.2} clicks over the level, {gap:.2} clicks of open space under it"
                ),
                None => format!(
                    "Upper floor at {at:+.2} clicks: no room for open space under it (3 clicks needed), so the floor just rises like Raise"
                ),
            }
        }
        mode => mode.label().to_string(),
    }
}

/// Tab: the next mode.
pub(super) fn cycle_mode(ws: &mut EditorWorkspace) {
    let next = ws.grid.verb.slab.mode.next();
    ws.grid.verb.slab.mode = next;
    ws.status = format!("{}: {}", next.label(), next.hint());
}

/// The option row's typed height, applied as one undo step.
pub(super) fn apply_typed(ws: &mut EditorWorkspace) {
    let live = match start(ws) {
        Ok(live) => live,
        Err(why) => {
            ws.status = why;
            return;
        }
    };
    let amount = psxed_project::grid::units::snap_to(
        (ws.grid.verb.slab.typed_clicks * CLICK_UNITS as f32).round() as i32,
        FINE_UNITS,
    );
    let mut gesture = VerbGesture {
        drag: VerbDrag::Slab(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus: Focus::none(0),
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
}

/// A slab drag in flight at `amount` grid units (poses, tests).
pub(crate) fn pose_drag(ws: &mut EditorWorkspace, amount: i32) -> Result<(), String> {
    let live = start(ws)?;
    let mut gesture = VerbGesture {
        drag: VerbDrag::Slab(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus: Focus::none(0),
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
    ws.grid.gesture = Gesture::Verb(Box::new(gesture));
    Ok(())
}
