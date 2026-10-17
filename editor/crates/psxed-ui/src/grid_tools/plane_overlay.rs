//! Drawing free planes over the views: outlines, the selection and its
//! handles, hover and the ghosts of what a gesture will make.
//!
//! The Top view shows every plane (it is the only place a flat plane over a
//! room is visible as a plan); the 3D view shows the picture itself, so only
//! the selected, hovered and ghost planes get outlines there (all of them
//! while the Plane tool is active). Handle positions come from
//! [`EditorWorkspace::plane_grip_screen`], the same list the hit test reads,
//! so what is drawn is what is grabbed.

use super::overlay::{fill, label_chip, CREATE, GHOST, HOVER, SELECT};
use super::pick::Cam;
use super::plane_math::{self as pm, Frame, Handle, Side};
use super::plane_pick::plane_points;
use super::plane_tool::{to_editor, PlaneGesture};
use super::view::{project_segment, GridView};
use super::*;

use psxed_project::grid::edit::PlaneId;
use psxed_project::grid::{GridWorld, PlaneShape};

const DECOR: Color32 = Color32::from_rgb(120, 190, 200);
const WALKABLE: Color32 = Color32::from_rgb(110, 210, 120);
const SOLID: Color32 = Color32::from_rgb(236, 160, 80);
const OFF: Color32 = Color32::from_rgb(110, 110, 120);
const YAW_KNOB: Color32 = Color32::from_rgb(110, 230, 130);
const PITCH_KNOB: Color32 = Color32::from_rgb(120, 170, 255);

fn class_color(plane: &GridPlane) -> Color32 {
    if !plane.enabled {
        return OFF;
    }
    match plane.shape() {
        PlaneShape::None => DECOR,
        PlaneShape::Platform if plane.collision == PlaneCollision::Walkable => WALKABLE,
        _ => SOLID,
    }
}

fn knob_color(handle: Handle) -> Color32 {
    match handle {
        Handle::Yaw => YAW_KNOB,
        Handle::Pitch => PITCH_KNOB,
        _ => Color32::WHITE,
    }
}

/// Draw the markers of a plane's handles at their screen points.
fn draw_grips(
    painter: &egui::Painter,
    centre: Option<Pos2>,
    points: &[(Handle, Pos2)],
    hovered: Option<Handle>,
    active: Option<Handle>,
) {
    for (handle, at) in points {
        // Stems from the centre to the two knobs.
        if matches!(handle, Handle::Yaw | Handle::Pitch) {
            if let Some(c) = centre {
                painter.line_segment([c, *at], Stroke::new(1.5, fill(knob_color(*handle), 170)));
            }
        }
    }
    for (handle, at) in points {
        let lit = Some(*handle) == hovered || Some(*handle) == active;
        let color = if lit { HOVER } else { knob_color(*handle) };
        match handle {
            Handle::Yaw | Handle::Pitch => {
                let r = if lit { 8.0 } else { 6.5 };
                painter.circle_filled(*at, r, color);
                painter.circle_stroke(*at, r + 1.0, Stroke::new(1.5, Color32::BLACK));
            }
            Handle::Edge(side) => {
                let half = if lit { 5.5 } else { 4.0 };
                let size = match side {
                    Side::Left | Side::Right => Vec2::new(half, half * 1.4),
                    _ => Vec2::new(half * 1.4, half),
                };
                let r = Rect::from_center_size(*at, size * 2.0);
                painter.rect_filled(r, 1.0, color);
                painter.rect_stroke(
                    r,
                    1.0,
                    Stroke::new(1.5, Color32::BLACK),
                    StrokeKind::Outside,
                );
            }
            Handle::Corner(..) => {
                let half = if lit { 6.5 } else { 5.0 };
                let r = Rect::from_center_size(*at, Vec2::splat(half * 2.0));
                painter.rect_filled(r, 1.0, if lit { HOVER } else { SELECT });
                painter.rect_stroke(
                    r,
                    1.0,
                    Stroke::new(1.5, Color32::BLACK),
                    StrokeKind::Outside,
                );
            }
            Handle::Body => {}
        }
    }
}

/// The gizmo state shared by both views: which handle the pointer is over
/// and which is being dragged.
fn grip_state(ws: &EditorWorkspace, id: PlaneId) -> (Option<Handle>, Option<Handle>) {
    let hovered = match &ws.grid.hover {
        GridHover::Grip(p, h) if *p == id => Some(*h),
        _ => None,
    };
    let active = match &ws.grid.gesture {
        Gesture::Plane(PlaneGesture::Grip(g)) if g.id == id => Some(g.handle),
        _ => None,
    };
    (hovered, active)
}

/// A one-line readout of a plane for the drag chip.
fn readout(plane: &GridPlane) -> String {
    format!(
        "{} x {}   yaw {} pitch {} roll {}",
        plane.size[0], plane.size[1], plane.rotation[0], plane.rotation[1], plane.rotation[2]
    )
}

impl EditorWorkspace {
    /// The candidate plane a Plane-tool gesture shows as a ghost.
    fn plane_ghost(&self, world: &GridWorld) -> Option<(usize, GridPlane)> {
        match &self.grid.gesture {
            Gesture::Plane(PlaneGesture::Place(place)) => {
                let plane = self.placed_plane(place)?;
                Some((place.room, plane))
            }
            Gesture::Idle => match &self.grid.hover {
                GridHover::PlaneSpot(room, plane) => {
                    world.rooms.get(*room)?;
                    Some((*room, (**plane).clone()))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Editor-space corners of every enabled, visible plane with its id.
    fn plane_outlines(&self, world: &GridWorld) -> Vec<(PlaneId, GridPlane, [[f32; 3]; 4])> {
        let mut out = Vec::new();
        for (room_index, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&room_index) {
                continue;
            }
            for (index, plane) in room.planes.iter().enumerate() {
                out.push((
                    PlaneId {
                        room: room_index,
                        index,
                    },
                    plane.clone(),
                    plane_points(world, room.y_offset, plane),
                ));
            }
        }
        out
    }

    fn paste_ghost_planes(&self, world: &GridWorld) -> Vec<[[f32; 3]; 4]> {
        let Some(ghost) = &self.grid.plane_paste else {
            return Vec::new();
        };
        let Some(at) = ghost.at else {
            return Vec::new();
        };
        let y_offset = world
            .rooms
            .get(self.grid_target_room())
            .map_or(0, |r| r.y_offset);
        ghost
            .clip
            .planes
            .iter()
            .map(|plane| {
                let mut moved = plane.clone();
                moved.center[0] += at[0] - ghost.clip.anchor[0];
                moved.center[2] += at[1] - ghost.clip.anchor[1];
                plane_points(world, y_offset, &moved)
            })
            .collect()
    }

    // ---- Top view ----

    /// Planes, selection and handles over the Top view.
    pub(crate) fn draw_plane_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let screen = |p: &[f32; 3]| transform.world_to_screen([p[0], p[2]]);
        let outline = |points: &[[f32; 3]; 4], color: Color32, width: f32, alpha: u8| {
            let s: Vec<Pos2> = points.iter().map(screen).collect();
            // A plane seen edge-on has no area; its line is the plane.
            let area = (s[1] - s[0]).x * (s[3] - s[0]).y - (s[1] - s[0]).y * (s[3] - s[0]).x;
            if area.abs() > 4.0 && alpha > 0 {
                painter.add(egui::Shape::convex_polygon(
                    s.clone(),
                    fill(color, alpha),
                    Stroke::NONE,
                ));
            }
            for i in 0..4 {
                painter.line_segment([s[i], s[(i + 1) % 4]], Stroke::new(width, color));
            }
        };
        let selected = &self.grid.plane_sel;
        let hovered = match &self.grid.hover {
            GridHover::Plane(id) | GridHover::Grip(id, _) => Some(*id),
            _ => None,
        };
        for (id, plane, points) in self.plane_outlines(world) {
            let color = class_color(&plane);
            if selected.contains(&id) {
                outline(&points, SELECT, 3.0, 70);
            } else if hovered == Some(id) {
                outline(&points, HOVER, 2.5, 50);
            } else {
                outline(&points, color, 2.0, 38);
            }
        }
        if let Some((room, plane)) = self.plane_ghost(world) {
            if let Some(r) = world.rooms.get(room) {
                let points = plane_points(world, r.y_offset, &plane);
                outline(&points, CREATE, 2.5, 60);
                let centre = screen(&[
                    (points[0][0] + points[2][0]) / 2.0,
                    0.0,
                    (points[0][2] + points[2][2]) / 2.0,
                ]);
                if matches!(self.grid.gesture, Gesture::Plane(_)) {
                    label_chip(painter, centre, Align2::CENTER_CENTER, &readout(&plane));
                }
            }
        }
        for points in self.paste_ghost_planes(world) {
            outline(&points, GHOST, 1.5, 40);
        }
        if let Some(id) = self.plane_primary() {
            let view = GridView::Top(transform);
            if let Some((frame, points)) = self.plane_grip_screen(id, view) {
                let (hover, active) = grip_state(self, id);
                let e = to_editor(world.scale, frame.center);
                let centre = Some(screen(&[e[0], 0.0, e[2]]));
                draw_grips(painter, centre, &points, hover, active);
            }
        }
    }

    // ---- 3D view ----

    /// Planes, selection and handles over the 3D view.
    pub(crate) fn draw_plane_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let segment = |a: [f32; 3], b: [f32; 3], color: Color32, width: f32| {
            if let Some([pa, pb]) = project_segment(camera, rect, a, b) {
                painter.line_segment([pa, pb], Stroke::new(width, color));
            }
        };
        let outline = |points: &[[f32; 3]; 4], color: Color32, width: f32, alpha: u8| {
            let projected: Vec<Pos2> = points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if projected.len() == 4 && alpha > 0 {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(color, alpha),
                    Stroke::NONE,
                ));
            }
            for i in 0..4 {
                segment(points[i], points[(i + 1) % 4], color, width);
            }
        };
        let all = self.grid.tool == GridTool::Plane;
        let hovered = match &self.grid.hover {
            GridHover::Plane(id) | GridHover::Grip(id, _) => Some(*id),
            _ => None,
        };
        for (id, plane, points) in self.plane_outlines(world) {
            if self.grid.plane_sel.contains(&id) {
                outline(&points, SELECT, 2.5, 56);
            } else if hovered == Some(id) {
                outline(&points, HOVER, 2.0, 40);
            } else if all {
                outline(&points, fill(class_color(&plane), 200), 1.2, 0);
            }
        }
        if let Some((room, plane)) = self.plane_ghost(world) {
            if let Some(r) = world.rooms.get(room) {
                let points = plane_points(world, r.y_offset, &plane);
                outline(&points, CREATE, 2.0, 70);
                if matches!(self.grid.gesture, Gesture::Plane(_)) {
                    let mid = [
                        (points[0][0] + points[2][0]) / 2.0,
                        (points[0][1] + points[2][1]) / 2.0,
                        (points[0][2] + points[2][2]) / 2.0,
                    ];
                    if let Some((s, _)) = cam.project(mid) {
                        label_chip(painter, s, Align2::CENTER_CENTER, &readout(&plane));
                    }
                }
            }
        }
        for points in self.paste_ghost_planes(world) {
            outline(&points, GHOST, 1.5, 36);
        }
        let Some(id) = self.plane_primary() else {
            return;
        };
        let view = GridView::Persp(rect);
        let Some((frame, points)) = self.plane_grip_screen(id, view) else {
            return;
        };
        let (hover, active) = grip_state(self, id);
        // The yaw ring: a circle in the horizontal plane through the centre.
        self.draw_yaw_ring(&frame, world, &segment);
        let centre = cam
            .project(to_editor(world.scale, frame.center))
            .map(|(s, _)| s);
        draw_grips(painter, centre, &points, hover, active);
        if let Gesture::Plane(PlaneGesture::Grip(grip)) = &self.grid.gesture {
            if let (Some(plane), Some(c)) = (id.get(world), centre) {
                if grip.handle != Handle::Body {
                    label_chip(
                        painter,
                        c + Vec2::new(0.0, 36.0),
                        Align2::CENTER_TOP,
                        &readout(plane),
                    );
                }
            }
        }
    }

    fn draw_yaw_ring(
        &self,
        frame: &Frame,
        world: &GridWorld,
        segment: &dyn Fn([f32; 3], [f32; 3], Color32, f32),
    ) {
        let radius = pm::yaw_radius(frame);
        let steps = 48;
        let at = |i: usize| {
            let a = std::f64::consts::TAU * (i % steps) as f64 / steps as f64;
            let p = [
                frame.center[0] + a.sin() * radius,
                frame.center[1],
                frame.center[2] + a.cos() * radius,
            ];
            to_editor(world.scale, p)
        };
        for i in 0..steps {
            segment(at(i), at(i + 1), fill(YAW_KNOB, 110), 1.2);
        }
    }
}
