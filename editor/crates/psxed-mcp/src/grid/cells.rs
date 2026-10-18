//! Cells of several sizes as tools: subdivide, merge and the read that shows
//! them.
//!
//! Thin wrappers over `psxed_project::grid::tree`, the code the editor's
//! Cells tool calls. A region is a rectangle of WORLD sectors, or, with
//! `unit: "click"`, of clicks (a click is a quarter of a sector, the finest
//! cell; world click `c` is world sector `c div 4`).

use std::fmt::Write as _;

use psxed_project::grid::tree::{self, check};
use psxed_project::grid::{GridRoom, GridWorld, LeafKey};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, pick_room, Cells};

/// What the corners of a rectangle count.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// WORLD sectors (the default).
    #[default]
    Sector,
    /// WORLD clicks: quarter sectors, 256 grid units.
    Click,
}

/// The inclusive rectangle of world clicks a request means.
pub fn click_rect(cells: &Cells, unit: Unit) -> Result<([i32; 2], [i32; 2]), String> {
    let (a, b) = (cells.from, cells.to.unwrap_or(cells.from));
    let (lo, hi) = (
        [a[0].min(b[0]), a[1].min(b[1])],
        [a[0].max(b[0]), a[1].max(b[1])],
    );
    match unit {
        Unit::Sector => {
            check_rect(&cells.rect())?;
            Ok(([lo[0] * 4, lo[1] * 4], [hi[0] * 4 + 3, hi[1] * 4 + 3]))
        }
        Unit::Click => {
            let count = (hi[0] - lo[0] + 1) as i64 * (hi[1] - lo[1] + 1) as i64;
            if count > 65_536 {
                return Err(format!(
                    "that rectangle is {count} clicks; one call handles at most 65536 (256 by 256)"
                ));
            }
            Ok((lo, hi))
        }
    }
}

/// Subdivide cells.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubdivideReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: a rectangle of WORLD sectors (or clicks, see `unit`).
    #[serde(flatten)]
    pub cells: Cells,
    /// What the corners count: `sector` (default) or `click`.
    pub unit: Option<Unit>,
    /// How fine: `one_level` (default: every cell in the region becomes four),
    /// `half` (cells of half a sector) or `quarter` (the finest cells).
    pub size: Option<String>,
}

fn level_of(size: Option<&str>) -> Result<Option<u8>, String> {
    match size {
        None | Some("one_level") | Some("one") => Ok(None),
        Some("half") => Ok(Some(3)),
        Some("quarter") => Ok(Some(4)),
        Some(other) => Err(format!(
            "unknown size {other:?}; use one_level, half or quarter"
        )),
    }
}

fn touching(room: &GridRoom, lo: [i32; 2], hi: [i32; 2]) -> Vec<LeafKey> {
    let mut keys = std::collections::BTreeSet::new();
    for z in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            if let Some((key, _)) = room.leaf_at([x, z]) {
                keys.insert(key);
            }
        }
    }
    keys.into_iter().collect()
}

/// The census of a room's leaves for a reply.
pub fn census(room: &GridRoom) -> String {
    let mut by = [0usize; 5];
    for (key, _) in room.leaves() {
        by[usize::from(key.level.min(4))] += 1;
    }
    let names = [
        "4 sectors",
        "2 sectors",
        "1 sector",
        "1/2 sector",
        "1/4 sector",
    ];
    let parts: Vec<String> = (0..5)
        .filter(|i| by[*i] > 0)
        .map(|i| format!("{} of {}", by[i], names[i]))
        .collect();
    format!("{} leaves: {}", room.leaf_count(), parts.join(", "))
}

/// Apply [`SubdivideReq`].
pub fn subdivide(world: &mut GridWorld, req: &SubdivideReq) -> Result<String, String> {
    let index = pick_room(world, req.room)?;
    let (lo, hi) = click_rect(&req.cells, req.unit.unwrap_or_default())?;
    let level = level_of(req.size.as_deref())?;
    let room = &mut world.rooms[index];
    let before = room.leaf_count();
    let keys = touching(room, lo, hi);
    if keys.is_empty() {
        return Err("no cell lies in that rectangle".to_string());
    }
    let mut did = 0;
    match level {
        None => {
            for key in keys {
                if room.leaf(key).is_some() && tree::subdivide(room, key).is_ok() {
                    did += 1;
                }
            }
        }
        Some(level) => {
            for z in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let n = room.leaf_count();
                    if tree::subdivide_to(room, [x, z], level).is_ok() && room.leaf_count() != n {
                        did += 1;
                    }
                }
            }
        }
    }
    let after = room.leaf_count();
    let mut out = format!(
        "room {index}: subdivided {did} cell(s); {before} -> {after} leaves ({} added, {} of them by balancing: neighbours more than one level coarser are split too).\n{}",
        after as i64 - before as i64,
        (after as i64 - before as i64 - 3 * did as i64).max(0),
        census(room)
    );
    let issues = check(room);
    if !issues.is_empty() {
        let _ = write!(out, "\nTREE ISSUES: {issues:?}");
    }
    if did == 0 {
        out.push_str("\nnothing to do: the cells there are already as fine as asked");
    }
    Ok(out)
}

/// Merge cells.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergeReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The region: a rectangle of WORLD sectors (or clicks, see `unit`).
    #[serde(flatten)]
    pub cells: Cells,
    /// What the corners count: `sector` (default) or `click`.
    pub unit: Option<Unit>,
    /// The coarsest cell a merge may make: `sector` (default), `two` or
    /// `four` sectors across.
    pub up_to: Option<String>,
}

/// Apply [`MergeReq`].
pub fn merge(world: &mut GridWorld, req: &MergeReq) -> Result<String, String> {
    let index = pick_room(world, req.room)?;
    let (lo, hi) = click_rect(&req.cells, req.unit.unwrap_or_default())?;
    let top = match req.up_to.as_deref() {
        None | Some("sector") => 2,
        Some("two") | Some("2") => 1,
        Some("four") | Some("4") => 0,
        Some(other) => return Err(format!("unknown up_to {other:?}; use sector, two or four")),
    };
    let room = &mut world.rooms[index];
    let before = room.leaf_count();
    let keys = touching(room, lo, hi);
    let merged = tree::merge_all(room, Some(&keys), top);
    let after = room.leaf_count();
    let mut out = format!(
        "room {index}: {merged} merge(s); {before} -> {after} leaves.\n{}",
        census(room)
    );
    if merged == 0 {
        let reasons: Vec<String> = keys
            .iter()
            .filter_map(|k| k.parent().filter(|p| p.level >= top))
            .filter_map(|p| tree::can_merge(room, p).err().map(|e| (p, e)))
            .take(5)
            .map(|(p, e)| {
                format!(
                    "the cell of level {} at clicks ({}, {}): {}",
                    p.level,
                    p.x,
                    p.z,
                    e.describe()
                )
            })
            .collect();
        if reasons.is_empty() {
            out.push_str("\nnothing to merge there");
        } else {
            let _ = write!(out, "\nnothing merged:\n  {}", reasons.join("\n  "));
        }
    }
    Ok(out)
}

/// Read the cells of a room.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LeavesReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The rectangle of WORLD sectors to map. Omit for the whole room (up to
    /// 32 sectors across).
    #[serde(flatten)]
    pub cells: Option<Cells>,
}

/// The cells of a room as text: how many of each size, then a map with one
/// character per click (a quarter sector) giving the level of the cell
/// there, `0` four sectors across, `1` two, `2` one sector, `3` half, `4`
/// a quarter, `.` none.
pub fn leaves(world: &GridWorld, req: &LeavesReq) -> Result<String, String> {
    let index = pick_room(world, req.room)?;
    let room = &world.rooms[index];
    let (lo, hi) = match &req.cells {
        Some(c) => {
            let r = c.rect();
            check_rect(&r)?;
            (r.min, r.max)
        }
        None => (
            room.origin,
            [
                room.origin[0] + room.width as i32 - 1,
                room.origin[1] + room.depth as i32 - 1,
            ],
        ),
    };
    if hi[0] - lo[0] + 1 > 32 {
        return Err(format!(
            "the map would be {} sectors wide; ask for 32 or fewer (give `from`/`to`)",
            hi[0] - lo[0] + 1
        ));
    }
    let mut out = format!("room {index}: {}\n", census(room));
    let issues = check(room);
    if issues.is_empty() {
        out.push_str("tree: balanced, big cells planar, hanging corners constrained\n");
    } else {
        let _ = writeln!(out, "TREE ISSUES: {issues:?}");
    }
    let _ = write!(out, "\n{:>6} ", "z\\x");
    for x in lo[0]..=hi[0] {
        let _ = write!(out, "{x:<4}");
    }
    out.push('\n');
    for z in lo[1]..=hi[1] {
        for sub in 0..4 {
            if sub == 0 {
                let _ = write!(out, "{z:>6} ");
            } else {
                let _ = write!(out, "{:>6} ", "");
            }
            for x in lo[0]..=hi[0] {
                for cx in 0..4 {
                    let click = [x * 4 + cx, z * 4 + sub];
                    out.push(match room.leaf_at(click) {
                        Some((key, _)) => char::from(b'0' + key.level),
                        None => '.',
                    });
                }
            }
            out.push('\n');
        }
    }
    out.push_str(
        "\none character per click (a quarter sector). 0 = a cell 4 sectors across, 1 = 2, 2 = a whole sector, 3 = half a sector, 4 = a quarter. '.' = no cell. Column labels are WORLD sectors.\n",
    );
    Ok(out)
}
