//! Drawing the level verbs over the views: the region, the marquee and brush
//! in flight, a path, and the amount of a drag.

use super::overlay::{fill, label_chip, PolygonFn, CREATE, HOVER};
use super::pick::Cam;
use super::verb_state::{RegionGesture, VerbDrag};
use super::view::{editor_xz, editor_y};
use super::*;
use psxed_project::grid::edit::verbs::ramp::RampDir;

use psxed_project::grid::edit::{CellRect, Region};
use psxed_project::grid::{GridWorld, SECTOR_UNITS};

/// The region's colour: warm, so it reads against the selection yellow and
/// the hover blue.
pub(super) const REGION: Color32 = Color32::from_rgb(255, 150, 60);

/// Editor-space `[nw, ne, se, sw]` plan corners of a world sector.
fn plan_corners(world: &GridWorld, cell: [i32; 2]) -> [[f32; 2]; 4] {
    let s = world.scale;
    let (x0, z0) = (cell[0] * SECTOR_UNITS, cell[1] * SECTOR_UNITS);
    [
        editor_xz(s, x0, z0),
        editor_xz(s, x0 + SECTOR_UNITS, z0),
        editor_xz(s, x0 + SECTOR_UNITS, z0 + SECTOR_UNITS),
        editor_xz(s, x0, z0 + SECTOR_UNITS),
    ]
}

/// The floor of `cell` as four editor-space points (at its corner heights),
/// or flat at the room's base when it has none.
pub(super) fn floor_points(world: &GridWorld, room: usize, cell: [i32; 2]) -> [[f32; 3]; 4] {
    let plan = plan_corners(world, cell);
    let r = world.rooms.get(room);
    let heights = r
        .and_then(|r| r.local(cell).and_then(|[x, z]| r.sector(x, z)))
        .and_then(|s| s.floor.as_ref())
        .map_or([0; 4], |f| f.heights);
    let y_offset = r.map_or(0, |r| r.y_offset);
    core::array::from_fn(|i| {
        [
            plan[i][0],
            editor_y(world, y_offset, heights[i]),
            plan[i][1],
        ]
    })
}

/// What the overlay shows of the region right now: the committed region, or
/// the pick in flight.
fn shown_region(ws: &EditorWorkspace) -> Option<(Region, bool)> {
    match &ws.grid.gesture {
        Gesture::Region(RegionGesture::Marquee {
            room,
            anchor,
            current,
            add,
        }) => {
            let mut region = if *add {
                ws.grid
                    .verb
                    .region
                    .clone()
                    .unwrap_or_else(|| Region::new(*room))
            } else {
                Region::new(*room)
            };
            region.extend(&Region::from_rect(
                *room,
                CellRect::from_corners(*anchor, *current),
            ));
            Some((region, true))
        }
        Gesture::Region(RegionGesture::Brush { cells, .. }) => Some((cells.clone(), true)),
        _ => ws.grid.verb.region.clone().map(|r| (r, false)),
    }
}

/// The text chip over a verb drag: mode, amount, and whether the cliff it
/// makes is climbable.
fn drag_chip(ws: &EditorWorkspace, world: &GridWorld) -> Option<String> {
    let Gesture::Verb(g) = &ws.grid.gesture else {
        return None;
    };
    let amount = super::verb_tool::amount_text(world, g.amount);
    Some(match &g.drag {
        VerbDrag::Break(live) => {
            return Some(super::break_tool::chip(
                live,
                g.amount,
                ws.grid.verb.brk.targets_label(),
            ))
        }
        VerbDrag::Rough(_) => return Some(super::rough_tool::chip(g.amount)),
        VerbDrag::Raise(d) => {
            let ru = psxed_project::grid::cook::RuntimeUnits::new(world.scale)
                .map(|u| u.rt(g.amount).abs())
                .ok();
            let step = psx_engine::WalkLimits::DEFAULT.step_up;
            let climb = match ru {
                Some(ru) if ru <= step => format!("riser {ru} ru: climbable (step {step} ru)"),
                Some(ru) => format!("riser {ru} ru: taller than the {step} ru step"),
                None => String::new(),
            };
            format!("{}  {amount}   {climb}", d.params().mode.label())
        }
        VerbDrag::Ramp(d) => {
            let max = g.max_rise.unwrap_or(0);
            let fits = if g.amount.abs() <= max {
                "walkable"
            } else {
                "too steep"
            };
            format!("Ramp from {}  {amount}   {fits}", d.params().from.label())
        }
        VerbDrag::Stairs(d) => super::stairs_tool::chip(world, d, g.amount),
    })
}

impl EditorWorkspace {
    /// Region, picks in flight and the drag chip over the Top view.
    pub(super) fn draw_verb_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        if !self.grid.tool.is_verb() {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        let cell_rect = |cell: [i32; 2]| {
            let c = plan_corners(world, cell);
            Rect::from_two_pos(screen(c[0]), screen(c[2]))
        };
        let mut bounds: Option<Rect> = None;
        if let Some((region, picking)) = shown_region(self) {
            for cell in region.cells() {
                let r = cell_rect(cell);
                bounds = Some(bounds.map_or(r, |b| b.union(r)));
                painter.rect_filled(r, 0.0, fill(REGION, if picking { 70 } else { 96 }));
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.5, fill(REGION, 200)),
                    StrokeKind::Inside,
                );
            }
        }
        for cell in &self.grid.verb.preview {
            if matches!(self.grid.gesture, Gesture::Idle) {
                painter.rect_stroke(
                    cell_rect(*cell),
                    0.0,
                    Stroke::new(1.5, HOVER),
                    StrokeKind::Inside,
                );
            }
        }
        self.draw_path_2d(painter, transform, world);
        self.draw_lasso_2d(painter, transform, world);
        self.draw_ramp_overlay_2d(painter, transform, world);
        self.draw_stairs_overlay_2d(painter, transform, world);
        if let (Some(b), Some(text)) = (bounds, drag_chip(self, world)) {
            label_chip(
                painter,
                b.center_top() - Vec2::new(0.0, 8.0),
                Align2::CENTER_BOTTOM,
                &text,
            );
        }
    }

    fn draw_path_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
        world: &GridWorld,
    ) {
        let centre = |cell: [i32; 2]| {
            let c = plan_corners(world, cell);
            transform.world_to_screen([(c[0][0] + c[2][0]) * 0.5, (c[0][1] + c[2][1]) * 0.5])
        };
        let points: Vec<Pos2> = self.grid.verb.path.iter().map(|c| centre(*c)).collect();
        for pair in points.windows(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(2.5, CREATE));
        }
        for p in points {
            painter.circle_filled(p, 5.0, CREATE);
        }
    }

    /// The lasso in progress over the Top view: its corners, the rubber band
    /// to the pointer and the sectors the shape would take.
    fn draw_lasso_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
        world: &GridWorld,
    ) {
        let points = super::lasso::shape(self);
        if self.grid.verb.lasso.is_empty() {
            return;
        }
        for cell in super::lasso::cells_of(&points) {
            let c = plan_corners(world, cell);
            let r = Rect::from_two_pos(
                transform.world_to_screen(c[0]),
                transform.world_to_screen(c[2]),
            );
            painter.rect_filled(r, 0.0, fill(CREATE, 56));
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, fill(CREATE, 150)),
                StrokeKind::Inside,
            );
        }
        let screen: Vec<Pos2> = points
            .iter()
            .map(|p| transform.world_to_screen(super::lasso::editor_of(world.scale, *p)))
            .collect();
        for pair in screen.windows(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(2.5, CREATE));
        }
        if let (Some(&first), Some(&last)) = (screen.first(), screen.last()) {
            if screen.len() > 2 {
                painter.add(egui::Shape::dashed_line(
                    &[last, first],
                    Stroke::new(1.5, fill(CREATE, 190)),
                    6.0,
                    4.0,
                ));
            }
        }
        let corners = self.grid.verb.lasso.len();
        for p in screen.iter().take(corners) {
            painter.circle_filled(*p, 5.0, CREATE);
        }
    }

    /// The same over the 3D view, a hand above the floor.
    fn draw_lasso_3d(&self, painter: &egui::Painter, rect: Rect, world: &GridWorld) {
        if self.grid.verb.lasso.is_empty() {
            return;
        }
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let room = self.grid.verb.lasso_room;
        let points = super::lasso::shape(self);
        let lifted: Vec<[f32; 3]> = points
            .iter()
            .map(|p| {
                let xz = super::lasso::editor_of(world.scale, *p);
                let cell = [p[0].floor() as i32, p[1].floor() as i32];
                let pts = floor_points(world, room, cell);
                let y = pts.iter().map(|q| q[1]).sum::<f32>() * 0.25 + 40.0;
                [xz[0], y, xz[1]]
            })
            .collect();
        for cell in super::lasso::cells_of(&points) {
            let pts = floor_points(world, room, cell);
            let projected: Vec<Pos2> = pts
                .iter()
                .filter_map(|q| cam.project(*q).map(|(s, _)| s))
                .collect();
            if projected.len() == 4 {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(CREATE, 50),
                    Stroke::new(1.0, fill(CREATE, 140)),
                ));
            }
        }
        for pair in lifted.windows(2) {
            if let Some([a, b]) = super::view::project_segment(camera, rect, pair[0], pair[1]) {
                painter.line_segment([a, b], Stroke::new(3.0, CREATE));
            }
        }
        for p in lifted.iter().take(self.grid.verb.lasso.len()) {
            if let Some((s, _)) = cam.project(*p) {
                painter.circle_filled(s, 5.0, CREATE);
            }
        }
    }

    /// Region, picks in flight, the level ghost and the drag chip over the
    /// 3D view.
    pub(super) fn draw_verb_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.grid.tool.is_verb() {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let polygon = |points: &[[f32; 3]], color: Color32, alpha: u8, width: f32| {
            let projected: Vec<Pos2> = points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if projected.len() == points.len() && alpha > 0 {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(color, alpha),
                    Stroke::NONE,
                ));
            }
            for i in 0..points.len() {
                if let Some([a, b]) = super::view::project_segment(
                    camera,
                    rect,
                    points[i],
                    points[(i + 1) % points.len()],
                ) {
                    painter.line_segment([a, b], Stroke::new(width, color));
                }
            }
        };
        let mut centre_sum = ([0.0f32; 3], 0usize);
        if let Some((region, picking)) = shown_region(self) {
            for cell in region.cells() {
                let pts = floor_points(world, region.room, cell);
                polygon(&pts, REGION, if picking { 60 } else { 84 }, 1.5);
                for p in pts {
                    for (acc, v) in centre_sum.0.iter_mut().zip(p) {
                        *acc += v;
                    }
                    centre_sum.1 += 1;
                }
            }
        }
        if matches!(self.grid.gesture, Gesture::Idle) {
            let room = self
                .grid
                .verb
                .region
                .as_ref()
                .map_or_else(|| self.grid_target_room(), |r| r.room);
            for cell in &self.grid.verb.preview {
                polygon(&floor_points(world, room, *cell), HOVER, 36, 1.5);
            }
        }
        // Waypoints of a path, lifted a hand above the floor.
        let room = self.grid.verb.region.as_ref().map_or(0, |r| r.room);
        let lifted: Vec<[f32; 3]> = self
            .grid
            .verb
            .path
            .iter()
            .map(|c| {
                let p = floor_points(world, room, *c);
                [
                    (p[0][0] + p[2][0]) * 0.5,
                    (p[0][1] + p[1][1] + p[2][1] + p[3][1]) * 0.25 + 40.0,
                    (p[0][2] + p[2][2]) * 0.5,
                ]
            })
            .collect();
        for pair in lifted.windows(2) {
            if let Some([a, b]) = super::view::project_segment(camera, rect, pair[0], pair[1]) {
                painter.line_segment([a, b], Stroke::new(3.0, CREATE));
            }
        }
        for p in &lifted {
            if let Some((s, _)) = cam.project(*p) {
                painter.circle_filled(s, 5.0, CREATE);
            }
        }
        self.draw_lasso_3d(painter, rect, world);
        self.draw_level_ghost_3d(&polygon, world);
        self.draw_ramp_overlay_3d(painter, &cam, &polygon, world);
        self.draw_stairs_overlay_3d(painter, &cam, &polygon, world);
        if centre_sum.1 > 0 {
            let n = centre_sum.1 as f32;
            let mid = [
                centre_sum.0[0] / n,
                centre_sum.0[1] / n + 60.0,
                centre_sum.0[2] / n,
            ];
            if let (Some((s, _)), Some(text)) = (cam.project(mid), drag_chip(self, world)) {
                label_chip(painter, s, Align2::CENTER_CENTER, &text);
            }
        }
    }

    /// Level mode: a translucent plane at the new height over the region.
    fn draw_level_ghost_3d(&self, polygon: &PolygonFn, world: &GridWorld) {
        use psxed_project::grid::edit::verbs::raise::RaiseMode;
        let Gesture::Verb(g) = &self.grid.gesture else {
            return;
        };
        let VerbDrag::Raise(d) = &g.drag else {
            return;
        };
        if d.params().mode != RaiseMode::Level {
            return;
        }
        let Some(region) = &self.grid.verb.region else {
            return;
        };
        let Some(b) = region.bounds() else { return };
        let y_offset = world.rooms.get(region.room).map_or(0, |r| r.y_offset);
        let y = editor_y(world, y_offset, d.params().reference + g.amount);
        let s = world.scale;
        let p = |x: i32, z: i32| {
            [
                s.to_editor(x * SECTOR_UNITS) as f32,
                y,
                s.to_editor(z * SECTOR_UNITS) as f32,
            ]
        };
        let quad = [
            p(b.min[0], b.min[1]),
            p(b.max[0] + 1, b.min[1]),
            p(b.max[0] + 1, b.max[1] + 1),
            p(b.min[0], b.max[1] + 1),
        ];
        polygon(&quad, Color32::from_rgb(120, 200, 255), 34, 2.0);
    }

    /// The A side of the Ramp being made or about to be, and whether a drag
    /// is in flight.
    fn ramp_sides(&self) -> Option<(Region, RampDir, bool)> {
        if self.grid.tool != GridTool::Ramp {
            return None;
        }
        let region = self.grid.verb.region.clone()?;
        if let Gesture::Verb(g) = &self.grid.gesture {
            if let VerbDrag::Ramp(d) = &g.drag {
                return Some((region, d.params().from, true));
            }
        }
        let from = self.grid.verb.ramp_from.or(self.grid.verb.ramp_hover)?;
        Some((region, from, false))
    }

    /// The A and B edges and an arrow over the Top view, with the slope
    /// class of every sector while a ramp is dragged.
    fn draw_ramp_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
        world: &GridWorld,
    ) {
        let Some((region, from, dragging)) = self.ramp_sides() else {
            return;
        };
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        if dragging {
            for (cell, class) in ramp_classes(world, &region) {
                let c = plan_corners(world, cell);
                let r = Rect::from_two_pos(screen(c[0]), screen(c[2]));
                painter.rect_filled(r, 0.0, fill(class_color(class), 120));
            }
        }
        let mut mids = [Pos2::ZERO; 2];
        for (i, dir) in [from, from.opposite()].into_iter().enumerate() {
            let (a, b) = side_line(world, &region, dir);
            let (sa, sb) = (screen(a), screen(b));
            let (color, width) = if i == 0 { (CREATE, 4.5) } else { (REGION, 2.5) };
            painter.line_segment([sa, sb], Stroke::new(width, color));
            mids[i] = sa + (sb - sa) * 0.5;
        }
        painter.arrow(mids[0], mids[1] - mids[0], Stroke::new(2.5, Color32::WHITE));
        label_chip(
            painter,
            mids[0],
            Align2::CENTER_CENTER,
            "A keeps its height",
        );
        label_chip(painter, mids[1], Align2::CENTER_CENTER, "B rises");
    }

    /// The same over the 3D view, following the floor corners.
    fn draw_ramp_overlay_3d(
        &self,
        painter: &egui::Painter,
        cam: &Cam,
        polygon: &PolygonFn,
        world: &GridWorld,
    ) {
        let Some((region, from, dragging)) = self.ramp_sides() else {
            return;
        };
        if dragging {
            for (cell, class) in ramp_classes(world, &region) {
                polygon(
                    &floor_points(world, region.room, cell),
                    class_color(class),
                    100,
                    1.0,
                );
            }
        }
        for (i, dir) in [from, from.opposite()].into_iter().enumerate() {
            let (color, width) = if i == 0 { (CREATE, 4.5) } else { (REGION, 2.5) };
            let mut mid = ([0.0f32; 3], 0usize);
            for (cell, pair) in side_cells(&region, dir) {
                let pts = floor_points(world, region.room, cell);
                let (a, b) = (pts[pair[0]], pts[pair[1]]);
                for p in [a, b] {
                    for (acc, v) in mid.0.iter_mut().zip(p) {
                        *acc += v;
                    }
                    mid.1 += 1;
                }
                if let (Some((pa, _)), Some((pb, _))) = (cam.project(a), cam.project(b)) {
                    painter.line_segment([pa, pb], Stroke::new(width, color));
                }
            }
            if mid.1 > 0 {
                let n = mid.1 as f32;
                let at = [mid.0[0] / n, mid.0[1] / n + 50.0, mid.0[2] / n];
                if let Some((s, _)) = cam.project(at) {
                    label_chip(
                        painter,
                        s,
                        Align2::CENTER_CENTER,
                        if i == 0 {
                            "A keeps its height"
                        } else {
                            "B rises"
                        },
                    );
                }
            }
        }
    }
}

/// The cells of `region` on side `dir` of its bounding box, with the two
/// floor corner indices (`[NW, NE, SE, SW]`) that lie on that side.
fn side_cells(region: &Region, dir: RampDir) -> Vec<([i32; 2], [usize; 2])> {
    let Some(b) = region.bounds() else {
        return Vec::new();
    };
    region
        .cells()
        .filter(|c| match dir {
            RampDir::North => c[1] == b.min[1],
            RampDir::South => c[1] == b.max[1],
            RampDir::West => c[0] == b.min[0],
            RampDir::East => c[0] == b.max[0],
        })
        .map(|c| {
            let pair = match dir {
                RampDir::North => [0, 1],
                RampDir::East => [1, 2],
                RampDir::South => [3, 2],
                RampDir::West => [0, 3],
            };
            (c, pair)
        })
        .collect()
}

/// Editor-space plan endpoints of side `dir` of the region's bounding box.
fn side_line(world: &GridWorld, region: &Region, dir: RampDir) -> ([f32; 2], [f32; 2]) {
    let b = region
        .bounds()
        .unwrap_or(psxed_project::grid::edit::CellRect {
            min: [0, 0],
            max: [0, 0],
        });
    let s = world.scale;
    let p = |x: i32, z: i32| editor_xz(s, x * SECTOR_UNITS, z * SECTOR_UNITS);
    match dir {
        RampDir::North => (p(b.min[0], b.min[1]), p(b.max[0] + 1, b.min[1])),
        RampDir::South => (p(b.min[0], b.max[1] + 1), p(b.max[0] + 1, b.max[1] + 1)),
        RampDir::West => (p(b.min[0], b.min[1]), p(b.min[0], b.max[1] + 1)),
        RampDir::East => (p(b.max[0] + 1, b.min[1]), p(b.max[0] + 1, b.max[1] + 1)),
    }
}

/// The slope class of each region sector's floor, for the tint of a Ramp.
fn ramp_classes(
    world: &GridWorld,
    region: &Region,
) -> Vec<([i32; 2], psxed_project::grid::SlopeClass)> {
    let Some(room) = world.rooms.get(region.room) else {
        return Vec::new();
    };
    let Ok(units) = psxed_project::grid::cook::RuntimeUnits::new(world.scale) else {
        return Vec::new();
    };
    let limits = psx_engine::WalkLimits::DEFAULT;
    region
        .cells()
        .filter_map(|cell| {
            let floor = room
                .local(cell)
                .and_then(|[x, z]| room.sector(x, z))?
                .floor
                .as_ref()?;
            Some((
                cell,
                psxed_project::grid::slope::surface_class(&units, room.y_offset, floor, &limits),
            ))
        })
        .collect()
}

/// A sector of a flight: its place, the tread it belongs to and whether it is
/// a landing.
type Tread = ([i32; 2], usize, bool);

/// Green to stand on, orange when too steep.
fn class_color(class: psxed_project::grid::SlopeClass) -> Color32 {
    use psxed_project::grid::SlopeClass;
    match class {
        SlopeClass::Flat | SlopeClass::Slope => Color32::from_rgb(96, 232, 132),
        SlopeClass::Steep => Color32::from_rgb(255, 112, 48),
    }
}

impl EditorWorkspace {
    /// The flight being dragged: `(sector, tread number, landing, refused)`.
    fn stairs_ghost(&self) -> Option<(Vec<Tread>, bool, usize)> {
        let Gesture::Verb(g) = &self.grid.gesture else {
            return None;
        };
        let VerbDrag::Stairs(d) = &g.drag else {
            return None;
        };
        let flight = d.plan(g.amount);
        Some((d.ghost(), flight.refusal.is_some(), flight.steps))
    }

    /// Treads of a flight over the Top view: each sector tinted by its step,
    /// numbered, landings blue, a refused flight orange.
    fn draw_stairs_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
        world: &GridWorld,
    ) {
        if self.grid.tool != GridTool::Stairs {
            return;
        }
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        // The path itself, once finished.
        let centre = |cell: [i32; 2]| {
            let c = plan_corners(world, cell);
            screen([(c[0][0] + c[2][0]) * 0.5, (c[0][1] + c[2][1]) * 0.5])
        };
        let way: Vec<Pos2> = self
            .grid
            .verb
            .stairs_path
            .iter()
            .map(|c| centre(*c))
            .collect();
        for pair in way.windows(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, CREATE));
        }
        let Some((ghost, refused, steps)) = self.stairs_ghost() else {
            return;
        };
        let mut tread_no = 0usize;
        let mut last = usize::MAX;
        for (cell, index, landing) in ghost {
            let c = plan_corners(world, cell);
            let r = Rect::from_two_pos(screen(c[0]), screen(c[2]));
            if index != last {
                last = index;
                if !landing {
                    tread_no += 1;
                }
            }
            painter.rect_filled(
                r,
                0.0,
                fill(stair_color(tread_no, steps, landing, refused), 150),
            );
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
            if r.width() > 26.0 && !landing && tread_no <= steps {
                painter.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    format!("{tread_no}"),
                    FontId::proportional(13.0),
                    Color32::WHITE,
                );
            }
        }
    }

    /// The same flight over the 3D view, at its floors.
    fn draw_stairs_overlay_3d(
        &self,
        painter: &egui::Painter,
        cam: &Cam,
        polygon: &PolygonFn,
        world: &GridWorld,
    ) {
        if self.grid.tool != GridTool::Stairs {
            return;
        }
        let Some(region) = &self.grid.verb.region else {
            return;
        };
        if let Some((ghost, refused, steps)) = self.stairs_ghost() {
            let (mut tread_no, mut last) = (0usize, usize::MAX);
            for (cell, index, landing) in ghost {
                if index != last {
                    last = index;
                    if !landing {
                        tread_no += 1;
                    }
                }
                let pts = floor_points(world, region.room, cell);
                polygon(
                    &pts,
                    stair_color(tread_no, steps, landing, refused),
                    120,
                    1.0,
                );
                if !landing && tread_no <= steps {
                    let mid = [
                        (pts[0][0] + pts[2][0]) * 0.5,
                        (pts[0][1] + pts[1][1] + pts[2][1] + pts[3][1]) * 0.25 + 30.0,
                        (pts[0][2] + pts[2][2]) * 0.5,
                    ];
                    if let Some((s, _)) = cam.project(mid) {
                        painter.text(
                            s,
                            Align2::CENTER_CENTER,
                            format!("{tread_no}"),
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );
                    }
                }
            }
        }
    }
}

/// Tread colour: a warm gradient up the flight, blue landings, orange when
/// the flight is refused.
fn stair_color(tread: usize, steps: usize, landing: bool, refused: bool) -> Color32 {
    if refused {
        return Color32::from_rgb(255, 112, 48);
    }
    if landing || tread > steps {
        return Color32::from_rgb(90, 140, 255);
    }
    let t = tread as f32 / steps.max(1) as f32;
    Color32::from_rgb(
        (255.0 * (0.55 + 0.45 * t)) as u8,
        (190.0 - 60.0 * t) as u8,
        80,
    )
}
