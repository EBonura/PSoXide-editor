//! Cell sizes (Alt+L) and the Cells tool's hover: every leaf outlined in a
//! colour that says its size, and labelled with it where there is room.
//!
//! | size | colour |
//! | --- | --- |
//! | 4 sectors | violet |
//! | 2 sectors | blue |
//! | 1 sector | grey |
//! | 1/2 sector | amber |
//! | 1/4 sector | red |

use super::overlay::fill;
use super::pick::Cam;
use super::view::leaf_square;
use super::*;

use psxed_project::grid::{GridWorld, LeafKey};

/// Outline colour of a leaf level.
pub(super) const fn level_color(level: u8) -> Color32 {
    match level {
        0 => Color32::from_rgb(168, 124, 244),
        1 => Color32::from_rgb(104, 164, 244),
        2 => Color32::from_rgb(190, 192, 204),
        3 => Color32::from_rgb(255, 190, 84),
        _ => Color32::from_rgb(255, 108, 92),
    }
}

/// The edge of a leaf in sectors, as a label.
pub(super) const fn size_label(level: u8) -> &'static str {
    match level {
        0 => "4",
        1 => "2",
        2 => "1",
        3 => "1/2",
        _ => "1/4",
    }
}

/// Smallest on-screen edge that still gets a label.
const MIN_LABEL_PIXELS: f32 = 22.0;
/// Most leaves one view outlines in 3D (the picture is egui shapes).
const MAX_3D_LEAVES: usize = 4000;

impl EditorWorkspace {
    /// The cell sizes over the Top view.
    pub(super) fn draw_leaf_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        if !self.grid.overlays.leaves {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let visible = painter.clip_rect();
        for (index, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&index) {
                continue;
            }
            for (key, _) in room.leaves() {
                if !room.contains_leaf(key) {
                    continue;
                }
                let ([x0, z0], size) = leaf_square(world.scale, key);
                let a = transform.world_to_screen([x0, z0]);
                let b = transform.world_to_screen([x0 + size, z0 + size]);
                let rect = Rect::from_two_pos(a, b);
                if !visible.intersects(rect) {
                    continue;
                }
                let color = level_color(key.level);
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(if key.level == 2 { 1.0 } else { 2.0 }, fill(color, 220)),
                    StrokeKind::Inside,
                );
                if rect.width().min(rect.height()) >= MIN_LABEL_PIXELS {
                    painter.text(
                        rect.left_top() + Vec2::new(3.0, 2.0),
                        Align2::LEFT_TOP,
                        size_label(key.level),
                        FontId::proportional(10.0),
                        fill(color, 255),
                    );
                }
            }
        }
        Self::draw_leaf_legend(painter, visible);
    }

    /// The cell sizes over the 3D view: each leaf's outline on its floor.
    pub(super) fn draw_leaf_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.grid.overlays.leaves {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let mut drawn = 0;
        for (index, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&index) {
                continue;
            }
            for (key, sector) in room.leaves() {
                let Some(floor) = sector.ground_floor().or(sector.ground_ceiling()) else {
                    continue;
                };
                if drawn >= MAX_3D_LEAVES {
                    break;
                }
                let points = leaf_outline(world, room.y_offset, key, floor);
                let color = fill(level_color(key.level), 230);
                for i in 0..4 {
                    if let Some([pa, pb]) =
                        super::view::project_segment(camera, rect, points[i], points[(i + 1) % 4])
                    {
                        painter.line_segment(
                            [pa, pb],
                            Stroke::new(if key.level == 2 { 1.2 } else { 2.2 }, color),
                        );
                    }
                }
                drawn += 1;
            }
        }
        Self::draw_leaf_legend(painter, rect);
    }

    /// The hovered leaf of the Cells tool in the 3D view.
    pub(super) fn draw_leaf_hover_3d(&self, painter: &egui::Painter, rect: Rect) {
        let GridHover::Leaf { room, key } = &self.grid.hover else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(r) = world.rooms.get(*room) else {
            return;
        };
        let Some(sector) = r.leaf(*key) else { return };
        let Some(surface) = sector.ground_floor().or(sector.ground_ceiling()) else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let points = leaf_outline(world, r.y_offset, *key, surface);
        let color = if painter.ctx().input(|i| i.modifiers.alt) {
            super::overlay::ERASE
        } else {
            super::overlay::CREATE
        };
        let projected: Vec<Pos2> = points
            .iter()
            .filter_map(|p| cam.project(*p).map(|(s, _)| s))
            .collect();
        if projected.len() == 4 {
            painter.add(egui::Shape::convex_polygon(
                projected,
                fill(color, 70),
                Stroke::NONE,
            ));
        }
        for i in 0..4 {
            if let Some([pa, pb]) =
                super::view::project_segment(camera, rect, points[i], points[(i + 1) % 4])
            {
                painter.line_segment([pa, pb], Stroke::new(2.5, color));
            }
        }
    }

    fn draw_leaf_legend(painter: &egui::Painter, rect: Rect) {
        let font = FontId::proportional(11.0);
        let rows = [
            (0u8, "4 sectors"),
            (1, "2"),
            (2, "1"),
            (3, "1/2"),
            (4, "1/4"),
        ];
        let at = Pos2::new(rect.min.x + 10.0, rect.max.y - 22.0);
        painter.rect_filled(
            Rect::from_min_size(at - Vec2::new(6.0, 4.0), Vec2::new(300.0, 20.0)),
            3.0,
            Color32::from_rgba_unmultiplied(12, 16, 22, 200),
        );
        let mut x = at.x;
        painter.text(
            Pos2::new(x, at.y),
            Align2::LEFT_TOP,
            "Cells  (Alt+L)",
            font.clone(),
            Color32::from_rgb(210, 214, 224),
        );
        x += 96.0;
        for (level, label) in rows {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, at.y + 3.0), Vec2::splat(9.0)),
                2.0,
                level_color(level),
            );
            let galley = painter.layout_no_wrap(
                label.to_string(),
                font.clone(),
                Color32::from_rgb(210, 214, 224),
            );
            let w = galley.size().x;
            painter.galley(
                Pos2::new(x + 13.0, at.y),
                galley,
                Color32::from_rgb(210, 214, 224),
            );
            x += 13.0 + w + 8.0;
        }
    }

    /// Options of the Cells tool, in the strip's option row.
    pub(super) fn draw_cells_options(&mut self, ui: &mut egui::Ui) {
        use super::cells_tool::{CellsTarget, MergeTop};
        ui.label(
            RichText::new("Subdivide to")
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
        );
        for target in CellsTarget::ALL {
            let on = self.grid.cells.target == target;
            if ui
                .selectable_label(on, target.label())
                .on_hover_text("How fine the brush and the Subdivide button go")
                .clicked()
            {
                self.grid.cells.target = target;
            }
        }
        ui.separator();
        ui.label(
            RichText::new("Merge up to")
                .color(STUDIO_TEXT_WEAK)
                .size(11.0),
        );
        for top in MergeTop::ALL {
            let on = self.grid.cells.merge_to == top;
            if ui
                .selectable_label(on, top.label())
                .on_hover_text("The coarsest cell a merge may make. A merge happens only when the bigger cell looks and stands exactly as the four did")
                .clicked()
            {
                self.grid.cells.merge_to = top;
            }
        }
        ui.separator();
        ui.add(
            egui::DragValue::new(&mut self.grid.cells.radius)
                .range(0..=8)
                .prefix("brush "),
        )
        .on_hover_text("Brush radius in quarter sectors ([ and ])");
        ui.separator();
        let has_selection = !self.grid.selection.is_empty();
        if ui
            .add_enabled(has_selection, egui::Button::new("Subdivide selection"))
            .on_hover_text("Enter: split the selected cells")
            .clicked()
        {
            super::cells_tool::apply_selection(self, false);
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Merge selection"))
            .on_hover_text("Shift+Enter: join the selected cells back where that changes nothing")
            .clicked()
        {
            super::cells_tool::apply_selection(self, true);
        }
    }
}

/// The four corners of a leaf's surface in editor units, `[nw, ne, se, sw]`.
fn leaf_outline(
    world: &GridWorld,
    y_offset: i32,
    key: LeafKey,
    surface: &psxed_project::grid::GridSurface,
) -> [[f32; 3]; 4] {
    let ([x0, z0], size) = leaf_square(world.scale, key);
    let h = surface.heights;
    let y = |i: usize| world.scale.to_editor(y_offset.saturating_add(h[i])) as f32;
    [
        [x0, y(0), z0],
        [x0 + size, y(1), z0],
        [x0 + size, y(2), z0 + size],
        [x0, y(3), z0 + size],
    ]
}
