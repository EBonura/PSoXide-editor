//! Where the pointer is: the Top view or the 3D view, and the geometry to
//! turn a screen position into the grid.
//!
//! Tools are written once against [`GridPointer`]; the two views differ only
//! in how a screen position reaches a sector (an orthographic plan lookup, or
//! a ray through the perspective camera) and in how a height gesture maps
//! pointer travel to grid units.

use super::*;

use psxed_project::grid::{GridScale, GridWorld, SECTOR_UNITS};

/// The view a pointer event came from.
#[derive(Clone, Copy)]
pub(crate) enum GridView {
    /// The orthographic Top view (the plan).
    Top(ViewportTransform),
    /// The 3D preview; the rectangle the camera image fills.
    Persp(Rect),
}

/// One pointer event, in whichever view it happened.
#[derive(Clone, Copy)]
pub(crate) struct GridPointer {
    /// The view.
    pub view: GridView,
    /// Pointer position.
    pub pos: Pos2,
    /// Where the primary button went down, when it is down.
    pub press: Option<Pos2>,
    /// Modifier keys.
    pub modifiers: egui::Modifiers,
}

impl GridPointer {
    /// Whether this is the Top view.
    pub fn is_top(&self) -> bool {
        matches!(self.view, GridView::Top(_))
    }

    /// The same pointer at the press position.
    pub fn at_press(&self) -> Self {
        Self {
            pos: self.press.unwrap_or(self.pos),
            ..*self
        }
    }

    /// Whether Shift is held.
    pub fn shift(&self) -> bool {
        self.modifiers.shift
    }

    /// Whether Ctrl or Cmd is held.
    pub fn command(&self) -> bool {
        self.modifiers.command || self.modifiers.ctrl
    }

    /// Whether Alt is held.
    pub fn alt(&self) -> bool {
        self.modifiers.alt
    }
}

/// World sector containing editor-space `(x, z)`.
pub(crate) fn cell_at_editor(scale: GridScale, x: f32, z: f32) -> [i32; 2] {
    let per_cell = f64::from(scale.num) * f64::from(SECTOR_UNITS) / f64::from(scale.den.max(1));
    [
        (f64::from(x) / per_cell).floor() as i32,
        (f64::from(z) / per_cell).floor() as i32,
    ]
}

/// Editor-space position of a grid coordinate (`x`, `z` in grid units).
pub(crate) fn editor_xz(scale: GridScale, gx: i32, gz: i32) -> [f32; 2] {
    [scale.to_editor(gx) as f32, scale.to_editor(gz) as f32]
}

/// Editor-space height of a room-relative grid height.
pub(crate) fn editor_y(world: &GridWorld, room_y_offset: i32, height: i32) -> f32 {
    world.scale.to_editor(room_y_offset.saturating_add(height)) as f32
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Distance along `dir` at which the ray hits triangle `abc`, if it does.
pub(crate) fn ray_triangle(
    origin: [f32; 3],
    dir: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> Option<f32> {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let t = sub(origin, a);
    let u = dot(t, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(t, e1);
    let v = dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = dot(e2, q) * inv;
    (distance > 1e-3).then_some(distance)
}

/// Screen position and camera depth of a world point, or `None` at or behind
/// the camera plane.
pub(crate) fn project_point(
    camera: ViewportCameraState,
    rect: Rect,
    world: [f32; 3],
) -> Option<(Pos2, f32)> {
    let basis = camera.basis();
    let v = sub(world, basis.position);
    let depth = dot(v, basis.forward);
    if depth <= 1.0 {
        return None;
    }
    let (nx, ny) = camera.normalized_panel_point_for_world(world)?;
    Some((
        Pos2::new(
            rect.center().x + nx * rect.width() * 0.5,
            rect.center().y + ny * rect.height() * 0.5,
        ),
        depth,
    ))
}

/// A world segment clipped to the near plane and projected to the screen.
pub(crate) fn project_segment(
    camera: ViewportCameraState,
    rect: Rect,
    a: [f32; 3],
    b: [f32; 3],
) -> Option<[Pos2; 2]> {
    const NEAR: f32 = 4.0;
    let basis = camera.basis();
    let da = dot(sub(a, basis.position), basis.forward);
    let db = dot(sub(b, basis.position), basis.forward);
    if da <= NEAR && db <= NEAR {
        return None;
    }
    let lerp = |from: [f32; 3], to: [f32; 3], t: f32| {
        [
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
            from[2] + (to[2] - from[2]) * t,
        ]
    };
    let (a, b) = if da <= NEAR {
        (lerp(a, b, (NEAR - da) / (db - da)), b)
    } else if db <= NEAR {
        (a, lerp(a, b, (NEAR - da) / (db - da)))
    } else {
        (a, b)
    };
    Some([
        project_point(camera, rect, a)?.0,
        project_point(camera, rect, b)?.0,
    ])
}

/// Where a camera ray meets the horizontal plane `y`, in editor units.
pub(crate) fn ray_plane_y(origin: [f32; 3], dir: [f32; 3], y: f32) -> Option<[f32; 3]> {
    if dir[1].abs() < 1e-6 {
        return None;
    }
    let t = (y - origin[1]) / dir[1];
    (t > 0.0).then(|| [origin[0] + dir[0] * t, y, origin[2] + dir[2] * t])
}

impl EditorWorkspace {
    /// The camera ray under a 3D-view pointer.
    pub(crate) fn grid_ray(&self, rect: Rect, pos: Pos2) -> Option<([f32; 3], [f32; 3])> {
        self.camera_ray_for_pointer(rect, pos)
    }

    /// Editor-space point under the pointer on the horizontal plane `y`
    /// (editor units). In the Top view the plane is implied.
    pub(crate) fn grid_plane_point(&self, ptr: &GridPointer, y: f32) -> Option<[f32; 3]> {
        match ptr.view {
            GridView::Top(transform) => {
                let [x, z] = transform.screen_to_world(ptr.pos);
                Some([x, y, z])
            }
            GridView::Persp(rect) => {
                let (origin, dir) = self.grid_ray(rect, ptr.pos)?;
                ray_plane_y(origin, dir, y)
            }
        }
    }

    /// Pixels the pointer must travel vertically to change `world_point`'s
    /// height by one grid unit, in the 3D view. Falls back to a fixed rate
    /// in the Top view, where a click is [`TOP_PIXELS_PER_CLICK`] pixels.
    pub(crate) fn grid_pixels_per_unit(&self, ptr: &GridPointer, world_point: [f32; 3]) -> f32 {
        use psxed_project::grid::CLICK_UNITS;
        match ptr.view {
            GridView::Top(_) => TOP_PIXELS_PER_CLICK / CLICK_UNITS as f32,
            GridView::Persp(rect) => {
                let Some(world) = self.project.active_scene().world.grid() else {
                    return 0.1;
                };
                let camera = self.viewport_3d_camera();
                let one_click = world.scale.to_editor(CLICK_UNITS) as f32;
                let (Some((a, _)), Some((b, _))) = (
                    project_point(camera, rect, world_point),
                    project_point(
                        camera,
                        rect,
                        [world_point[0], world_point[1] + one_click, world_point[2]],
                    ),
                ) else {
                    return 0.1;
                };
                ((a.y - b.y).abs() / CLICK_UNITS as f32).max(0.002)
            }
        }
    }
}

/// Pointer travel that counts as one height click in the Top view.
pub(crate) const TOP_PIXELS_PER_CLICK: f32 = 18.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_follow_the_scale_and_floor_towards_negative() {
        let scale = GridScale { num: 21, den: 16 };
        assert_eq!(cell_at_editor(scale, 0.0, 0.0), [0, 0]);
        assert_eq!(cell_at_editor(scale, 1343.0, 1344.0), [0, 1]);
        assert_eq!(cell_at_editor(scale, -1.0, -1344.0), [-1, -1]);
        assert_eq!(cell_at_editor(scale, -1345.0, 0.0), [-2, 0]);
    }

    #[test]
    fn rays_hit_triangles_in_front_only() {
        let a = [0.0, 0.0, 0.0];
        let b = [10.0, 0.0, 0.0];
        let c = [0.0, 0.0, 10.0];
        let hit = ray_triangle([2.0, 5.0, 2.0], [0.0, -1.0, 0.0], a, b, c);
        assert!((hit.unwrap() - 5.0).abs() < 1e-4);
        assert!(ray_triangle([8.0, 5.0, 8.0], [0.0, -1.0, 0.0], a, b, c).is_none());
        assert!(ray_triangle([2.0, 5.0, 2.0], [0.0, 1.0, 0.0], a, b, c).is_none());
    }

    #[test]
    fn plane_hits_need_a_downward_ray_for_a_lower_plane() {
        let hit = ray_plane_y([0.0, 10.0, 0.0], [0.0, -0.5, 0.5], 0.0).unwrap();
        assert!((hit[2] - 10.0).abs() < 1e-3);
        assert!(ray_plane_y([0.0, 10.0, 0.0], [0.0, 0.5, 0.5], 0.0).is_none());
    }
}
