//! Storeys as tools: the derived storey list, one column's spans, the slab
//! verbs (insert, cut, fill) and the shift above a height.
//!
//! Thin wrappers over `psxed_project::grid::{storeys, edit::spans}`, the code
//! the editor calls. Heights are grid units relative to the room's offset, as
//! in every grid tool; replies also print clicks. A rectangle is four world
//! sector numbers `[x0, z0, x1, z1]`, inclusive.

use std::fmt::Write as _;

use psxed_project::grid::derive::cell_walls;
use psxed_project::grid::edit::spans::{
    cut_hole, fill_hole, insert_slab, shift_above, CutScope, SpanOutcome, SpanRefusal,
};
use psxed_project::grid::edit::CellRect;
use psxed_project::grid::storeys::{storeys, SLAB_THICKNESS, STOREY_PITCH};
use psxed_project::grid::GridWorld;
use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, clicks, pick_room, Materials};

/// Read the storeys of the world.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StoreysReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
}

/// Read one cell's column.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ColumnReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Omit when the world has one room.
    pub room: Option<usize>,
    /// The world sector `[x, z]`.
    pub cell: [i32; 2],
}

/// Insert or fill a slab over rectangles of cells.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SlabReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Omit when the world has one room.
    pub room: Option<usize>,
    /// Rectangles `[x0, z0, x1, z1]` of world sectors, inclusive.
    pub rects: Vec<[i32; 4]>,
    /// The walking surface of the slab: grid units relative to the room.
    pub y: i32,
    /// Slab thickness in grid units. Default one click (256).
    pub thickness: Option<i32>,
    /// Material of the slab's top. Default: the span's own underside.
    pub material: Option<String>,
}

/// Cut the slab at a height out of rectangles of cells.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CutReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Omit when the world has one room.
    pub room: Option<usize>,
    /// Rectangles `[x0, z0, x1, z1]` of world sectors, inclusive.
    pub rects: Vec<[i32; 4]>,
    /// The height of the slab's walking surface (grid units, room relative).
    pub y: i32,
    /// `one` (default): the slab whose floor is at `y`. `all_above`: every
    /// slab at or above `y`.
    pub which: Option<String>,
}

/// Shift every surface at or above a height.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShiftReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Omit when the world has one room.
    pub room: Option<usize>,
    /// Rectangles `[x0, z0, x1, z1]`. Omit for the whole room.
    pub rects: Option<Vec<[i32; 4]>>,
    /// The height from which everything moves (grid units, room relative).
    pub y: i32,
    /// How far (grid units, positive is up).
    pub delta: i32,
}

/// The rectangles a request names, checked against the size cap.
fn rects_of(rects: &[[i32; 4]]) -> Result<Vec<CellRect>, String> {
    if rects.is_empty() {
        return Err("give at least one rectangle [x0, z0, x1, z1]".to_string());
    }
    rects
        .iter()
        .map(|r| {
            let rect = CellRect::from_corners([r[0], r[1]], [r[2], r[3]]);
            check_rect(&rect)?;
            Ok(rect)
        })
        .collect()
}

/// Add one rectangle's outcome to a running total.
fn add(total: &mut SpanOutcome, part: SpanOutcome) {
    total.changed += part.changed;
    total.skipped.no_span += part.skipped.no_span;
    total.skipped.sloped += part.skipped.sloped;
    total.skipped.headroom += part.skipped.headroom;
    total.skipped.span_cap += part.skipped.span_cap;
    total.skipped.no_slab += part.skipped.no_slab;
    total.refused = total.refused.or(part.refused);
}

/// Turn a refusal into the error the tool returns (nothing changed).
fn refusal_text(reason: SpanRefusal) -> String {
    match reason {
        SpanRefusal::NoSuchRoom => "no such room".to_string(),
        SpanRefusal::MixedCells => {
            "the room has cells of several sizes: the storey verbs work on whole sectors (S1)"
                .to_string()
        }
        SpanRefusal::BadThickness => {
            "the slab thickness must be between 0 and the minimum headroom (768 grid units)"
                .to_string()
        }
        SpanRefusal::Interpenetrates { cell } => format!(
            "the shift would make the column at sector [{}, {}] overlap itself; nothing changed",
            cell[0], cell[1]
        ),
    }
}

/// The skip counts of an outcome as words, for a reply.
fn skips_text(outcome: &SpanOutcome) -> String {
    let s = outcome.skipped;
    if s.total() == 0 {
        return "none".to_string();
    }
    let mut parts = Vec::new();
    if s.no_span > 0 {
        parts.push(format!("{} with no span at that height", s.no_span));
    }
    if s.sloped > 0 {
        parts.push(format!("{} where a sloped surface crosses it", s.sloped));
    }
    if s.headroom > 0 {
        parts.push(format!(
            "{} with less than 3 clicks of headroom",
            s.headroom
        ));
    }
    if s.span_cap > 0 {
        parts.push(format!("{} at the span cap", s.span_cap));
    }
    if s.no_slab > 0 {
        parts.push(format!("{} with no slab at that height", s.no_slab));
    }
    parts.join(", ")
}

/// The storey list as one line for a reply.
fn storey_line(world: &GridWorld) -> String {
    let found = storeys(world);
    if found.is_empty() {
        return "no storeys (a floor height needs 8 cells)".to_string();
    }
    let parts: Vec<String> = found
        .iter()
        .map(|s| format!("{} ({} cells)", clicks(s.height), s.cells))
        .collect();
    format!("storeys at clicks {}", parts.join(", "))
}

/// Every storey of the world, with how many spans each column holds.
pub fn storeys_text(world: &GridWorld) -> String {
    let found = storeys(world);
    let mut out = String::new();
    if found.is_empty() {
        out.push_str("no storeys: no floor height is shared by 8 cells\n");
    }
    for (ordinal, storey) in found.iter().enumerate() {
        let _ = writeln!(
            out,
            "storey {ordinal}: floor at {} clicks ({} grid units), {} cells",
            clicks(storey.height),
            storey.height,
            storey.cells
        );
    }
    let _ = writeln!(
        out,
        "storey pitch {} clicks; slab {} clicks; minimum headroom 3 clicks; column cap 8 spans",
        STOREY_PITCH / 256,
        SLAB_THICKNESS / 256
    );
    for (index, room) in world.rooms.iter().enumerate() {
        let mut by = [0usize; 9];
        for (_, leaf) in room.leaves.iter() {
            by[leaf.span_count().min(8)] += 1;
        }
        let parts: Vec<String> = (1..9)
            .filter(|n| by[*n] > 0)
            .map(|n| format!("{} with {n}", by[n]))
            .collect();
        let _ = writeln!(
            out,
            "room {index}: leaves by span count: {}",
            parts.join(", ")
        );
    }
    out.trim_end().to_string()
}

/// One cell's column as text: each span, the gaps between them and the
/// derived walls the cell hosts.
pub fn column_text(world: &GridWorld, room_index: usize, cell: [i32; 2]) -> Result<String, String> {
    let room = world
        .rooms
        .get(room_index)
        .ok_or_else(|| format!("no room {room_index}"))?;
    let Some([x, z]) = room.local(cell) else {
        return Err(format!(
            "sector [{}, {}] is outside room {room_index}",
            cell[0], cell[1]
        ));
    };
    let Some(sector) = room.sector(x, z) else {
        return Ok(format!(
            "room {room_index}, sector [{}, {}]: no column (empty)",
            cell[0], cell[1]
        ));
    };
    let mut out = format!(
        "room {room_index}, sector [{}, {}]: {} span(s)\n",
        cell[0],
        cell[1],
        sector.span_count()
    );
    let mut below: Option<i32> = None;
    for (index, span) in sector.spans().iter().enumerate() {
        let floor = span.floor.as_ref().map(|f| f.height_range());
        let ceiling = span.ceiling.as_ref().map(|c| c.height_range());
        let show = |range: Option<(i32, i32)>, open: &str| match range {
            Some((lo, hi)) if lo == hi => clicks(lo),
            Some((lo, hi)) => format!("{}..{}", clicks(lo), clicks(hi)),
            None => open.to_string(),
        };
        let gap = match (below, floor) {
            (Some(top), Some((lo, _))) if lo > top => {
                format!(", solid below it {} clicks", clicks(lo - top))
            }
            _ => String::new(),
        };
        let _ = writeln!(
            out,
            "  span {index}: floor {} ceiling {}{gap}",
            show(floor, "none (pit)"),
            show(ceiling, "none (open sky)")
        );
        below = ceiling.map(|(_, hi)| hi);
    }
    let walls = cell_walls(room, cell);
    if walls.is_empty() {
        out.push_str("  derived walls: none\n");
    } else {
        for wall in walls {
            let _ = writeln!(
                out,
                "  derived {} on the {} edge, owner [{}, {}]{}",
                wall.kind.label(),
                wall.edge.label(),
                wall.owner[0],
                wall.owner[1],
                if wall.blocks() {
                    ""
                } else {
                    " (does not block)"
                }
            );
        }
    }
    Ok(out.trim_end().to_string())
}

/// Apply a slab insert or fill over the request's rectangles.
pub fn slab(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &SlabReq,
    fill: bool,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rects = rects_of(&req.rects)?;
    let material = Materials::new(project).resolve_opt(req.material.as_deref())?;
    let thickness = req.thickness.unwrap_or(SLAB_THICKNESS);
    let mut total = SpanOutcome::default();
    for rect in rects {
        let part = if fill {
            fill_hole(world, room, rect, req.y, thickness, material)
        } else {
            insert_slab(world, room, rect, req.y, thickness, material)
        };
        add(&mut total, part);
    }
    if let Some(reason) = total.refused {
        return Err(refusal_text(reason));
    }
    let verb = if fill { "filled" } else { "inserted a slab at" };
    Ok(format!(
        "room {room}: {verb} {} clicks: {} cell(s) changed; skipped: {}\n{}",
        clicks(req.y),
        total.changed,
        skips_text(&total),
        storey_line(world)
    ))
}

/// Apply a cut over the request's rectangles.
pub fn cut(world: &mut GridWorld, req: &CutReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rects = rects_of(&req.rects)?;
    let scope = match req.which.as_deref() {
        None | Some("one") => CutScope::One,
        Some("all_above") => CutScope::AllAbove,
        Some(other) => {
            return Err(format!("unknown which {other:?}; use one or all_above"));
        }
    };
    let mut total = SpanOutcome::default();
    for rect in rects {
        add(&mut total, cut_hole(world, room, rect, req.y, scope));
    }
    if let Some(reason) = total.refused {
        return Err(refusal_text(reason));
    }
    Ok(format!(
        "room {room}: cut the slab at {} clicks: {} cell(s) changed; skipped: {}\n{}",
        clicks(req.y),
        total.changed,
        skips_text(&total),
        storey_line(world)
    ))
}

/// Apply a shift over the request's rectangles, or the whole room.
pub fn shift(world: &mut GridWorld, req: &ShiftReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let mut total = SpanOutcome::default();
    match &req.rects {
        None => add(&mut total, shift_above(world, room, None, req.y, req.delta)),
        Some(rects) => {
            for rect in rects_of(rects)? {
                add(
                    &mut total,
                    shift_above(world, room, Some(rect), req.y, req.delta),
                );
            }
        }
    }
    if let Some(reason) = total.refused {
        return Err(refusal_text(reason));
    }
    Ok(format!(
        "room {room}: moved everything at or above {} clicks by {} clicks: {} cell(s) changed\n{}",
        clicks(req.y),
        clicks(req.delta),
        total.changed,
        storey_line(world)
    ))
}

/// Refuse a rectangle that holds a column of several spans. The surface and
/// level verbs edit the ground span only, and a tool that guessed which span
/// a request meant would silently edit the wrong floor, so it refuses instead.
/// Names the first such cell.
pub fn refuse_several_spans(
    world: &GridWorld,
    room_index: usize,
    rect: CellRect,
) -> Result<(), String> {
    let Some(room) = world.rooms.get(room_index) else {
        return Ok(());
    };
    for cell in rect.cells() {
        let Some([x, z]) = room.local(cell) else {
            continue;
        };
        let Some(sector) = room.sector(x, z) else {
            continue;
        };
        if !sector.is_single_span() {
            return Err(format!(
                "sector [{}, {}] has {} spans; this tool edits only the ground span and refuses rather than guess which one you mean. Read the column with grid_column and edit it with grid_insert_slab, grid_cut_hole or grid_shift_above",
                cell[0],
                cell[1],
                sector.span_count()
            ));
        }
    }
    Ok(())
}
