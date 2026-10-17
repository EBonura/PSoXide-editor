//! Drawing the level verbs over the views: the region, the marquee and brush
//! in flight, a path, and the amount of a drag.

use super::overlay::{fill, label_chip, PolygonFn, CREATE, HOVER};
use super::pick::Cam;
use super::verb_state::{RegionGesture, VerbDrag};
use super::view::{editor_xz, editor_y};
use super::*;

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
        self.draw_level_ghost_3d(&polygon, world);
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
        let VerbDrag::Raise(d) = &g.drag;
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
}
