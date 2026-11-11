//! What the Edit tool draws over the views: the slide arrow of a wall, the
//! Move handle, and the typed amount.

use super::*;

const ARROW: Color32 = Color32::from_rgb(255, 196, 64);

impl EditorWorkspace {
    /// Handles and chips of the Edit tool over the 3D view.
    pub(crate) fn draw_edit_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        if self.grid.tool != GridTool::Edit {
            return;
        }
        let mut walls: Vec<FaceRef> = self.grid.edit.arrow_wall.into_iter().collect();
        for element in self.grid.selection.items() {
            if let GridElement::Face(face) = element {
                if wall_handle::WallHandle::of(*face).is_some() && !walls.contains(face) {
                    walls.push(*face);
                }
            }
        }
        if self.grid.edit.drag.is_none() {
            for face in walls.into_iter().take(16) {
                if let Some((at, anchor)) = target::arrow_position(self, rect, face) {
                    if let Some((base, _)) =
                        view::project_point(self.viewport_3d_camera(), rect, anchor)
                    {
                        painter.line_segment([base, at], Stroke::new(1.5, ARROW));
                    }
                    arrow(painter, at);
                }
            }
        }
        if let Some((at, _)) = repeat_move::handle_position(self, GridView::Persp(rect)) {
            move_handle(painter, at);
        }
        self.draw_edit_chip(painter, rect);
    }

    /// The Move handle over the plan.
    pub(crate) fn draw_edit_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        if self.grid.tool != GridTool::Edit {
            return;
        }
        if let Some((at, _)) = repeat_move::handle_position(self, GridView::Top(transform)) {
            move_handle(painter, at);
        }
        self.draw_edit_chip(painter, painter.clip_rect());
    }

    fn draw_edit_chip(&self, painter: &egui::Painter, rect: Rect) {
        let text = &self.grid.edit.hint;
        if self.grid.edit.drag.is_some() && !text.is_empty() {
            overlay_chip(painter, rect, text);
        }
    }
}

fn overlay_chip(painter: &egui::Painter, rect: Rect, text: &str) {
    super::super::overlay::label_chip(
        painter,
        rect.center_bottom() - Vec2::new(0.0, 14.0),
        Align2::CENTER_BOTTOM,
        text,
    );
}

fn arrow(painter: &egui::Painter, at: Pos2) {
    let tri = vec![
        at + Vec2::new(0.0, -9.0),
        at + Vec2::new(8.0, 5.0),
        at + Vec2::new(-8.0, 5.0),
    ];
    painter.add(egui::Shape::convex_polygon(
        tri,
        ARROW,
        Stroke::new(1.0, Color32::BLACK),
    ));
}

fn move_handle(painter: &egui::Painter, at: Pos2) {
    painter.circle(
        at,
        8.0,
        Color32::from_rgba_unmultiplied(80, 200, 255, 200),
        Stroke::new(1.5, Color32::WHITE),
    );
    painter.circle_stroke(at, 3.0, Stroke::new(1.5, Color32::BLACK));
}
