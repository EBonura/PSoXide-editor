//! The Build tools: Room, Corridor, Opening, Arcade, Colonnade, Bridge,
//! Building, Cathedral, Terrain patch, Ruin, Door, Lever and Checkpoint.
//!
//! One gesture serves them all. A press (or a click) starts it from a
//! snapshot of the document; every pointer move restores the snapshot and
//! applies the SDK operation again with the shape drawn so far, so the 3D
//! view and the plan show the real result live (the ghost is the document
//! itself, not a drawing of it). A refusal leaves the document as it was and
//! says why in the hint bar. Release (or Enter, for a path) keeps the result
//! as one undo step; Esc puts the document back exactly.
//!
//! * [`options`]: what the options bar edits.
//! * [`run`]: the SDK operation of each kind (pure over a project).
//! * [`styles`]: the Style picker's list and Capture bay.
//! * [`panel`]: the options bar of each tool.

use super::*;

use psxed_project::grid::edit::CellRect;
use psxed_project::grid::leaf::CLICKS_PER_SECTOR;
use psxed_project::grid::sector::WallEdge;
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS, SECTOR_UNITS};

use super::edit::amount::Amount;
use super::view::editor_y;

pub(crate) mod options;
pub(super) mod panel;
pub(super) mod run;
pub(super) mod styles;

pub(crate) use options::BuildKind;
use options::{Options, RoomShape, StyleEntry};
use run::{Geo, Outcome, WallRun};

/// A Build gesture in flight.
#[derive(Clone, Debug)]
pub(crate) struct Drag {
    kind: BuildKind,
    /// The document as it was at the start: Esc puts it back exactly.
    before: Box<ProjectDocument>,
    /// What has been drawn.
    geo: Geo,
    /// Where the pointer is now, appended to a path or a bridge while it is
    /// drawn (not yet a waypoint).
    tentative: Option<([i32; 2], Option<i32>)>,
    /// The undo step is recorded.
    recorded: bool,
    last: Outcome,
    /// Shift held: pointer travel is the vertical amount. The pointer y and
    /// the amount when it began.
    lift: Option<(f32, i32)>,
    /// Room offset of the room the gesture started in, grid units.
    y_offset: i32,
    /// The click a wall run began at.
    anchor_click: i32,
}

/// The state of the Build tools. Not saved with the project.
#[derive(Default)]
pub(crate) struct BuildState {
    /// What the tool says now, for the hint bar.
    pub hint: String,
    pub opts: Options,
    pub(super) drag: Option<Drag>,
    /// Digits typed during a gesture.
    pub(super) amount: Amount,
    /// The style list, read when the tool is chosen and after a capture.
    pub styles: Vec<StyleEntry>,
    pub styles_loaded: bool,
    /// The name Capture bay stores the style under.
    pub capture_name: String,
    /// A lever just placed that waits for the click on its door, with the
    /// document before it was placed.
    pub(super) lever: Option<(NodeId, Box<ProjectDocument>)>,
}

impl BuildState {
    /// Forget any gesture and typed number (a tool change).
    pub(crate) fn abandon(&mut self) {
        self.drag = None;
        self.lever = None;
        self.amount = Amount::default();
        self.hint.clear();
    }
}

impl EditorWorkspace {
    /// What the Build tool in use says, for the hint bar; empty when none.
    pub(crate) fn build_hint(&self) -> &str {
        if BuildKind::of(self.grid.tool).is_some() {
            &self.grid.build.hint
        } else {
            ""
        }
    }

    /// Re-read the style list (the tool was chosen, or a style was captured).
    pub(crate) fn refresh_styles(&mut self) {
        self.grid.build.styles = styles::list(&self.project_dir);
        self.grid.build.styles_loaded = true;
    }
}

fn kind_of(ws: &EditorWorkspace) -> Option<BuildKind> {
    BuildKind::of(ws.grid.tool)
}

// ---- where the pointer is on the ground ----

/// The room, its offset and the floor height the gesture is aimed at.
fn ground(ws: &EditorWorkspace, ptr: &GridPointer) -> (usize, i32, i32) {
    let (room, y_offset) = super::rect_tool::room_offset(ws);
    let (base, _) = super::rect_tool::aimed_height(ws, ptr, SurfaceKind::Floor, room);
    let room_hit = match ptr.view {
        GridView::Persp(rect) => ws.pick_face_3d(rect, ptr.pos).map(|h| h.face.room),
        GridView::Top(_) => None,
    };
    match room_hit {
        Some(r) if r != room => {
            let off = ws
                .project
                .active_scene()
                .world
                .grid()
                .and_then(|w| w.rooms.get(r))
                .map_or(0, |r| r.y_offset);
            (r, off, base)
        }
        _ => (room, y_offset, base),
    }
}

fn cell_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    y_offset: i32,
    base: i32,
) -> Option<[i32; 2]> {
    let world = ws.project.active_scene().world.grid()?;
    let y = editor_y(world, y_offset, base);
    let p = ws.grid_plane_point(ptr, y)?;
    Some(view::cell_at_editor(world.scale, p[0], p[2]))
}

/// The floor height (room relative) of the sector `cell` in `room`, if it
/// has one.
fn floor_at(ws: &EditorWorkspace, room: usize, cell: [i32; 2]) -> Option<i32> {
    let r = ws.project.active_scene().world.grid()?.rooms.get(room)?;
    let [x, z] = r.local(cell)?;
    let f = r.sector(x, z)?.ground_floor()?;
    Some(f.heights.iter().sum::<i32>() / 4)
}

/// A lattice corner under the pointer, in sector coordinates (a lasso corner).
fn corner_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    y_offset: i32,
    base: i32,
) -> Option<[f64; 2]> {
    let world = ws.project.active_scene().world.grid()?;
    let y = editor_y(world, y_offset, base);
    let p = ws.grid_plane_point(ptr, y)?;
    let per = f64::from(world.scale.to_editor(SECTOR_UNITS)).max(1.0);
    Some([
        (f64::from(p[0]) / per).round(),
        (f64::from(p[2]) / per).round(),
    ])
}

/// The click along a wall under the pointer (world clicks), for a wall of
/// `side` seen from `cell`.
fn along_click(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    side: WallEdge,
    cell: [i32; 2],
    hit: [f32; 3],
) -> Option<i32> {
    let world = ws.project.active_scene().world.grid()?;
    let scale = world.scale;
    let point = match ptr.view {
        GridView::Top(t) => {
            let [x, z] = t.screen_to_world(ptr.pos);
            [x, 0.0, z]
        }
        GridView::Persp(rect) => {
            let (o, d) = ws.grid_ray(rect, ptr.pos)?;
            let sector = scale.to_editor(SECTOR_UNITS) as f32;
            let (axis, c) = match side {
                WallEdge::North => (2, cell[1] as f32 * sector),
                WallEdge::South => (2, (cell[1] + 1) as f32 * sector),
                WallEdge::West => (0, cell[0] as f32 * sector),
                _ => (0, (cell[0] + 1) as f32 * sector),
            };
            if d[axis].abs() > 1e-4 {
                let t = (c - o[axis]) / d[axis];
                [o[0] + d[0] * t, 0.0, o[2] + d[2] * t]
            } else {
                hit
            }
        }
    };
    let click = view::click_at_editor(scale, point[0], point[2]);
    Some(match side {
        WallEdge::North | WallEdge::South => click[0],
        _ => click[1],
    })
}

/// The wall under the pointer as a run of one click, or `None`.
fn wall_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<WallRun> {
    let hit = super::edit::target::at(ws, ptr);
    let face = hit.target.wall()?;
    let (side, span) = match face.target {
        psxed_project::grid::edit::FaceTarget::Derived { edge, span, .. } => (edge, span),
        psxed_project::grid::edit::FaceTarget::Wall { edge, .. } => (edge, 0),
        _ => return None,
    };
    if !matches!(
        side,
        WallEdge::North | WallEdge::East | WallEdge::South | WallEdge::West
    ) {
        return None;
    }
    let cell = [
        face.leaf.x.div_euclid(CLICKS_PER_SECTOR),
        face.leaf.z.div_euclid(CLICKS_PER_SECTOR),
    ];
    let at = along_click(ws, ptr, side, cell, hit.point)?;
    Some(WallRun {
        room: face.room,
        cell,
        side,
        span,
        from: at,
        to: at + 1,
    })
}

// ---- gesture ----

fn begin(ws: &mut EditorWorkspace, kind: BuildKind, geo: Geo, y_offset: i32) {
    let geo_anchor = geo.wall.map_or(0, |w| w.from);
    ws.grid.build.amount = Amount::default();
    ws.grid.build.drag = Some(Drag {
        kind,
        before: Box::new(ws.project.clone()),
        geo,
        tentative: None,
        recorded: false,
        last: Outcome::default(),
        lift: None,
        y_offset,
        anchor_click: geo_anchor,
    });
    ws.grid.gesture = Gesture::Edit;
}

/// Restore the document and apply the kind over what is drawn, then say what
/// came of it.
fn refresh(ws: &mut EditorWorkspace) {
    let Some(mut drag) = ws.grid.build.drag.take() else {
        return;
    };
    let was_changed = drag.last.changed;
    ws.project = (*drag.before).clone();
    let mut geo = drag.geo.clone();
    if let Some((cell, floor)) = drag.tentative {
        match drag.kind {
            BuildKind::Corridor => geo.path.push((cell, floor)),
            BuildKind::Bridge => geo.ends.push((cell, floor.unwrap_or(geo.base))),
            _ => {}
        }
    }
    let out = run::apply(&mut ws.project, drag.kind, &ws.grid.build.opts, &geo);
    if out.changed && !drag.recorded {
        ws.history.record((*drag.before).clone());
        drag.recorded = true;
    } else if !out.changed && drag.recorded {
        let _ = ws.history.pop_last();
        drag.recorded = false;
    }
    if out.changed || was_changed {
        ws.grid_changed();
    }
    ws.grid.build.hint = hint_of(&drag, &out, ws.grid.build.amount.text());
    drag.last = out;
    ws.grid.build.drag = Some(drag);
}

fn hint_of(drag: &Drag, out: &Outcome, typed: &str) -> String {
    let mut text = out.text.clone();
    if let Some((name, _)) = drag.kind.amount() {
        if !typed.is_empty() {
            text += &format!(" | {name}: {typed}");
        } else if !out.refused {
            text += &format!(" | type a number for the {name}");
        }
    }
    text + " | Esc cancels"
}

/// Keep the result: one undo step, or nothing when it came to nothing.
fn finish(ws: &mut EditorWorkspace) {
    ws.grid.gesture = Gesture::Idle;
    let Some(drag) = ws.grid.build.drag.take() else {
        return;
    };
    ws.grid.build.amount = Amount::default();
    if !drag.last.changed {
        if drag.recorded {
            let _ = ws.history.pop_last();
        }
        ws.project = *drag.before;
        ws.grid_changed();
        if drag.last.refused {
            ws.status = drag.last.text.clone();
            ws.grid.build.hint = drag.last.text;
        } else {
            ws.grid.build.hint.clear();
        }
        return;
    }
    // A lever waits for its door.
    if drag.kind == BuildKind::Lever {
        let lever = ws
            .project
            .active_scene()
            .nodes()
            .iter()
            .filter(|n| n.name.starts_with("Lever "))
            .map(|n| n.id)
            .next_back();
        if let Some(id) = lever {
            ws.grid.build.lever = Some((id, drag.before));
        }
    }
    ws.status = drag.last.text.clone();
    ws.grid.build.hint = drag.last.text;
    ws.after_grid_selection_changed();
}

/// Esc: the document is put back exactly as it was.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    if let Some((_, before)) = ws.grid.build.lever.take() {
        ws.project = *before;
        ws.grid_changed();
        ws.history.pop_last();
        ws.status = "Cancelled: the lever is removed".to_string();
        ws.grid.build.hint.clear();
        return true;
    }
    let Some(drag) = ws.grid.build.drag.take() else {
        return false;
    };
    if drag.recorded {
        let _ = ws.history.pop_last();
    }
    ws.project = *drag.before;
    ws.grid_changed();
    ws.grid.gesture = Gesture::Idle;
    ws.grid.build.amount = Amount::default();
    ws.grid.build.hint.clear();
    ws.status = "Cancelled: the document is as it was".to_string();
    true
}

// ---- pointer events ----

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(kind) = kind_of(ws) else { return };
    if !ws.grid.build.styles_loaded {
        ws.refresh_styles();
    }
    let multi = ws
        .grid
        .build
        .drag
        .as_ref()
        .is_some_and(|d| matches!(d.kind, BuildKind::Corridor | BuildKind::Bridge));
    if multi {
        let (room, y_offset, base) = ground(ws, ptr);
        let _ = (room, y_offset);
        if let Some(cell) = cell_under(ws, ptr, y_offset, base) {
            let floor = floor_at(ws, room, cell);
            if let Some(d) = ws.grid.build.drag.as_mut() {
                if d.tentative != Some((cell, floor)) {
                    d.tentative = Some((cell, floor));
                    refresh(ws);
                }
            }
        }
        return;
    }
    if ws.grid.build.drag.is_none() && ws.grid.build.lever.is_none() {
        ws.grid.build.hint = idle_hint(kind).to_string();
    }
    if matches!(kind, BuildKind::Opening | BuildKind::Door) && ws.grid.build.drag.is_none() {
        ws.grid.build.hint = match wall_under(ws, ptr) {
            Some(_) => "Drag along this wall".to_string(),
            None => "Point at a wall".to_string(),
        };
    }
}

fn idle_hint(kind: BuildKind) -> &'static str {
    match kind {
        BuildKind::Room => "Drag a rectangle on the floor",
        BuildKind::Corridor => "Click the first waypoint",
        BuildKind::Opening | BuildKind::Door => "Point at a wall",
        BuildKind::Arcade | BuildKind::Colonnade => "Drag a line on the floor",
        BuildKind::Bridge => "Click one end of the deck",
        BuildKind::Building => "Drag a footprint",
        BuildKind::Cathedral => "Drag the nave, west to east",
        BuildKind::Terrain => "Drag the region the ground changes in",
        BuildKind::Ruin => "Drag a region to damage",
        BuildKind::Lever => "Click a floor to place the lever",
        BuildKind::Checkpoint => "Click a floor to place a checkpoint",
    }
}

/// Which kinds a press-drag starts (the others work by clicks).
fn drags(kind: BuildKind, ws: &EditorWorkspace) -> bool {
    match kind {
        BuildKind::Room => ws.grid.build.opts.room.shape != RoomShape::Lasso,
        BuildKind::Corridor | BuildKind::Bridge | BuildKind::Lever | BuildKind::Checkpoint => false,
        BuildKind::Ruin => !ws.grid.build.opts.ruin.whole_room && !ws.grid.build.opts.ruin.painting,
        _ => true,
    }
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(kind) = kind_of(ws) else { return };
    if !drags(kind, ws) {
        if kind == BuildKind::Ruin && ws.grid.build.opts.ruin.painting {
            paint_keep(ws, ptr);
        }
        return;
    }
    let at = ptr.at_press();
    start(ws, kind, &at);
    if ws.grid.build.drag.is_some() {
        refresh(ws);
    }
}

/// Start a drag at the press position `at`.
fn start(ws: &mut EditorWorkspace, kind: BuildKind, at: &GridPointer) {
    let (room, y_offset, base) = ground(ws, at);
    let mut geo = Geo {
        room,
        base,
        ..Geo::default()
    };
    if matches!(kind, BuildKind::Opening | BuildKind::Door) {
        let Some(run) = wall_under(ws, at) else {
            ws.grid.build.hint = "Point at a wall first".to_string();
            ws.status = "Point at a wall first: an opening is cut in a wall".to_string();
            return;
        };
        geo.room = run.room;
        geo.wall = Some(run);
        let off = ws
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(run.room))
            .map_or(0, |r| r.y_offset);
        begin(ws, kind, geo, off);
        return;
    }
    let Some(cell) = cell_under(ws, at, y_offset, base) else {
        return;
    };
    geo.a = cell;
    geo.b = cell;
    begin(ws, kind, geo, y_offset);
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(kind) = kind_of(ws) else { return };
    if kind == BuildKind::Ruin && ws.grid.build.opts.ruin.painting {
        paint_keep(ws, ptr);
        return;
    }
    let Some(d) = ws.grid.build.drag.as_ref() else {
        return;
    };
    // A typed number holds the amount.
    let typed = !ws.grid.build.amount.is_empty();
    let (room, y_offset, base) = (d.geo.room, d.y_offset, d.geo.base);
    let wall = d.geo.wall;
    let lift = d.lift;
    if ptr.shift() && kind.amount().is_some() && !typed {
        // Height mode: the shape stays, pointer travel is the amount.
        let current = amount_units(&ws.grid.build.opts, kind);
        let (y0, v0) = lift.unwrap_or((ptr.pos.y, current));
        let origin = ground_point(ws, d.geo.a, y_offset, base);
        let ppu = ws.grid_pixels_per_unit(ptr, origin);
        let step = if ptr.alt() { FINE_UNITS } else { CLICK_UNITS };
        let raw = ((y0 - ptr.pos.y) / ppu).round() as i32;
        let value = snap_to(v0 + raw, step);
        if let Some(d) = ws.grid.build.drag.as_mut() {
            d.lift = Some((y0, v0));
        }
        set_amount_units(&mut ws.grid.build.opts, kind, value);
        refresh(ws);
        return;
    }
    if lift.is_some() {
        if let Some(d) = ws.grid.build.drag.as_mut() {
            d.lift = None;
        }
    }
    if let Some(run) = wall {
        if let Some(at) = along_click(ws, ptr, run.side, run.cell, [0.0; 3]) {
            if let Some(d) = ws.grid.build.drag.as_mut() {
                if let Some(w) = d.geo.wall.as_mut() {
                    // The run grows from where it began.
                    w.from = d.anchor_click.min(at);
                    w.to = d.anchor_click.max(at) + 1;
                }
            }
            refresh(ws);
        }
        return;
    }
    let _ = room;
    if let Some(cell) = cell_under(ws, ptr, y_offset, base) {
        if let Some(d) = ws.grid.build.drag.as_mut() {
            if d.geo.b != cell {
                d.geo.b = cell;
            }
        }
        refresh(ws);
    }
}

fn ground_point(ws: &EditorWorkspace, cell: [i32; 2], y_offset: i32, base: i32) -> [f32; 3] {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return [0.0; 3];
    };
    let s = world.scale.to_editor(SECTOR_UNITS) as f32;
    [
        (cell[0] as f32 + 0.5) * s,
        editor_y(world, y_offset, base),
        (cell[1] as f32 + 0.5) * s,
    ]
}

/// The amount of `kind` in the units the Shift-drag counts (grid units for
/// the clicks kinds, whole numbers for the others).
fn amount_units(o: &Options, kind: BuildKind) -> i32 {
    match kind {
        BuildKind::Room => o.room.height,
        BuildKind::Opening => o.opening.top,
        BuildKind::Door => o.door.height,
        BuildKind::Bridge => o.bridge.width,
        BuildKind::Terrain => o.terrain.amplitude * CLICK_UNITS,
        BuildKind::Ruin => o.ruin.intensity as i32 * CLICK_UNITS,
        BuildKind::Corridor => o.corridor.width,
        BuildKind::Arcade | BuildKind::Colonnade => o.arcade.bays,
        BuildKind::Building => o.building.storeys,
        BuildKind::Cathedral => o.cathedral.bay,
        BuildKind::Lever | BuildKind::Checkpoint => 0,
    }
}

fn set_amount_units(o: &mut Options, kind: BuildKind, value: i32) {
    o.set_amount(kind, value);
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    if kind_of(ws).is_some()
        && !matches!(
            ws.grid.build.drag.as_ref().map(|d| d.kind),
            Some(BuildKind::Corridor | BuildKind::Bridge)
        )
    {
        finish(ws);
    }
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(kind) = kind_of(ws) else { return };
    let (room, y_offset, base) = ground(ws, ptr);
    match kind {
        BuildKind::Corridor | BuildKind::Bridge => {
            let Some(cell) = cell_under(ws, ptr, y_offset, base) else {
                return;
            };
            let floor = floor_at(ws, room, cell);
            if ws.grid.build.drag.is_none() {
                let geo = Geo {
                    room,
                    base,
                    ..Geo::default()
                };
                begin(ws, kind, geo, y_offset);
            }
            if let Some(d) = ws.grid.build.drag.as_mut() {
                d.tentative = None;
                if kind == BuildKind::Corridor {
                    d.geo.path.push((cell, floor));
                } else {
                    d.geo.ends.push((cell, floor.unwrap_or(base)));
                }
            }
            refresh(ws);
            if kind == BuildKind::Bridge
                && ws
                    .grid
                    .build
                    .drag
                    .as_ref()
                    .is_some_and(|d| d.geo.ends.len() >= 2)
            {
                finish(ws);
            }
        }
        BuildKind::Room if ws.grid.build.opts.room.shape == RoomShape::Lasso => {
            let Some(corner) = corner_under(ws, ptr, y_offset, base) else {
                return;
            };
            if ws.grid.build.drag.is_none() {
                let geo = Geo {
                    room,
                    base,
                    ..Geo::default()
                };
                begin(ws, kind, geo, y_offset);
            }
            if let Some(d) = ws.grid.build.drag.as_mut() {
                d.geo.poly.push(corner);
                // The rectangle of a lasso is its bounds, for the options
                // that read one.
                let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
                for p in &d.geo.poly {
                    for i in 0..2 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                }
                d.geo.a = [lo[0] as i32, lo[1] as i32];
                d.geo.b = [
                    (hi[0] as i32 - 1).max(lo[0] as i32),
                    (hi[1] as i32 - 1).max(lo[1] as i32),
                ];
            }
            refresh(ws);
        }
        BuildKind::Lever | BuildKind::Checkpoint => {
            let Some(cell) = cell_under(ws, ptr, y_offset, base) else {
                return;
            };
            if kind == BuildKind::Lever {
                if let Some((lever, _)) = ws.grid.build.lever.as_ref().map(|(i, b)| (*i, b.clone()))
                {
                    match run::nearest_door(&ws.project, cell) {
                        Some((_, name)) => {
                            let opts = ws.grid.build.opts.clone();
                            run::bind_lever(&mut ws.project, lever, &name, &opts);
                            ws.grid.build.lever = None;
                            ws.grid_changed();
                            ws.status = format!("Lever operates {name}");
                            ws.grid.build.hint = ws.status.clone();
                        }
                        None => {
                            ws.status =
                                "That is not a door: click the door the lever operates".to_string();
                            ws.grid.build.hint = ws.status.clone();
                        }
                    }
                    return;
                }
            }
            let geo = Geo {
                room,
                base,
                spot: Some(cell),
                ..Geo::default()
            };
            begin(ws, kind, geo, y_offset);
            refresh(ws);
            finish(ws);
        }
        BuildKind::Ruin if ws.grid.build.opts.ruin.painting => paint_keep(ws, ptr),
        BuildKind::Opening | BuildKind::Door => {
            // A click is a door two clicks wide at the pointer.
            start(ws, kind, ptr);
            if let Some(d) = ws.grid.build.drag.as_mut() {
                if let Some(w) = d.geo.wall.as_mut() {
                    w.from -= 1;
                    w.to = w.from + 2;
                }
            }
            if ws.grid.build.drag.is_some() {
                refresh(ws);
                finish(ws);
            } else {
                ws.status = "Point at a wall".to_string();
            }
        }
        _ => {
            if kind == BuildKind::Ruin && ws.grid.build.opts.ruin.whole_room {
                begin(
                    ws,
                    kind,
                    Geo {
                        room,
                        base,
                        whole_room: true,
                        ..Geo::default()
                    },
                    y_offset,
                );
                refresh(ws);
                finish(ws);
                return;
            }
            // A click is one sector.
            start(ws, kind, ptr);
            if ws.grid.build.drag.is_some() {
                refresh(ws);
                finish(ws);
            }
        }
    }
}

/// Paint the kept path of a ruin with a disc of one sector (Shift removes).
fn paint_keep(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let (_, y_offset, base) = ground(ws, ptr);
    let Some(cell) = cell_under(ws, ptr, y_offset, base) else {
        return;
    };
    let keep = &mut ws.grid.build.opts.ruin.keep;
    if ptr.shift() {
        keep.remove(&cell);
    } else {
        keep.insert(cell);
    }
    ws.grid.build.hint = format!("{} sectors kept clear | Shift removes", keep.len());
}

// ---- keys ----

fn press_key(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

const DIGITS: [(egui::Key, u8); 10] = [
    (egui::Key::Num0, 0),
    (egui::Key::Num1, 1),
    (egui::Key::Num2, 2),
    (egui::Key::Num3, 3),
    (egui::Key::Num4, 4),
    (egui::Key::Num5, 5),
    (egui::Key::Num6, 6),
    (egui::Key::Num7, 7),
    (egui::Key::Num8, 8),
    (egui::Key::Num9, 9),
];

/// This frame's keys of the Build tools. Returns whether the grid shortcuts
/// stop here.
pub(super) fn handle_keys(ws: &mut EditorWorkspace, ctx: &egui::Context) -> bool {
    let Some(kind) = kind_of(ws) else {
        return false;
    };
    // The seed of a terrain patch or ruin: Shift+X draws the next.
    if matches!(kind, BuildKind::Terrain | BuildKind::Ruin)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::X))
    {
        panel::reroll(ws, kind);
        return true;
    }
    let busy = ws.grid.build.drag.is_some() || ws.grid.build.lever.is_some();
    if !busy {
        return false;
    }
    if press_key(ctx, egui::Key::Escape) {
        cancel(ws);
        return true;
    }
    if ws.grid.build.drag.is_none() {
        return false;
    }
    if press_key(ctx, egui::Key::Enter) {
        finish(ws);
        return true;
    }
    let mut changed = false;
    for (key, digit) in DIGITS {
        if press_key(ctx, key) {
            ws.grid.build.amount.digit(digit);
            changed = true;
        }
    }
    if press_key(ctx, egui::Key::Period) {
        ws.grid.build.amount.point();
        changed = true;
    }
    if press_key(ctx, egui::Key::Backspace) {
        ws.grid.build.amount.backspace();
        changed = true;
    }
    press_key(ctx, egui::Key::Delete);
    if changed {
        if let Some((_, clicks)) = kind.amount() {
            let a = &ws.grid.build.amount;
            let value = if clicks { a.units() } else { a.steps() };
            if let Some(v) = value {
                ws.grid.build.opts.set_amount(kind, v);
            }
        }
        refresh(ws);
        return true;
    }
    false
}

// ---- the outline the overlay draws ----

/// What the gesture has drawn, as polylines in editor space and the chip.
pub(super) fn outline(ws: &EditorWorkspace) -> Option<(Vec<Vec<[f32; 3]>>, String)> {
    let drag = ws.grid.build.drag.as_ref()?;
    let world = ws.project.active_scene().world.grid()?;
    let s = world.scale.to_editor(SECTOR_UNITS) as f32;
    let y = editor_y(world, drag.y_offset, drag.geo.base);
    let centre = |c: [i32; 2]| [(c[0] as f32 + 0.5) * s, y, (c[1] as f32 + 0.5) * s];
    let mut lines: Vec<Vec<[f32; 3]>> = Vec::new();
    let g = &drag.geo;
    match drag.kind {
        BuildKind::Corridor => {
            let mut pts: Vec<[f32; 3]> = g.path.iter().map(|&(c, _)| centre(c)).collect();
            if let Some((c, _)) = drag.tentative {
                pts.push(centre(c));
            }
            lines.push(pts);
        }
        BuildKind::Bridge => {
            let mut pts: Vec<[f32; 3]> = g.ends.iter().map(|&(c, _)| centre(c)).collect();
            if let Some((c, _)) = drag.tentative {
                pts.push(centre(c));
            }
            lines.push(pts);
        }
        BuildKind::Arcade | BuildKind::Colonnade => lines.push(vec![centre(g.a), centre(g.b)]),
        BuildKind::Opening | BuildKind::Door => {
            let w = g.wall?;
            let per = s / CLICKS_PER_SECTOR as f32;
            let (c0, c1) = (w.from as f32 * per, w.to as f32 * per);
            let fixed = match w.side {
                WallEdge::North => w.cell[1] as f32 * s,
                WallEdge::South => (w.cell[1] + 1) as f32 * s,
                WallEdge::West => w.cell[0] as f32 * s,
                _ => (w.cell[0] + 1) as f32 * s,
            };
            let lift =
                amount_units(&ws.grid.build.opts, drag.kind) as f32 * s / SECTOR_UNITS as f32;
            let p = |along: f32, up: f32| match w.side {
                WallEdge::North | WallEdge::South => [along, y + up, fixed],
                _ => [fixed, y + up, along],
            };
            lines.push(vec![
                p(c0, 0.0),
                p(c1, 0.0),
                p(c1, lift),
                p(c0, lift),
                p(c0, 0.0),
            ]);
        }
        _ if !g.poly.is_empty() => {
            let mut pts: Vec<[f32; 3]> = g
                .poly
                .iter()
                .map(|p| [p[0] as f32 * s, y, p[1] as f32 * s])
                .collect();
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
            lines.push(pts);
        }
        _ => {
            let r: CellRect = g.rect();
            let (x0, z0) = (r.min[0] as f32 * s, r.min[1] as f32 * s);
            let (x1, z1) = ((r.max[0] + 1) as f32 * s, (r.max[1] + 1) as f32 * s);
            lines.push(vec![
                [x0, y, z0],
                [x1, y, z0],
                [x1, y, z1],
                [x0, y, z1],
                [x0, y, z0],
            ]);
        }
    }
    let text = if drag.last.refused || !drag.last.changed {
        drag.last.text.clone()
    } else {
        String::new()
    };
    Some((lines, text))
}
