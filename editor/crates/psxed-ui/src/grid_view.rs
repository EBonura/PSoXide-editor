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
/// A wall the room derives from a height difference.
const WALL_DERIVED: Color32 = Color32::from_rgb(140, 200, 232);
/// A derived wall the user hid.
const WALL_HIDDEN: Color32 = Color32::from_rgba_premultiplied(70, 90, 105, 120);
const SECTOR_LINE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 90);
const SPLIT_LINE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);
/// Hatching over a void triangle.
const VOID_HATCH: Color32 = Color32::from_rgba_premultiplied(150, 156, 170, 150);
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
        !floor.is_planar() || floor.heights_b.is_some() || floor.tri_b.is_some() || floor.void != 0
    })
}

/// Darken and hatch the void triangles of a sector's floor (its ceiling when
/// it has none): the half that is not there.
fn draw_voids(painter: &egui::Painter, corners: [Pos2; 4], sector: &GridSector, hatch: bool) {
    let Some(surface) = sector.floor.as_ref().or(sector.ceiling.as_ref()) else {
        return;
    };
    if surface.void == 0 {
        return;
    }
    for tri in 0..2usize {
        if !surface.is_void(tri) {
            continue;
        }
        let ids = surface.split.triangle(tri);
        let other = surface.split.triangle(1 - tri);
        painter.add(egui::Shape::convex_polygon(
            ids.iter().map(|&i| corners[i]).collect(),
            Color32::from_rgba_unmultiplied(8, 10, 14, 175),
            egui::Stroke::NONE,
        ));
        if !hatch {
            continue;
        }
        // Hatching parallel to the diagonal, from the corner that is only in
        // this triangle.
        let Some(&right) = ids.iter().find(|i| !other.contains(i)) else {
            continue;
        };
        let legs: Vec<Pos2> = ids
            .iter()
            .filter(|&&i| i != right)
            .map(|&i| corners[i])
            .collect();
        for t in [0.34_f32, 0.67] {
            let a = corners[right] + (legs[0] - corners[right]) * t;
            let b = corners[right] + (legs[1] - corners[right]) * t;
            painter.line_segment([a, b], egui::Stroke::new(1.0, VOID_HATCH));
        }
    }
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

/// Draw the walls a derived room computes for `cell`, on the edges that
/// host them.
fn draw_derived_walls(
    painter: &egui::Painter,
    corners: [Pos2; 4],
    room: &GridRoom,
    cell: [i32; 2],
    thickness: f32,
) {
    let point = |p: [i32; 2]| match p {
        [0, 0] => corners[0],
        [1, 0] => corners[1],
        [1, 1] => corners[2],
        _ => corners[3],
    };
    for wall in psxed_project::grid::derive::cell_walls(room, cell) {
        let (a, b) = wall.edge.endpoints();
        let color = if wall.hidden {
            WALL_HIDDEN
        } else if wall.kind == psxed_project::grid::DerivedKind::Closure && !wall.blocks() {
            WALL_PASSABLE
        } else {
            WALL_DERIVED
        };
        painter.line_segment(
            [point(a), point(b)],
            egui::Stroke::new(thickness * 0.8, color),
        );
    }
}

/// Which rooms the Top view shows and emphasises.
///
/// Rooms can overlap in plan (that is how floors stack), so rooms draw from
/// the lowest `y_offset` up and a room above another is drawn translucent.
/// With an active room, every other room fades so the active one reads on top.
#[derive(Clone, Copy)]
pub(crate) struct GridRoomView<'a> {
    pub(crate) hidden: &'a HashSet<usize>,
    pub(crate) active: Option<usize>,
}

impl GridRoomView<'_> {
    /// Visible room indices, bottom of the stack first, the active room last.
    pub(crate) fn draw_order(&self, world: &GridWorld) -> Vec<usize> {
        let mut order: Vec<usize> = (0..world.rooms.len())
            .filter(|i| !self.hidden.contains(i))
            .collect();
        order.sort_by_key(|i| (self.active == Some(*i), world.rooms[*i].y_offset, *i));
        order
    }

    /// Opacity factor of a room's fills.
    pub(crate) fn fade(&self, world: &GridWorld, room: usize) -> f32 {
        match self.active {
            Some(active) if active != room => 0.4,
            Some(_) => 1.0,
            None => {
                let lowest = world.rooms.iter().map(|r| r.y_offset).min().unwrap_or(0);
                if world.rooms[room].y_offset > lowest {
                    0.75
                } else {
                    1.0
                }
            }
        }
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
    view: &GridRoomView,
) {
    let (low, high) = floor_range(world);
    let span = (high - low).max(1) as f32;
    let font = egui::FontId::proportional(10.0);
    let visible = painter.clip_rect();
    let order = view.draw_order(world);
    let mut frames: Vec<Option<Rect>> = vec![None; world.rooms.len()];
    for &room_index in &order {
        let room = &world.rooms[room_index];
        let fade = view.fade(world, room_index);
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
            painter.rect_filled(rect, 0.0, fill.gamma_multiply(fade));
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
            draw_voids(painter, corners, sector, size >= MIN_OUTLINE_PIXELS);
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
        frames[room_index] = bounds;
    }
    for &room_index in &order {
        let room = &world.rooms[room_index];
        let derived = room.wall_mode == psxed_project::grid::WallMode::Derived;
        for (x, z, sector) in room.iter() {
            if sector.wall_count() == 0 && !derived {
                continue;
            }
            let (rect, corners) = sector_screen_rect(world, room, transform, x, z);
            if !visible.intersects(rect) {
                continue;
            }
            let size = rect.width().min(rect.height());
            let thickness = (size * 0.1).clamp(2.0, 4.5);
            if derived {
                draw_derived_walls(painter, corners, room, room.world_cell(x, z), thickness);
            }
            draw_walls(painter, corners, sector, thickness);
        }
    }
    for &room_index in &order {
        let room = &world.rooms[room_index];
        let Some(rect) = frames[room_index] else {
            continue;
        };
        let active = view.active == Some(room_index);
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(
                if active { 2.5 } else { 1.5 },
                if active { STUDIO_ACCENT } else { ROOM_OUTLINE },
            ),
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

impl EditorWorkspace {
    /// Room chips in the corner of the Top view when a grid world has more
    /// than one room: an eye toggles a room's visibility, the name makes it
    /// the active room (click again to clear).
    pub(crate) fn draw_grid_room_chips(&mut self, parent: &mut egui::Ui, rect: Rect) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        if world.rooms.len() < 2 {
            return;
        }
        let rooms: Vec<(String, i32)> = world
            .rooms
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let name = if r.name.is_empty() {
                    format!("Room {i}")
                } else {
                    r.name.clone()
                };
                (name, r.y_offset)
            })
            .collect();
        // A child ui over the viewport's top-right corner. It is not an
        // `egui::Area`, so it shows on the first frame (an Area spends one
        // frame measuring itself, which a single headless capture misses).
        const CHIP_WIDTH: f32 = 200.0;
        let chip_rect = Rect::from_min_size(
            rect.right_top() + Vec2::new(-CHIP_WIDTH - 12.0, 12.0),
            Vec2::new(CHIP_WIDTH, 40.0 + 24.0 * rooms.len() as f32),
        );
        let mut ui = parent.new_child(
            egui::UiBuilder::new()
                .max_rect(chip_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let ui = &mut ui;
        {
            {
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .stroke(egui::Stroke::new(1.0, STUDIO_BORDER))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Rooms")
                                .color(STUDIO_TEXT_WEAK)
                                .size(11.0),
                        );
                        for (index, (name, y_offset)) in rooms.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let mut shown = !self.grid_hidden_rooms.contains(&index);
                                if ui.checkbox(&mut shown, "").changed() {
                                    if shown {
                                        self.grid_hidden_rooms.remove(&index);
                                    } else {
                                        self.grid_hidden_rooms.insert(index);
                                    }
                                }
                                let label = if *y_offset == 0 {
                                    name.clone()
                                } else {
                                    format!("{name}  (y {y_offset:+})")
                                };
                                let active = self.grid_active_room == Some(index);
                                if ui.selectable_label(active, label).clicked() {
                                    self.grid_active_room = (!active).then_some(index);
                                }
                            });
                        }
                    });
            }
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
        // World x 1280..2560 maps right of centre; z 0..1280 runs down.
        assert_eq!(r.left(), 200.0 + 1280.0);
        assert_eq!(r.width(), 1280.0);
        assert_eq!(corners[0], r.left_top());
        assert_eq!(corners[2], r.right_bottom());
    }
}
