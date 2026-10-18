//! The level verbs and the two overlays, as tools.
//!
//! Each verb is a thin wrapper over the same pure function the editor's
//! gesture calls (`psxed_project::grid::edit::verbs`), so a slope or a flight
//! made here is made by exactly the code a drag reaches. The overlays are
//! reads: the walkability analysis (`psxed_project::grid::walk`) and the
//! performance heatmap model (`psxed_project::grid::heat`).

use std::fmt::Write as _;

use psxed_project::grid::edit::verbs::fine::{run_fine, FineDrag, FineReport, FineVerb};
use psxed_project::grid::edit::verbs::raise::{
    default_mode, height_under, raise_region, RaiseMode, RaiseParams,
};
use psxed_project::grid::edit::verbs::ramp::{RampDir, RampDrag, RampParams};
use psxed_project::grid::edit::verbs::stairs::{RiserOption, StairsDrag, StairsParams};
use psxed_project::grid::edit::{CellRect, Region};
use psxed_project::grid::heat::{heatmap, HeatOptions};
use psxed_project::grid::units::{snap_to, CLICK_UNITS, FINE_UNITS};
use psxed_project::grid::walk::{self, analyse, EdgeKind, WalkLimits};
use psxed_project::grid::GridWorld;
use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

use super::build::Surface;
use super::{check_rect, pick_room, Cells, Materials, Snap};

fn clicks_text(units: i32) -> String {
    format!(
        "{:+.2} clicks ({units:+} gu)",
        units as f32 / CLICK_UNITS as f32
    )
}

fn wall_material(
    project: &ProjectDocument,
    name: &Option<String>,
) -> Result<Option<psxed_project::ResourceId>, String> {
    name.as_deref()
        .map(|n| Materials::new(project).resolve(n))
        .transpose()
}

/// The cell level a `size` names: 2 sector (default), 3 half, 4 quarter.
fn size_level(size: &Option<String>) -> Result<u8, String> {
    match size.as_deref() {
        None | Some("sector") => Ok(2),
        Some("half") => Ok(3),
        Some("quarter") => Ok(4),
        Some(other) => Err(format!(
            "unknown size {other:?}; use sector, half or quarter"
        )),
    }
}

/// The cells of a rectangle of cells of `level`.
fn rect_cells(rect: psxed_project::grid::edit::CellRect) -> Vec<[i32; 2]> {
    rect.cells().collect()
}

/// Raise or lower a region of floors.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaiseReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: a rectangle of WORLD sectors. Sectors with no floor are
    /// skipped and counted.
    #[serde(flatten)]
    pub cells: Cells,
    /// Grid units, positive raises, negative lowers (256 a click). Snapped by
    /// `snap`.
    pub amount: i32,
    /// `offset` (every corner moves by the amount, shape kept) or `level`
    /// (every corner set to `reference` plus the amount). Default: level on a
    /// flat region, offset otherwise.
    pub mode: Option<String>,
    /// The height level mode measures from, grid units relative to the room.
    /// Default: the floor under the region's first sector.
    pub reference: Option<i32>,
    /// Ceilings move by the same delta as the floor under them (default true);
    /// false holds a floor one click under its ceiling.
    pub follow_ceiling: Option<bool>,
    /// Material (name or id) of the derived walls of the shaped sectors.
    pub wall_material: Option<String>,
    /// `click` (default) or `fine`.
    pub snap: Option<Snap>,
    /// What the rectangle's corners count: `sector` (default) or `click` (a
    /// quarter sector). With `click`, the cells inside the rectangle move
    /// and a cell it cuts through is subdivided to fit, so one step of a
    /// fine staircase is one call.
    pub unit: Option<super::cells::Unit>,
    /// What a cell is: `sector` (default), `half` or `quarter`. The
    /// rectangle's corners then count cells of that size (WORLD anchored:
    /// half cell `h` is world click `2h`, quarter cell `q` is click `q`), and
    /// the verb runs on exactly those cells, splitting larger ones to fit.
    pub size: Option<String>,
    /// Which span of a column to edit: `y` or `storey`. Needed where a
    /// sector has several spans.
    #[serde(flatten)]
    pub aim: super::aim::SpanAim,
}

/// Apply [`RaiseReq`].
pub fn raise(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &RaiseReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    if req.unit == Some(super::cells::Unit::Click) {
        return raise_clicks(world, project, req, room);
    }
    let rect = req.cells.rect();
    check_rect(&rect)?;
    super::aim::aimed(world, room, rect, &req.aim, |world| {
        raise_in(world, project, req, room, rect)
    })
}

/// The edit itself, run with the aimed span standing in as the ground span.
fn raise_in(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &RaiseReq,
    room: usize,
    rect: CellRect,
) -> Result<String, String> {
    let level = size_level(&req.size)?;
    let region = Region::from_rect(room, rect).at_level(level);
    if level != 2 {
        let (amount, note) = req.snap.unwrap_or_default().apply(req.amount);
        let params = RaiseParams {
            mode: match req.mode.as_deref() {
                None | Some("offset") => RaiseMode::Offset,
                Some("level") => RaiseMode::Level,
                Some(other) => return Err(format!("unknown mode {other:?}; use offset or level")),
            },
            follow_ceiling: req.follow_ceiling.unwrap_or(true),
            wall_material: wall_material(project, &req.wall_material)?,
            reference: req.reference.unwrap_or(0),
        };
        let cells = rect_cells(rect);
        let report = run_fine(world, room, level, &cells, FineVerb::Raise(params), amount);
        let Some(FineReport::Applied(applied)) = report else {
            return Err(match report {
                Some(FineReport::Refused(why)) => why,
                _ => "nothing to raise".to_string(),
            });
        };
        let mut out = format!(
            "room {room}: {} {} cell(s) of {} by {}",
            if amount < 0 { "lowered" } else { "raised" },
            applied.moved,
            req.size.as_deref().unwrap_or("sector"),
            clicks_text(amount)
        );
        if let Some(note) = note {
            let _ = write!(out, "\n{note}");
        }
        return Ok(out);
    }
    let (amount, note) = req.snap.unwrap_or_default().apply(req.amount);
    let first = region.cells().next().unwrap_or([0, 0]);
    let mode = match req.mode.as_deref() {
        None => default_mode(world, &region),
        Some("offset") => RaiseMode::Offset,
        Some("level") => RaiseMode::Level,
        Some(other) => return Err(format!("unknown mode {other:?}; use offset or level")),
    };
    let params = RaiseParams {
        mode,
        follow_ceiling: req.follow_ceiling.unwrap_or(true),
        wall_material: wall_material(project, &req.wall_material)?,
        reference: req
            .reference
            .or_else(|| height_under(world, room, first))
            .unwrap_or(0),
    };
    let applied = raise_region(world, &region, params, amount)
        .ok_or("nothing to raise: no sector of the rectangle has a floor")?;
    let mut out = format!(
        "room {room}: {} {} sector(s) by {} in {} mode",
        if amount < 0 { "lowered" } else { "raised" },
        applied.moved,
        clicks_text(amount),
        mode.label()
    );
    if let Some(note) = note {
        let _ = write!(out, "\n{note}");
    }
    if applied.skipped > 0 {
        let _ = write!(
            out,
            "\n{} sector(s) had no floor and were skipped",
            applied.skipped
        );
    }
    if applied.clamped > 0 {
        let _ = write!(
            out,
            "\n{} floor(s) held one click under their ceiling",
            applied.clamped
        );
    }
    Ok(out)
}

/// [`raise`] over a rectangle of clicks.
fn raise_clicks(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &RaiseReq,
    room: usize,
) -> Result<String, String> {
    use psxed_project::grid::edit::verbs::raise_leaves::raise_click_rect;
    let (lo, hi) = super::cells::click_rect(&req.cells, super::cells::Unit::Click)?;
    if req.aim.y.is_some() || req.aim.storey.is_some() {
        return Err(
            "y and storey aim a whole sector's span; unit: click edits cells smaller than a sector, which hold one floor each. Leave them out, or raise whole sectors"
                .to_string(),
        );
    }
    super::storeys::refuse_several_spans_in_clicks(world, room, lo, hi)?;
    let (amount, note) = req.snap.unwrap_or_default().apply(req.amount);
    let mode = match req.mode.as_deref() {
        None | Some("offset") => RaiseMode::Offset,
        Some("level") => RaiseMode::Level,
        Some(other) => return Err(format!("unknown mode {other:?}; use offset or level")),
    };
    let reference = req
        .reference
        .or_else(|| {
            let (_, leaf) = world.rooms.get(room)?.leaf_at(lo)?;
            let f = leaf.ground_floor()?;
            Some(snap_to(f.heights.iter().sum::<i32>() / 4, FINE_UNITS))
        })
        .unwrap_or(0);
    let params = RaiseParams {
        mode,
        follow_ceiling: req.follow_ceiling.unwrap_or(true),
        wall_material: wall_material(project, &req.wall_material)?,
        reference,
    };
    let applied = raise_click_rect(world, room, lo, hi, params, amount)
        .ok_or("nothing to raise: no cell lies in that rectangle")?;
    let mut out = format!(
        "room {room}: {} {} cell(s) by {} in {} mode ({})",
        if amount < 0 { "lowered" } else { "raised" },
        applied.moved,
        clicks_text(amount),
        mode.label(),
        super::cells::census(&world.rooms[room]),
    );
    if let Some(note) = note {
        let _ = write!(out, "\n{note}");
    }
    if applied.skipped > 0 {
        let _ = write!(out, "\n{} cell(s) had no floor", applied.skipped);
    }
    Ok(out)
}

/// Slope a region from one side to the opposite side.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RampReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: a rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// The side that keeps its height (the A edge): `north`, `east`, `south`
    /// or `west`. The opposite side rises.
    pub keep: String,
    /// Grid units the far side rises (negative falls). Omit with `walkable`
    /// true to take the steepest slope the character walks.
    pub rise: Option<i32>,
    /// Hold the rise to the steepest walkable slope (the motor's own limit).
    /// Default false: a steeper rise is made and reported as too steep.
    pub walkable: Option<bool>,
    /// Ceilings move with the floors (default true).
    pub follow_ceiling: Option<bool>,
    /// Material (name or id) of the derived walls of the shaped sectors.
    pub wall_material: Option<String>,
    /// What a cell is: `sector` (default), `half` or `quarter` (see
    /// grid_raise). The rectangle counts cells of that size and the slope
    /// runs over them.
    pub size: Option<String>,
    /// Which span of a column to edit: `y` or `storey`. Needed where a
    /// sector has several spans.
    #[serde(flatten)]
    pub aim: super::aim::SpanAim,
}

fn parse_dir(name: &str) -> Result<RampDir, String> {
    match name.to_ascii_lowercase().as_str() {
        "north" | "n" => Ok(RampDir::North),
        "east" | "e" => Ok(RampDir::East),
        "south" | "s" => Ok(RampDir::South),
        "west" | "w" => Ok(RampDir::West),
        other => Err(format!(
            "unknown side {other:?}; use north, east, south or west"
        )),
    }
}

/// Apply [`RampReq`].
pub fn ramp(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &RampReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    super::aim::aimed(world, room, rect, &req.aim, |world| {
        ramp_in(world, project, req, room, rect)
    })
}

/// The edit itself, run with the aimed span standing in as the ground span.
fn ramp_in(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &RampReq,
    room: usize,
    rect: CellRect,
) -> Result<String, String> {
    let level = size_level(&req.size)?;
    let region = Region::from_rect(room, rect).at_level(level);
    let params = RampParams {
        from: parse_dir(&req.keep)?,
        follow_ceiling: req.follow_ceiling.unwrap_or(true),
        wall_material: wall_material(project, &req.wall_material)?,
    };
    if region.is_fine(world) {
        let cells = rect_cells(rect);
        let drag = FineDrag::begin(world, room, level, &cells, FineVerb::Ramp(params))
            .ok_or("nothing to ramp there")?;
        let max = drag.max_walkable_rise(world).unwrap_or(0);
        let rise = match (req.rise, req.walkable) {
            (Some(r), _) => snap_to(r, FINE_UNITS),
            (None, Some(true)) => max,
            (None, _) => {
                return Err("give a rise, or walkable true for the steepest walkable slope".into())
            }
        };
        let rise = if req.walkable == Some(true) {
            rise.clamp(-max, max)
        } else {
            rise
        };
        return match drag.apply(world, rise) {
            FineReport::Applied(applied) => Ok(format!(
                "room {room}: ramp from the {} edge over {} cell(s) of {}: {} at the far side, {}; the steepest walkable rise here is {}\n{} cell(s) sloped",
                params.from.label(),
                cells.len(),
                req.size.as_deref().unwrap_or("sector"),
                clicks_text(rise),
                if rise.abs() <= max { "walkable" } else { "TOO STEEP to stand on" },
                clicks_text(max),
                applied.moved
            )),
            FineReport::Refused(why) => Err(why),
            _ => Err("nothing to ramp there".to_string()),
        };
    }
    let drag = RampDrag::begin(world, &region, params)
        .ok_or("nothing to ramp: no sector of the rectangle has a floor")?;
    let max = drag.max_walkable_rise();
    let wanted = match (req.rise, req.walkable) {
        (Some(r), _) => snap_to(r, FINE_UNITS),
        (None, Some(true)) => max,
        (None, _) => {
            return Err("give a rise, or walkable true for the steepest walkable slope".into())
        }
    };
    let rise = if req.walkable == Some(true) {
        wanted.clamp(-max, max)
    } else {
        wanted
    };
    let applied = drag.apply(world, rise);
    let units =
        psxed_project::grid::cook::RuntimeUnits::new(world.scale).map_err(|e| e.to_string())?;
    let depth = drag.depth().max(1);
    let per = units.rt(rise) as f32 / depth as f32;
    let angle = per.atan2(units.sector() as f32).to_degrees();
    let mut out = format!(
        "room {room}: ramp from the {} edge (height {}) over {depth} sector(s): {} at the far side, {per:+.1} runtime units a sector, {angle:.1} degrees, {}; the steepest walkable rise here is {}",
        params.from.label(),
        drag.base(),
        clicks_text(rise),
        if rise.abs() <= max { "walkable" } else { "TOO STEEP to stand on" },
        clicks_text(max),
    );
    let _ = write!(out, "\n{} sector(s) sloped", applied.moved);
    if applied.skipped > 0 {
        let _ = write!(out, ", {} had no floor and were skipped", applied.skipped);
    }
    Ok(out)
}

/// Lay a flight of steps along a path.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StairsReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Waypoints, WORLD sectors `[x, z]`, joined by a four-connected walk.
    /// Each sector of the path is one tread; a turn is a landing.
    pub path: Vec<[i32; 2]>,
    /// Sectors across (default 1).
    pub width: Option<i32>,
    /// Total rise in grid units (negative goes down). The first tread stands
    /// one riser above the floor at the start of the path.
    pub rise: i32,
    /// `click` (one click, default), `max` (the tallest riser the motor steps
    /// up) or a number of grid units, held to that limit.
    pub riser: Option<serde_json::Value>,
    /// Slope every tread instead of stepping (default false).
    pub ramp: Option<bool>,
    /// Ceilings move with the floors (default true).
    pub follow_ceiling: Option<bool>,
    /// Material (name or id) of the derived walls of the shaped sectors.
    pub wall_material: Option<String>,
    /// What a cell is: `sector` (default), `half` or `quarter`. The path's
    /// waypoints and the width count cells of that size, and each cell of the
    /// path is one tread (a quarter cell tread is a fine step).
    pub size: Option<String>,
    /// Which span of a column to edit: `y` or `storey`. Needed where a
    /// sector of the path has several spans.
    #[serde(flatten)]
    pub aim: super::aim::SpanAim,
}

/// Apply [`StairsReq`].
pub fn stairs(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &StairsReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    if req.path.is_empty() {
        return Err("the path needs at least one waypoint".to_string());
    }
    let riser = match &req.riser {
        None => RiserOption::Click,
        Some(serde_json::Value::String(s)) if s == "click" => RiserOption::Click,
        Some(serde_json::Value::String(s)) if s == "max" => RiserOption::Max,
        Some(serde_json::Value::Number(n)) => RiserOption::Typed(n.as_i64().unwrap_or(0) as i32),
        Some(other) => {
            return Err(format!(
                "unknown riser {other}; use \"click\", \"max\" or a number"
            ))
        }
    };
    let params = StairsParams {
        riser,
        ramp: req.ramp.unwrap_or(false),
        width: req.width.unwrap_or(1).clamp(1, 16),
        follow_ceiling: req.follow_ceiling.unwrap_or(true),
        wall_material: wall_material(project, &req.wall_material)?,
    };
    let level = size_level(&req.size)?;
    let fine = level != 2 || !world.rooms[room].leaves.is_sectors_only();
    if fine {
        let footprint: Vec<[i32; 2]> = Region::from_path(room, &req.path, params.width)
            .cells()
            .collect();
        let verb = FineVerb::Stairs {
            params,
            path: req.path.clone(),
        };
        let report = run_fine(
            world,
            room,
            level,
            &footprint,
            verb,
            snap_to(req.rise, FINE_UNITS),
        );
        return match report {
            Some(FineReport::Flight(flight)) => {
                if let Some(refusal) = flight.refusal {
                    return Err(format!(
                        "{refusal:?}: use ramp true, a longer path or a smaller rise"
                    ));
                }
                Ok(format!(
                    "room {room}: {} step(s) of {} over {} path cell(s) of {}, total {}",
                    flight.steps,
                    clicks_text(flight.riser),
                    flight.cells,
                    req.size.as_deref().unwrap_or("sector"),
                    clicks_text(flight.total)
                ))
            }
            Some(FineReport::Refused(why)) => Err(why),
            _ => Err("the path crosses no cell".to_string()),
        };
    }
    let cells: Vec<[i32; 2]> = Region::from_path(room, &req.path, params.width)
        .cells()
        .collect();
    super::aim::aimed_cells(world, room, cells, &req.aim, |world| {
        let drag = StairsDrag::begin(world, room, &req.path, params)
            .ok_or("the path crosses no sector of the room")?;
        let rise = snap_to(req.rise, FINE_UNITS);
        let flight = drag.apply(world, rise);
        if let Some(refusal) = flight.refusal {
            return Err(match refusal {
            psxed_project::grid::edit::verbs::stairs::Refusal::TooSteep { riser, limit } => format!(
                "too steep for treads: the shortest riser this path allows is {} but the character steps up at most {} (grid units). Use ramp true, a longer path or a smaller rise",
                riser, limit
            ),
            psxed_project::grid::edit::verbs::stairs::Refusal::NoFloor => {
                "the path is outside the room".to_string()
            }
        });
        }
        let mut out = format!(
            "room {room}: {} step(s) of {} over {} path sector(s), width {}, total {}",
            flight.steps,
            clicks_text(flight.riser),
            flight.cells,
            params.width,
            clicks_text(flight.total)
        );
        if flight.applied.created > 0 {
            let _ = write!(
                out,
                "\n{} floor(s) made where the path crossed holes",
                flight.applied.created
            );
        }
        if flight.applied.skipped > 0 {
            let _ = write!(
                out,
                "\n{} sector(s) outside the room were skipped",
                flight.applied.skipped
            );
        }
        let _ = CLICK_UNITS;
        Ok(out)
    })
}

/// Wind stairs round a ring of sectors, a slab per tread.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpiralReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The ring: waypoints, WORLD sectors `[x, z]`, in walking order. Each
    /// pair is joined by a four-connected walk and the last waypoint joins
    /// back to the first. The result must be at least four distinct sectors.
    /// A straight run of the ring is treads, a turn is a landing.
    pub ring: Vec<[i32; 2]>,
    /// Storeys to climb (default 1).
    pub storeys: Option<usize>,
    /// The climb of one storey, grid units (256 a click). The treads per
    /// storey follow from it and the riser, and the climb is `treads per
    /// storey x riser` as laid.
    pub rise_per_storey: i32,
    /// `click` (one click, default), `max` (the tallest riser the character
    /// steps up) or a number of grid units, held to that limit.
    pub riser: Option<serde_json::Value>,
    /// Material (name or id) of the treads' walking surface.
    pub material: Option<String>,
    /// Which floor of the ring's first sector the climb starts from: `y` or
    /// `storey`. Needed where a sector of the ring has several spans.
    #[serde(flatten)]
    pub aim: super::aim::SpanAim,
}

/// Apply [`SpiralReq`]: lay a tread slab at every visit of the walk round
/// the ring (the `Spiral` tool's code).
pub fn spiral_stairs(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &SpiralReq,
) -> Result<String, String> {
    use psxed_project::grid::edit::verbs::spiral::{
        base_height, ring_cells, SpiralDrag, SpiralParams,
    };
    let room = pick_room(world, req.room)?;
    if req.ring.is_empty() {
        return Err("the ring needs waypoints".to_string());
    }
    let ring = ring_cells(&req.ring).ok_or_else(|| {
        psxed_project::grid::edit::verbs::spiral::SpiralRefusal::NotARing.describe()
    })?;
    let riser = match &req.riser {
        None => RiserOption::Click,
        Some(serde_json::Value::String(s)) if s == "click" => RiserOption::Click,
        Some(serde_json::Value::String(s)) if s == "max" => RiserOption::Max,
        Some(serde_json::Value::Number(n)) => RiserOption::Typed(n.as_i64().unwrap_or(0) as i32),
        Some(other) => {
            return Err(format!(
                "unknown riser {other}; use \"click\", \"max\" or a number"
            ))
        }
    };
    let params = SpiralParams {
        storeys: req.storeys.unwrap_or(1).clamp(1, 16),
        riser,
        material: Materials::new(project).resolve_opt(req.material.as_deref())?,
    };
    // The floor the climb starts from, as the aim sees the first sector.
    let base = match super::aim::aim_focus(world, room, ring.iter().copied(), &req.aim)? {
        Some(focus) => base_height(&focus.view(world), room, &ring),
        None => base_height(world, room, &ring),
    };
    let drag = SpiralDrag::begin_at(world, room, &ring, params, base).ok_or(
        "the room is missing or has cells of several sizes (the spiral works on whole sectors)",
    )?;
    let report = drag.apply(world, req.rise_per_storey);
    if let Some(refusal) = report.refused {
        return Err(refusal.describe());
    }
    let mut out = format!(
        "room {room}: spiral of {} treads (riser {}) round a ring of {} sectors, {} per storey as laid, {} storey(s), total {}, {} lap(s), {} landing visit(s)\n{} slab(s) laid",
        report.treads,
        clicks_text(report.riser),
        ring.len(),
        clicks_text(report.per_storey),
        params.storeys,
        clicks_text(report.total),
        report.laps,
        report.landings,
        report.slabs.changed
    );
    if report.already > 0 {
        let _ = write!(
            out,
            "; {} visit(s) already had a floor there",
            report.already
        );
    }
    if report.slabs.skipped.total() > 0 {
        let _ = write!(out, "\nskipped: {}", report.slabs.skipped.describe());
    }
    Ok(out)
}

/// Set whether floors or ceilings are walkable (solid to the character).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetWalkableReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `floor` (default) or `ceiling`.
    pub kind: Option<Surface>,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `false` makes a ghost: a floor you fall through, a ceiling that does
    /// not stop the head. `true` restores it.
    pub walkable: bool,
}

/// Apply [`SetWalkableReq`].
pub fn set_walkable(world: &mut GridWorld, req: &SetWalkableReq) -> Result<String, String> {
    use psxed_project::grid::edit::FaceRef;
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let kind = req.kind.unwrap_or(Surface::Floor);
    let (mut changed, mut missing) = (0usize, 0usize);
    for cell in rect.cells() {
        let face = match kind {
            Surface::Floor => FaceRef::floor(room, cell),
            Surface::Ceiling => FaceRef::ceiling(room, cell),
        };
        match face.surface_mut(world) {
            Some(s) => {
                changed += usize::from(s.walkable != req.walkable);
                s.walkable = req.walkable;
            }
            None => missing += 1,
        }
    }
    Ok(format!(
        "room {room}: {changed} {} sector(s) now {}walkable; {missing} sector(s) had none",
        kind.kind().label(),
        if req.walkable { "" } else { "NOT " }
    ))
}

/// Read the walkability analysis.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WalkabilityReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Only this room (the map and the lists). Omit for every room.
    pub room: Option<usize>,
}

/// The walkability analysis, as text: counts, a map and the flagged edges.
pub fn walkability(
    world: &GridWorld,
    project: &ProjectDocument,
    req: &WalkabilityReq,
) -> Result<String, String> {
    let spawns = walk::player_spawns(project, world);
    let a = analyse(world, &spawns, &WalkLimits::DEFAULT)
        .ok_or("the world's scale cannot be cooked")?;
    let c = a.counts();
    let mut out = format!(
        "Walkability with the character motor's numbers: step up {} ru, body {} ru, steepest floor normal {:.3} (1 click = {} ru).\n{} too steep, {} low headroom, {} step(s) taller than the body climbs, {} drop(s) to nothing, {} unreachable. ",
        a.limits.step_up,
        a.limits.body_height,
        f32::from(a.limits.min_floor_normal_y_q12) / 4096.0,
        a.click_ru,
        c[0], c[1], c[2], c[3], c[4]
    );
    out.push_str(&if a.reach_ran {
        format!("Reachability ran from {} spawn(s).\n", spawns.len())
    } else {
        "Reachability did not run: no player spawn stands on a standable floor.\n".to_string()
    });
    out.push_str("Map: . walkable, S steep, L low headroom, G ghost floor, U unreachable, blank no floor. North (-Z) at the top.\n");
    for (ri, (room, walk_room)) in world.rooms.iter().zip(&a.rooms).enumerate() {
        if req.room.is_some_and(|r| r != ri) {
            continue;
        }
        let _ = writeln!(
            out,
            "room {ri} ({} x {}, origin {:?}):",
            room.width, room.depth, room.origin
        );
        for z in 0..room.depth as i32 {
            let mut line = String::new();
            for x in 0..room.width as i32 {
                let cell = [room.origin[0] + x, room.origin[1] + z];
                line.push(match walk_room.cell(cell) {
                    None => ' ',
                    Some(w) if w.steep => 'S',
                    Some(w) if w.low_headroom => 'L',
                    Some(w) if w.ghost => 'G',
                    Some(w) if w.unreachable => 'U',
                    Some(_) => '.',
                });
            }
            let _ = writeln!(out, "  z{:>3} {line}", room.origin[1] + z);
        }
    }
    let flagged: Vec<_> = a
        .edges
        .iter()
        .filter(|e| req.room.is_none_or(|r| r == e.room))
        .collect();
    if flagged.len() > 30 {
        out.push_str("(only the first 30 flagged edges are listed)\n");
    }
    for e in flagged.into_iter().take(30) {
        let _ = writeln!(
            out,
            "{} room {} sector {:?} {} edge{}",
            match e.kind {
                EdgeKind::StepTooTall => "step too tall:",
                EdgeKind::Drop => "drop:",
            },
            e.room,
            e.cell,
            e.edge.label(),
            if e.delta_ru > 0 {
                format!(" ({} ru)", e.delta_ru)
            } else {
                String::new()
            }
        );
    }
    Ok(out)
}

/// Read the performance heatmap model.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HeatmapReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Only this room. Omit for every room.
    pub room: Option<usize>,
    /// Sample every n-th sector (default 1, finest).
    pub stride: Option<u32>,
    /// Yaws sampled per spot (default 8).
    pub yaws: Option<u32>,
    /// Cycles the frame spends outside the world (default 528000: 470000
    /// player plus 58000 sky).
    pub reserve: Option<u32>,
}

/// The heatmap, as text: the model's labels, the budget, percentiles, a map in
/// tenths of the world budget and the hottest spots.
pub fn heat(world: &GridWorld, req: &HeatmapReq) -> Result<String, String> {
    let defaults = HeatOptions::default();
    let options = HeatOptions {
        stride: req.stride.unwrap_or(1).max(1),
        yaws: req.yaws.unwrap_or(8).clamp(1, 32),
        reserve: req.reserve.unwrap_or(defaults.reserve),
        ..defaults
    };
    let map = heatmap(world, options)?;
    let budget = options.world_budget();
    let mut out = format!(
        "PERFORMANCE MODEL, not a measurement: cost = visible quads x {} + near-plane quads x {} cycles, worst of {} yaws, over a world budget of {} cycles (1,128,960 per 30 fps frame minus {} reserved for player and sky). The camera is the player's spot lifted {} ru and pulled back {} ru (held by the edge of the floor); wall records, pitch and portal views are not modelled.\n{} spot(s) over {} quad(s); cook {:.0} ms, sampling {:.0} ms.\n",
        options.per_quad, options.per_near_quad, options.yaws, budget, options.reserve, options.eye, options.arm,
        map.samples.len(), map.quads_total, map.cook_ms, map.sample_ms
    );
    let pct = |c: u32| c as f32 * 100.0 / budget as f32;
    let _ = writeln!(
        out,
        "modelled world cycles: p50 {} ({:.0}%), p90 {} ({:.0}%), max {} ({:.0}%); quads p50 {}, p90 {}",
        map.percentile(50), pct(map.percentile(50)),
        map.percentile(90), pct(map.percentile(90)),
        map.percentile(100), pct(map.percentile(100)),
        map.quads_percentile(50), map.quads_percentile(90),
    );
    if let Some(w) = map.worst() {
        let _ = writeln!(
            out,
            "worst spot: room {} sector {:?}, yaw {}/{}: {} quads ({} near) = {} cycles, {:.0}% of the world budget",
            w.room, w.cell, w.yaw, options.yaws, w.quads, w.near, w.cycles, pct(w.cycles)
        );
    }
    out.push_str("Map in tenths of the world budget (0 to 9, + at 100% or more, blank no spot). North (-Z) at the top.\n");
    for (ri, room) in world.rooms.iter().enumerate() {
        if req.room.is_some_and(|r| r != ri) {
            continue;
        }
        let _ = writeln!(out, "room {ri}:");
        for z in 0..room.depth as i32 {
            let mut line = String::new();
            for x in 0..room.width as i32 {
                let cell = [room.origin[0] + x, room.origin[1] + z];
                line.push(match map.at(ri, cell) {
                    None => ' ',
                    Some(s) => {
                        let tenths = (map.heat(s) * 10.0) as u32;
                        if tenths >= 10 {
                            '+'
                        } else {
                            char::from(b'0' + tenths as u8)
                        }
                    }
                });
            }
            let _ = writeln!(out, "  z{:>3} {line}", room.origin[1] + z);
        }
    }
    let mut hot: Vec<_> = map
        .samples
        .iter()
        .filter(|s| req.room.is_none_or(|r| r == s.room))
        .collect();
    hot.sort_by_key(|s| core::cmp::Reverse(s.cycles));
    out.push_str("hottest spots:\n");
    for s in hot.iter().take(8) {
        let _ = writeln!(
            out,
            "  room {} sector {:?}: {} quads, {} near, {:.0}%",
            s.room,
            s.cell,
            s.quads,
            s.near,
            pct(s.cycles)
        );
    }
    Ok(out)
}
