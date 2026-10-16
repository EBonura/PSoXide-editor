//! 2D drawing of a grid world in the Top view.
//!
//! Sectors are filled by floor height (blue low, orange high, like the
//! bonnie-32 plan view), split diagonals are marked, walls are drawn on
//! their edges, and each room gets an outline and a name. This is display
//! only; the grid tools plug in beside it and share [`sector_screen_rect`].

use super::*;

use psxed_project::grid::{
    GridRoom, GridSector, GridSplit, GridWorld, WallEdge, CLICK_UNITS, SECTOR_UNITS,
};

const ROOM_OUTLINE: Color32 = Color32::from_rgb(64, 150, 170);
const ROOM_LABEL: Color32 = Color32::from_rgb(110, 190, 205);
const CEILING_ONLY: Color32 = Color32::from_rgb(54, 58, 70);
const WALLS_ONLY: Color32 = Color32::from_rgb(44, 48, 56);
const WALL_SOLID: Color32 = Color32::from_rgb(236, 236, 242);
const WALL_PASSABLE: Color32 = Color32::from_rgb(232, 196, 90);
const SECTOR_LINE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 90);
const SPLIT_LINE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);
/// Smallest on-screen sector edge that still gets its outline.
const MIN_OUTLINE_PIXELS: f32 = 5.0;
/// Smallest on-screen sector edge that gets a height label.
const MIN_LABEL_PIXELS: f32 = 38.0;

/// Screen rectangle of local sector `(x, z)` of `room`, and the editor-space
/// corner positions `[nw, ne, se, sw]` it was built from.
pub(crate) fn sector_screen_rect(
    world: &GridWorld,
    room: &GridRoom,
    transform: ViewportTransform,
    x: usize,
    z: usize,
) -> (Rect, [Pos2; 4]) {
    let [cx, cz] = room.world_cell(x, z);
    let edge = |cell: i32| world.scale.to_editor(cell.saturating_mul(SECTOR_UNITS)) as f32;
    let (x0, x1) = (edge(cx), edge(cx + 1));
    let (z0, z1) = (edge(cz), edge(cz + 1));
    let corners = [
        transform.world_to_screen([x0, z0]),
        transform.world_to_screen([x1, z0]),
        transform.world_to_screen([x1, z1]),
        transform.world_to_screen([x0, z1]),
    ];
    (Rect::from_two_pos(corners[0], corners[2]), corners)
}

/// Blue (low) to orange (high) ramp, `t` in `0..=1`.
fn height_color(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let low = [44.0, 84.0, 150.0];
    let mid = [78.0, 128.0, 112.0];
    let high = [220.0, 150.0, 70.0];
    let (from, to, u) = if t < 0.5 {
        (low, mid, t * 2.0)
    } else {
        (mid, high, (t - 0.5) * 2.0)
    };
    let c = |i: usize| (from[i] + (to[i] - from[i]) * u) as u8;
    Color32::from_rgb(c(0), c(1), c(2))
}

/// Lowest and highest floor corner over the world, grid units including each
/// room's vertical offset.
fn floor_range(world: &GridWorld) -> (i32, i32) {
    let mut range = (i32::MAX, i32::MIN);
    for room in &world.rooms {
        for (_, _, sector) in room.iter() {
            if let Some(floor) = &sector.floor {
                let (low, high) = floor.height_range();
                range = (
                    range.0.min(room.y_offset + low),
                    range.1.max(room.y_offset + high),
                );
            }
        }
    }
    if range.0 > range.1 {
        (0, 0)
    } else {
        range
    }
}

fn mean_floor(room: &GridRoom, sector: &GridSector) -> Option<f32> {
    let floor = sector.floor.as_ref()?;
    let total: i32 = floor.heights.iter().sum();
    Some((room.y_offset as f32) + total as f32 / 4.0)
}

/// Whether the floor's diagonal changes what is drawn and is worth marking.
fn split_matters(sector: &GridSector) -> bool {
    sector.floor.as_ref().is_some_and(|floor| {
        !floor.is_planar() || floor.heights_b.is_some() || floor.tri_b.is_some()
    })
}

fn wall_stroke(solid: bool, thickness: f32) -> egui::Stroke {
    egui::Stroke::new(thickness, if solid { WALL_SOLID } else { WALL_PASSABLE })
}

fn draw_walls(painter: &egui::Painter, corners: [Pos2; 4], sector: &GridSector, thickness: f32) {
    // Corner order is [NW, NE, SE, SW]; map a sector-local endpoint
    // `[x, z]` (each 0 or 1) onto it.
    let point = |p: [i32; 2]| match p {
        [0, 0] => corners[0],
        [1, 0] => corners[1],
        [1, 1] => corners[2],
        _ => corners[3],
    };
    for edge in WallEdge::ALL {
        let walls = sector.walls(edge);
        if walls.is_empty() {
            continue;
        }
        let (a, b) = edge.endpoints();
        let solid = walls.iter().any(|w| w.solid);
        painter.line_segment([point(a), point(b)], wall_stroke(solid, thickness));
    }
}

/// Draw `world` in the Top view: all sector fills first, then the walls on
/// top of them (a neighbour's fill must not cover this sector's walls), then
/// the room outlines.
pub(crate) fn draw_grid_world(
    painter: &egui::Painter,
    transform: ViewportTransform,
    world: &GridWorld,
    show_labels: bool,
) {
    let (low, high) = floor_range(world);
    let span = (high - low).max(1) as f32;
    let font = egui::FontId::proportional(10.0);
    let visible = painter.clip_rect();
    let mut frames: Vec<Option<Rect>> = Vec::with_capacity(world.rooms.len());
    for room in &world.rooms {
        let mut bounds: Option<Rect> = None;
        for (x, z, sector) in room.iter() {
            let (rect, corners) = sector_screen_rect(world, room, transform, x, z);
            bounds = Some(bounds.map_or(rect, |b| b.union(rect)));
            if !visible.intersects(rect) {
                continue;
            }
            let fill = match mean_floor(room, sector) {
                Some(mean) => height_color((mean - low as f32) / span),
                None if sector.ceiling.is_some() => CEILING_ONLY,
                None => WALLS_ONLY,
            };
            painter.rect_filled(rect, 0.0, fill);
            let size = rect.width().min(rect.height());
            if size >= MIN_OUTLINE_PIXELS {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, SECTOR_LINE),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(floor) = &sector.floor {
                if split_matters(sector) && size >= MIN_OUTLINE_PIXELS {
                    let (a, b) = match floor.split {
                        GridSplit::NwSe => (corners[0], corners[2]),
                        GridSplit::NeSw => (corners[1], corners[3]),
                    };
                    painter.line_segment([a, b], egui::Stroke::new(1.0, SPLIT_LINE));
                }
            }
            if show_labels && size >= MIN_LABEL_PIXELS {
                if let Some(mean) = mean_floor(room, sector) {
                    let clicks = (mean / CLICK_UNITS as f32).round() as i32;
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{clicks}"),
                        font.clone(),
                        Color32::from_rgba_premultiplied(0, 0, 0, 170),
                    );
                }
            }
        }
        frames.push(bounds);
    }
    for room in &world.rooms {
        for (x, z, sector) in room.iter() {
            if sector.wall_count() == 0 {
                continue;
            }
            let (rect, corners) = sector_screen_rect(world, room, transform, x, z);
            if !visible.intersects(rect) {
                continue;
            }
            let size = rect.width().min(rect.height());
            draw_walls(painter, corners, sector, (size * 0.1).clamp(2.0, 4.5));
        }
    }
    for (room_index, (room, frame)) in world.rooms.iter().zip(frames).enumerate() {
        let Some(rect) = frame else { continue };
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.5, ROOM_OUTLINE),
            egui::StrokeKind::Outside,
        );
        if show_labels {
            let name = if room.name.is_empty() {
                format!("Room {room_index}")
            } else {
                room.name.clone()
            };
            painter.text(
                rect.left_top() + Vec2::new(2.0, -3.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                egui::FontId::proportional(11.0),
                ROOM_LABEL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::{GridSurface, GridWall};

    #[test]
    fn height_ramp_runs_blue_to_orange() {
        let low = height_color(0.0);
        let high = height_color(1.0);
        assert!(low.b() > low.r());
        assert!(high.r() > high.b());
        assert_eq!(height_color(-3.0), low);
        assert_eq!(height_color(9.0), high);
    }

    #[test]
    fn floor_range_includes_room_offsets() {
        let mut world = GridWorld::new();
        let mut room = GridRoom::new("r", 1, 1);
        room.y_offset = 512;
        room.sector_or_insert(0, 0).unwrap().floor = Some(GridSurface::flat(256, None));
        world.rooms.push(room);
        assert_eq!(floor_range(&world), (768, 768));
        assert_eq!(floor_range(&GridWorld::new()), (0, 0));
    }

    #[test]
    fn only_bent_or_split_level_floors_mark_their_diagonal() {
        let mut sector = GridSector {
            floor: Some(GridSurface::flat(0, None)),
            ..GridSector::default()
        };
        assert!(!split_matters(&sector));
        sector.floor.as_mut().unwrap().heights = [0, 0, 256, 0];
        assert!(split_matters(&sector));
    }

    #[test]
    fn sector_rect_follows_the_scale_and_the_view_orientation() {
        let mut world = GridWorld::new();
        let mut room = GridRoom::new("r", 2, 2);
        room.origin = [1, 0];
        room.sector_or_insert(0, 0)
            .unwrap()
            .north
            .push(GridWall::span(0, 256, None));
        world.rooms.push(room);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0));
        let transform = ViewportTransform::from_focus(rect, OrthographicView::Top, [0.0, 0.0], 1.0);
        let room = &world.rooms[0];
        let (r, corners) = sector_screen_rect(&world, room, transform, 0, 0);
        // World x 1344..2688 maps right of centre; z 0..1344 runs down.
        assert_eq!(r.left(), 200.0 + 1344.0);
        assert_eq!(r.width(), 1344.0);
        assert_eq!(corners[0], r.left_top());
        assert_eq!(corners[2], r.right_bottom());
    }
}
