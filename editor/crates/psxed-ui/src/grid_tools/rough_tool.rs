//! The Rough verb in the editor: pick a region, press M, then press on it
//! and drag up for the rock level (0 to 3, about 24 pixels a level). Rough
//! displaces the corners of a sector's faces in the render only, so the walls
//! read as rock while collision stays on the grid.
//!
//! The drag starts from the highest level the region already holds, so a
//! press that does not move changes nothing. Enter or the release commits as
//! one undo step; Esc puts the document back exactly.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::rough::{set_level, set_level_cells};
use psxed_project::grid::edit::verbs::Snapshot;
use psxed_project::grid::leaf::edge_clicks;
use psxed_project::grid::rough::{Rough, MAX_LEVEL};
use psxed_project::grid::{GridRoom, GridWorld};

/// Pixels of upward travel for one level.
const PIXELS_PER_LEVEL: f32 = 24.0;

/// What the option row keeps for the Rough tool.
#[derive(Clone, Debug, Default)]
pub(crate) struct RoughOptions {
    /// The typed level of the option row's Apply.
    pub typed_level: u8,
}

/// A rough drag in flight.
#[derive(Clone, Debug)]
pub(crate) struct RoughLive {
    room: usize,
    /// The cells, in cells of `level`.
    cells: Vec<[i32; 2]>,
    /// The cell level: 2 sectors, or finer (the leaves of a fine region are
    /// set one by one, from the room as it began).
    level: u8,
    /// The sector snapshot every change restores (sector regions only).
    snapshot: Option<Snapshot>,
    /// The room as it began, restored before every change of a fine region.
    start_room: Option<Box<GridRoom>>,
    /// The room's settings when the gesture began (a level above zero gives
    /// a room without amplitudes the defaults).
    start_rough: Rough,
    /// The highest level the region held; the drag moves from it.
    pub base: u8,
    /// The document before the gesture, for the exact cancel.
    pub(super) start: Box<ProjectDocument>,
    recorded: bool,
}

impl RoughLive {
    /// The document to record for undo, the first time this is asked.
    pub(super) fn take_record(&mut self) -> Option<ProjectDocument> {
        (!std::mem::replace(&mut self.recorded, true)).then(|| (*self.start).clone())
    }

    /// Whether the gesture has changed the document.
    pub(super) fn recorded(&self) -> bool {
        self.recorded
    }
}

fn start(ws: &EditorWorkspace) -> Option<RoughLive> {
    let region = ws.grid.verb.region.clone()?;
    let world = ws.project.active_scene().world.grid()?;
    let room = world.rooms.get(region.room)?;
    let level = region.level();
    let fine = super::verb_tool::through_lens(ws, &region);
    // The leaves of a fine region, or the sectors of a sector one, that hold
    // anything, and the highest rough level among them.
    let (cells, base, snapshot) = if fine {
        let e = edge_clicks(level);
        let leaves: Vec<_> = region
            .cells()
            .filter_map(|c| {
                room.leaf_at([c[0] * e, c[1] * e])
                    .map(|(_, s)| (c, s.rough))
            })
            .collect();
        let base = leaves.iter().map(|(_, r)| *r).max().unwrap_or(0);
        let cells: Vec<[i32; 2]> = leaves.into_iter().map(|(c, _)| c).collect();
        (cells, base, None)
    } else {
        let cells: Vec<[i32; 2]> = region
            .cells()
            .filter(|&c| {
                room.local(c)
                    .and_then(|[x, z]| room.sector(x, z))
                    .is_some_and(|s| !s.is_empty())
            })
            .collect();
        let base = cells
            .iter()
            .filter_map(|&c| room.local(c).and_then(|[x, z]| room.sector(x, z)))
            .map(|s| s.rough)
            .max()
            .unwrap_or(0);
        let snapshot = Snapshot::capture(world, region.room, &cells)?;
        (cells, base, Some(snapshot))
    };
    if cells.is_empty() {
        return None;
    }
    Some(RoughLive {
        room: region.room,
        cells,
        level,
        snapshot,
        start_room: fine.then(|| Box::new(room.clone())),
        start_rough: room.rough,
        base,
        start: Box::new(ws.project.clone()),
        recorded: false,
    })
}

/// Press on the region: start the level drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    _room: usize,
    _cell: [i32; 2],
) -> bool {
    let Some(live) = start(ws) else {
        ws.status = "Nothing to roughen: the region holds no sector".to_string();
        return false;
    };
    let base = live.base;
    ws.status = format!(
        "Rough: {} sectors, now level {base}. Drag up for rougher, down for smoother",
        live.cells.len(),
    );
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag: VerbDrag::Rough(Box::new(live)),
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: PIXELS_PER_LEVEL,
        amount: i32::from(base),
        before: None,
        max_rise: None,
    }));
    true
}

/// The level the pointer is at: the base plus whole levels of travel.
pub(super) fn level_at(g: &VerbGesture, live: &RoughLive, ptr: &GridPointer) -> i32 {
    let travel = ((g.press_y - ptr.pos.y) / g.pixels_per_unit.max(1e-4)).round() as i32;
    (i32::from(live.base) + travel).clamp(0, i32::from(MAX_LEVEL))
}

/// Set the region to `level` (the gesture's `amount`), from the start.
/// Returns the status line.
pub(super) fn apply(world: &mut GridWorld, live: &RoughLive, level: i32) -> String {
    match (&live.snapshot, &live.start_room) {
        (Some(snapshot), _) => {
            snapshot.restore(world);
            if let Some(room) = world.rooms.get_mut(live.room) {
                room.rough = live.start_rough;
            }
        }
        (None, Some(start)) => {
            if let Some(room) = world.rooms.get_mut(live.room) {
                *room = (**start).clone();
            }
        }
        (None, None) => {}
    }
    let level = level.clamp(0, i32::from(MAX_LEVEL)) as u8;
    if level != live.base {
        if live.snapshot.is_some() {
            set_level(world, live.room, &live.cells, level);
        } else {
            for &cell in &live.cells {
                set_level_cells(world, live.room, live.level, cell, cell, level);
            }
        }
    }
    describe(world, live.room, live.cells.len(), level)
}

/// "Rough level 2 on 12 sectors: up to 2 gu (1 ru) either way".
fn describe(world: &GridWorld, room: usize, sectors: usize, level: u8) -> String {
    let amp = world
        .rooms
        .get(room)
        .map_or(0, |r| r.rough.level_amp(level));
    let ru = psxed_project::grid::cook::RuntimeUnits::new(world.scale)
        .map(|u| format!(" ({} ru)", u.rt(amp)))
        .unwrap_or_default();
    if level == 0 {
        format!("Rough level 0 on {sectors} sectors: smooth")
    } else {
        format!("Rough level {level} on {sectors} sectors: up to {amp} gu{ru} either way")
    }
}

/// The text over a rough drag.
pub(super) fn chip(level: i32) -> String {
    format!("Rough level {level}")
}

/// A rough drag in flight at `level` (poses, tests).
pub(crate) fn pose_drag(ws: &mut EditorWorkspace, level: i32) -> Result<(), String> {
    let live = start(ws).ok_or("no region with a sector: use region=x0,z0:x1,z1 first")?;
    let mut gesture = VerbGesture {
        drag: VerbDrag::Rough(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: PIXELS_PER_LEVEL,
        amount: level.clamp(0, i32::from(MAX_LEVEL)),
        before: None,
        max_rise: None,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
    ws.grid.gesture = Gesture::Verb(Box::new(gesture));
    Ok(())
}

/// The option row's typed level, applied as one undo step.
pub(super) fn apply_typed(ws: &mut EditorWorkspace) {
    if ws.grid.verb.region.is_none() {
        ws.status = "Pick a region first".to_string();
        return;
    }
    let Some(live) = start(ws) else {
        ws.status = "Nothing to roughen: the region holds no sector".to_string();
        return;
    };
    let mut gesture = VerbGesture {
        drag: VerbDrag::Rough(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: PIXELS_PER_LEVEL,
        amount: i32::from(ws.grid.verb.rough.typed_level.min(MAX_LEVEL)),
        before: None,
        max_rise: None,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
}
