//! Free planes: bridges, ramps, slanted walls, banners.

use psxed_project::face_uv::UvMode;
use psxed_project::grid::edit::{
    add_plane, remove_planes, set_plane_material, update_plane, PlaneId, PlaneRefusal,
};
use psxed_project::grid::plane::{MAX_PLANE_SIZE, MIN_PLANE_SIZE};
use psxed_project::grid::{GridPlane, GridWorld, PlaneCollision, PlaneShape, PlaneUvMode};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{pick_room, Materials};

/// Where and how big a plane is, either explicitly or by the two ends of its
/// long axis.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct PlaneGeometry {
    /// Centre `[x, y, z]` in grid units: x and z on the world lattice (1024
    /// per sector), y relative to the room's y_offset.
    pub center: Option<[i32; 3]>,
    /// Width and height in grid units (64 to 8192 each). For a flat plane
    /// the height axis runs along Z at yaw 0.
    pub size: Option<[i32; 2]>,
    /// `[yaw, pitch, roll]` in degrees. At zero a plane stands like the
    /// north wall of a sector facing south. Pitch 90 lays it flat facing up,
    /// -90 flat facing down; yaw then turns it about the vertical axis.
    pub rotation: Option<[i32; 3]>,
    /// Shorthand for the rotation: `wall` (upright), `floor` (pitch 90) or
    /// `ceiling` (pitch -90). Yaw and roll stay as given.
    pub orientation: Option<String>,
    /// A bridge or ramp by its ends: the midpoint of the plane's low or
    /// near end `[x, y, z]` (grid units, y room-relative). Needs `to` and
    /// `width`; computes centre, size, yaw and pitch (the plane's height
    /// axis runs from `from` to `to`, rising when `to` is higher).
    pub from: Option<[i32; 3]>,
    /// The other end's midpoint.
    pub to: Option<[i32; 3]>,
    /// Plane width across the span, grid units.
    pub width: Option<i32>,
}

/// What the span helper solved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solved {
    /// Centre.
    pub center: [i32; 3],
    /// Width, length.
    pub size: [i32; 2],
    /// Yaw, pitch, roll.
    pub rotation: [i32; 3],
}

/// Centre, size and rotation of a plane whose long axis runs between two
/// points. The plane lies flat when the ends are level and tilts to follow a
/// slope; its high side is the `to` end.
pub fn span(from: [i32; 3], to: [i32; 3], width: i32) -> Result<Solved, String> {
    let d = [
        f64::from(to[0] - from[0]),
        f64::from(to[1] - from[1]),
        f64::from(to[2] - from[2]),
    ];
    let horizontal = d[0].hypot(d[2]);
    let length = (horizontal * horizontal + d[1] * d[1]).sqrt();
    if length < f64::from(MIN_PLANE_SIZE) {
        return Err(format!(
            "from and to are {length:.0} units apart; a plane is at least {MIN_PLANE_SIZE} long"
        ));
    }
    if length > f64::from(MAX_PLANE_SIZE) {
        return Err(format!(
            "from and to are {length:.0} units apart; a plane is at most {MAX_PLANE_SIZE} long. Join several planes."
        ));
    }
    if horizontal < 1.0 {
        return Err("from and to must differ horizontally (a vertical plane is a wall: use orientation wall)".to_string());
    }
    // The height axis points uphill at heading (-sin yaw, -cos yaw), tilted
    // up by theta; pitch is 90 - theta (see GridPlane::axes).
    let theta = d[1].atan2(horizontal).to_degrees();
    let yaw = (-d[0]).atan2(-d[2]).to_degrees();
    Ok(Solved {
        center: [
            (from[0] + to[0]) / 2,
            (from[1] + to[1]) / 2,
            (from[2] + to[2]) / 2,
        ],
        size: [width, length.round() as i32],
        rotation: [yaw.round() as i32, (90.0 - theta).round() as i32, 0],
    })
}

fn parse_collision(name: &str) -> Result<PlaneCollision, String> {
    match name.to_ascii_lowercase().as_str() {
        "decor" | "none" => Ok(PlaneCollision::Decor),
        "walkable" => Ok(PlaneCollision::Walkable),
        "solid" => Ok(PlaneCollision::Solid),
        other => Err(format!(
            "unknown collision {other:?}; use decor, walkable or solid"
        )),
    }
}

fn parse_uv_mode(name: &str) -> Result<PlaneUvMode, String> {
    match name.to_ascii_lowercase().as_str() {
        "tile" => Ok(PlaneUvMode::Tile),
        "stretch" => Ok(PlaneUvMode::Stretch),
        other => Err(format!("unknown uv_mode {other:?}; use tile or stretch")),
    }
}

fn refusal(refusal: PlaneRefusal) -> String {
    refusal.describe()
}

/// The finished geometry of a request over a base plane.
fn apply_geometry(plane: &mut GridPlane, geometry: &PlaneGeometry) -> Result<(), String> {
    if let (Some(from), Some(to)) = (geometry.from, geometry.to) {
        let width = geometry.width.or(Some(plane.size[0])).unwrap_or(1024);
        let solved = span(from, to, width)?;
        plane.center = solved.center;
        plane.size = solved.size;
        plane.rotation = solved.rotation;
    } else if geometry.from.is_some() || geometry.to.is_some() {
        return Err("a span needs both from and to".to_string());
    }
    if let Some(center) = geometry.center {
        plane.center = center;
    }
    if let Some(size) = geometry.size {
        plane.size = size;
    }
    if let Some(rotation) = geometry.rotation {
        plane.rotation = rotation;
    }
    if let Some(orientation) = &geometry.orientation {
        plane.rotation[1] = match orientation.to_ascii_lowercase().as_str() {
            "wall" => 0,
            "floor" => 90,
            "ceiling" => -90,
            other => {
                return Err(format!(
                    "unknown orientation {other:?}; use wall, floor or ceiling"
                ))
            }
        };
    }
    Ok(())
}

/// Describe a plane in one block.
pub fn describe(index: usize, plane: &GridPlane, mats: &Materials<'_>) -> String {
    let shape = match plane.shape() {
        PlaneShape::None => "no collision",
        PlaneShape::Platform => "platform (stand on it)",
        PlaneShape::Wall => "wall segment",
        PlaneShape::Blocker => "blocker box (over-blocks; cook warns)",
    };
    let (lo, hi) = plane.cell_span();
    format!(
        "plane {index}: center {:?}, size {:?}, rotation [yaw, pitch, roll] {:?}, normal y {:.2}\n  material {}, uv {}{}, {}, collision {:?} -> {shape}, camera pass {:?}{}{}\n  covers sectors {:?}..{:?}",
        plane.center,
        plane.size,
        plane.rotation,
        plane.normal_y(),
        mats.label(plane.material),
        match plane.uv_mode {
            PlaneUvMode::Tile => "tile",
            PlaneUvMode::Stretch => "stretch",
        },
        if plane.uv.mode == UvMode::Inherit {
            String::new()
        } else {
            format!(" {:?}", plane.uv.mode)
        },
        if plane.double_sided { "both sides" } else { "front only" },
        plane.collision,
        plane.camera_pass,
        if plane.fullbright { ", fullbright" } else { "" },
        if plane.enabled { "" } else { ", DISABLED" },
        lo,
        hi
    )
}

/// List the planes of a room.
pub fn list(
    world: &GridWorld,
    mats: &Materials<'_>,
    room: Option<usize>,
) -> Result<String, String> {
    let rooms: Vec<usize> = match room {
        Some(_) => vec![pick_room(world, room)?],
        None => (0..world.rooms.len()).collect(),
    };
    let mut out = String::new();
    let mut total = 0;
    for r in rooms {
        for (i, plane) in world.rooms[r].planes.iter().enumerate() {
            total += 1;
            out.push_str(&format!("room {r} {}\n", describe(i, plane, mats)));
        }
    }
    if total == 0 {
        return Ok("no planes".to_string());
    }
    Ok(out)
}

/// Add a plane.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddPlaneReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms. The room grows
    /// to hold the plane.
    pub room: Option<usize>,
    /// Placement.
    #[serde(flatten)]
    pub geometry: PlaneGeometry,
    /// Material name or id. Blend mode and alpha cutout come from it.
    pub material: Option<String>,
    /// `tile` (repeat every sector from the top-left corner, the default) or
    /// `stretch` (one whole texture across the plane).
    pub uv_mode: Option<String>,
    /// Draw both sides. Default false.
    pub double_sided: Option<bool>,
    /// `decor` (drawn only, the default), `walkable` (a bridge, platform or
    /// ramp to stand on) or `solid` (a wall or fence).
    pub collision: Option<String>,
    /// Draw at the material tint with no light baked in.
    pub fullbright: Option<bool>,
    /// Whether the camera arm passes through. Omit for automatic (on for
    /// translucent or alpha-cut materials).
    pub camera_pass: Option<bool>,
}

/// Apply [`AddPlaneReq`].
pub fn add(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &AddPlaneReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let mut plane = GridPlane::new([0, 0, 0], [1024, 1024]);
    let g = &req.geometry;
    if g.center.is_none() && g.from.is_none() {
        return Err("give a center or a from/to span".to_string());
    }
    apply_geometry(&mut plane, g)?;
    plane.material = mats.resolve_opt(req.material.as_deref())?;
    if let Some(mode) = &req.uv_mode {
        plane.uv_mode = parse_uv_mode(mode)?;
    }
    plane.double_sided = req.double_sided.unwrap_or(false);
    if let Some(c) = &req.collision {
        plane.collision = parse_collision(c)?;
    }
    plane.fullbright = req.fullbright.unwrap_or(false);
    plane.camera_pass = req.camera_pass;
    let id = add_plane(world, room, plane).map_err(refusal)?;
    let plane = id.get(world).ok_or("the plane vanished")?;
    Ok(format!("added {}", describe(id.index, plane, mats)))
}

/// Change a plane.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPlaneReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Plane index in the room (grid_planes lists them). Indices shift when
    /// an earlier plane is deleted.
    pub index: usize,
    /// New placement; omitted parts stay.
    #[serde(flatten)]
    pub geometry: PlaneGeometry,
    /// Material name or id ("none" for untextured).
    pub material: Option<String>,
    /// `tile` or `stretch`.
    pub uv_mode: Option<String>,
    /// Draw both sides.
    pub double_sided: Option<bool>,
    /// `decor`, `walkable` or `solid`.
    pub collision: Option<String>,
    /// Fullbright.
    pub fullbright: Option<bool>,
    /// Camera pass-through: true, false.
    pub camera_pass: Option<bool>,
    /// Hide (false) or show (true) the plane without deleting it.
    pub enabled: Option<bool>,
}

/// Apply [`SetPlaneReq`].
pub fn set(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &SetPlaneReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let id = PlaneId {
        room,
        index: req.index,
    };
    if !id.exists(world) {
        return Err(format!(
            "room {room} has {} plane(s); no plane {}",
            world.rooms[room].planes.len(),
            req.index
        ));
    }
    let material = match req.material.as_deref() {
        None => None,
        Some(name) => Some(mats.resolve_opt(Some(name))?),
    };
    let uv_mode = req.uv_mode.as_deref().map(parse_uv_mode).transpose()?;
    let collision = req.collision.as_deref().map(parse_collision).transpose()?;
    let mut failure: Option<String> = None;
    let changed = update_plane(world, id, |plane| {
        if let Err(e) = apply_geometry(plane, &req.geometry) {
            failure = Some(e);
            return;
        }
        if let Some(mode) = uv_mode {
            plane.uv_mode = mode;
        }
        if let Some(v) = req.double_sided {
            plane.double_sided = v;
        }
        if let Some(c) = collision {
            plane.collision = c;
        }
        if let Some(v) = req.fullbright {
            plane.fullbright = v;
        }
        if let Some(v) = req.camera_pass {
            plane.camera_pass = Some(v);
        }
        if let Some(v) = req.enabled {
            plane.enabled = v;
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }
    let mut changed = changed;
    if let Some(material) = material {
        changed |= set_plane_material(world, id, material);
    }
    let plane = id.get(world).ok_or("the plane vanished")?;
    Ok(format!(
        "{}{}",
        if changed { "updated " } else { "no change: " },
        describe(id.index, plane, mats)
    ))
}

/// Delete planes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeletePlaneReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Plane indices to remove (higher ones go first, so all stay valid).
    pub indices: Vec<usize>,
}

/// Apply [`DeletePlaneReq`].
pub fn delete(world: &mut GridWorld, req: &DeletePlaneReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let ids: Vec<PlaneId> = req
        .indices
        .iter()
        .map(|&index| PlaneId { room, index })
        .collect();
    let removed = remove_planes(world, &ids);
    if removed == 0 {
        return Err(format!("room {room} has no plane at {:?}", req.indices));
    }
    Ok(format!(
        "removed {removed} plane(s) from room {room}; the rest are renumbered (grid_planes)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::grid::test_support::fixture;

    fn near(a: [i32; 3], b: [i32; 3], tolerance: i32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
    }

    /// The span helper puts the plane's two ends where it was told, for a
    /// level bridge and for a ramp heading each way.
    #[test]
    fn a_span_reaches_its_endpoints() {
        for (from, to) in [
            ([1024, 0, 1024], [1024, 0, 4096]),
            ([1024, 0, 4096], [1024, 512, 1024]),
            ([4096, 0, 2048], [2048, 768, 2048]),
            ([2048, 0, 2048], [4096, 0, 2048]),
            ([2048, 256, 2048], [3072, 0, 3072]),
        ] {
            let solved = span(from, to, 1024).unwrap();
            let mut plane = GridPlane::new(solved.center, solved.size);
            plane.rotation = solved.rotation;
            // The plane's height axis: top edge midpoint and bottom edge
            // midpoint are the two ends in some order.
            let top = plane.point_grid(f64::from(solved.size[0]) / 2.0, 0.0);
            let bottom =
                plane.point_grid(f64::from(solved.size[0]) / 2.0, f64::from(solved.size[1]));
            let tol = 40;
            let ok = (near(top, to, tol) && near(bottom, from, tol))
                || (near(top, from, tol) && near(bottom, to, tol));
            assert!(
                ok,
                "{from:?}->{to:?}: top {top:?} bottom {bottom:?} {solved:?}"
            );
            // The uphill end is the TOP edge of the plane's height axis.
            if to[1] > from[1] {
                assert!(
                    near(top, to, tol),
                    "the uphill end is the top: {top:?} vs {to:?}"
                );
            }
        }
    }

    #[test]
    fn a_walkable_ramp_is_a_platform_and_is_listed() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = add(
            &mut w,
            &mats,
            &AddPlaneReq {
                scene: None,
                room: None,
                geometry: PlaneGeometry {
                    from: Some([1024, 0, 4096]),
                    to: Some([1024, 512, 2048]),
                    width: Some(1024),
                    ..PlaneGeometry::default()
                },
                material: Some("Graybox / Cliff 64".into()),
                uv_mode: None,
                double_sided: Some(true),
                collision: Some("walkable".into()),
                fullbright: None,
                camera_pass: None,
            },
        )
        .unwrap();
        assert!(out.contains("platform"), "{out}");
        assert_eq!(w.rooms[0].planes.len(), 1);
        assert!(list(&w, &mats, None).unwrap().contains("plane 0"));
        let out = set(
            &mut w,
            &mats,
            &SetPlaneReq {
                scene: None,
                room: None,
                index: 0,
                geometry: PlaneGeometry::default(),
                material: None,
                uv_mode: None,
                double_sided: Some(false),
                collision: None,
                fullbright: None,
                camera_pass: None,
                enabled: None,
            },
        )
        .unwrap();
        assert!(out.contains("updated"), "{out}");
        delete(
            &mut w,
            &DeletePlaneReq {
                scene: None,
                room: None,
                indices: vec![0],
            },
        )
        .unwrap();
        assert!(w.rooms[0].planes.is_empty());
    }
}
