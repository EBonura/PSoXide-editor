//! The lasso: a region source for the level verbs. Click the corners of a
//! shape (Shift snaps each new segment to 45 degrees), Enter closes it, and
//! the sectors whose centres lie inside become the region (the same
//! rasterisation `grid_polygon` uses). Esc drops the shape in progress.
//!
//! Points are kept in sector coordinates: an integer is a lattice corner, so
//! sector `(i, j)` has its centre at `(i + 0.5, j + 0.5)`.

use super::verb_state::RegionSource;
use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::outline::polygon_cells;
use psxed_project::grid::edit::Region;
use psxed_project::grid::{GridScale, SECTOR_UNITS};

/// Sector coordinates of editor-space `(x, z)`.
fn sector_coords(scale: GridScale, x: f32, z: f32) -> [f64; 2] {
    let per = f64::from(scale.num) * f64::from(SECTOR_UNITS) / f64::from(scale.den.max(1));
    [f64::from(x) / per, f64::from(z) / per]
}

/// Editor-space `[x, z]` of a point in sector coordinates.
pub(super) fn editor_of(scale: GridScale, p: [f64; 2]) -> [f32; 2] {
    let per = f64::from(scale.num) * f64::from(SECTOR_UNITS) / f64::from(scale.den.max(1));
    [(p[0] * per) as f32, (p[1] * per) as f32]
}

/// The corners of the shape in progress, with the rubber band's end.
pub(super) fn shape(ws: &EditorWorkspace) -> Vec<[f64; 2]> {
    let mut points = ws.grid.verb.lasso.clone();
    points.extend(ws.grid.verb.lasso_cursor);
    points
}

/// The room and the point, in sector coordinates, under the pointer.
fn point_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(usize, [f64; 2])> {
    let world = ws.project.active_scene().world.grid()?;
    match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            let room = ws
                .top_room_at(cell_at_editor(world.scale, x, z))
                .unwrap_or_else(|| ws.grid_target_room());
            Some((room, sector_coords(world.scale, x, z)))
        }
        GridView::Persp(rect) => {
            if let Some(hit) = ws.pick_face_3d(rect, ptr.pos) {
                return Some((
                    hit.face.room,
                    sector_coords(world.scale, hit.point[0], hit.point[2]),
                ));
            }
            let room = ws.grid_target_room();
            let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
            let y = super::view::editor_y(world, y_offset, 0);
            let p = ws.grid_plane_point(ptr, y)?;
            Some((room, sector_coords(world.scale, p[0], p[2])))
        }
    }
}

/// `raw` with the new segment from `last` snapped to the nearest of the
/// eight directions and to whole sectors along it.
pub(crate) fn snap_45(last: [f64; 2], raw: [f64; 2]) -> [f64; 2] {
    let (dx, dz) = (raw[0] - last[0], raw[1] - last[1]);
    let len = dx.hypot(dz);
    if len < 1e-9 {
        return raw;
    }
    let octant = (dz.atan2(dx) / std::f64::consts::FRAC_PI_4).round() as i32;
    let (ux, uz) = match octant.rem_euclid(8) {
        0 => (1.0, 0.0),
        1 => (1.0, 1.0),
        2 => (0.0, 1.0),
        3 => (-1.0, 1.0),
        4 => (-1.0, 0.0),
        5 => (-1.0, -1.0),
        6 => (0.0, -1.0),
        _ => (1.0, -1.0),
    };
    let diagonal = ux != 0.0 && uz != 0.0;
    let steps = if diagonal {
        (len / std::f64::consts::SQRT_2).round()
    } else {
        len.round()
    }
    .max(1.0);
    [last[0] + ux * steps, last[1] + uz * steps]
}

/// The point a click at `ptr` adds: the pointer's, snapped when Shift is
/// held and there is a previous point.
fn next_point(ws: &EditorWorkspace, ptr: &GridPointer, raw: [f64; 2]) -> [f64; 2] {
    match ws.grid.verb.lasso.last() {
        Some(&last) if ptr.shift() => snap_45(last, raw),
        _ => raw,
    }
}

/// Hover: where the next point would land, for the rubber band.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.verb.lasso_cursor = None;
    if ws.grid.verb.source != RegionSource::Lasso || ws.grid.verb.lasso.is_empty() {
        return;
    }
    if let Some((_, raw)) = point_under(ws, ptr) {
        ws.grid.verb.lasso_cursor = Some(next_point(ws, ptr, raw));
    }
}

/// A click: the next corner of the shape.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((room, raw)) = point_under(ws, ptr) else {
        return;
    };
    if ws.grid.verb.lasso.is_empty() {
        ws.grid.verb.lasso_room = room;
        ws.grid.verb.lasso_add = ptr.shift();
        if !ptr.shift() {
            ws.grid.verb.region = None;
        }
    }
    let point = next_point(ws, ptr, raw);
    if ws.grid.verb.lasso.last() != Some(&point) {
        ws.grid.verb.lasso.push(point);
    }
    ws.grid.verb.lasso_cursor = None;
    let n = ws.grid.verb.lasso.len();
    ws.status = if n < 3 {
        format!("Lasso: {n} points. Click the corners, Enter closes")
    } else {
        format!(
            "Lasso: {n} points, {} sectors. Enter closes, Shift snaps to 45 degrees",
            cells_of(&ws.grid.verb.lasso).len()
        )
    };
}

/// The sectors a shape of `points` covers.
pub(crate) fn cells_of(points: &[[f64; 2]]) -> Vec<[i32; 2]> {
    polygon_cells(points).into_iter().collect()
}

/// The region a lasso of `points` makes in `room`.
pub(crate) fn region_of(room: usize, points: &[[f64; 2]]) -> Region {
    Region::from_cells(room, cells_of(points))
}

/// Enter: close the shape into the region. Returns whether a lasso was in
/// progress (the key is then taken).
pub(super) fn close(ws: &mut EditorWorkspace) -> bool {
    if ws.grid.verb.lasso.is_empty() {
        return false;
    }
    if ws.grid.verb.lasso.len() < 3 {
        ws.status = "A lasso needs at least three points. Esc drops it".to_string();
        return true;
    }
    let room = ws.grid.verb.lasso_room;
    let mut region = region_of(room, &ws.grid.verb.lasso);
    if ws.grid.verb.lasso_add {
        if let Some(old) = ws.grid.verb.region.as_ref().filter(|r| r.room == room) {
            let mut merged = old.clone();
            merged.extend(&region);
            region = merged;
        }
    }
    ws.grid.verb.lasso.clear();
    ws.grid.verb.lasso_cursor = None;
    ws.status = if region.is_empty() {
        "The lasso holds no sector centre: draw a bigger shape".to_string()
    } else {
        format!(
            "Region: {} sectors from the lasso. Press on it and drag",
            region.len()
        )
    };
    ws.grid.verb.region = (!region.is_empty()).then_some(region);
    true
}

/// Esc: drop the shape in progress. Returns whether there was one.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    if ws.grid.verb.lasso.is_empty() {
        return false;
    }
    ws.grid.verb.lasso.clear();
    ws.grid.verb.lasso_cursor = None;
    ws.status = "Lasso cancelled".to_string();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_snaps_a_segment_to_eight_directions_and_whole_sectors() {
        assert_eq!(snap_45([1.0, 1.0], [4.2, 1.3]), [4.0, 1.0]);
        assert_eq!(snap_45([1.0, 1.0], [3.4, 3.1]), [3.0, 3.0]);
        assert_eq!(snap_45([1.0, 1.0], [-0.9, 1.4]), [-1.0, 1.0]);
        assert_eq!(snap_45([2.0, 2.0], [1.1, 0.8]), [1.0, 1.0]);
        // Never a zero length segment.
        assert_eq!(snap_45([2.0, 2.0], [2.1, 2.0]), [3.0, 2.0]);
    }
}
