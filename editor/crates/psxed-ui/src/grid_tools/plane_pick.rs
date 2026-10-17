//! Picking free planes: the cache entries, the 3D ray test and the Top view
//! outline test, and the combined "what surface is under the pointer" the
//! Plane tool places on.

use super::pick::FaceHit;
use super::view::{ray_plane_y, ray_triangle, GridPointer, GridView};
use super::*;

use psxed_project::grid::edit::PlaneId;
use psxed_project::grid::GridWorld;

/// A plane, as the picker sees it.
pub(crate) struct PickPlane {
    pub id: PlaneId,
    /// Corners top-left, top-right, bottom-right, bottom-left, editor units.
    pub points: [[f32; 3]; 4],
    /// Unit front normal.
    pub normal: [f32; 3],
}

/// Editor-space corners of a plane (the same rounding the faces use).
pub(crate) fn plane_points(
    world: &GridWorld,
    y_offset: i32,
    plane: &psxed_project::grid::GridPlane,
) -> [[f32; 3]; 4] {
    plane.corners().map(|g| {
        [
            world.scale.to_editor(g[0]) as f32,
            world.scale.to_editor(y_offset + g[1]) as f32,
            world.scale.to_editor(g[2]) as f32,
        ]
    })
}

pub(crate) fn build_planes(world: &GridWorld) -> Vec<PickPlane> {
    let mut out = Vec::new();
    for (room_index, room) in world.rooms.iter().enumerate() {
        for (index, plane) in room.planes.iter().enumerate() {
            if !plane.enabled {
                continue;
            }
            out.push(PickPlane {
                id: PlaneId {
                    room: room_index,
                    index,
                },
                points: plane_points(world, room.y_offset, plane),
                normal: plane.normal().map(|v| v as f32),
            });
        }
    }
    out
}

/// What a 3D pointer touches.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceHit {
    /// Hit point, editor units.
    pub point: [f32; 3],
    /// Unit normal on the side the ray came from.
    pub normal: [f32; 3],
    /// Distance along the ray.
    pub depth: f32,
    /// The plane hit, when the surface is a free plane.
    pub plane: Option<PlaneId>,
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = (((p - a).dot(ab)) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn inside_polygon(p: Pos2, polygon: &[Pos2]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl EditorWorkspace {
    /// Nearest plane under a 3D pointer, from either side.
    pub(crate) fn pick_plane_3d(&self, rect: Rect, pos: Pos2) -> Option<SurfaceHit> {
        let (origin, dir) = self.camera_ray_for_pointer(rect, pos)?;
        self.with_pick_cache(|cache| {
            let mut best: Option<SurfaceHit> = None;
            for pp in &cache.planes {
                if self.grid_hidden_rooms.contains(&pp.id.room) {
                    continue;
                }
                let p = &pp.points;
                let hit = ray_triangle(origin, dir, p[0], p[1], p[2])
                    .or_else(|| ray_triangle(origin, dir, p[0], p[2], p[3]));
                let Some(depth) = hit else { continue };
                if best.is_none_or(|b| depth < b.depth) {
                    let facing =
                        pp.normal[0] * dir[0] + pp.normal[1] * dir[1] + pp.normal[2] * dir[2];
                    let flip = if facing > 0.0 { -1.0 } else { 1.0 };
                    best = Some(SurfaceHit {
                        point: [
                            origin[0] + dir[0] * depth,
                            origin[1] + dir[1] * depth,
                            origin[2] + dir[2] * depth,
                        ],
                        normal: pp.normal.map(|v| v * flip),
                        depth,
                        plane: Some(pp.id),
                    });
                }
            }
            best
        })
        .flatten()
    }

    /// The nearest face or plane under a 3D pointer.
    pub(crate) fn pick_surface_3d(&self, rect: Rect, pos: Pos2) -> Option<SurfaceHit> {
        let face: Option<FaceHit> = self.pick_face_3d(rect, pos);
        let plane = self.pick_plane_3d(rect, pos);
        let from_face = |hit: FaceHit| SurfaceHit {
            point: hit.point,
            normal: hit.normal,
            depth: hit.depth,
            plane: None,
        };
        match (face, plane) {
            (Some(f), Some(p)) if f.depth < p.depth => Some(from_face(f)),
            (_, Some(p)) => Some(p),
            (Some(f), None) => Some(from_face(f)),
            (None, None) => None,
        }
    }

    /// The planes the pointer is on in the Top view: inside the outline or
    /// within `pixels` of it. The highest comes first.
    pub(crate) fn top_planes_at(
        &self,
        transform: ViewportTransform,
        pos: Pos2,
        pixels: f32,
    ) -> Vec<PlaneId> {
        self.with_pick_cache(|cache| {
            let mut found: Vec<(f32, PlaneId)> = Vec::new();
            for pp in &cache.planes {
                if self.grid_hidden_rooms.contains(&pp.id.room) {
                    continue;
                }
                let screen: Vec<Pos2> = pp
                    .points
                    .iter()
                    .map(|p| transform.world_to_screen([p[0], p[2]]))
                    .collect();
                let near = (0..4)
                    .any(|i| distance_to_segment(pos, screen[i], screen[(i + 1) % 4]) <= pixels);
                if !(near || inside_polygon(pos, &screen)) {
                    continue;
                }
                let top = pp.points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                found.push((top, pp.id));
            }
            found.sort_by(|a, b| b.0.total_cmp(&a.0));
            found.into_iter().map(|(_, id)| id).collect()
        })
        .unwrap_or_default()
    }

    /// The plane a pointer is over, in whichever view, when it is in front of
    /// the faces behind it. In the Top view a repeated click takes the next
    /// thing down (see `cycle`), so a plane only answers when it is the one
    /// the pointer's cycle step reaches.
    pub(crate) fn plane_under(&self, ptr: &GridPointer) -> Option<PlaneId> {
        match ptr.view {
            GridView::Top(_) => match self.top_choice(ptr) {
                cycle::TopChoice::Plane(id) => Some(id),
                cycle::TopChoice::Layer(_) => None,
            },
            GridView::Persp(rect) => {
                let plane = self.pick_plane_3d(rect, ptr.pos)?;
                match self.pick_face_3d(rect, ptr.pos) {
                    Some(face) if face.depth < plane.depth => None,
                    _ => plane.plane,
                }
            }
        }
    }

    /// Where a new plane would go for the pointer: the surface point and its
    /// normal in editor units, or the horizontal plane at the room's floor
    /// level when the pointer touches nothing.
    pub(crate) fn plane_surface(
        &self,
        ptr: &GridPointer,
        room: usize,
    ) -> Option<([f32; 3], [f32; 3])> {
        let world = self.project.active_scene().world.grid()?;
        let level = world.rooms.get(room).map_or(0, |r| r.y_offset);
        let level = world.scale.to_editor(level) as f32;
        match ptr.view {
            GridView::Top(transform) => {
                let [x, z] = transform.screen_to_world(ptr.pos);
                let y = world
                    .floor_height_at(x.round() as i32, z.round() as i32, i32::MAX / 2)
                    .map_or(level, |h| h as f32);
                Some(([x, y, z], [0.0, 1.0, 0.0]))
            }
            GridView::Persp(rect) => {
                if let Some(hit) = self.pick_surface_3d(rect, ptr.pos) {
                    return Some((hit.point, hit.normal));
                }
                let (origin, dir) = self.camera_ray_for_pointer(rect, ptr.pos)?;
                ray_plane_y(origin, dir, level).map(|p| (p, [0.0, 1.0, 0.0]))
            }
        }
    }
}
