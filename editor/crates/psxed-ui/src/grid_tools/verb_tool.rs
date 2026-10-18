//! The region front end every level verb shares, and the amount drag.
//!
//! Interaction is region, then verb, then amount: pick sectors with a
//! marquee, a brush or a path; the region stays; press on it and drag up or
//! down to set the amount (a click of 256 units, Alt for 64); release to
//! commit; Esc puts everything back exactly. Pressing outside the region
//! starts a new one (Shift adds to it).

use super::cell_size::{cell_at, cell_units};
use super::*;

use psxed_project::grid::edit::verbs::fine::{FineDrag, FineReport, FineVerb};
use psxed_project::grid::edit::verbs::raise::{
    default_mode, height_under, RaiseDrag, RaiseMode, RaiseParams,
};
use psxed_project::grid::edit::verbs::ramp::RampDrag;
use psxed_project::grid::edit::{CellRect, Region};
use psxed_project::grid::leaf::edge_clicks;
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{GridWorld, CLICK_UNITS, FINE_UNITS};

use super::verb_state::{RegionGesture, RegionSource, VerbDrag, VerbGesture};

/// Largest brush radius and path width the `[` and `]` keys reach.
const MAX_BRUSH: i32 = 6;

/// The room and the cell under the pointer, in cells of the cell size the
/// tools use now.
pub(super) fn cell_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(usize, [i32; 2])> {
    let world = ws.project.active_scene().world.grid()?;
    let level = ws.grid.cell_size.level();
    match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            let room = ws
                .top_room_at(cell_at(world.scale, x, z, 2))
                .unwrap_or_else(|| ws.grid_target_room());
            Some((room, cell_at(world.scale, x, z, level)))
        }
        GridView::Persp(rect) => {
            if let Some(hit) = ws.pick_face_3d(rect, ptr.pos) {
                if level == 2 {
                    return Some((hit.face.room, hit.face.cell()));
                }
                let [x, _, z] = hit.point;
                return Some((hit.face.room, cell_at(world.scale, x, z, level)));
            }
            let room = ws.grid_target_room();
            let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
            let y = super::view::editor_y(world, y_offset, 0);
            let p = ws.grid_plane_point(ptr, y)?;
            Some((room, cell_at(world.scale, p[0], p[2], level)))
        }
    }
}

/// Whether the verb acts through the library's fine verbs (the lens) rather
/// than the sector code: the MCP's rule. Raise goes fine only for cells
/// smaller than a sector; the others also for a room with leaves of several
/// sizes.
pub(super) fn through_lens(ws: &EditorWorkspace, region: &Region) -> bool {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    match ws.grid.tool {
        GridTool::Raise => region.level() != 2,
        _ => region.is_fine(world),
    }
}

/// Room-relative height of the floor under `cell` of `level`: the sector's
/// height, or the leaf's at a smaller size (its corners' average, snapped).
pub(super) fn floor_height_at(world: &GridWorld, room: usize, cell: [i32; 2], level: u8) -> i32 {
    if level == 2 {
        return height_under(world, room, cell).unwrap_or(0);
    }
    let e = edge_clicks(level);
    world
        .rooms
        .get(room)
        .and_then(|r| r.leaf_at([cell[0] * e, cell[1] * e]))
        .and_then(|(_, s)| s.ground_floor())
        .map_or(0, |f| {
            snap_to(f.heights.iter().sum::<i32>() / 4, FINE_UNITS)
        })
}

/// The drag the active verb makes over `region`, pressed on `cell`: the
/// sector drag, or the fine drag through the lens (see [`through_lens`]). A
/// Ramp also returns the largest rise it can walk; a Raise sets the Level
/// reference the drag measures from. `None` when there is nothing to do.
pub(super) fn verb_drag_over(
    ws: &mut EditorWorkspace,
    region: &Region,
    cell: [i32; 2],
) -> Option<(VerbDrag, Option<i32>)> {
    let fine = through_lens(ws, region);
    let level = region.level();
    let cells: Vec<[i32; 2]> = region.cells().collect();
    let world = ws.project.active_scene().world.grid()?;
    // (drag, largest walkable rise, Level reference to remember)
    let (drag, max, reference) = match ws.grid.tool {
        GridTool::Raise if fine => {
            let reference = floor_height_at(world, region.room, cell, level);
            let params = RaiseParams {
                mode: ws.grid.verb.raise_mode.unwrap_or(RaiseMode::Offset),
                follow_ceiling: ws.grid.verb.follow_ceiling,
                wall_material: ws.grid.material,
                reference,
            };
            let drag = FineDrag::begin(world, region.room, level, &cells, FineVerb::Raise(params))?;
            (VerbDrag::Fine(Box::new(drag)), None, Some(reference))
        }
        GridTool::Raise => {
            let (drag, reference) = raise_drag(ws, region, cell)?;
            (VerbDrag::Raise(drag), None, Some(reference))
        }
        GridTool::Ramp if fine => {
            let params = super::ramp_tool::params_at(ws, region, cell);
            let drag = FineDrag::begin(world, region.room, level, &cells, FineVerb::Ramp(params))?;
            let max = drag.max_walkable_rise(world);
            (VerbDrag::Fine(Box::new(drag)), max, None)
        }
        GridTool::Ramp => {
            let params = super::ramp_tool::params_at(ws, region, cell);
            let drag = RampDrag::begin(world, region, params)?;
            let max = drag.max_walkable_rise();
            (VerbDrag::Ramp(drag), Some(max), None)
        }
        GridTool::Stairs => (super::stairs_tool::drag_for(ws)?, None, None),
        _ => return None,
    };
    if let Some(reference) = reference {
        ws.grid.verb.reference = reference;
    }
    Some((drag, max))
}

/// Switching to a verb: adopt the Select tool's cells as the region when
/// there is none.
pub(super) fn on_tool_changed(ws: &mut EditorWorkspace) {
    ws.grid.verb.path.clear();
    ws.grid.verb.lasso.clear();
    ws.grid.verb.lasso_cursor = None;
    ws.grid.verb.preview.clear();
    if ws.grid.tool == GridTool::Stairs {
        // A flight is a path: the other sources make no order of treads.
        ws.grid.verb.source = RegionSource::Path;
    }
    // The selection is sectors: it only becomes a region at sector size.
    if !ws.grid.tool.is_verb() || ws.grid.verb.region.is_some() || ws.grid.cell_size.level() != 2 {
        return;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let cells = ws.grid.selection.cells();
    let Some(&(room, _)) = cells.iter().next() else {
        return;
    };
    let region = Region::from_cells(
        room,
        cells
            .iter()
            .filter(|(r, cell)| {
                *r == room
                    && world.rooms.get(room).is_some_and(|ro| {
                        ro.local(*cell)
                            .and_then(|[x, z]| ro.sector(x, z))
                            .is_some_and(|s| s.ground_floor().is_some())
                    })
            })
            .map(|(_, cell)| *cell),
    );
    if !region.is_empty() {
        ws.status = format!("Region: {} sectors from the selection", region.len());
        ws.grid.verb.region = Some(region);
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = GridHover::None;
    let Some((room, cell)) = cell_under(ws, ptr) else {
        ws.grid.verb.preview.clear();
        return;
    };
    let radius = ws.grid.verb.brush_radius;
    ws.grid.verb.preview = match ws.grid.verb.source {
        RegionSource::Brush => Region::brush(room, cell, radius).cells().collect(),
        RegionSource::Marquee | RegionSource::Path | RegionSource::Lasso => vec![cell],
    };
    super::lasso::hover(ws, ptr);
    super::ramp_tool::hover(ws, cell);
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((room, cell)) = cell_under(ws, &ptr.at_press()) else {
        return;
    };
    let verb = &ws.grid.verb;
    let inside = verb.path.is_empty()
        && verb.lasso.is_empty()
        && verb
            .region
            .as_ref()
            .is_some_and(|r| r.room == room && r.contains(cell));
    if inside && !ptr.shift() && begin_amount(ws, ptr, room, cell) {
        return;
    }
    let add = ptr.shift();
    match ws.grid.verb.source {
        RegionSource::Marquee => {
            ws.grid.gesture = Gesture::Region(RegionGesture::Marquee {
                room,
                anchor: cell,
                current: cell,
                add,
            });
        }
        RegionSource::Brush => {
            let mut cells = base_region(ws, room, add);
            cells.extend(&Region::brush(room, cell, ws.grid.verb.brush_radius));
            ws.grid.verb.region = Some(cells.clone());
            ws.grid.gesture = Gesture::Region(RegionGesture::Brush { room, cells });
        }
        RegionSource::Path | RegionSource::Lasso => {}
    }
}

/// The region a new pick starts from: the old one when adding to it, else
/// an empty one at the cell size the tools use now.
fn base_region(ws: &EditorWorkspace, room: usize, add: bool) -> Region {
    match (&ws.grid.verb.region, add) {
        (Some(r), true) if r.room == room => r.clone(),
        _ => Region::new_at(room, ws.grid.cell_size.level()),
    }
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    match &ws.grid.gesture {
        Gesture::Region(RegionGesture::Marquee { .. }) => {
            if let Some((_, cell)) = cell_under(ws, ptr) {
                if let Gesture::Region(RegionGesture::Marquee { current, .. }) =
                    &mut ws.grid.gesture
                {
                    *current = cell;
                }
            }
        }
        Gesture::Region(RegionGesture::Brush { .. }) => {
            if let Some((room, cell)) = cell_under(ws, ptr) {
                let radius = ws.grid.verb.brush_radius;
                if let Gesture::Region(RegionGesture::Brush { room: r, cells, .. }) =
                    &mut ws.grid.gesture
                {
                    if *r == room {
                        cells.extend(&Region::brush(room, cell, radius));
                        ws.grid.verb.region = Some(cells.clone());
                    }
                }
            }
        }
        Gesture::Verb(_) => update_amount(ws, ptr),
        _ => {}
    }
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Region(RegionGesture::Marquee {
            room,
            anchor,
            current,
            add,
        }) => {
            let mut region = base_region(ws, room, add);
            region.extend(&Region::from_rect(
                room,
                CellRect::from_corners(anchor, current),
            ));
            describe_region(ws, &region);
            ws.grid.verb.region = Some(region);
        }
        Gesture::Region(RegionGesture::Brush { cells, .. }) => {
            describe_region(ws, &cells);
            ws.grid.verb.region = Some(cells);
        }
        Gesture::Verb(g) => {
            // The amount stays in the status line; the region stays too.
            let _ = g;
        }
        other => ws.grid.gesture = other,
    }
}

/// A click without a drag: one sector (or one brush disc) as the region, or
/// a waypoint of a path.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((room, cell)) = cell_under(ws, ptr) else {
        return;
    };
    let add = ptr.shift();
    if ws.grid.tool == GridTool::Slab && !add && super::slab_tool::click(ws, room, cell) {
        return;
    }
    match ws.grid.verb.source {
        RegionSource::Path => {
            // A path belongs to one room.
            if ws.grid.verb.region.as_ref().is_some_and(|r| r.room != room)
                && ws.grid.verb.path.is_empty()
            {
                ws.grid.verb.region = None;
            }
            if ws.grid.verb.path.last() != Some(&cell) {
                ws.grid.verb.path.push(cell);
            }
            let region = Region::from_path(room, &ws.grid.verb.path, ws.grid.verb.path_width)
                .at_level(ws.grid.cell_size.level());
            ws.status = format!(
                "Path: {} waypoints, {} sectors. Enter finishes",
                ws.grid.verb.path.len(),
                region.len()
            );
            ws.grid.verb.region = Some(region);
        }
        RegionSource::Lasso => {
            if ws.grid.cell_size.level() == 2 {
                super::lasso::click(ws, ptr);
            } else {
                ws.status = "The lasso draws whole sectors: pick Sector for it".to_string();
            }
        }
        RegionSource::Marquee | RegionSource::Brush => {
            let mut region = base_region(ws, room, add);
            let radius = match ws.grid.verb.source {
                RegionSource::Brush => ws.grid.verb.brush_radius,
                _ => 0,
            };
            region.extend(&Region::brush(room, cell, radius));
            describe_region(ws, &region);
            ws.grid.verb.region = Some(region);
        }
    }
}

/// Enter during a Break or Rough drag: commit it where it is (the release
/// that follows does nothing more). Returns whether there was one.
fn commit_live(ws: &mut EditorWorkspace) -> bool {
    let live = matches!(
        &ws.grid.gesture,
        Gesture::Verb(g) if matches!(g.drag, VerbDrag::Break(_) | VerbDrag::Rough(_))
    );
    if live {
        ws.grid.gesture = Gesture::Idle;
        ws.status = format!("{} (committed)", ws.status);
    }
    live
}

/// Enter: the path in progress is finished; the next click starts a new one.
pub(super) fn finish_path(ws: &mut EditorWorkspace) -> bool {
    if commit_live(ws) {
        return true;
    }
    if super::lasso::close(ws) {
        return true;
    }
    if ws.grid.verb.path.is_empty() {
        return false;
    }
    ws.grid.verb.path.clear();
    if let Some(region) = &ws.grid.verb.region {
        ws.status = format!(
            "Region: {} sectors. Press on it and drag up or down",
            region.len()
        );
    }
    true
}

/// Esc: put an in-flight verb back, else drop the pick in progress, else
/// forget the region. Returns whether anything was cancelled.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Verb(g) => {
            match g.drag {
                // These keep the document they began from: put it back
                // as it was, byte for byte.
                VerbDrag::Break(live) if live.recorded() => {
                    ws.project = *live.start;
                    ws.grid_changed();
                }
                VerbDrag::Rough(live) if live.recorded() => {
                    ws.project = *live.start;
                    ws.grid_changed();
                }
                VerbDrag::Break(_) | VerbDrag::Rough(_) => {}
                // A slab drag records its change at once, whatever the amount.
                VerbDrag::Slab(_) => {
                    if g.before.is_none() {
                        ws.do_undo();
                    }
                }
                // A recorded step means the document changed: take it back.
                _ => {
                    if g.before.is_none() && g.amount != 0 {
                        ws.do_undo();
                    }
                }
            }
            ws.status = "Cancelled".to_string();
            true
        }
        Gesture::Region(_) => {
            ws.status = "Cancelled".to_string();
            true
        }
        other => {
            ws.grid.gesture = other;
            if !ws.grid.tool.is_verb() {
                return false;
            }
            if super::lasso::cancel(ws) {
                return true;
            }
            let had = ws.grid.verb.region.is_some() || !ws.grid.verb.path.is_empty();
            ws.grid.verb.clear();
            if had {
                ws.status = "Region cleared".to_string();
            }
            had
        }
    }
}

fn describe_region(ws: &mut EditorWorkspace, region: &Region) {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let (with_floor, skipped) = region.with_floors(world);
    ws.status = if skipped == 0 {
        format!(
            "Region: {} sectors. Press on it and drag up or down",
            with_floor.len()
        )
    } else {
        format!(
            "Region: {} sectors with a floor, {skipped} without. Press on it and drag up or down",
            with_floor.len()
        )
    };
}

/// Press on the region: start the amount drag of the active verb.
fn begin_amount(ws: &mut EditorWorkspace, ptr: &GridPointer, room: usize, cell: [i32; 2]) -> bool {
    match ws.grid.tool {
        GridTool::Raise => begin_raise(ws, ptr, room, cell),
        GridTool::Ramp => super::ramp_tool::begin(ws, ptr, room, cell),
        GridTool::Stairs => super::stairs_tool::begin(ws, ptr, room, cell),
        GridTool::Break => super::break_tool::begin(ws, ptr, room, cell),
        GridTool::Rough => super::rough_tool::begin(ws, ptr, room, cell),
        GridTool::Slab => super::slab_tool::begin(ws, ptr, room, cell),
        _ => false,
    }
}

/// Pixels per grid unit for an amount drag grabbed on `cell`.
pub(super) fn drag_rate(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
    height: i32,
) -> f32 {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return 0.1;
    };
    let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
    let unit = cell_units(ws.grid.cell_size.level());
    let centre = |c: i32| world.scale.to_editor(c * unit + unit / 2) as f32;
    let grab = [
        centre(cell[0]),
        super::view::editor_y(world, y_offset, height),
        centre(cell[1]),
    ];
    ws.grid_pixels_per_unit(&ptr.at_press(), grab)
}

/// A sector Raise drag on `region` with the current options, and the Level
/// reference it measures from (the height of `cell`).
fn raise_drag(ws: &EditorWorkspace, region: &Region, cell: [i32; 2]) -> Option<(RaiseDrag, i32)> {
    let world = ws.project.active_scene().world.grid()?;
    let reference = height_under(world, region.room, cell).unwrap_or(0);
    let mode = ws
        .grid
        .verb
        .raise_mode
        .unwrap_or_else(|| default_mode(world, region));
    let params = RaiseParams {
        mode,
        follow_ceiling: ws.grid.verb.follow_ceiling,
        wall_material: ws.grid.material,
        reference,
    };
    RaiseDrag::begin(world, region, params).map(|d| (d, reference))
}

fn begin_raise(ws: &mut EditorWorkspace, ptr: &GridPointer, room: usize, cell: [i32; 2]) -> bool {
    let Some(region) = ws.grid.verb.region.clone() else {
        return false;
    };
    let Some((drag, _)) = verb_drag_over(ws, &region, cell) else {
        ws.status = "Nothing to raise: no sector of the region has a floor".to_string();
        return false;
    };
    let ppu = drag_rate(ws, ptr, room, cell, ws.grid.verb.reference);
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag,
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: ppu,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
    }));
    true
}

/// A scripted drag in flight at `amount` grid units (poses, tests).
pub(crate) fn pose_drag(ws: &mut EditorWorkspace, amount: i32) -> Result<(), String> {
    let region = ws
        .grid
        .verb
        .region
        .clone()
        .ok_or("no region: use region=x0,z0:x1,z1 first")?;
    let first = region.cells().next().unwrap_or([0, 0]);
    let (drag, max_rise) = match ws.grid.tool {
        GridTool::Raise | GridTool::Ramp => {
            verb_drag_over(ws, &region, first).ok_or("nothing to raise or ramp")?
        }
        GridTool::Stairs => (
            super::stairs_tool::drag_for(ws).ok_or("no path: use stairs-path=")?,
            None,
        ),
        GridTool::Slab => return super::slab_tool::pose_drag(ws, amount),
        other => return Err(format!("the {} tool has no drag to pose", other.label())),
    };
    let mut gesture = VerbGesture {
        drag,
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount,
        before: Some(Box::new(ws.project.clone())),
        max_rise,
    };
    apply_amount(ws, &mut gesture);
    ws.grid.gesture = Gesture::Verb(Box::new(gesture));
    Ok(())
}

/// The amount the pointer is at, snapped to a click (64 with Alt; heights
/// stay on that lattice even with Shift+Alt).
pub(super) fn snapped_amount(g: &VerbGesture, ptr: &GridPointer) -> i32 {
    let step = if ptr.alt() { FINE_UNITS } else { CLICK_UNITS };
    let raw = ((g.press_y - ptr.pos.y) / g.pixels_per_unit.max(1e-4)).round() as i32;
    snap_to(raw, step)
}

fn update_amount(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Verb(mut g) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    let mut amount = match &g.drag {
        VerbDrag::Break(_) => super::break_tool::intensity_at(&g, ptr),
        VerbDrag::Rough(live) => super::rough_tool::level_at(&g, live, ptr),
        _ => snapped_amount(&g, ptr),
    };
    if is_ramp(&g.drag) {
        amount = super::ramp_tool::clamp(ws, amount, g.max_rise);
    }
    if amount != g.amount {
        g.amount = amount;
        apply_amount(ws, &mut g);
    }
    ws.grid.gesture = Gesture::Verb(g);
}

/// Apply the gesture's amount to the live document, recording the undo step
/// at the first change.
pub(super) fn apply_amount(ws: &mut EditorWorkspace, g: &mut VerbGesture) {
    if let Some(before) = g.before.take() {
        ws.history.record(*before);
    }
    // Break and Rough keep the document they started from (for the exact
    // cancel) and record it for undo at their first change.
    let first = match &mut g.drag {
        VerbDrag::Break(live) => live.take_record(),
        VerbDrag::Rough(live) => live.take_record(),
        _ => None,
    };
    if let Some(document) = first {
        ws.history.record(document);
    }
    if is_ramp(&g.drag) {
        g.amount = super::ramp_tool::clamp(ws, g.amount, g.max_rise);
    }
    let size = ws.grid.cell_size.label();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let line = match &mut g.drag {
        VerbDrag::Raise(drag) => {
            let applied = drag.apply(world, g.amount);
            if g.amount != 0 {
                ws.grid.verb.last_raise = g.amount.abs();
            }
            let mode = drag.params().mode.label();
            format!(
                "{} {}{}",
                if g.amount < 0 { "Lower" } else { "Raise" },
                amount_text(world, g.amount),
                counts_text(&applied, mode)
            )
        }
        VerbDrag::Ramp(drag) => {
            let max = g.max_rise.unwrap_or(0);
            let applied = drag.apply(world, g.amount);
            let mut line = super::ramp_tool::readout(world, drag, g.amount, max);
            if applied.skipped > 0 {
                line += &format!(", {} without a floor skipped", applied.skipped);
            }
            line
        }
        VerbDrag::Stairs(drag) => {
            let flight = drag.apply(world, g.amount);
            super::stairs_tool::readout(world, &flight, g.amount)
        }
        VerbDrag::Fine(drag) => {
            let report = drag.apply(world, g.amount);
            match (drag.verb(), report) {
                (FineVerb::Raise(params), FineReport::Applied(applied)) => {
                    if g.amount != 0 {
                        ws.grid.verb.last_raise = g.amount.abs();
                    }
                    format!(
                        "{} {}{}",
                        if g.amount < 0 { "Lower" } else { "Raise" },
                        amount_text(world, g.amount),
                        counts_text(&applied, params.mode.label())
                    )
                }
                (FineVerb::Ramp(params), FineReport::Applied(applied)) => {
                    let walk = if g.amount.abs() <= g.max_rise.unwrap_or(0) {
                        "walkable"
                    } else {
                        "too steep to stand on"
                    };
                    format!(
                        "Ramp {} from the {} edge over {} {size} cell(s), {walk}",
                        amount_text(world, g.amount),
                        params.from.label(),
                        applied.moved,
                    )
                }
                (FineVerb::Stairs { .. }, FineReport::Flight(flight)) => {
                    super::stairs_tool::readout(world, &flight, g.amount)
                }
                (_, FineReport::Refused(why)) => why,
                _ => "Nothing to change there".to_string(),
            }
        }
        VerbDrag::Break(live) => {
            let (line, detail) = super::break_tool::apply(world, live, g.amount);
            ws.grid.verb.readout = detail;
            ws.status = line;
            ws.grid_changed();
            return;
        }
        VerbDrag::Rough(live) => super::rough_tool::apply(world, live, g.amount),
        VerbDrag::Slab(live) => super::slab_tool::apply(world, live, g.amount),
    };
    ws.grid.verb.readout = line.clone();
    ws.status = line;
    ws.grid_changed();
}

/// Whether a drag slopes a region (the sector Ramp or a fine one).
fn is_ramp(drag: &VerbDrag) -> bool {
    match drag {
        VerbDrag::Ramp(_) => true,
        VerbDrag::Fine(d) => matches!(d.verb(), FineVerb::Ramp(_)),
        _ => false,
    }
}

/// The Raise mode of a Raise drag, sector or fine.
fn raise_mode_of(drag: &VerbDrag) -> Option<RaiseMode> {
    match drag {
        VerbDrag::Raise(d) => Some(d.params().mode),
        VerbDrag::Fine(d) => match d.verb() {
            FineVerb::Raise(params) => Some(params.mode),
            _ => None,
        },
        _ => None,
    }
}

/// Set the Raise mode of a Raise drag in flight.
fn set_raise_mode(drag: &mut VerbDrag, mode: RaiseMode) {
    match drag {
        VerbDrag::Raise(d) => d.set_mode(mode),
        VerbDrag::Fine(d) => {
            if let FineVerb::Raise(params) = d.verb() {
                let params = RaiseParams { mode, ..*params };
                d.set_verb(FineVerb::Raise(params));
            }
        }
        _ => {}
    }
}

/// "+2.00 clicks (+512 gu, +40 ru)".
pub(super) fn amount_text(world: &GridWorld, amount: i32) -> String {
    let ru = psxed_project::grid::cook::RuntimeUnits::new(world.scale)
        .map(|u| format!(", {:+} ru", u.rt(amount)))
        .unwrap_or_default();
    format!(
        "{:+.2} clicks ({amount:+} gu{ru})",
        amount as f32 / CLICK_UNITS as f32
    )
}

fn counts_text(applied: &psxed_project::grid::edit::verbs::Applied, mode: &str) -> String {
    let mut text = format!(", {} sectors, {mode}", applied.moved);
    if applied.skipped > 0 {
        text += &format!(", {} without a floor skipped", applied.skipped);
    }
    if applied.clamped > 0 {
        text += &format!(", {} held a click under the ceiling", applied.clamped);
    }
    text
}

/// Tab: the other Raise mode, live when a drag is in flight.
pub(super) fn toggle_mode(ws: &mut EditorWorkspace) {
    if ws.grid.tool == GridTool::Break {
        super::break_tool::cycle_targets(ws);
        return;
    }
    if ws.grid.tool == GridTool::Stairs {
        super::stairs_tool::toggle_ramp(ws);
        return;
    }
    if ws.grid.tool == GridTool::Slab {
        super::slab_tool::cycle_mode(ws);
        return;
    }
    if ws.grid.tool == GridTool::Ramp {
        ws.grid.verb.ramp_clamp = !ws.grid.verb.ramp_clamp;
        ws.status = if ws.grid.verb.ramp_clamp {
            "Ramp held to the steepest walkable slope".to_string()
        } else {
            "Ramp free: it may be too steep to stand on".to_string()
        };
        return;
    }
    if ws.grid.tool != GridTool::Raise {
        return;
    }
    let mut live = match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Verb(g) => Some(g),
        other => {
            ws.grid.gesture = other;
            None
        }
    };
    let in_flight = live.as_deref().and_then(|g| raise_mode_of(&g.drag));
    let current = in_flight.unwrap_or_else(|| {
        let world = ws.project.active_scene().world.grid();
        ws.grid
            .verb
            .raise_mode
            .unwrap_or_else(|| match (world, &ws.grid.verb.region) {
                (Some(w), Some(r)) if r.level() == 2 => default_mode(w, r),
                _ => RaiseMode::Offset,
            })
    });
    let next = current.toggled();
    ws.grid.verb.raise_mode = Some(next);
    if let Some(g) = live.as_mut() {
        set_raise_mode(&mut g.drag, next);
        apply_amount(ws, g);
    }
    if let Some(g) = live {
        ws.grid.gesture = Gesture::Verb(g);
    }
    ws.status = format!("Raise mode: {}", next.label());
}

/// `[` and `]` in a verb: the brush radius or the path width.
pub(super) fn adjust_size(ws: &mut EditorWorkspace, delta: i32) {
    let verb = &mut ws.grid.verb;
    match verb.source {
        RegionSource::Brush => {
            verb.brush_radius = (verb.brush_radius + delta).clamp(0, MAX_BRUSH);
            ws.status = format!("Brush radius {}", verb.brush_radius);
        }
        RegionSource::Path => {
            verb.path_width = (verb.path_width + delta).clamp(1, MAX_BRUSH * 2 + 1);
            ws.status = format!("Path width {}", verb.path_width);
            if let Some(region) = verb.region.as_ref().filter(|_| !verb.path.is_empty()) {
                verb.region = Some(
                    Region::from_path(region.room, &verb.path, verb.path_width)
                        .at_level(region.level()),
                );
            }
        }
        RegionSource::Marquee | RegionSource::Lasso => {}
    }
}

/// The typed amount of the option row, applied to the region in one step.
pub(crate) fn apply_typed(ws: &mut EditorWorkspace) {
    match ws.grid.tool {
        GridTool::Break => return super::break_tool::apply_typed(ws),
        GridTool::Rough => return super::rough_tool::apply_typed(ws),
        GridTool::Slab => return super::slab_tool::apply_typed(ws),
        _ => {}
    }
    let amount = snap_to(
        (ws.grid.verb.typed_clicks * CLICK_UNITS as f32).round() as i32,
        FINE_UNITS,
    );
    let Some(region) = ws.grid.verb.region.clone() else {
        ws.status = "Pick a region first".to_string();
        return;
    };
    let first = region.cells().next().unwrap_or([0, 0]);
    let drag = match ws.grid.tool {
        GridTool::Raise | GridTool::Ramp => verb_drag_over(ws, &region, first),
        GridTool::Stairs => super::stairs_tool::drag_for(ws).map(|d| (d, None)),
        _ => None,
    };
    let Some((drag, max_rise)) = drag else {
        ws.status = "Nothing to apply there: no sector of the region has a floor".to_string();
        return;
    };
    let mut gesture = VerbGesture {
        drag,
        press_y: 0.0,
        pixels_per_unit: 1.0,
        amount,
        before: Some(Box::new(ws.project.clone())),
        max_rise,
    };
    apply_amount(ws, &mut gesture);
}

/// The Cut corners chip: crack the outer triangle of every convex corner of
/// the region down by the last raise (a click when there has been none), one
/// undo step.
pub(crate) fn cut_corners(ws: &mut EditorWorkspace) {
    use psxed_project::grid::edit::outline::{cut_corners, CornerCut};
    let Some(region) = ws.grid.verb.region.clone() else {
        ws.status = "Pick a region first".to_string();
        return;
    };
    if region.level() != 2 {
        ws.status = "Cut corners works on whole sectors: pick Sector for it".to_string();
        return;
    }
    let drop = match ws.grid.verb.last_raise {
        0 => CLICK_UNITS,
        last => last,
    };
    let cells: Vec<[i32; 2]> = region.cells().collect();
    let footprint: std::collections::BTreeSet<[i32; 2]> = cells.iter().copied().collect();
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let report = cut_corners(
        world,
        region.room,
        &cells,
        Some(&footprint),
        CornerCut::Crack { drop },
    );
    if report.cut == 0 {
        ws.status = if report.skipped > 0 {
            format!(
                "No corner cut: {} already hold another diagonal",
                report.skipped
            )
        } else {
            "No convex corner in the region".to_string()
        };
        return;
    }
    ws.history.record(before);
    ws.status = format!(
        "Cut {} corners by {} units{}",
        report.cut,
        drop,
        if report.skipped > 0 {
            format!(", {} left alone (another diagonal)", report.skipped)
        } else {
            String::new()
        }
    );
    ws.grid_changed();
}
