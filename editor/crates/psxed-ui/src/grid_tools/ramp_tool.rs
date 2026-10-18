//! The Ramp verb in the editor: pick a region, then press on it near the
//! side that should keep its height (the A edge) and drag up or down for how
//! far the opposite side rises.
//!
//! The side nearest the pointer is the A edge, shown lit before the press; R
//! turns the choice a quarter turn and holds it. Tab clamps the amount to the
//! steepest slope the character motor stands on.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::verbs::ramp::{RampDir, RampDrag, RampParams};
use psxed_project::grid::edit::Region;

/// The side of the region's bounding box nearest `cell` (ties break in the
/// order north, east, south, west).
pub(super) fn nearest_side(region: &Region, cell: [i32; 2]) -> Option<RampDir> {
    let b = region.bounds()?;
    let dist = |d: RampDir| match d {
        RampDir::North => cell[1] - b.min[1],
        RampDir::East => b.max[0] - cell[0],
        RampDir::South => b.max[1] - cell[1],
        RampDir::West => cell[0] - b.min[0],
    };
    RampDir::ALL.into_iter().min_by_key(|d| dist(*d))
}

/// Hover: the side a press here would keep (A).
pub(super) fn hover(ws: &mut EditorWorkspace, cell: [i32; 2]) {
    ws.grid.verb.ramp_hover = None;
    if ws.grid.tool != GridTool::Ramp {
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
        .or_else(|| nearest_side(region, cell));
}

/// R: the A edge turns a quarter turn and stays.
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
    ws.status = format!("Ramp keeps its {} edge", next.label());
}

/// The parameters of a ramp pressed at `cell`.
pub(super) fn params_at(ws: &EditorWorkspace, region: &Region, cell: [i32; 2]) -> RampParams {
    RampParams {
        from: ws
            .grid
            .verb
            .ramp_from
            .or_else(|| nearest_side(region, cell))
            .unwrap_or(RampDir::North),
        follow_ceiling: ws.grid.verb.follow_ceiling,
        wall_material: ws.grid.material,
    }
}

/// Press on the region: start the amount drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    room: usize,
    cell: [i32; 2],
) -> bool {
    let Some(region) = ws.grid.verb.region.clone() else {
        return false;
    };
    let from = params_at(ws, &region, cell).from;
    let Some((drag, max)) = super::verb_tool::verb_drag_over(ws, &region, cell) else {
        ws.status = "Nothing to ramp: no sector of the region has a floor".to_string();
        return false;
    };
    let base = match &drag {
        VerbDrag::Ramp(d) => d.base(),
        _ => ws.project.active_scene().world.grid().map_or(0, |w| {
            super::verb_tool::floor_height_at(w, region.room, cell, region.level())
        }),
    };
    ws.grid.verb.ramp_hover = Some(from);
    let ppu = super::verb_tool::drag_rate(ws, ptr, room, cell, base);
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag,
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: ppu,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: max,
    }));
    true
}

/// The amount a Ramp gesture applies: the pointer's, held to the walkable
/// limit when the clamp is on.
pub(super) fn clamp(ws: &EditorWorkspace, amount: i32, max: Option<i32>) -> i32 {
    match max {
        Some(max) if ws.grid.verb.ramp_clamp => amount.clamp(-max, max),
        _ => amount,
    }
}

/// The status line of a ramp: rise, slope and whether it can be walked.
pub(super) fn readout(
    world: &psxed_project::grid::GridWorld,
    drag: &RampDrag,
    amount: i32,
    max: i32,
) -> String {
    let rt = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let depth = drag.depth().max(1);
    let (rise_ru, sector_ru) = rt
        .as_ref()
        .map_or((0, 0), |u| (u.rt(amount), u.sector() as i32));
    let per_sector = rise_ru as f32 / depth as f32;
    let angle = per_sector.atan2(sector_ru as f32).to_degrees();
    let walk = if amount.abs() <= max {
        "walkable"
    } else {
        "too steep to stand on"
    };
    let clicks = |v: i32| v as f32 / psxed_project::grid::CLICK_UNITS as f32;
    format!(
        "Ramp {:+.2} clicks ({rise_ru:+} ru) from the {} edge over {depth} sectors: {per_sector:+.1} ru a sector, {angle:.1} deg, {walk} (steepest {:.2})",
        clicks(amount),
        drag.params().from.label(),
        clicks(max),
    )
}
