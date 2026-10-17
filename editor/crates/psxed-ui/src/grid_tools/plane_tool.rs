//! The Plane tool, and the plane gestures Select shares with it.
//!
//! * **Place** (Plane tool, press on a surface or empty floor): a plane
//!   appears on the surface under the pointer, facing out of it, snapped.
//!   A click gives the default size centred there; dragging sizes it in the
//!   surface's own plane with the press point as one corner.
//! * **Select** a plane by clicking it (either tool, either view). Shift
//!   extends, Ctrl or Cmd toggles.
//! * **Handles** of the selected plane (see `plane_math`): corners and edges
//!   stretch, the green ring knob turns it about the vertical axis, the
//!   arrow tip tilts it, a drag on the body slides it (Shift for up and down).
//!
//! Every drag recomputes the plane from the one it started with, so nothing
//! accumulates; the undo step is recorded at the first change.

use super::pick::Cam;
use super::plane_math::{self as pm, Frame, Handle, Side, Snap, V3};
use super::view::{GridPointer, GridView};
use super::*;

use psxed_project::grid::edit::{add_plane, remove_planes, update_plane, PlaneRefusal};
use psxed_project::grid::plane::{rotation_facing, DEFAULT_PLANE_SIZE};
use psxed_project::grid::GridScale;

/// How far a new plane sits off the surface it was placed on, grid units, so
/// it never fights the surface for the depth buffer.
const PLACE_OFFSET: f64 = 24.0;
/// Pointer travel, in grid units along both plane axes, below which a press
/// and release place the default-size plane.
const CLICK_SLOP: f64 = 96.0;
/// Pixels within which a handle is grabbed.
const GRIP_PIXELS: f32 = 10.0;

/// A plane being dragged out.
#[derive(Clone, Debug)]
pub(crate) struct PlacePlane {
    pub room: usize,
    /// Where the press landed, offset off the surface, absolute grid units.
    pub origin: V3,
    pub rotation: [i32; 3],
    /// Where the pointer is now, in the placement plane.
    pub current: V3,
    /// The snap the modifiers asked for at the last pointer position.
    pub snap: Snap,
}

/// A handle being dragged.
#[derive(Clone, Debug)]
pub(crate) struct GripDrag {
    pub id: PlaneId,
    pub handle: Handle,
    pub original: GridPlane,
    pub y_offset: i32,
    /// The other selected planes, carried along by a body drag.
    pub peers: Vec<(PlaneId, GridPlane)>,
    /// The pointer's point in the plane's surface when it was grabbed.
    pub press_point: V3,
    pub press_y: f32,
    pub pixels_per_unit: f32,
    /// The document before the first change, for the undo step.
    pub before: Option<Box<ProjectDocument>>,
}

/// The plane gesture in flight.
#[derive(Clone, Debug)]
pub(crate) enum PlaneGesture {
    Place(PlacePlane),
    Grip(Box<GripDrag>),
}

// ---- units and rays ----

/// Editor-space point to absolute grid units.
pub(crate) fn to_grid(scale: GridScale, p: [f32; 3]) -> V3 {
    let k = f64::from(scale.den.max(1)) / f64::from(scale.num.max(1));
    p.map(|v| f64::from(v) * k)
}

/// Absolute grid units to an editor-space point.
pub(crate) fn to_editor(scale: GridScale, p: V3) -> [f32; 3] {
    let k = f64::from(scale.num) / f64::from(scale.den.max(1));
    p.map(|v| (v * k) as f32)
}

/// Intersection of a ray with the plane through `point` with `normal`.
fn ray_plane(origin: V3, dir: V3, point: V3, normal: V3) -> Option<V3> {
    let denom = pm::dot(dir, normal);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = pm::dot(pm::sub(point, origin), normal) / denom;
    (t > 0.0).then(|| pm::add(origin, pm::scale(dir, t)))
}

impl EditorWorkspace {
    /// The pointer's ray in grid units.
    fn grid_ray_f64(&self, rect: Rect, pos: Pos2) -> Option<(V3, V3)> {
        let scale = self.project.active_scene().world.grid()?.scale;
        let (origin, dir) = self.grid_ray(rect, pos)?;
        Some((to_grid(scale, origin), dir.map(f64::from)))
    }

    /// The pointer on the plane through `point` with `normal` (grid units).
    /// In the Top view the point under the pointer on that plane, found
    /// along the vertical.
    fn pointer_on_plane(&self, ptr: &GridPointer, point: V3, normal: V3) -> Option<V3> {
        let scale = self.project.active_scene().world.grid()?.scale;
        match ptr.view {
            GridView::Persp(rect) => {
                let (o, d) = self.grid_ray_f64(rect, ptr.pos)?;
                ray_plane(o, d, point, normal)
            }
            GridView::Top(transform) => {
                let [x, z] = transform.screen_to_world(ptr.pos);
                let g = to_grid(scale, [x, 0.0, z]);
                let y = if normal[1].abs() > 1e-3 {
                    point[1]
                        - (normal[0] * (g[0] - point[0]) + normal[2] * (g[2] - point[2]))
                            / normal[1]
                } else {
                    point[1]
                };
                Some([g[0], y, g[2]])
            }
        }
    }

    /// Where a handle drag finds the pointer: in the surface for stretching
    /// and sliding, in the horizontal plane for yaw, in the vertical plane
    /// through the facing for pitch.
    fn grip_point(&self, ptr: &GridPointer, frame: &Frame, handle: Handle) -> Option<V3> {
        match handle {
            Handle::Yaw => self.pointer_on_plane(ptr, frame.center, [0.0, 1.0, 0.0]),
            Handle::Pitch => {
                let facing = frame.facing();
                self.pointer_on_plane(ptr, frame.center, [facing[2], 0.0, -facing[0]])
            }
            _ => self.pointer_on_plane(ptr, frame.center, frame.normal),
        }
    }

    // ---- selection ----

    /// The primary selected plane, if it still exists.
    pub(crate) fn plane_primary(&self) -> Option<PlaneId> {
        let world = self.project.active_scene().world.grid()?;
        self.grid
            .plane_sel
            .iter()
            .rev()
            .find(|id| id.exists(world))
            .copied()
    }

    /// Plane selection changed: faces and nodes stand down, the room and the
    /// palette follow the primary plane.
    pub(crate) fn after_plane_selection_changed(&mut self) {
        let Some(primary) = self.plane_primary() else {
            return;
        };
        self.grid.selection.clear();
        self.clear_node_selection_state();
        self.grid.last_room = primary.room;
        let material = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| primary.get(w))
            .and_then(|p| p.material);
        if material.is_some() {
            self.grid.palette_scroll_to = material;
        }
    }

    /// Select exactly `ids`.
    pub(crate) fn select_planes(&mut self, ids: Vec<PlaneId>) {
        self.grid.plane_sel = ids;
        self.after_plane_selection_changed();
    }

    /// Drop selected planes that no longer exist.
    pub(crate) fn retain_valid_planes(&mut self) {
        match self.project.active_scene().world.grid() {
            Some(world) => self.grid.plane_sel.retain(|id| id.exists(world)),
            None => self.grid.plane_sel.clear(),
        }
    }

    // ---- handles ----

    /// The handles of `frame` that can be grabbed in the view.
    fn visible_grips(frame: &Frame, top: bool) -> Vec<Handle> {
        let horizontal = |axis: V3| axis[0].hypot(axis[2]) > 0.3;
        let mut out = vec![Handle::Yaw];
        if !top {
            out.push(Handle::Pitch);
        }
        for handle in Handle::grips() {
            let usable = !top
                || match handle {
                    Handle::Edge(Side::Left | Side::Right) => horizontal(frame.right),
                    Handle::Edge(_) => horizontal(frame.up),
                    Handle::Corner(..) => horizontal(frame.right) && horizontal(frame.up),
                    _ => true,
                };
            if usable {
                out.push(handle);
            }
        }
        out
    }

    /// Screen positions of a plane's handles in the pointer's view.
    pub(crate) fn plane_grip_screen(
        &self,
        id: PlaneId,
        view: GridView,
    ) -> Option<(Frame, Vec<(Handle, Pos2)>)> {
        let world = self.project.active_scene().world.grid()?;
        let room = world.rooms.get(id.room)?;
        let plane = id.get(world)?;
        let frame = Frame::of(plane, room.y_offset);
        let scale = world.scale;
        let top = matches!(view, GridView::Top(_));
        let cam = match view {
            GridView::Persp(rect) => Some(Cam::new(self.viewport_3d_camera(), rect)),
            GridView::Top(_) => None,
        };
        let project = |p: V3| -> Option<Pos2> {
            let e = to_editor(scale, p);
            match (&cam, view) {
                (Some(cam), _) => cam.project(e).map(|(s, _)| s),
                (None, GridView::Top(t)) => Some(t.world_to_screen([e[0], e[2]])),
                _ => None,
            }
        };
        let points = Self::visible_grips(&frame, top)
            .into_iter()
            .filter_map(|h| Some((h, project(pm::handle_position(&frame, h))?)))
            .collect();
        Some((frame, points))
    }

    /// The handle of the primary selected plane under the pointer.
    pub(crate) fn grip_at(&self, ptr: &GridPointer) -> Option<(PlaneId, Handle)> {
        let id = self.plane_primary()?;
        let (_, points) = self.plane_grip_screen(id, ptr.view)?;
        points
            .into_iter()
            .map(|(h, s)| (h, s.distance(ptr.pos)))
            .filter(|(_, d)| *d <= GRIP_PIXELS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| (id, h))
    }

    // ---- placement ----

    /// The heading of the 3D camera snapped to a quarter turn, degrees.
    fn camera_heading(&self, ptr: &GridPointer) -> i32 {
        if ptr.is_top() {
            return 0;
        }
        let forward = self.viewport_3d_camera().basis().forward;
        let degrees = f64::from(forward[0])
            .atan2(f64::from(forward[2]))
            .to_degrees();
        ((degrees / 90.0).round() * 90.0) as i32
    }

    /// Origin (lifted off the surface), rotation and surface normal a press
    /// at the pointer would place a plane with, absolute grid units.
    fn placement(&self, ptr: &GridPointer, room: usize) -> Option<(V3, [i32; 3], V3)> {
        let scale = self.project.active_scene().world.grid()?.scale;
        let (point, normal) = self.plane_surface(ptr, room)?;
        let hit = to_grid(scale, point);
        let n = normal.map(f64::from);
        let rotation = rotation_facing(n, self.camera_heading(ptr));
        let snap = Snap::from_modifiers(ptr.modifiers);
        // Snap the two axes along the surface, then slide along the normal
        // to stay on its plane.
        let dominant = (0..3)
            .max_by(|a, b| n[*a].abs().total_cmp(&n[*b].abs()))
            .unwrap_or(1);
        let mut snapped = hit;
        for k in 0..3 {
            if k != dominant {
                snapped[k] = snap.apply(hit[k]);
            }
        }
        if n[dominant].abs() > 1e-6 {
            let slide: f64 = (0..3)
                .filter(|k| *k != dominant)
                .map(|k| n[k] * (snapped[k] - hit[k]))
                .sum();
            snapped[dominant] = hit[dominant] - slide / n[dominant];
        }
        Some((pm::add(snapped, pm::scale(n, PLACE_OFFSET)), rotation, n))
    }

    /// The plane a press-and-release at `place.current` makes.
    pub(crate) fn placed_plane(&self, place: &PlacePlane) -> Option<GridPlane> {
        let snap = place.snap;
        let world = self.project.active_scene().world.grid()?;
        let y_offset = world.rooms.get(place.room)?.y_offset;
        let mut plane = GridPlane::new([0; 3], DEFAULT_PLANE_SIZE);
        plane.rotation = place.rotation;
        plane.collision = self.grid.new_plane.collision;
        plane.double_sided = self.grid.new_plane.double_sided;
        plane.material = self.grid_new_material(place.room, true);
        let frame = Frame::of(&plane, 0);
        let d = pm::sub(place.current, place.origin);
        let (a, b) = (pm::dot(d, frame.right), pm::dot(d, frame.up));
        let centre = if a.abs() < CLICK_SLOP && b.abs() < CLICK_SLOP {
            place.origin
        } else {
            let extent = |v: f64| {
                snap.apply(v.abs())
                    .max(f64::from(DEFAULT_PLANE_SIZE[0]) / 4.0)
            };
            let (sa, sb) = (extent(a), extent(b));
            plane.size = [sa as i32, sb as i32];
            pm::add(
                place.origin,
                pm::add(
                    pm::scale(frame.right, a.signum() * sa / 2.0),
                    pm::scale(frame.up, b.signum() * sb / 2.0),
                ),
            )
        };
        plane.center = [
            centre[0].round() as i32,
            centre[1].round() as i32 - y_offset,
            centre[2].round() as i32,
        ];
        plane.normalize();
        Some(plane)
    }

    // ---- gestures ----

    fn begin_place(&mut self, ptr: &GridPointer) -> bool {
        let room = self.grid_target_room();
        if self
            .project
            .active_scene()
            .world
            .grid()
            .is_none_or(|w| w.rooms.is_empty())
        {
            self.status = "Draw a floor first: a plane needs a room".to_string();
            return false;
        }
        let at_press = ptr.at_press();
        let Some((origin, rotation, _)) = self.placement(&at_press, room) else {
            self.status = "Point at a surface to place a plane".to_string();
            return false;
        };
        self.grid.gesture = Gesture::Plane(PlaneGesture::Place(PlacePlane {
            room,
            origin,
            rotation,
            current: origin,
            snap: Snap::from_modifiers(ptr.modifiers),
        }));
        true
    }

    fn drag_place(&mut self, ptr: &GridPointer) {
        let Gesture::Plane(PlaneGesture::Place(place)) = &self.grid.gesture else {
            return;
        };
        let (origin, rotation) = (place.origin, place.rotation);
        let mut probe = GridPlane::new([0; 3], DEFAULT_PLANE_SIZE);
        probe.rotation = rotation;
        let normal = Frame::of(&probe, 0).normal;
        let Some(point) = self.pointer_on_plane(ptr, origin, normal) else {
            return;
        };
        if let Gesture::Plane(PlaneGesture::Place(place)) = &mut self.grid.gesture {
            place.current = point;
            place.snap = Snap::from_modifiers(ptr.modifiers);
        }
    }

    fn finish_place(&mut self) {
        let Gesture::Plane(PlaneGesture::Place(place)) = std::mem::take(&mut self.grid.gesture)
        else {
            return;
        };
        let Some(plane) = self.placed_plane(&place) else {
            return;
        };
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        match add_plane(world, place.room, plane) {
            Ok(id) => {
                self.history.record(before);
                self.select_planes(vec![id]);
                self.status = format!(
                    "Placed plane {} in room {}: drag its handles, set Collision in the Inspector",
                    id.index, id.room
                );
                self.grid_changed();
            }
            Err(refusal) => self.status = refusal.describe(),
        }
    }

    fn begin_grip(&mut self, ptr: &GridPointer, id: PlaneId, handle: Handle) -> bool {
        let Some(world) = self.project.active_scene().world.grid() else {
            return false;
        };
        let (Some(room), Some(plane)) = (world.rooms.get(id.room), id.get(world)) else {
            return false;
        };
        let y_offset = room.y_offset;
        let frame = Frame::of(plane, y_offset);
        let Some(press_point) = self.grip_point(&ptr.at_press(), &frame, handle) else {
            return false;
        };
        let peers = if handle == Handle::Body {
            self.grid
                .plane_sel
                .iter()
                .filter(|p| **p != id)
                .filter_map(|p| Some((*p, p.get(world)?.clone())))
                .collect()
        } else {
            Vec::new()
        };
        let pixels_per_unit = self.grid_pixels_per_unit(ptr, to_editor(world.scale, frame.center));
        self.grid.gesture = Gesture::Plane(PlaneGesture::Grip(Box::new(GripDrag {
            id,
            handle,
            original: plane.clone(),
            y_offset,
            peers,
            press_point,
            press_y: ptr.press.unwrap_or(ptr.pos).y,
            pixels_per_unit,
            before: None,
        })));
        true
    }

    fn drag_grip(&mut self, ptr: &GridPointer) {
        let Gesture::Plane(PlaneGesture::Grip(grip)) = std::mem::take(&mut self.grid.gesture)
        else {
            return;
        };
        let mut grip = *grip;
        let snap = Snap::from_modifiers(ptr.modifiers);
        let frame = Frame::of(&grip.original, grip.y_offset);
        let point = self.grip_point(ptr, &frame, grip.handle);
        let edited = match (grip.handle, point) {
            (Handle::Body, _) => {
                let delta = if ptr.shift() {
                    let dy = f64::from(grip.press_y - ptr.pos.y)
                        / f64::from(grip.pixels_per_unit.max(1e-4));
                    [0.0, dy, 0.0]
                } else {
                    let Some(p) = point else {
                        self.grid.gesture = Gesture::Plane(PlaneGesture::Grip(Box::new(grip)));
                        return;
                    };
                    let mut d = pm::sub(p, grip.press_point);
                    if ptr.is_top() {
                        d[1] = 0.0;
                    }
                    d
                };
                Some(pm::translate(&grip.original, grip.y_offset, delta, snap))
            }
            (Handle::Yaw, Some(p)) => Some(pm::yaw_toward(&grip.original, grip.y_offset, p, snap)),
            (Handle::Pitch, Some(p)) => {
                Some(pm::pitch_toward(&grip.original, grip.y_offset, p, snap))
            }
            (h @ (Handle::Edge(_) | Handle::Corner(..)), Some(p)) => {
                Some(pm::stretch(&grip.original, grip.y_offset, h, p, snap))
            }
            _ => None,
        };
        if let Some(edited) = edited {
            self.apply_grip(&mut grip, edited);
        }
        self.grid.gesture = Gesture::Plane(PlaneGesture::Grip(Box::new(grip)));
    }

    /// Write `edited` over the dragged plane (and carry the peers along).
    fn apply_grip(&mut self, grip: &mut GripDrag, edited: GridPlane) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let moved = grip.id.get(world).is_none_or(|p| *p != edited);
        if !moved {
            return;
        }
        if grip.before.is_none() {
            grip.before = Some(Box::new(self.project.clone()));
        }
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        let shift = [
            edited.center[0] - grip.original.center[0],
            edited.center[1] - grip.original.center[1],
            edited.center[2] - grip.original.center[2],
        ];
        if !update_plane(world, grip.id, |p| *p = edited.clone()) {
            return;
        }
        for (peer, original) in &grip.peers {
            update_plane(world, *peer, |p| {
                *p = original.clone();
                for (c, d) in p.center.iter_mut().zip(shift) {
                    *c += d;
                }
            });
        }
        self.status = grip_status(grip.handle, &edited);
        self.grid_changed();
    }

    fn finish_grip(&mut self) {
        if let Gesture::Plane(PlaneGesture::Grip(grip)) = std::mem::take(&mut self.grid.gesture) {
            if let Some(before) = grip.before {
                self.history.record(*before);
            }
        }
    }
}

fn grip_status(handle: Handle, plane: &GridPlane) -> String {
    match handle {
        Handle::Edge(_) | Handle::Corner(..) => {
            format!("Plane size {} x {}", plane.size[0], plane.size[1])
        }
        Handle::Yaw | Handle::Pitch => format!(
            "Plane yaw {} pitch {} roll {}",
            plane.rotation[0], plane.rotation[1], plane.rotation[2]
        ),
        Handle::Body => format!(
            "Plane at {}, {}, {}",
            plane.center[0], plane.center[1], plane.center[2]
        ),
    }
}

// ---- the event entry points `events::dispatch` calls ----

/// Whether `tool` works on planes.
fn active_tool(ws: &EditorWorkspace) -> bool {
    matches!(ws.grid.tool, GridTool::Select | GridTool::Plane)
}

/// Pointer moved without a button. Returns whether a plane owns the hover.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    if !active_tool(ws) {
        return false;
    }
    if let Some((id, handle)) = ws.grip_at(ptr) {
        ws.grid.hover = GridHover::Grip(id, handle);
        return true;
    }
    if let Some(id) = ws.plane_under(ptr) {
        ws.grid.hover = GridHover::Plane(id);
        return true;
    }
    if ws.grid.tool == GridTool::Plane {
        let room = ws.grid_target_room();
        let spot = ws.placement(ptr, room).and_then(|(origin, rotation, _)| {
            ws.placed_plane(&PlacePlane {
                room,
                origin,
                rotation,
                current: origin,
                snap: Snap::CLICK,
            })
        });
        ws.grid.hover = match spot {
            Some(plane) => GridHover::PlaneSpot(room, Box::new(plane)),
            None => GridHover::None,
        };
        return true;
    }
    false
}

/// Primary button down. Returns whether a plane took the press.
pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    ws.grid.plane_press = false;
    if !active_tool(ws) || !matches!(ws.grid.gesture, Gesture::Idle) {
        return false;
    }
    let at = ptr.at_press();
    if let Some((id, handle)) = ws.grip_at(&at) {
        if ws.begin_grip(ptr, id, handle) {
            ws.grid.plane_press = true;
            return true;
        }
    }
    if let Some(id) = ws.plane_under(&at) {
        let selected = ws.grid.plane_sel.contains(&id);
        ws.grid.plane_press = true;
        if ptr.command() {
            // Toggle, no drag.
            if selected {
                ws.grid.plane_sel.retain(|p| *p != id);
            } else {
                ws.grid.plane_sel.push(id);
            }
            ws.after_plane_selection_changed();
            return true;
        }
        if ptr.shift() && !selected {
            ws.grid.plane_sel.push(id);
            ws.after_plane_selection_changed();
            return true;
        }
        if !selected {
            ws.select_planes(vec![id]);
        } else if ws.plane_primary() != Some(id) {
            ws.grid.plane_sel.retain(|p| *p != id);
            ws.grid.plane_sel.push(id);
        }
        ws.begin_grip(ptr, id, Handle::Body);
        ws.status = "Drag to slide the plane, Shift to move it up and down".to_string();
        return true;
    }
    if ws.grid.tool == GridTool::Plane && ws.begin_place(ptr) {
        ws.grid.plane_press = true;
        return true;
    }
    false
}

/// Pointer moved with the button down.
pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    match &ws.grid.gesture {
        Gesture::Plane(PlaneGesture::Place(_)) => {
            ws.drag_place(ptr);
            true
        }
        Gesture::Plane(PlaneGesture::Grip(_)) => {
            ws.drag_grip(ptr);
            true
        }
        _ => false,
    }
}

/// Primary button up.
pub(super) fn release(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    match &ws.grid.gesture {
        Gesture::Plane(PlaneGesture::Place(_)) => {
            ws.drag_place(ptr);
            ws.finish_place();
            true
        }
        Gesture::Plane(PlaneGesture::Grip(_)) => {
            ws.finish_grip();
            true
        }
        _ => false,
    }
}

/// A click (egui reports a press that never moved past the drag threshold
/// only as a click, with no press before it): consumed when the press that
/// began it was a plane's; otherwise a click on a plane selects it, and with
/// the Plane tool a click on a surface places the default plane there.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    if std::mem::take(&mut ws.grid.plane_press) {
        return true;
    }
    if !active_tool(ws) {
        return false;
    }
    if ws.grip_at(ptr).is_some() {
        return true;
    }
    if let Some(id) = ws.plane_under(ptr) {
        let selected = ws.grid.plane_sel.contains(&id);
        if ptr.command() {
            if selected {
                ws.grid.plane_sel.retain(|p| *p != id);
            } else {
                ws.grid.plane_sel.push(id);
            }
        } else if ptr.shift() {
            ws.grid.plane_sel.retain(|p| *p != id);
            ws.grid.plane_sel.push(id);
        } else {
            ws.grid.plane_sel = vec![id];
        }
        ws.after_plane_selection_changed();
        ws.status = format!("{} plane(s) selected", ws.grid.plane_sel.len());
        return true;
    }
    if ws.grid.tool == GridTool::Plane && ws.begin_place(ptr) {
        ws.finish_place();
        return true;
    }
    false
}

/// Escape: cancel a plane gesture, undoing what a drag already did.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Plane(PlaneGesture::Grip(grip)) => {
            if let Some(before) = grip.before {
                ws.project = *before;
                ws.grid_changed();
            }
            true
        }
        Gesture::Plane(PlaneGesture::Place(_)) => true,
        other => {
            ws.grid.gesture = other;
            false
        }
    }
}

// ---- editing the selection ----

/// Delete: remove the selected planes. Returns whether any were selected.
pub(super) fn delete_selected(ws: &mut EditorWorkspace) -> bool {
    ws.retain_valid_planes();
    if ws.grid.plane_sel.is_empty() {
        return false;
    }
    let ids = std::mem::take(&mut ws.grid.plane_sel);
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let removed = remove_planes(world, &ids);
    if removed > 0 {
        ws.history.record(before);
        ws.status = format!(
            "Deleted {removed} plane{}",
            if removed == 1 { "" } else { "s" }
        );
        ws.grid_changed();
    }
    true
}

/// Move the selected planes up or down by `amount` grid units.
pub(super) fn nudge_selected(ws: &mut EditorWorkspace, amount: i32) -> bool {
    ws.retain_valid_planes();
    if ws.grid.plane_sel.is_empty() {
        return false;
    }
    let ids = ws.grid.plane_sel.clone();
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let mut changed = false;
    for id in ids {
        changed |= update_plane(world, id, |p| p.center[1] += amount);
    }
    if changed {
        ws.history.record(before);
        ws.grid_changed();
    }
    true
}

/// Turn the selected planes a quarter about their own vertical axes.
pub(super) fn turn_selected(ws: &mut EditorWorkspace) -> bool {
    ws.retain_valid_planes();
    if ws.grid.plane_sel.is_empty() {
        return false;
    }
    let ids = ws.grid.plane_sel.clone();
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let mut changed = false;
    for id in ids {
        changed |= update_plane(world, id, |p| p.rotation[0] += 90);
    }
    if changed {
        ws.history.record(before);
        ws.status = "Turned the plane a quarter".to_string();
        ws.grid_changed();
    }
    true
}

/// Ctrl+C with planes selected. Returns whether it copied.
pub(super) fn copy(ws: &mut EditorWorkspace) -> bool {
    ws.retain_valid_planes();
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    match PlaneClipboard::capture(world, &ws.grid.plane_sel) {
        Some(clip) => {
            ws.status = format!("Copied {} planes", clip.planes.len());
            ws.grid.plane_clip = Some(clip);
            ws.grid.clipboard = None;
            true
        }
        None => false,
    }
}

/// Ctrl+V with planes on the clipboard. Returns whether a ghost started.
pub(super) fn begin_paste(ws: &mut EditorWorkspace) -> bool {
    let Some(clip) = ws.grid.plane_clip.clone() else {
        return false;
    };
    ws.grid.gesture = Gesture::Idle;
    ws.grid.paste = None;
    ws.grid.plane_paste = Some(PlanePaste { clip, at: None });
    ws.status = "Paste planes: click to place, R turn, H flip, Esc cancel".to_string();
    true
}

/// Where the paste ghost hangs for the pointer, grid units.
fn paste_spot(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<[i32; 2]> {
    let room = ws.grid_target_room();
    let (point, _) = ws.plane_surface(ptr, room)?;
    let scale = ws.project.active_scene().world.grid()?.scale;
    let g = to_grid(scale, point);
    let snap = Snap::from_modifiers(ptr.modifiers);
    Some([snap.apply(g[0]) as i32, snap.apply(g[2]) as i32])
}

pub(super) fn paste_hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let at = paste_spot(ws, ptr);
    if let Some(ghost) = ws.grid.plane_paste.as_mut() {
        ghost.at = at;
    }
}

pub(super) fn paste_click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    paste_hover(ws, ptr);
    let Some(ghost) = ws.grid.plane_paste.clone() else {
        return;
    };
    let Some(at) = ghost.at else { return };
    let room = ws.grid_target_room();
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let (ids, refusal): (Vec<PlaneId>, Option<PlaneRefusal>) = ghost.clip.paste(world, room, at);
    if let Some(refusal) = refusal {
        ws.status = format!("Cannot paste there: {}", refusal.describe());
    } else {
        ws.status = format!("Pasted {} planes", ids.len());
    }
    if !ids.is_empty() {
        ws.history.record(before);
        ws.select_planes(ids);
        ws.grid_changed();
    }
    if !ptr.shift() {
        ws.grid.plane_paste = None;
    }
}

/// H, V and R while the plane ghost is up.
pub(super) fn transform_ghost(ws: &mut EditorWorkspace, key: egui::Key) {
    if let Some(ghost) = ws.grid.plane_paste.as_mut() {
        match key {
            egui::Key::R => ghost.clip.turn(),
            egui::Key::H => ghost.clip.flip_x(),
            _ => {}
        }
    }
}

// ---- painting ----

/// The plane the Paint tool is over, if it is in front of the faces.
pub(super) fn paint_target(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<PlaneId> {
    ws.plane_under(ptr)
}

/// Apply `material` to a plane, for painting. Returns whether it changed.
pub(super) fn paint(ws: &mut EditorWorkspace, id: PlaneId, material: Option<ResourceId>) -> bool {
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    psxed_project::grid::edit::set_plane_material(world, id, material)
}
