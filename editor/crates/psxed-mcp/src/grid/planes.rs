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
    /// `[yaw, pitch, roll]` in whole degrees. At zero the plane stands
    /// upright facing SOUTH (+Z), like the north wall of a sector as seen from
    /// inside. PITCH tips its face up: +90 lays it flat facing up (a floor),
    /// -90 flat facing down (a ceiling); +15 on an upright wall leans the TOP
    /// 15 degrees away from the side it faces (the face looks a little up),
    /// -15 leans the top toward that side (an overhang). YAW then turns the
    /// whole thing about the vertical axis: 90 faces east (+X), 180 north
    /// (-Z), -90 west (-X). ROLL spins the plane about its own normal (a
    /// ramp's cross-slope). Examples: floor `[0, 90, 0]`; wall facing +X
    /// `[90, 0, 0]`; that wall leaning back 15 degrees `[90, 15, 0]`. The
    /// `facing`/`lean_deg` and `normal` parameters compute these for you.
    pub rotation: Option<[i32; 3]>,
    /// Shorthand for the rotation: `wall` (upright), `floor` (pitch 90) or
    /// `ceiling` (pitch -90). Yaw and roll stay as given.
    pub orientation: Option<String>,
    /// The side the plane's face looks toward, instead of a rotation:
    /// `north` (-Z), `south` (+Z), `east` (+X), `west` (-X), also `+x`, `-x`,
    /// `+z`, `-z`, or `up` / `down` for a floor or ceiling. Sets yaw and
    /// pitch (roll 0). Combine with `lean_deg` to tip a wall.
    pub facing: Option<String>,
    /// With `facing`: whole degrees the TOP of the plane leans away from the
    /// side it faces (-89 to 89). 0 stands upright; 15 is a battered wall
    /// seen from inside the room; -15 is an overhang. Becomes the pitch.
    pub lean_deg: Option<f64>,
    /// The plane's front normal `[x, y, z]` (any length, not zero), instead
    /// of a rotation: x east, y up, z south. Floor `[0, 1, 0]`, wall facing
    /// east `[1, 0, 0]`, wall facing east leaning back 15 degrees `[0.966,
    /// 0.259, 0]`. Rounded to whole degrees; the reply reads the result back.
    pub normal: Option<[f64; 3]>,
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

/// Yaw and pitch (whole degrees) that turn a plane's normal `[0, 0, 1]` into
/// `normal`: the normal of yaw `a`, pitch `b` is
/// `[cos b sin a, sin b, cos b cos a]`.
pub fn rotation_for_normal(normal: [f64; 3]) -> Result<[i32; 3], String> {
    let length = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !(length.is_finite() && length > 1e-9) {
        return Err("normal must be a non-zero vector".to_string());
    }
    let n = normal.map(|v| v / length);
    let pitch = n[1].clamp(-1.0, 1.0).asin().to_degrees();
    // Straight up or down, the heading is free: keep it at 0.
    let yaw = if n[0].hypot(n[2]) < 1e-9 {
        0.0
    } else {
        n[0].atan2(n[2]).to_degrees()
    };
    Ok([yaw.round() as i32, pitch.round() as i32, 0])
}

/// The compass heading (degrees of yaw) of a `facing` name, and whether it
/// is a floor or ceiling.
fn facing_yaw(name: &str) -> Result<(i32, Option<i32>), String> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "south" | "+z" => (0, None),
        "east" | "+x" => (90, None),
        "north" | "-z" => (180, None),
        "west" | "-x" => (-90, None),
        "up" | "floor" => (0, Some(90)),
        "down" | "ceiling" => (0, Some(-90)),
        other => {
            return Err(format!(
                "unknown facing {other:?}; use north, south, east, west (or +x, -x, +z, -z), up or down"
            ))
        }
    })
}

/// Apply `normal` or `facing` and `lean_deg`, which replace the rotation.
fn apply_facing(plane: &mut GridPlane, geometry: &PlaneGeometry) -> Result<(), String> {
    let given = [
        geometry.rotation.is_some(),
        geometry.normal.is_some(),
        geometry.facing.is_some(),
    ];
    if given.iter().filter(|g| **g).count() > 1 {
        return Err("give only one of rotation, normal and facing".to_string());
    }
    if geometry.lean_deg.is_some() && geometry.facing.is_none() {
        return Err(
            "lean_deg goes with facing (it tips the plane away from that side)".to_string(),
        );
    }
    if (geometry.normal.is_some() || geometry.facing.is_some()) && geometry.orientation.is_some() {
        return Err(
            "orientation sets the pitch; do not combine it with normal or facing".to_string(),
        );
    }
    if let Some(normal) = geometry.normal {
        plane.rotation = rotation_for_normal(normal)?;
    }
    if let Some(name) = &geometry.facing {
        let (yaw, flat) = facing_yaw(name)?;
        let lean = geometry.lean_deg.unwrap_or(0.0);
        if !(-89.0..=89.0).contains(&lean) {
            return Err(format!("lean_deg must be within -89..89, got {lean}"));
        }
        if flat.is_some() && geometry.lean_deg.is_some() {
            return Err("lean_deg does not apply to a floor or ceiling facing".to_string());
        }
        plane.rotation = [yaw, flat.unwrap_or(lean.round() as i32), 0];
    }
    Ok(())
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
    apply_facing(plane, geometry)?;
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

/// What a rotation comes to in words: which way the face looks and how far
/// the plane leans.
pub fn pose_words(plane: &GridPlane) -> String {
    let n = plane.normal();
    let normal = format!("normal [{:.2}, {:.2}, {:.2}]", n[0], n[1], n[2]);
    if n[1] > 0.995 {
        return format!("faces up (a floor), {normal}");
    }
    if n[1] < -0.995 {
        return format!("faces down (a ceiling), {normal}");
    }
    let compass = |x: f64, z: f64| {
        let degrees = x.atan2(z).to_degrees().rem_euclid(360.0);
        // yaw 0 south, 90 east, 180 north, 270 west
        ["south", "east", "north", "west"][((degrees + 45.0) / 90.0) as usize % 4]
    };
    let side = compass(n[0], n[2]);
    let lean = n[1].asin().to_degrees();
    if lean.abs() < 0.5 {
        format!("upright, faces {side}, {normal}")
    } else if lean > 0.0 {
        format!(
            "faces {side}, top leans {lean:.0} degrees away from that side (face tips up), {normal}"
        )
    } else {
        format!(
            "faces {side}, top leans {:.0} degrees toward that side (overhang), {normal}",
            -lean
        )
    }
}

/// A plane within this many grid units of a floor or ceiling shares its
/// depth with it and z-fights (half a fine step; heights sit on a 64 lattice).
pub const COPLANAR_EPSILON: i32 = 32;

/// A copy of `world` whose ceilings are its floors, so the floor height
/// function answers for ceilings.
fn ceilings_as_floors(world: &GridWorld) -> GridWorld {
    let mut copy = world.clone();
    for room in &mut copy.rooms {
        for z in 0..room.depth {
            for x in 0..room.width {
                if let Some(sector) = room.sector_mut(x, z) {
                    let ceiling = sector.take_ground_ceiling();
                    sector.set_ground_floor(ceiling);
                }
            }
        }
    }
    copy
}

/// A warning when the plane lies within [`COPLANAR_EPSILON`] grid units of a
/// floor or ceiling over part of its area: the two draw at the same depth and
/// flicker. `None` when it clears every surface. A plane's `y` is relative to
/// its room, so `y: 0` is the room's own floor level.
pub fn coplanar_note(world: &GridWorld, room: usize, plane: &GridPlane) -> Option<String> {
    if !plane.enabled || plane.normal()[1].abs() < 0.2 {
        // Upright planes (and nearly upright ones) are not floor or ceiling
        // lookalikes; wall-height coincidences are not checked.
        return None;
    }
    let scale = world.scale;
    let r = world.rooms.get(room)?;
    let (w, h) = (f64::from(plane.size[0]), f64::from(plane.size[1]));
    let nx = ((w / 256.0).ceil() as usize).clamp(1, 32);
    let nt = ((h / 256.0).ceil() as usize).clamp(1, 32);
    let ceilings = ceilings_as_floors(world);
    let (mut floor_hits, mut ceiling_hits, mut samples) = (0usize, 0usize, 0usize);
    let mut nearest = i32::MAX;
    for i in 0..nx {
        for j in 0..nt {
            let p = plane.point(
                (i as f64 + 0.5) * w / nx as f64,
                (j as f64 + 0.5) * h / nt as f64,
            );
            samples += 1;
            let (ex, ez) = (
                scale.to_editor(p[0].round() as i32),
                scale.to_editor(p[2].round() as i32),
            );
            let y_world = r.y_offset + p[1].round() as i32;
            let eps_editor = scale.to_editor(COPLANAR_EPSILON).max(1);
            let y_editor = scale.to_editor(y_world);
            for (source, hits) in [(world, &mut floor_hits), (&ceilings, &mut ceiling_hits)] {
                if let Some(surface) = source.floor_height_at(ex, ez, y_editor + eps_editor) {
                    let gap = (y_editor - surface).abs();
                    if gap <= eps_editor {
                        *hits += 1;
                        nearest = nearest.min(gap);
                    }
                }
            }
        }
    }
    if floor_hits + ceiling_hits == 0 {
        return None;
    }
    let what = match (floor_hits > 0, ceiling_hits > 0) {
        (true, true) => "a floor and a ceiling",
        (true, false) => "a floor",
        _ => "a ceiling",
    };
    Some(format!(
        "warning: this plane is within {COPLANAR_EPSILON} grid units of {what} at {} of {samples} sample points (closest {} editor units apart), so the two draw at the same depth and z-fight. Plane y is relative to the room (y 0 is the room's own floor level): lift the plane by at least 64 grid units, or give the floor a different height instead.",
        floor_hits.max(ceiling_hits),
        nearest
    ))
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
        "plane {index}: center {:?}, size {:?}, rotation [yaw, pitch, roll] {:?}, normal y {:.2}\n  {}\n  material {}, uv {}{}, {}, collision {:?} -> {shape}, camera pass {:?}{}{}\n  covers sectors {:?}..{:?}",
        plane.center,
        plane.size,
        plane.rotation,
        plane.normal_y(),
        pose_words(plane),
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
    let mut out = format!("added {}", describe(id.index, plane, mats));
    if let Some(note) = coplanar_note(world, room, plane) {
        out.push('\n');
        out.push_str(&note);
    }
    Ok(out)
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
    let mut out = format!(
        "{}{}",
        if changed { "updated " } else { "no change: " },
        describe(id.index, plane, mats)
    );
    if let Some(note) = coplanar_note(world, room, plane) {
        out.push('\n');
        out.push_str(&note);
    }
    Ok(out)
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

    fn plane_at(geometry: PlaneGeometry) -> Result<GridPlane, String> {
        let mut plane = GridPlane::new([0, 0, 0], [1024, 1024]);
        apply_geometry(&mut plane, &geometry)?;
        Ok(plane)
    }

    fn near_f(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 0.02)
    }

    #[test]
    fn facing_and_lean_compute_yaw_pitch_and_roll() {
        let wall = |facing: &str, lean: Option<f64>| {
            plane_at(PlaneGeometry {
                center: Some([0, 0, 0]),
                facing: Some(facing.into()),
                lean_deg: lean,
                ..PlaneGeometry::default()
            })
            .unwrap()
        };
        // The documented examples.
        assert_eq!(wall("south", None).rotation, [0, 0, 0]);
        assert_eq!(wall("east", None).rotation, [90, 0, 0]);
        assert_eq!(wall("north", None).rotation, [180, 0, 0]);
        assert_eq!(wall("west", None).rotation, [-90, 0, 0]);
        assert_eq!(wall("+x", Some(15.0)).rotation, [90, 15, 0]);
        // The normal says what the words say: facing +X, leaning back, the
        // face tips up (normal y positive); an overhang tips it down.
        assert!(near_f(wall("east", None).normal(), [1.0, 0.0, 0.0]));
        let back = wall("east", Some(15.0)).normal();
        assert!(near_f(back, [0.966, 0.259, 0.0]), "{back:?}");
        let over = wall("east", Some(-15.0)).normal();
        assert!(near_f(over, [0.966, -0.259, 0.0]), "{over:?}");
        // The top edge of a leaning wall moves AWAY from the side it faces.
        let leaning = wall("east", Some(15.0));
        let top = leaning.point(512.0, 0.0);
        let bottom = leaning.point(512.0, 1024.0);
        assert!(
            top[0] < bottom[0],
            "top {top:?} bottom {bottom:?}: away from +X"
        );
        // Floors and ceilings.
        assert_eq!(wall("up", None).rotation[1], 90);
        assert_eq!(wall("down", None).rotation[1], -90);
        assert!(near_f(wall("up", None).normal(), [0.0, 1.0, 0.0]));
        // Mistakes are refused.
        for bad in [
            PlaneGeometry {
                facing: Some("sideways".into()),
                ..PlaneGeometry::default()
            },
            PlaneGeometry {
                facing: Some("east".into()),
                lean_deg: Some(120.0),
                ..PlaneGeometry::default()
            },
            PlaneGeometry {
                lean_deg: Some(10.0),
                ..PlaneGeometry::default()
            },
            PlaneGeometry {
                facing: Some("east".into()),
                rotation: Some([0, 0, 0]),
                ..PlaneGeometry::default()
            },
            PlaneGeometry {
                facing: Some("east".into()),
                orientation: Some("wall".into()),
                ..PlaneGeometry::default()
            },
            PlaneGeometry {
                facing: Some("up".into()),
                lean_deg: Some(5.0),
                ..PlaneGeometry::default()
            },
        ] {
            assert!(plane_at(bad).is_err());
        }
    }

    #[test]
    fn a_normal_vector_round_trips_through_the_rotation() {
        for normal in [
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.966, 0.259, 0.0],
            [-0.5, 0.5, 0.707],
            [3.0, 4.0, 0.0],
        ] {
            let plane = plane_at(PlaneGeometry {
                center: Some([0, 0, 0]),
                normal: Some(normal),
                ..PlaneGeometry::default()
            })
            .unwrap();
            let len = (normal.iter().map(|v| v * v).sum::<f64>()).sqrt();
            let want = normal.map(|v| v / len);
            // Whole degrees: the normal is within about a degree.
            let n = plane.normal();
            let dot: f64 = (0..3).map(|i| n[i] * want[i]).sum();
            assert!(dot > 0.9998, "{normal:?} -> {:?} -> {n:?}", plane.rotation);
            assert_eq!(plane.rotation[2], 0);
        }
        assert!(rotation_for_normal([0.0; 3]).is_err());
        assert!(rotation_for_normal([f64::NAN, 0.0, 1.0]).is_err());
    }

    #[test]
    fn the_reply_reads_the_pose_back_in_words() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = add(
            &mut w,
            &mats,
            &AddPlaneReq {
                scene: None,
                room: None,
                geometry: PlaneGeometry {
                    center: Some([2048, 1000, 2048]),
                    facing: Some("east".into()),
                    lean_deg: Some(15.0),
                    ..PlaneGeometry::default()
                },
                material: None,
                uv_mode: None,
                double_sided: None,
                collision: None,
                fullbright: None,
                camera_pass: None,
            },
        )
        .unwrap();
        assert!(
            out.contains("rotation [yaw, pitch, roll] [90, 15, 0]"),
            "{out}"
        );
        assert!(
            out.contains("faces east, top leans 15 degrees away from that side"),
            "{out}"
        );
        let upright = pose_words(
            &plane_at(PlaneGeometry {
                center: Some([0; 3]),
                rotation: Some([180, 0, 0]),
                ..PlaneGeometry::default()
            })
            .unwrap(),
        );
        assert!(upright.starts_with("upright, faces north"), "{upright}");
        let floor = pose_words(
            &plane_at(PlaneGeometry {
                center: Some([0; 3]),
                orientation: Some("floor".into()),
                ..PlaneGeometry::default()
            })
            .unwrap(),
        );
        assert!(floor.starts_with("faces up"), "{floor}");
    }

    #[test]
    fn a_plane_at_floor_height_warns_and_one_above_it_does_not() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let req = |y: i32, orientation: &str| AddPlaneReq {
            scene: None,
            room: None,
            geometry: PlaneGeometry {
                center: Some([3072, y, 3072]),
                size: Some([1024, 1024]),
                orientation: Some(orientation.into()),
                ..PlaneGeometry::default()
            },
            material: None,
            uv_mode: None,
            double_sided: Some(true),
            collision: None,
            fullbright: None,
            camera_pass: None,
        };
        // The starter's floor is at y 0 and its ceiling at 2048.
        let at_floor = add(&mut w, &mats, &req(0, "floor")).unwrap();
        assert!(
            at_floor.contains("warning: this plane is within"),
            "{at_floor}"
        );
        assert!(at_floor.contains("a floor"), "{at_floor}");
        assert!(at_floor.contains("z-fight"), "{at_floor}");
        assert!(at_floor.contains("room"), "{at_floor}");
        // Within half a fine step still counts; a whole fine step clears it.
        assert!(add(&mut w, &mats, &req(COPLANAR_EPSILON, "floor"))
            .unwrap()
            .contains("warning"));
        let lifted = add(&mut w, &mats, &req(64, "floor")).unwrap();
        assert!(!lifted.contains("warning"), "{lifted}");
        // The ceiling counts too.
        let at_ceiling = add(&mut w, &mats, &req(2048, "ceiling")).unwrap();
        assert!(at_ceiling.contains("a ceiling"), "{at_ceiling}");
        // An upright plane is not a floor lookalike.
        let wall = add(&mut w, &mats, &req(0, "wall")).unwrap();
        assert!(!wall.contains("warning"), "{wall}");
        // Over a pit there is no floor to fight with.
        let (p2, mut w2) = fixture();
        let mats2 = Materials::new(&p2);
        for x in 2..=3 {
            for z in 2..=3 {
                w2.rooms[0].sector_mut(x, z).unwrap().set_ground_floor(None);
            }
        }
        let over_pit = add(&mut w2, &mats2, &req(0, "floor")).unwrap();
        assert!(!over_pit.contains("warning"), "{over_pit}");
    }
}
