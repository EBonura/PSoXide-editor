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

/// Screen rectangle of leaf `key`, and the editor-space corner positions
/// `[nw, ne, se, sw]` it was built from.
pub(crate) fn leaf_screen_rect(
    world: &GridWorld,
    transform: ViewportTransform,
    key: psxed_project::grid::LeafKey,
) -> (Rect, [Pos2; 4]) {
    let click = CLICK_UNITS;
    let n = key.edge_clicks();
    let edge = |c: i32| world.scale.to_editor(c.saturating_mul(click)) as f32;
    let (x0, x1) = (edge(key.x), edge(key.x + n));
    let (z0, z1) = (edge(key.z), edge(key.z + n));
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
        for (_, sector) in room.leaves() {
            if let Some(floor) = sector.ground_floor() {
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

fn mean_floor(room: &GridRoom, sector: &GridSector, span: usize) -> Option<f32> {
    let floor = sector.spans().get(span)?.floor.as_ref()?;
    let total: i32 = floor.heights.iter().sum();
    Some((room.y_offset as f32) + total as f32 / 4.0)
}

/// What the Top view shows of a column at the storey cursor
/// (`docs/grid/multi-storey.md`, 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slice {
    /// The span open at the cursor, drawn at full strength.
    Active(usize),
    /// Nothing open at the cursor and a span below it: a dim ghost of the
    /// storey underneath.
    Below(usize),
    /// Nothing open and nothing below: spans only overhead.
    Above,
}

/// Which span of `sector` the slice at `level` (room-relative grid units)
/// shows. A column of one span, or no cursor, is its ground span.
pub(crate) fn slice_of(sector: &GridSector, level: Option<i32>) -> Slice {
    let Some(y) = level.filter(|_| !sector.is_single_span()) else {
        return Slice::Active(0);
    };
    if let Some(open) = sector.span_at(y, 0) {
        return Slice::Active(open);
    }
    let below = sector
        .spans()
        .iter()
        .rposition(|s| s.floor.as_ref().is_some_and(|f| f.height_range().0 <= y));
    below.map_or(Slice::Above, Slice::Below)
}

/// Whether a derived wall belongs to the slice the Top view shows at `level`
/// (grid units, room offsets included): a column of one span has only its own
/// walls, and in a column of several only the walls the span shown at the
/// level hosts are drawn and picked, so the storeys stacked on one edge do
/// not pile into one line.
pub(crate) fn wall_in_slice(
    room: &GridRoom,
    wall: &psxed_project::grid::DerivedWall,
    level: Option<i32>,
) -> bool {
    span_in_slice(room, wall.host, wall.span, level)
}

/// [`wall_in_slice`] for a wall known only by its host sector and span.
fn span_in_slice(room: &GridRoom, host: [i32; 2], span: u8, level: Option<i32>) -> bool {
    let Some(sector) = room.local(host).and_then(|[x, z]| room.sector(x, z)) else {
        return true;
    };
    if sector.is_single_span() {
        return true;
    }
    matches!(
        slice_of(sector, level.map(|y| y - room.y_offset)),
        Slice::Active(i) if i == usize::from(span)
    )
}

/// Whether the floor's diagonal changes what is drawn and is worth marking.
fn split_matters(sector: &GridSector) -> bool {
    sector.ground_floor().is_some_and(|floor| {
        !floor.is_planar() || floor.heights_b.is_some() || floor.tri_b.is_some() || floor.void != 0
    })
}

/// Darken and hatch the void triangles of a sector's floor (its ceiling when
/// it has none): the half that is not there.
fn draw_voids(painter: &egui::Painter, corners: [Pos2; 4], sector: &GridSector, hatch: bool) {
    let Some(surface) = sector.ground_floor().or(sector.ground_ceiling()) else {
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

/// How the Top view strokes one derived wall.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WallClass {
    Hidden,
    Passable,
    Derived,
}

impl WallClass {
    fn color(self) -> Color32 {
        match self {
            WallClass::Hidden => WALL_HIDDEN,
            WallClass::Passable => WALL_PASSABLE,
            WallClass::Derived => WALL_DERIVED,
        }
    }
}

/// Edge, class and span of every derived wall hosted by `cell`.
fn derived_wall_lines(room: &GridRoom, cell: [i32; 2]) -> Vec<(WallEdge, WallClass, u8)> {
    psxed_project::grid::derive::cell_walls(room, cell)
        .into_iter()
        .map(|wall| {
            let class = if wall.hidden {
                WallClass::Hidden
            } else if wall.kind == psxed_project::grid::DerivedKind::Closure && !wall.blocks() {
                WallClass::Passable
            } else {
                WallClass::Derived
            };
            (wall.edge, class, wall.span)
        })
        .collect()
}

/// Endpoints (grid units) and class of every segment a room with leaves of
/// several sizes derives for `cell`.
fn leaf_wall_lines(room: &GridRoom, cell: [i32; 2]) -> Vec<([i32; 2], [i32; 2], WallClass)> {
    psxed_project::grid::leaf_derive::cell_wall_segments(room, cell)
        .into_iter()
        .map(|seg| {
            let class = if seg.hidden {
                WallClass::Hidden
            } else if seg.kind == psxed_project::grid::DerivedKind::Closure && !seg.blocks {
                WallClass::Passable
            } else {
                WallClass::Derived
            };
            (seg.a, seg.b, class)
        })
        .collect()
}

type LeafWallLines = std::rc::Rc<[([i32; 2], [i32; 2], WallClass)]>;
type CellWallLines = std::rc::Rc<[(WallEdge, WallClass, u8)]>;

/// The derived wall strokes of a world, per room and cell, kept for one
/// document revision. Deriving a cell's walls walks its neighbours, so doing
/// it for every visible cell on every frame was the Top view's whole cost on
/// a tall level; the strokes only change when the document does.
#[derive(Default)]
pub(crate) struct DerivedWallLines {
    rev: std::cell::Cell<Option<u64>>,
    cells: std::cell::RefCell<HashMap<(usize, [i32; 2]), CellWallLines>>,
    leaves: std::cell::RefCell<HashMap<(usize, [i32; 2]), LeafWallLines>>,
}

impl DerivedWallLines {
    fn sync(&self, rev: u64) {
        if self.rev.get() != Some(rev) {
            self.cells.borrow_mut().clear();
            self.leaves.borrow_mut().clear();
            self.rev.set(Some(rev));
        }
    }
}

/// Where the Top view finds derived wall strokes: a revision-keyed cache, or
/// (tests, one-off draws) nowhere, in which case they are derived on the spot.
#[derive(Clone, Copy)]
pub(crate) struct WallLineSource<'a> {
    cache: &'a DerivedWallLines,
    rev: u64,
}

impl<'a> WallLineSource<'a> {
    pub(crate) fn new(cache: &'a DerivedWallLines, rev: u64) -> Self {
        cache.sync(rev);
        Self { cache, rev }
    }
}

fn cached_cell_lines(
    source: Option<WallLineSource>,
    room_index: usize,
    room: &GridRoom,
    cell: [i32; 2],
) -> CellWallLines {
    let Some(source) = source else {
        return derived_wall_lines(room, cell).into();
    };
    debug_assert_eq!(source.cache.rev.get(), Some(source.rev));
    if let Some(hit) = source.cache.cells.borrow().get(&(room_index, cell)) {
        return hit.clone();
    }
    let lines: CellWallLines = derived_wall_lines(room, cell).into();
    source
        .cache
        .cells
        .borrow_mut()
        .insert((room_index, cell), lines.clone());
    lines
}

fn cached_leaf_lines(
    source: Option<WallLineSource>,
    room_index: usize,
    room: &GridRoom,
    cell: [i32; 2],
) -> LeafWallLines {
    let Some(source) = source else {
        return leaf_wall_lines(room, cell).into();
    };
    if let Some(hit) = source.cache.leaves.borrow().get(&(room_index, cell)) {
        return hit.clone();
    }
    let lines: LeafWallLines = leaf_wall_lines(room, cell).into();
    source
        .cache
        .leaves
        .borrow_mut()
        .insert((room_index, cell), lines.clone());
    lines
}

/// Draw the walls a derived room computes for a cell, on the edges that
/// host them.
fn draw_derived_walls(
    painter: &egui::Painter,
    corners: [Pos2; 4],
    room: &GridRoom,
    cell: [i32; 2],
    lines: &[(WallEdge, WallClass, u8)],
    thickness: f32,
    level: Option<i32>,
) {
    let point = |p: [i32; 2]| match p {
        [0, 0] => corners[0],
        [1, 0] => corners[1],
        [1, 1] => corners[2],
        _ => corners[3],
    };
    for (edge, class, span) in lines {
        if !span_in_slice(room, cell, *span, level) {
            continue;
        }
        let (a, b) = edge.endpoints();
        painter.line_segment(
            [point(a), point(b)],
            egui::Stroke::new(thickness * 0.8, class.color()),
        );
    }
}

/// Draw the walls a room with leaves of several sizes derives for a cell.
fn draw_leaf_walls(
    painter: &egui::Painter,
    transform: ViewportTransform,
    world: &GridWorld,
    lines: &[([i32; 2], [i32; 2], WallClass)],
    thickness: f32,
) {
    let at = |p: [i32; 2]| {
        transform.world_to_screen([
            world.scale.to_editor(p[0]) as f32,
            world.scale.to_editor(p[1]) as f32,
        ])
    };
    for (a, b, class) in lines {
        painter.line_segment(
            [at(*a), at(*b)],
            egui::Stroke::new(thickness, class.color()),
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
    /// The storey cursor in grid units, room offsets included: where a
    /// column holds several floors, the view draws the span open at it, the
    /// storey below as a dim ghost and what is overhead as an outline.
    pub(crate) level: Option<i32>,
    /// Derived wall strokes kept per document revision, or `None` to derive
    /// them on the spot.
    pub(crate) walls: Option<WallLineSource<'a>>,
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
        for (key, sector) in room.leaves() {
            if !room.contains_leaf(key) {
                continue;
            }
            let (rect, corners) = leaf_screen_rect(world, transform, key);
            bounds = Some(bounds.map_or(rect, |b| b.union(rect)));
            if !visible.intersects(rect) {
                continue;
            }
            let slice = slice_of(sector, view.level.map(|y| y - room.y_offset));
            let shown = match slice {
                Slice::Active(i) | Slice::Below(i) => i,
                Slice::Above => 0,
            };
            let fill = match mean_floor(room, sector, shown) {
                Some(mean) => height_color((mean - low as f32) / span),
                None if sector.ground_ceiling().is_some() => CEILING_ONLY,
                None => WALLS_ONLY,
            };
            match slice {
                Slice::Active(_) => {
                    painter.rect_filled(rect, 0.0, fill.gamma_multiply(fade));
                }
                Slice::Below(_) => {
                    painter.rect_filled(rect, 0.0, fill.gamma_multiply(fade * 0.3));
                }
                // A slab overhead: an outline, so a stairwell reads as a gap.
                Slice::Above => {
                    painter.rect_stroke(
                        rect,
                        0.0,
                        egui::Stroke::new(1.0, SECTOR_LINE),
                        egui::StrokeKind::Inside,
                    );
                    painter.line_segment(
                        [rect.left_bottom(), rect.right_top()],
                        egui::Stroke::new(1.0, SECTOR_LINE),
                    );
                    continue;
                }
            }
            let size = rect.width().min(rect.height());
            if size >= MIN_OUTLINE_PIXELS {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, SECTOR_LINE),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(floor) = sector.ground_floor().filter(|_| shown == 0) {
                if split_matters(sector) && size >= MIN_OUTLINE_PIXELS {
                    let (a, b) = match floor.split {
                        GridSplit::NwSe => (corners[0], corners[2]),
                        GridSplit::NeSw => (corners[1], corners[3]),
                    };
                    painter.line_segment([a, b], egui::Stroke::new(1.0, SPLIT_LINE));
                }
            }
            if shown == 0 {
                draw_voids(painter, corners, sector, size >= MIN_OUTLINE_PIXELS);
            }
            if show_labels && size >= MIN_LABEL_PIXELS && matches!(slice, Slice::Active(_)) {
                if let Some(mean) = mean_floor(room, sector, shown) {
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
        let mixed = !room.leaves.is_sectors_only();
        for (key, sector) in room.leaves() {
            if (sector.wall_count() == 0 && !derived) || !room.contains_leaf(key) {
                continue;
            }
            let (rect, corners) = leaf_screen_rect(world, transform, key);
            if !visible.intersects(rect) {
                continue;
            }
            let size = rect.width().min(rect.height());
            let thickness = (size * 0.1).clamp(2.0, 4.5);
            if derived && !mixed {
                let lines = cached_cell_lines(view.walls, room_index, room, key.cell());
                draw_derived_walls(
                    painter,
                    corners,
                    room,
                    key.cell(),
                    &lines,
                    thickness,
                    view.level,
                );
            }
            draw_walls(painter, corners, sector, thickness);
        }
        if derived && mixed {
            for [x, z] in room.face_cells() {
                let (rect, _) = sector_screen_rect(world, room, transform, x, z);
                if !visible.intersects(rect) {
                    continue;
                }
                let thickness = (rect.width().min(rect.height()) * 0.05).clamp(1.5, 4.0);
                let lines = cached_leaf_lines(view.walls, room_index, room, room.world_cell(x, z));
                draw_leaf_walls(painter, transform, world, &lines, thickness);
            }
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
        room.sector_or_insert(0, 0)
            .unwrap()
            .set_ground_floor(Some(GridSurface::flat(256, None)));
        world.rooms.push(room);
        assert_eq!(floor_range(&world), (768, 768));
        assert_eq!(floor_range(&GridWorld::new()), (0, 0));
    }

    #[test]
    fn only_bent_or_split_level_floors_mark_their_diagonal() {
        let mut sector = GridSector::with_surfaces(Some(GridSurface::flat(0, None)), None);
        assert!(!split_matters(&sector));
        sector.ground_floor_mut().unwrap().heights = [0, 0, 256, 0];
        assert!(split_matters(&sector));
    }

    /// Every shape a Top view draw of `world` produced.
    fn top_shapes(world: &GridWorld) -> Vec<egui::epaint::Shape> {
        let ctx = egui::Context::default();
        let hidden = HashSet::new();
        let view = GridRoomView {
            hidden: &hidden,
            active: None,
            level: None,
            walls: None,
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 800.0))),
                ..egui::RawInput::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let rect = ui.max_rect();
                    let transform = ViewportTransform::from_focus(
                        rect,
                        OrthographicView::Top,
                        [2048.0, 2048.0],
                        0.2,
                    );
                    draw_grid_world(ui.painter(), transform, world, false, &view);
                });
            },
        );
        output.shapes.into_iter().map(|s| s.shape).collect()
    }

    fn cracked_world() -> GridWorld {
        let mut world = GridWorld::new();
        let mut room = GridRoom::new("r", 3, 3);
        room.wall_mode = psxed_project::grid::WallMode::Derived;
        for z in 0..3 {
            for x in 0..3 {
                room.sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_floor(Some(GridSurface::flat(0, None)));
            }
        }
        let cell = room
            .sector_or_insert(1, 1)
            .unwrap()
            .ground_floor_mut()
            .unwrap();
        cell.heights_b = Some([512; 4]);
        cell.split = GridSplit::NwSe;
        world.rooms.push(room);
        world
    }

    #[test]
    fn a_derived_diagonal_face_is_drawn_as_a_diagonal_line_in_the_wall_colour() {
        let shapes = top_shapes(&cracked_world());
        let diagonal = shapes.iter().any(|s| match s {
            egui::epaint::Shape::LineSegment { points, stroke } => {
                stroke.color == WALL_DERIVED
                    && (points[1].x - points[0].x).abs() > 4.0
                    && ((points[1].x - points[0].x).abs() - (points[1].y - points[0].y).abs()).abs()
                        < 0.5
            }
            _ => false,
        });
        assert!(
            diagonal,
            "no diagonal derived wall line in {} shapes",
            shapes.len()
        );
    }

    #[test]
    fn a_void_triangle_is_darkened_and_hatched() {
        let mut world = cracked_world();
        let surface = world.rooms[0]
            .sector_mut(1, 1)
            .unwrap()
            .ground_floor_mut()
            .unwrap();
        surface.heights_b = None;
        surface.void = psxed_project::grid::surface::VOID_B;
        let shapes = top_shapes(&world);
        let tint = Color32::from_rgba_unmultiplied(8, 10, 14, 175);
        let darkened = shapes.iter().any(|s| match s {
            egui::epaint::Shape::Path(p) => p.fill == tint,
            _ => false,
        });
        assert!(darkened, "no void triangle fill");
        let hatch = shapes
            .iter()
            .filter(|s| matches!(s, egui::epaint::Shape::LineSegment { stroke, .. } if stroke.color == VOID_HATCH))
            .count();
        assert_eq!(hatch, 2);
        // The sector next door is not hatched or darkened.
        let plain = top_shapes(&cracked_world());
        assert!(!plain
            .iter()
            .any(|s| matches!(s, egui::epaint::Shape::Path(p) if p.fill == tint)));
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
