//! The rock look: Break, Rough and the paint rules.
//!
//! Wrappers over `psxed_project::grid::edit::verbs::break_verb`,
//! `edit::rough` and `PaintRules`, the functions the editor's gestures call.

use std::fmt::Write as _;

use psxed_project::grid::cook::collision as cook_collision;
use psxed_project::grid::edit::outline::polygon_cells;
use psxed_project::grid::edit::rough as edit_rough;
use psxed_project::grid::edit::verbs::break_verb::{
    BreakDrag, BreakParams, BreakReport, BreakSnap, BreakTargets, WalkCount,
};
use psxed_project::grid::edit::verbs::rules::{ensure_strata, reapply_floor_rules};
use psxed_project::grid::edit::{CellRect, Region};
use psxed_project::grid::faces::face_count;
use psxed_project::grid::paint_rules::{HeightRule, PaintRules};
use psxed_project::grid::walk::{self, Spawn, WalkLimits};
use psxed_project::grid::GridWorld;
use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, pick_room, Materials, MAX_CELLS_PER_CALL};

/// A disc of sectors.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
pub struct BrushReq {
    /// Centre, WORLD sector `[x, z]`.
    pub centre: [i32; 2],
    /// Radius in sectors (0 is the sector itself, 1 a plus of five).
    pub radius: i32,
}

/// A path of waypoints, widened sideways.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct PathReq {
    /// Waypoints, WORLD sectors, joined by a four-connected walk.
    pub points: Vec<[i32; 2]>,
    /// Sectors across (default 1).
    pub width: Option<i32>,
}

/// Where a tool acts: one of a rectangle (`from`, `to`), a `brush`, a `path`
/// or a `polygon`.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct RegionReq {
    /// One corner of a rectangle of WORLD sectors (inclusive).
    pub from: Option<[i32; 2]>,
    /// The opposite corner. Omit for the single sector `from`.
    pub to: Option<[i32; 2]>,
    /// A disc of sectors.
    pub brush: Option<BrushReq>,
    /// A winding path.
    pub path: Option<PathReq>,
    /// A closed polygon in sector coordinates (numbers; cell `[3, 2]` has its
    /// centre at `[3.5, 2.5]`): the sectors whose centres it contains.
    pub polygon: Option<Vec<[f64; 2]>>,
}

impl RegionReq {
    /// The region in `room`.
    pub fn region(&self, room: usize) -> Result<Region, String> {
        let kinds = usize::from(self.from.is_some())
            + usize::from(self.brush.is_some())
            + usize::from(self.path.is_some())
            + usize::from(self.polygon.is_some());
        if kinds != 1 {
            return Err(
                "give exactly one region: from (with an optional to), brush, path or polygon"
                    .to_string(),
            );
        }
        let region = if let Some(from) = self.from {
            let rect = CellRect::from_corners(from, self.to.unwrap_or(from));
            check_rect(&rect)?;
            Region::from_rect(room, rect)
        } else if let Some(b) = self.brush {
            Region::brush(room, b.centre, b.radius.clamp(0, 12))
        } else if let Some(p) = &self.path {
            if p.points.is_empty() {
                return Err("the path needs at least one waypoint".to_string());
            }
            Region::from_path(room, &p.points, p.width.unwrap_or(1))
        } else {
            let cells = polygon_cells(self.polygon.as_deref().unwrap_or(&[]));
            Region::from_cells(room, cells)
        };
        if region.len() > MAX_CELLS_PER_CALL {
            return Err(format!(
                "that region is {} sectors; one call handles at most {MAX_CELLS_PER_CALL}",
                region.len()
            ));
        }
        Ok(region)
    }
}

/// Break the walls, floors and ceilings of a region.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BreakReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: `from`/`to`, `brush`, `path` or `polygon`.
    #[serde(flatten)]
    pub region: RegionReq,
    /// Any of `walls`, `floor`, `ceiling`. Default walls and floor. Walls
    /// crumbles plateau lips, bites plateau edges and ragges open wall tops;
    /// floor tilts and cracks tiles; ceiling does the same to the roof.
    pub targets: Option<Vec<String>>,
    /// 0 to 100: the chance and the size of every change (default 40).
    pub intensity: Option<u32>,
    /// A number: the same seed gives the same result. Omit to take the next
    /// one after the last break.
    pub seed: Option<u32>,
    /// The largest vertical change at intensity 100, in fine steps of 64
    /// grid units (default 4: one click, the Cathedral's tiles).
    pub amplitude: Option<i32>,
    /// `fine` (default, 64 grid units) or `click` (256).
    pub snap: Option<String>,
    /// Hold every change to the walkability mask (default true): nothing
    /// that adds a floor too steep to stand on, a step taller than the
    /// character climbs or too little headroom survives.
    pub keep_walkable: Option<bool>,
    /// Sectors `[x, z]` the player starts in, for the route check. Default:
    /// every player spawn.
    pub spawn: Option<Vec<[i32; 2]>>,
    /// `keep` (default) or `mix`: a cracked triangle takes the material of
    /// the floor it now borders.
    pub materials: Option<String>,
    /// Give every broken sector this rough level (0 to 3, default 0).
    pub jitter: Option<u8>,
    /// Compute and report, change nothing.
    pub dry_run: Option<bool>,
    /// Restore the checkpoint the last break took and break again with the
    /// next seed (or `seed`): the loop for choosing a look.
    pub reroll: Option<bool>,
}

/// The checkpoint a break takes before it writes.
pub const BREAK_CHECKPOINT: &str = "before-break";

/// Everything `grid_break` needs besides the world, parsed.
pub struct BreakPlan {
    /// The room.
    pub room: usize,
    /// The region.
    pub region: Region,
    /// The verb's parameters.
    pub params: BreakParams,
    /// 0 to 100.
    pub intensity: u32,
    /// The seed, when the request gave one.
    pub seed: Option<u32>,
}

/// Parse a request against the world and project.
pub fn plan_break(
    world: &GridWorld,
    project: &ProjectDocument,
    req: &BreakReq,
) -> Result<BreakPlan, String> {
    let room = pick_room(world, req.room)?;
    let region = req.region.region(room)?;
    let mut targets = BreakTargets {
        walls: false,
        floor: false,
        ceiling: false,
    };
    match &req.targets {
        None => {
            targets.walls = true;
            targets.floor = true;
        }
        Some(list) => {
            for t in list {
                match t.to_ascii_lowercase().as_str() {
                    "walls" | "wall" => targets.walls = true,
                    "floor" | "floors" => targets.floor = true,
                    "ceiling" | "ceilings" => targets.ceiling = true,
                    other => {
                        return Err(format!(
                            "unknown target {other:?}; use walls, floor or ceiling"
                        ))
                    }
                }
            }
            if !(targets.walls || targets.floor || targets.ceiling) {
                return Err("targets is empty".to_string());
            }
        }
    }
    let spawns: Vec<Spawn> = match &req.spawn {
        Some(cells) => cells.iter().map(|&cell| Spawn { room, cell }).collect(),
        None => walk::player_spawns(project, world),
    };
    let snap = match req.snap.as_deref() {
        None | Some("fine") => BreakSnap::Fine,
        Some("click") => BreakSnap::Click,
        Some(other) => return Err(format!("unknown snap {other:?}; use fine or click")),
    };
    let params = BreakParams {
        targets,
        amplitude: req.amplitude.unwrap_or(4).clamp(1, 32),
        snap,
        keep_walkable: req.keep_walkable.unwrap_or(true),
        spawns,
        mix_materials: match req.materials.as_deref() {
            None | Some("keep") => false,
            Some("mix") => true,
            Some(other) => return Err(format!("unknown materials {other:?}; use keep or mix")),
        },
        jitter: req.jitter.unwrap_or(0).min(3),
        limits: WalkLimits::DEFAULT,
    };
    Ok(BreakPlan {
        room,
        region,
        params,
        intensity: req.intensity.unwrap_or(40).min(100),
        seed: req.seed,
    })
}

/// The collision bytes a room cooks to: wall records, extension records and
/// split-level records (what a break adds).
fn collision_bytes(world: &GridWorld, room: usize) -> Option<usize> {
    let units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok()?;
    let t = cook_collision::build(&units, world.rooms.get(room)?, room).ok()?;
    Some(t.walls.len() * 12 + t.exts.len() * 8 + t.splitbs.len() * 16)
}

fn walk_line(c: WalkCount) -> String {
    format!(
        "{} standable, {} unreachable, {} step(s) over the climb limit, {} steep",
        c.standable, c.unreachable, c.steps, c.steep
    )
}

/// Run a break on `world` with `seed` and describe it.
pub fn run_break(
    world: &mut GridWorld,
    plan: &BreakPlan,
    seed: u32,
    first_seed_note: Option<&str>,
) -> Result<String, String> {
    let drag = BreakDrag::begin(world, &plan.region, plan.params.clone())
        .ok_or("nothing to break: no sector of the region has a floor or a ceiling")?;
    let faces = face_count(world);
    let bytes = collision_bytes(world, plan.room);
    let report = drag.apply(world, plan.intensity, seed);
    let faces_after = face_count(world);
    let bytes_after = collision_bytes(world, plan.room);
    Ok(describe(
        &report,
        drag.len(),
        (faces, faces_after),
        bytes.zip(bytes_after),
        first_seed_note,
    ))
}

fn describe(
    r: &BreakReport,
    sectors: usize,
    faces: (usize, usize),
    bytes: Option<(usize, usize)>,
    note: Option<&str>,
) -> String {
    let mut out = format!(
        "break seed {} intensity {}: {} of {} sector(s) changed\n  walls: {} plateau lip(s) crumbled, {} edge bite(s), {} open wall top(s) ragged ({} stubbed)\n  floor: {} interior vertex move(s), {} tile(s) cracked, {} gap(s)\n  ceiling: {} change(s)\n  faces {} -> {} ({:+})",
        r.seed,
        r.intensity,
        r.touched,
        sectors,
        r.crumbled,
        r.bites,
        r.tops,
        r.stubs,
        r.tilted,
        r.cracked,
        r.gaps,
        r.ceiling,
        faces.0,
        faces.1,
        faces.1 as i64 - faces.0 as i64
    );
    if let Some((a, b)) = bytes {
        let _ = write!(
            out,
            ", collision {a} -> {b} B ({:+}: 24 B per cracked cell, 12 B per wall record)",
            b as i64 - a as i64
        );
    }
    if let Some([before, after]) = r.walk {
        let _ = write!(
            out,
            "\n  walkability: {} -> {}",
            walk_line(before),
            walk_line(after)
        );
    } else {
        out.push_str(
            "\n  walkability: not checked for the route (no player spawn stands on a floor); steps, slopes and headroom were still held",
        );
    }
    if r.rejected > 0 {
        let _ = write!(
            out,
            "\n  {} operation(s) refused by the walkability mask ({} went through smaller):",
            r.rejected, r.shrunk
        );
        for reason in &r.rejects {
            let _ = write!(out, "\n    - {reason}");
        }
        if r.rejected > r.rejects.len() {
            let _ = write!(out, "\n    - ... and {} more", r.rejected - r.rejects.len());
        }
    } else if r.shrunk > 0 {
        let _ = write!(
            out,
            "\n  {} operation(s) went through smaller to keep the route",
            r.shrunk
        );
    }
    if let Some(note) = note {
        let _ = write!(out, "\n{note}");
    }
    out
}

/// Paint the rough level of a region and set the room's jitter.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoughReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: `from`/`to`, `brush`, `path` or `polygon`. Omit it to only
    /// change the room's settings.
    #[serde(flatten)]
    pub region: RegionReq,
    /// 0 to 3: how rough those sectors are (0 smooths them). Level 1, 2 and 3
    /// displace the corners of every face that meets at a lattice point near
    /// them by up to the room's amplitude for that level.
    pub level: Option<u8>,
    /// The room's seed for the jitter and the tint noise.
    pub seed: Option<u16>,
    /// The room's amplitudes at levels 1, 2 and 3 in GRID units either way
    /// (default 1, 2 and 4 runtime units). More than a third of the
    /// character's radius is a validation warning: collision stays on the
    /// grid, so the rock would clip the model.
    pub amp: Option<[u8; 3]>,
    /// Brightness noise on rough faces, percent either way (0 to 100).
    pub tint: Option<u8>,
}

/// Apply [`RoughReq`].
pub fn rough(world: &mut GridWorld, req: &RoughReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let has_region = req.region.from.is_some()
        || req.region.brush.is_some()
        || req.region.path.is_some()
        || req.region.polygon.is_some();
    if !has_region && req.level.is_some() {
        return Err("level needs a region (from, brush, path or polygon)".to_string());
    }
    if !has_region && req.seed.is_none() && req.amp.is_none() && req.tint.is_none() {
        return Err("give a region and level, or seed, amp or tint".to_string());
    }
    let mut out = String::new();
    if let Some(level) = req.level {
        let region = req.region.region(room)?;
        let cells: Vec<[i32; 2]> = region.cells().collect();
        let changed = edit_rough::set_level(world, room, &cells, level);
        let _ = write!(
            out,
            "room {room}: {changed} of {} sector(s) set to rough level {}",
            cells.len(),
            level.min(3)
        );
    }
    let set = edit_rough::set_params(world, room, req.seed, req.amp, req.tint);
    let r = &world.rooms[room].rough;
    if set || out.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        let _ = write!(
            out,
            "room {room} rough settings: seed {}, amplitude {:?} grid units at levels 1/2/3, tint {}%",
            r.seed, r.amp, r.tint
        );
    }
    out.push_str("\nRender only: collision is not moved, so keep amplitudes under a third of the character's radius (4 runtime units). The preview and Play show the same jitter.");
    Ok(out)
}

/// A material per height band.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct HeightRuleReq {
    /// Where bands 1, 2 and 3 begin: room-relative heights in GRID units,
    /// ascending (rounded to 64). Up to three.
    pub at: Vec<i32>,
    /// One material (name or id, or "none" to leave a band alone) per band,
    /// low to high: one more than `at`.
    pub materials: Vec<String>,
}

fn height_rule(mats: &Materials<'_>, req: &HeightRuleReq) -> Result<HeightRule, String> {
    if req.at.len() > 3 || req.materials.len() > 4 || req.materials.len() != req.at.len() + 1 {
        return Err(
            "a height rule has up to three thresholds in at and one more material than thresholds"
                .to_string(),
        );
    }
    if req.at.windows(2).any(|w| w[0] >= w[1]) {
        return Err("at must be ascending".to_string());
    }
    let mut rule = HeightRule::default();
    for (i, h) in req.at.iter().enumerate() {
        rule.at[i] = i16::try_from(psxed_project::grid::units::snap_to(*h, 64) / 64)
            .map_err(|_| format!("threshold {h} is out of range"))?;
    }
    for (i, m) in req.materials.iter().enumerate() {
        rule.materials[i] = mats.resolve_opt(Some(m))?;
    }
    Ok(rule)
}

/// Set the room's smart paint rules.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PaintRulesReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Floor material by slope class: `[flat, slope, steep]` (name, id or
    /// "none").
    pub floor_by_class: Option<[String; 3]>,
    /// Floor material by height (scree low, rock high).
    pub floor_by_height: Option<HeightRuleReq>,
    /// The material the verbs give the derived walls of the sectors they
    /// shape ("none" clears).
    pub wall: Option<String>,
    /// Wall material by height: strata. The verbs make it the room's default
    /// band set with an absolute cut line per threshold.
    pub wall_by_height: Option<HeightRuleReq>,
    /// The trim material of the foot of every wall ("none" clears).
    pub trim: Option<String>,
    /// Apply the rules now to the floors of this region (rectangle, brush,
    /// path or polygon) that were not painted by hand, and make the strata.
    #[serde(flatten)]
    pub reapply: RegionReq,
}

/// Apply [`PaintRulesReq`].
pub fn paint_rules(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PaintRulesReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let mut rules: PaintRules = world.rooms[room].defaults.rules;
    if let Some([flat, slope, steep]) = &req.floor_by_class {
        rules.floor_by_class = [
            mats.resolve_opt(Some(flat))?,
            mats.resolve_opt(Some(slope))?,
            mats.resolve_opt(Some(steep))?,
        ];
    }
    if let Some(h) = &req.floor_by_height {
        rules.floor_by_height = height_rule(mats, h)?;
    }
    if let Some(w) = &req.wall {
        rules.wall = mats.resolve_opt(Some(w))?;
    }
    if let Some(h) = &req.wall_by_height {
        rules.wall_by_height = height_rule(mats, h)?;
    }
    if let Some(t) = &req.trim {
        rules.trim = mats.resolve_opt(Some(t))?;
    }
    world.rooms[room].defaults.rules = rules;
    let mut out = format!(
        "room {room} paint rules: floor by class {:?}, floor by height {} band(s), wall {:?}, wall by height {} band(s), trim {:?}",
        rules.floor_by_class.map(|m| mats.short(m)),
        rules.floor_by_height.bands(),
        mats.short(rules.wall),
        rules.wall_by_height.bands(),
        mats.short(rules.trim),
    );
    let has_region = req.reapply.from.is_some()
        || req.reapply.brush.is_some()
        || req.reapply.path.is_some()
        || req.reapply.polygon.is_some();
    if has_region {
        let region = req.reapply.region(room)?;
        let cells: Vec<[i32; 2]> = region.cells().collect();
        let painted = reapply_floor_rules(world, room, &cells);
        let strata = ensure_strata(&mut world.rooms[room]);
        let _ = write!(
            out,
            "\nreapplied to {} sector(s): {painted} floor(s) painted{}",
            cells.len(),
            if strata {
                "; the strata band set was made the room default"
            } else {
                ""
            }
        );
    } else if ensure_strata(&mut world.rooms[room]) {
        out.push_str("\nthe strata band set was made the room default");
    }
    out.push_str("\nRules run inside the verbs (raise, ramp, stairs, break) on floors not painted by hand; reapply with a region runs them now.");
    Ok(out)
}
