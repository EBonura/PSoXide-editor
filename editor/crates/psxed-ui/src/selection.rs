use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaterialTarget {
    BrushFace { brush: usize, face: usize },
}

pub(crate) fn describe_material_target(target: MaterialTarget) -> String {
    match target {
        MaterialTarget::BrushFace { brush, face } => {
            format!("brush {} face {}", brush + 1, face + 1)
        }
    }
}

pub(crate) fn push_unique_material_target(
    targets: &mut Vec<MaterialTarget>,
    target: MaterialTarget,
) {
    if !targets.contains(&target) {
        targets.push(target);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoxPropMaterialAssignment {
    pub(crate) material: ResourceId,
    pub(crate) targets: usize,
    pub(crate) updated: usize,
}

/// Kind label for an [`EntityBounds`]. Drives picking
/// priorities and per-kind gizmo rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityBoundKind {
    /// Model-backed `MeshInstance` with parsed model bounds.
    Model,
    /// Legacy / unbound `MeshInstance` -- fallback box.
    MeshFallback,
    /// Flat `ImageProp`.
    ImageProp,
    /// Editable boxed prop.
    BoxProp,
    /// Low-poly procedural radial prop.
    CylinderProp,
    /// Tile-snapped procedural arch.
    ArchProp,
    /// `SpawnPoint` (player or non-player).
    SpawnPoint,
    /// `PointLight`. Marker box only -- radius ring is drawn
    /// separately so a wide-radius light doesn't intercept
    /// every click in the room.
    PointLight,
    /// `ParticleEmitter`.
    ParticleEmitter,
    /// Entity hosting a procedural Point of Interest beacon.
    PointOfInterest,
    /// Ground-anchored Horizon vitality field.
    VitalityCircleHorizon,
    /// Ground-anchored Zenith vitality field.
    VitalityCircleZenith,
    /// Shared breakable state accepting Horizon damage.
    DestructibleHorizon,
    /// Shared breakable state accepting Zenith damage.
    DestructibleZenith,
    /// Shared breakable state accepting either attack channel.
    DestructibleBoth,
    /// Placed `Logic` graph node (trigger volume / relay /
    /// multisource / door).
    Logic,
}

/// World-space AABB for one selectable scene entity.
/// Nodes live in raw world units, so the bound sits at the node translation
/// lifted by its half extent and lines up with rendered models, markers, and
/// lights.
#[derive(Debug, Clone, Copy)]
pub struct EntityBounds {
    /// Owning scene-tree node id.
    pub node: NodeId,
    /// Bound class for visual styling + picking priority.
    pub kind: EntityBoundKind,
    /// World-space AABB centre.
    pub center: [f32; 3],
    /// World-space half-extents along X / Y / Z. Always
    /// positive.
    pub half_extents: [f32; 3],
    /// Authored Y rotation in degrees. Stored on the bound
    /// so the renderer can draw a facing arrow without
    /// re-walking the scene tree.
    pub yaw_degrees: f32,
}

/// Result of a successful entity-bound pick.
#[derive(Debug, Clone, Copy)]
pub struct EntityBoundHit {
    /// Hit node.
    pub node: NodeId,
    /// Distance from the ray origin to the first hit slab,
    /// in world units. Used to compare against grid hits and
    /// other entity hits.
    pub distance: f32,
    /// World-space hit point along the ray.
    pub point: [f32; 3],
    /// Bounds that produced the hit.
    pub bounds: EntityBounds,
}

/// Slab-intersection ray-vs-AABB. Returns the smallest
/// non-negative `t` for which `origin + t * dir` lands on
/// the box surface (or inside it).
///
/// * `dir` is *not* required to be unit length; the returned
///   `t` is in the same units as `dir`. When the editor uses
///   normalized rays (`camera_ray_for_pointer`), `t` lands in
///   world units.
/// * Box must have positive `half_extents`. Zero-extent boxes
///   never hit.
/// * Rays starting *inside* the box return `t = 0` so callers
///   can still pick something they're standing on.
pub fn ray_intersects_aabb(
    origin: [f32; 3],
    dir: [f32; 3],
    center: [f32; 3],
    half_extents: [f32; 3],
) -> Option<f32> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for axis in 0..3 {
        let half = half_extents[axis];
        if half <= 0.0 {
            return None;
        }
        let lo = center[axis] - half;
        let hi = center[axis] + half;
        let o = origin[axis];
        let d = dir[axis];
        if d.abs() < 1e-6 {
            // Ray parallel to this axis -- only hits if origin
            // is between the slabs.
            if o < lo || o > hi {
                return None;
            }
        } else {
            let inv = 1.0 / d;
            let t1 = (lo - o) * inv;
            let t2 = (hi - o) * inv;
            let (t_near, t_far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
            if t_near > t_min {
                t_min = t_near;
            }
            if t_far < t_max {
                t_max = t_far;
            }
            if t_min > t_max {
                return None;
            }
        }
    }
    if t_max < 0.0 {
        return None;
    }
    Some(if t_min < 0.0 { 0.0 } else { t_min })
}

/// Intersect a ray with the horizontal plane `y = plane_y`.
/// Used by the entity-drag path to project mouse-move into
/// world-space on the same plane the entity lives on.
/// Returns `None` for parallel rays or hits behind the camera.
pub fn ray_intersects_horizontal_plane(
    origin: [f32; 3],
    dir: [f32; 3],
    plane_y: f32,
) -> Option<[f32; 3]> {
    if dir[1].abs() < 1e-6 {
        return None;
    }
    let t = (plane_y - origin[1]) / dir[1];
    if t < 0.0 {
        return None;
    }
    Some([origin[0] + dir[0] * t, plane_y, origin[2] + dir[2] * t])
}

pub(crate) fn ray_intersects_axis_aligned_plane(
    origin: [f32; 3],
    dir: [f32; 3],
    normal_axis: PrimitiveGizmoAxis,
    plane_coord: f32,
) -> Option<[f32; 3]> {
    let axis = normal_axis.index();
    if dir[axis].abs() < 1e-6 {
        return None;
    }
    let t = (plane_coord - origin[axis]) / dir[axis];
    if t < 0.0 {
        return None;
    }
    let mut hit = [
        origin[0] + dir[0] * t,
        origin[1] + dir[1] * t,
        origin[2] + dir[2] * t,
    ];
    hit[axis] = plane_coord;
    Some(hit)
}

#[cfg(test)]
mod entity_bounds_tests {
    use super::ray_intersects_aabb as ray_aabb;
    use super::ray_intersects_horizontal_plane as ray_plane;

    #[test]
    fn ray_aabb_hits_centred_box() {
        // Ray along +Z toward origin AABB at distance 10.
        let t = ray_aabb(
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        assert!(t.is_some());
        // Hit should land on the near slab at t = 9.
        assert!((t.unwrap() - 9.0).abs() < 1e-3);
    }

    #[test]
    fn ray_aabb_misses_offset_box() {
        // Box offset to +X by 100 -- a +Z ray at origin misses.
        let t = ray_aabb(
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            [100.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        assert!(t.is_none());
    }

    #[test]
    fn ray_aabb_origin_inside_box_returns_zero() {
        let t = ray_aabb(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
        );
        assert_eq!(t, Some(0.0));
    }

    #[test]
    fn ray_aabb_zero_extent_never_hits() {
        let t = ray_aabb(
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
        );
        assert!(t.is_none());
    }

    #[test]
    fn ray_aabb_ray_parallel_to_slab() {
        // Ray on the X axis at Y=10, box at Y=0. Parallel +X
        // ray never enters the Y slab so it must miss.
        let t = ray_aabb(
            [0.0, 10.0, 0.0],
            [1.0, 0.0, 0.0],
            [50.0, 0.0, 0.0],
            [5.0, 5.0, 5.0],
        );
        assert!(t.is_none());
    }

    #[test]
    fn ray_aabb_nearest_of_two_boxes() {
        // Two co-axial boxes; near box at z=10, far box at
        // z=50. Nearest t corresponds to the near box.
        let near = ray_aabb(
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        let far = ray_aabb(
            [0.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 50.0],
            [1.0, 1.0, 1.0],
        );
        assert!(near.unwrap() < far.unwrap());
    }

    #[test]
    fn ray_plane_hits_horizontal_plane_below() {
        // Camera 100 above origin looking down → +Z forward,
        // -Y up. Hit floor plane y=0 at t=100.
        let p = ray_plane([0.0, 100.0, 0.0], [0.0, -1.0, 0.0], 0.0);
        assert!(p.is_some());
        let p = p.unwrap();
        assert!((p[1] - 0.0).abs() < 1e-3);
    }

    #[test]
    fn ray_plane_misses_when_parallel() {
        let p = ray_plane([0.0, 100.0, 0.0], [1.0, 0.0, 0.0], 0.0);
        assert!(p.is_none());
    }

    #[test]
    fn ray_plane_misses_when_behind_camera() {
        // Ray points away from the plane.
        let p = ray_plane([0.0, 100.0, 0.0], [0.0, 1.0, 0.0], 0.0);
        assert!(p.is_none());
    }
}
