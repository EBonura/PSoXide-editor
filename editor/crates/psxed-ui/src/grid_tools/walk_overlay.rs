//! The walkability overlay (Alt+W): what the character can stand on and walk
//! to, drawn over the Top and 3D views.
//!
//! Orange floors are too steep to stand on, blue ones have less headroom than
//! the body, red edges are steps taller than the body climbs, yellow dashed
//! edges drop to nothing, and hatched floors are walkable but no player spawn
//! can reach them. The numbers are the character motor's own (see
//! `psxed_project::grid::walk`).

use super::analysis::WalkRun;
use super::overlay::fill;
use super::pick::Cam;
use super::verb_overlay::floor_points;
use super::view::editor_xz;
use super::*;

use psxed_project::grid::walk::{CellWalk, EdgeKind};
use psxed_project::grid::{GridWorld, WallEdge, SECTOR_UNITS};

const STEEP: Color32 = Color32::from_rgb(255, 140, 40);
const LOW: Color32 = Color32::from_rgb(70, 130, 255);
const TALL: Color32 = Color32::from_rgb(255, 64, 64);
const DROP: Color32 = Color32::from_rgb(255, 224, 64);
const HATCH: Color32 = Color32::from_rgb(255, 255, 255);
const GHOST: Color32 = Color32::from_rgb(170, 170, 190);

/// The tint of a sector, none when it is plainly walkable.
fn tint(cell: CellWalk) -> Option<Color32> {
    if cell.steep {
        Some(STEEP)
    } else if cell.low_headroom {
        Some(LOW)
    } else if cell.ghost {
        Some(GHOST)
    } else {
        None
    }
}

/// Plan corners of `edge` of `cell`, editor space.
fn edge_ends(world: &GridWorld, cell: [i32; 2], edge: WallEdge) -> [[f32; 2]; 2] {
    let (a, b) = edge.endpoints();
    let at = |p: [i32; 2]| {
        editor_xz(
            world.scale,
            (cell[0] + p[0]) * SECTOR_UNITS,
            (cell[1] + p[1]) * SECTOR_UNITS,
        )
    };
    [at(a), at(b)]
}

/// The two floor corner indices (`[NW, NE, SE, SW]`) an edge joins: a side
/// of the sector, or its diagonal for a fold between the triangles.
fn corner_pair(edge: WallEdge) -> [usize; 2] {
    match edge {
        WallEdge::North => [0, 1],
        WallEdge::East => [1, 2],
        WallEdge::South => [3, 2],
        WallEdge::West => [0, 3],
        WallEdge::Nwse => [0, 2],
        WallEdge::Nesw => [1, 3],
    }
}

fn legend_rows(run: &WalkRun) -> Vec<(Color32, String)> {
    let a = &run.analysis;
    let c = a.counts();
    let step = a.limits.step_up;
    let clicks = if a.click_ru > 0 {
        step as f32 / a.click_ru as f32
    } else {
        0.0
    };
    vec![
        (STEEP, format!("too steep to stand on: {}", c[0])),
        (
            LOW,
            format!("headroom under {} ru: {}", a.limits.body_height, c[1]),
        ),
        (
            TALL,
            format!("step taller than {step} ru ({clicks:.0} clicks): {}", c[2]),
        ),
        (DROP, format!("drops to nothing (falls): {}", c[3])),
        (
            HATCH,
            if a.reach_ran {
                format!("walkable, no spawn reaches it: {}", c[4])
            } else {
                "reachability off: no player spawn on a floor".to_string()
            },
        ),
    ]
}

impl EditorWorkspace {
    /// Draw the legend box in the lower left of `rect`.
    pub(super) fn draw_overlay_legend(
        painter: &egui::Painter,
        rect: Rect,
        title: &str,
        rows: &[(Color32, String)],
        foot: &str,
    ) {
        let font = FontId::proportional(12.0);
        let line = 17.0;
        let width = rows
            .iter()
            .map(|(_, t)| {
                painter
                    .layout_no_wrap(t.clone(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            })
            .fold(
                painter
                    .layout_no_wrap(title.to_string(), font.clone(), Color32::WHITE)
                    .size()
                    .x,
                f32::max,
            )
            .max(
                painter
                    .layout_no_wrap(foot.to_string(), font.clone(), Color32::WHITE)
                    .size()
                    .x,
            )
            + 34.0;
        let height = line * (rows.len() as f32 + 2.0) + 12.0;
        let top_left = Pos2::new(rect.left() + 10.0, rect.bottom() - height - 10.0);
        let r = Rect::from_min_size(top_left, Vec2::new(width, height));
        painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(12, 16, 22, 225));
        painter.text(
            r.min + Vec2::new(8.0, 6.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(12.5),
            Color32::WHITE,
        );
        for (i, (color, text)) in rows.iter().enumerate() {
            let y = r.min.y + 6.0 + line * (i as f32 + 1.0);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(r.min.x + 8.0, y + 3.0), Vec2::splat(10.0)),
                2.0,
                *color,
            );
            painter.text(
                Pos2::new(r.min.x + 24.0, y),
                Align2::LEFT_TOP,
                text,
                font.clone(),
                Color32::from_rgb(220, 224, 232),
            );
        }
        painter.text(
            Pos2::new(r.min.x + 8.0, r.max.y - 8.0 - 0.0),
            Align2::LEFT_BOTTOM,
            foot,
            FontId::proportional(11.0),
            Color32::from_rgb(150, 156, 170),
        );
    }

    fn walk_foot(run: &WalkRun) -> String {
        format!(
            "step {} ru, body {} ru, floor normal {:.2} | {} spawn(s) | {:.2} ms",
            run.analysis.limits.step_up,
            run.analysis.limits.body_height,
            f32::from(run.analysis.limits.min_floor_normal_y_q12) / 4096.0,
            run.spawns,
            run.millis
        )
    }

    /// Whether the walkability overlay is drawn: switched on (Alt+W), or the
    /// Sculpt tool is active (shaping the ground with the body's limits in
    /// sight).
    pub(crate) fn walk_overlay_on(&self) -> bool {
        self.grid.overlays.walk || self.grid.tool == GridTool::Sculpt
    }

    /// The overlay over the Top view.
    pub(super) fn draw_walk_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        if !self.walk_overlay_on() {
            return;
        }
        let (Some(world), Some(run)) = (self.project.active_scene().world.grid(), self.walk_run())
        else {
            return;
        };
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        let visible = painter.clip_rect();
        for (ri, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&ri) {
                continue;
            }
            let Some(walk) = run.analysis.rooms.get(ri) else {
                continue;
            };
            for [x, z] in room.face_cells() {
                let cell = room.world_cell(x, z);
                let Some(w) = walk.cell(cell) else { continue };
                let a = screen(editor_xz(
                    world.scale,
                    cell[0] * SECTOR_UNITS,
                    cell[1] * SECTOR_UNITS,
                ));
                let b = screen(editor_xz(
                    world.scale,
                    (cell[0] + 1) * SECTOR_UNITS,
                    (cell[1] + 1) * SECTOR_UNITS,
                ));
                let r = Rect::from_two_pos(a, b);
                if !visible.intersects(r) {
                    continue;
                }
                if let Some(color) = tint(w) {
                    painter.rect_filled(r, 0.0, fill(color, 120));
                }
                if w.unreachable {
                    // Diagonal hatching, three strokes a sector.
                    let n = 4;
                    for k in 1..n {
                        let t = k as f32 / n as f32;
                        painter.line_segment(
                            [
                                Pos2::new(r.left(), r.top() + r.height() * t),
                                Pos2::new(r.left() + r.width() * t, r.top()),
                            ],
                            Stroke::new(1.2, fill(HATCH, 210)),
                        );
                        painter.line_segment(
                            [
                                Pos2::new(r.right(), r.top() + r.height() * (1.0 - t)),
                                Pos2::new(r.left() + r.width() * (1.0 - t), r.bottom()),
                            ],
                            Stroke::new(1.2, fill(HATCH, 210)),
                        );
                    }
                }
            }
        }
        for edge in &run.analysis.edges {
            if self.grid_hidden_rooms.contains(&edge.room) {
                continue;
            }
            let [a, b] = edge_ends(world, edge.cell, edge.edge);
            let (sa, sb) = (screen(a), screen(b));
            match edge.kind {
                EdgeKind::StepTooTall => {
                    painter.line_segment([sa, sb], Stroke::new(4.0, TALL));
                }
                EdgeKind::Drop => {
                    painter.add(egui::Shape::dashed_line(
                        &[sa, sb],
                        Stroke::new(3.0, DROP),
                        6.0,
                        4.0,
                    ));
                }
            }
        }
        let rect = painter.clip_rect();
        Self::draw_overlay_legend(
            painter,
            rect,
            "Walkability  (Alt+W)",
            &legend_rows(&run),
            &Self::walk_foot(&run),
        );
    }

    /// The overlay over the 3D view.
    pub(super) fn draw_walk_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.walk_overlay_on() {
            return;
        }
        let (Some(world), Some(run)) = (self.project.active_scene().world.grid(), self.walk_run())
        else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let fill_poly = |points: &[[f32; 3]], color: Color32, alpha: u8| {
            let projected: Vec<Pos2> = points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if projected.len() == points.len() {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(color, alpha),
                    Stroke::NONE,
                ));
            }
        };
        let line = |a: [f32; 3], b: [f32; 3], stroke: Stroke| {
            if let Some([pa, pb]) = super::view::project_segment(camera, rect, a, b) {
                painter.line_segment([pa, pb], stroke);
            }
        };
        for (ri, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&ri) {
                continue;
            }
            let Some(walk) = run.analysis.rooms.get(ri) else {
                continue;
            };
            for [x, z] in room.face_cells() {
                let cell = room.world_cell(x, z);
                let Some(w) = walk.cell(cell) else { continue };
                let pts = floor_points(world, ri, cell);
                // A hand above the floor so the tint does not fight it.
                let lifted = pts.map(|p| [p[0], p[1] + 6.0, p[2]]);
                if let Some(color) = tint(w) {
                    fill_poly(&lifted, color, 120);
                }
                if w.unreachable {
                    let stroke = Stroke::new(1.2, fill(HATCH, 210));
                    line(lifted[0], lifted[2], stroke);
                    line(lifted[1], lifted[3], stroke);
                }
            }
        }
        for edge in &run.analysis.edges {
            if self.grid_hidden_rooms.contains(&edge.room) {
                continue;
            }
            let pts = floor_points(world, edge.room, edge.cell);
            let pair = corner_pair(edge.edge);
            let lift = |p: [f32; 3]| [p[0], p[1] + 10.0, p[2]];
            let (a, b) = (lift(pts[pair[0]]), lift(pts[pair[1]]));
            match edge.kind {
                EdgeKind::StepTooTall => line(a, b, Stroke::new(4.0, TALL)),
                EdgeKind::Drop => {
                    // Dashes by hand: four on, four off.
                    for k in 0..4 {
                        let (t0, t1) = (k as f32 * 0.25, k as f32 * 0.25 + 0.14);
                        let at = |t: f32| {
                            [
                                a[0] + (b[0] - a[0]) * t,
                                a[1] + (b[1] - a[1]) * t,
                                a[2] + (b[2] - a[2]) * t,
                            ]
                        };
                        line(at(t0), at(t1), Stroke::new(3.0, DROP));
                    }
                }
            }
        }
        Self::draw_overlay_legend(
            painter,
            rect,
            "Walkability  (Alt+W)",
            &legend_rows(&run),
            &Self::walk_foot(&run),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::GridScale;

    #[test]
    fn a_diagonal_edge_flag_is_drawn_corner_to_corner_in_both_views() {
        assert_eq!(corner_pair(WallEdge::Nwse), [0, 2]);
        assert_eq!(corner_pair(WallEdge::Nesw), [1, 3]);
        assert_eq!(corner_pair(WallEdge::West), [0, 3]);
        let world = GridWorld {
            scale: GridScale { num: 1, den: 1 },
            ..GridWorld::default()
        };
        // Top view: the plan ends of the NW to SE diagonal of sector (2, 1).
        let [a, b] = edge_ends(&world, [2, 1], WallEdge::Nwse);
        assert_eq!(a, [2.0 * 1024.0, 1024.0]);
        assert_eq!(b, [3.0 * 1024.0, 2.0 * 1024.0]);
        let [a, b] = edge_ends(&world, [2, 1], WallEdge::Nesw);
        assert_eq!(a, [3.0 * 1024.0, 1024.0]);
        assert_eq!(b, [2.0 * 1024.0, 2.0 * 1024.0]);
    }
}
