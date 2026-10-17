//! The performance heatmap overlay (Alt+P): the model of what the world
//! renderer would spend at every spot, tinted from green through amber to red
//! against the frame budget. Stale results are dimmed until the current one
//! arrives.

use super::analysis::HeatRun;
use super::overlay::fill;
use super::pick::Cam;
use super::verb_overlay::floor_points;
use super::view::editor_xz;
use super::*;

use psxed_project::grid::SECTOR_UNITS;

const COOL: Color32 = Color32::from_rgb(80, 210, 120);
const WARM: Color32 = Color32::from_rgb(245, 190, 60);
const HOT: Color32 = Color32::from_rgb(240, 70, 60);

/// Colour of a heat (1.0 is the world's share of the frame): green under
/// 60 percent, amber to 90, red above.
pub(super) fn heat_color(heat: f32) -> Color32 {
    if heat < 0.6 {
        COOL
    } else if heat < 0.9 {
        WARM
    } else {
        HOT
    }
}

fn legend(run: &HeatRun) -> (Vec<(Color32, String)>, String) {
    let m = &run.map;
    let o = &m.options;
    let budget = o.world_budget() as f32;
    let pct = |c: u32| c as f32 * 100.0 / budget;
    let mut rows = vec![
        (COOL, "under 60% of the world budget".to_string()),
        (WARM, "60 to 90%".to_string()),
        (HOT, "over 90%".to_string()),
    ];
    if let Some(w) = m.worst() {
        rows.push((
            Color32::WHITE,
            format!(
                "worst spot {:.0}%: {} quads, {} near, room {} sector {},{}",
                pct(w.cycles),
                w.quads,
                w.near,
                w.room,
                w.cell[0],
                w.cell[1]
            ),
        ));
    }
    rows.push((
        Color32::from_rgb(150, 156, 170),
        format!(
            "p50 {:.0}%, p90 {:.0}% | {} spots, {} quads",
            pct(m.percentile(50)),
            pct(m.percentile(90)),
            m.samples.len(),
            m.quads_total
        ),
    ));
    let state = if run.fresh {
        ""
    } else if run.computing {
        " | recomputing, tint is stale"
    } else {
        " | waiting for the edit to settle, tint is stale"
    };
    let foot = format!(
        "MODEL: quads x {} + near x {} cycles, budget {} - reserve {} | cook {:.0} ms, sampling {:.0} ms{state}",
        o.per_quad,
        o.per_near_quad,
        o.budget,
        o.reserve,
        m.cook_ms,
        m.sample_ms
    );
    (rows, foot)
}

impl EditorWorkspace {
    fn heat_alpha(run: &HeatRun) -> u8 {
        if run.fresh {
            140
        } else {
            60
        }
    }

    /// The overlay over the Top view.
    pub(super) fn draw_heat_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        if !self.grid.overlays.perf {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(run) = self.heat_run() else {
            self.draw_heat_waiting(painter);
            return;
        };
        let alpha = Self::heat_alpha(&run);
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        for s in &run.map.samples {
            if self.grid_hidden_rooms.contains(&s.room) {
                continue;
            }
            let a = screen(editor_xz(
                world.scale,
                s.cell[0] * SECTOR_UNITS,
                s.cell[1] * SECTOR_UNITS,
            ));
            let b = screen(editor_xz(
                world.scale,
                (s.cell[0] + 1) * SECTOR_UNITS,
                (s.cell[1] + 1) * SECTOR_UNITS,
            ));
            painter.rect_filled(
                Rect::from_two_pos(a, b),
                0.0,
                fill(heat_color(run.map.heat(s)), alpha),
            );
        }
        let (rows, foot) = legend(&run);
        Self::draw_overlay_legend(
            painter,
            painter.clip_rect(),
            "Performance heatmap  (Alt+P)",
            &rows,
            &foot,
        );
        if !run.fresh {
            painter
                .ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    /// The overlay over the 3D view.
    pub(super) fn draw_heat_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.grid.overlays.perf {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(run) = self.heat_run() else {
            self.draw_heat_waiting(painter);
            return;
        };
        let alpha = Self::heat_alpha(&run);
        let cam = Cam::new(self.viewport_3d_camera(), rect);
        for s in &run.map.samples {
            if self.grid_hidden_rooms.contains(&s.room) {
                continue;
            }
            let pts = floor_points(world, s.room, s.cell).map(|p| [p[0], p[1] + 6.0, p[2]]);
            let projected: Vec<Pos2> = pts
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if projected.len() == 4 {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(heat_color(run.map.heat(s)), alpha),
                    Stroke::NONE,
                ));
            }
        }
        let (rows, foot) = legend(&run);
        Self::draw_overlay_legend(painter, rect, "Performance heatmap  (Alt+P)", &rows, &foot);
        if !run.fresh {
            painter
                .ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    /// Nothing to show yet: say why.
    fn draw_heat_waiting(&self, painter: &egui::Painter) {
        let text = match self.heat_error() {
            Some(e) => format!("Performance heatmap: the world does not cook ({e})"),
            None => {
                "Performance heatmap: computing (a model of the world render cost)...".to_string()
            }
        };
        super::overlay::label_chip(
            painter,
            painter.clip_rect().left_bottom() + Vec2::new(14.0, -14.0),
            Align2::LEFT_BOTTOM,
            &text,
        );
        painter
            .ctx()
            .request_repaint_after(std::time::Duration::from_millis(120));
    }
}
