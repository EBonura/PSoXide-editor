//! 3D preview screenshots.
//!
//! The camera is an orbit camera, so a shot is a target point plus a heading
//! and a distance. Rendering itself stays in the frontend binary, which owns
//! the PSX raster path and a headless wgpu device. Pulling that in would turn
//! this crate into a 75-second build for one function, so the shot is taken
//! by running `frontend dump-editor-preview` with the solved camera.

use std::path::{Path, PathBuf};
use std::process::Command;

use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::resolve_scene;

/// Default heading: straight along the scene's Z axis.
const DEFAULT_YAW: u16 = 0;
/// Default pitch: a few degrees short of a full turn, which looks down at the
/// target from above (the eye sits at `-sin(pitch)` in Y).
const DEFAULT_PITCH: u16 = 3776;
/// Never place the eye closer than this to its target.
const MIN_RADIUS: i32 = 320;

/// A solved orbit camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    /// Orbit yaw, 4096 per turn.
    pub yaw_q12: u16,
    /// Orbit pitch, 4096 per turn. Under a quarter turn puts the eye below
    /// the target; near a full turn puts it above.
    pub pitch_q12: u16,
    /// Distance from target to eye, world units.
    pub radius: i32,
    /// World point the camera looks at.
    pub target: [i32; 3],
}

/// An orbit camera framing `target` from `distance` away.
pub fn frame(target: [i32; 3], distance: i32, yaw: Option<u16>, pitch: Option<u16>) -> Shot {
    Shot {
        yaw_q12: yaw.unwrap_or(DEFAULT_YAW),
        pitch_q12: pitch.unwrap_or(DEFAULT_PITCH),
        radius: distance.max(MIN_RADIUS),
        target,
    }
}

/// One camera pose, in EDITOR units (what nodes and `grid_rooms` extents use).
///
/// Three ways to say where the camera is, in this order of precedence:
///
/// 1. an **eye pose**: `position` plus either `look_at` or a heading
///    (`yaw_deg`/`pitch_deg`, or the raw `yaw`/`pitch` in 4096 per turn);
/// 2. an **orbit**: `target` (or `node`) plus `distance`, `yaw`, `pitch`;
/// 3. nothing: orbit the middle of the world.
///
/// Heading convention (the editor's orbit camera): yaw 0 looks toward -Z
/// (north), yaw 90 degrees looks toward -X (west). Pitch is positive looking
/// UP in `pitch_deg` (an eye pose) and in the raw `pitch` of an orbit
/// positive puts the eye BELOW the target, so a high orbit pitch near a full
/// turn (4096 - 320, say) looks down from above.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct ShotSpec {
    /// Name for this shot in the reply.
    pub label: Option<String>,
    /// Eye position `[x, y, z]`, editor units. Needs `look_at` or a heading.
    pub position: Option<[i32; 3]>,
    /// With `position`: the point to look at.
    pub look_at: Option<[i32; 3]>,
    /// Heading in degrees, 0 looks north (-Z), 90 west (-X), 180 south, 270
    /// east. With `position`.
    pub yaw_deg: Option<f32>,
    /// Pitch in degrees, positive looks up. With `position`. Default 0.
    pub pitch_deg: Option<f32>,
    /// World point to orbit. Omit and give a `node` to frame an entity, or
    /// neither to frame the middle of the world.
    pub target: Option<[i32; 3]>,
    /// Node name or id to orbit.
    pub node: Option<String>,
    /// Orbit radius, editor units. Default 3000.
    pub distance: Option<i32>,
    /// Raw yaw, 4096 per turn. Default 0.
    pub yaw: Option<u16>,
    /// Raw orbit pitch, 4096 per turn. Near a full turn looks down from
    /// above the target, small values look slightly upward. Default 3776.
    pub pitch: Option<u16>,
}

fn q12_from_degrees(degrees: f32) -> u16 {
    (degrees / 360.0 * 4096.0).round().rem_euclid(4096.0) as u16
}

fn orbit_offset(yaw_q12: u16, pitch_q12: u16) -> [f32; 3] {
    let tau = std::f32::consts::TAU;
    let (y, p) = (
        f32::from(yaw_q12) / 4096.0 * tau,
        f32::from(pitch_q12) / 4096.0 * tau,
    );
    [p.cos() * y.sin(), -p.sin(), p.cos() * y.cos()]
}

/// The orbit camera whose eye sits exactly at `position` with a given
/// heading. The eye is `target + radius * orbit_offset`, so the target lies
/// `radius` ahead of the eye.
pub fn eye_shot(position: [i32; 3], yaw_q12: u16, pitch_q12: u16) -> Shot {
    let radius = MIN_RADIUS;
    let offset = orbit_offset(yaw_q12, pitch_q12);
    let target = [
        (position[0] as f32 - radius as f32 * offset[0]).round() as i32,
        (position[1] as f32 - radius as f32 * offset[1]).round() as i32,
        (position[2] as f32 - radius as f32 * offset[2]).round() as i32,
    ];
    Shot {
        yaw_q12,
        pitch_q12,
        radius,
        target,
    }
}

/// Heading (raw q12 yaw and pitch) from `position` toward `look_at`.
pub fn heading_toward(position: [i32; 3], look_at: [i32; 3]) -> Result<(u16, u16), String> {
    let d = [
        (look_at[0] - position[0]) as f32,
        (look_at[1] - position[1]) as f32,
        (look_at[2] - position[2]) as f32,
    ];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if length < 1.0 {
        return Err("position and look_at are the same point".to_string());
    }
    // offset = -d/|d| = [cos p sin y, -sin p, cos p cos y]
    let pitch = (d[1] / length).asin().to_degrees();
    let yaw = (-d[0]).atan2(-d[2]).to_degrees();
    Ok((q12_from_degrees(yaw), q12_from_degrees(pitch)))
}

/// Solve one [`ShotSpec`] to an orbit camera. `node_position` looks a node
/// up by name or id.
pub fn solve(
    project: &ProjectDocument,
    scene: Option<usize>,
    spec: &ShotSpec,
) -> Result<Shot, String> {
    if let Some(position) = spec.position {
        let (yaw, pitch) = match spec.look_at {
            Some(look_at) => heading_toward(position, look_at)?,
            None => (
                spec.yaw_deg.map(q12_from_degrees).or(spec.yaw).unwrap_or(0),
                spec.pitch_deg
                    .map(q12_from_degrees)
                    .or(spec.pitch)
                    .unwrap_or(0),
            ),
        };
        return Ok(eye_shot(position, yaw, pitch));
    }
    if spec.look_at.is_some() {
        return Err("look_at needs a position (the eye); for an orbit give target".to_string());
    }
    let target = match (spec.target, spec.node.as_deref()) {
        (Some(point), _) => point,
        (None, Some(needle)) => {
            let index = resolve_scene(project, scene)?;
            let scene_doc = &project.scenes[index];
            let id = crate::nodes::find_node(scene_doc, needle)?;
            scene_doc
                .node(id)
                .ok_or_else(|| format!("node {needle:?} vanished"))?
                .transform
                .translation
                .map(|value| value.round() as i32)
        }
        (None, None) => world_center(project, scene)?,
    };
    Ok(frame(
        target,
        spec.distance.unwrap_or(3000),
        spec.yaw.or(spec.yaw_deg.map(q12_from_degrees)),
        spec.pitch,
    ))
}

/// Where the eye ends up, for the reply text.
pub fn eye_of(shot: &Shot) -> [i32; 3] {
    let o = orbit_offset(shot.yaw_q12, shot.pitch_q12);
    [
        (shot.target[0] as f32 + shot.radius as f32 * o[0]).round() as i32,
        (shot.target[1] as f32 + shot.radius as f32 * o[1]).round() as i32,
        (shot.target[2] as f32 + shot.radius as f32 * o[2]).round() as i32,
    ]
}

/// The middle of the scene's grid world, a little above the first room's
/// floor, in editor units.
pub fn world_center(
    project: &ProjectDocument,
    scene_index: Option<usize>,
) -> Result<[i32; 3], String> {
    let index = resolve_scene(project, scene_index)?;
    let world = project.scenes[index]
        .world
        .grid()
        .ok_or("the scene has no grid world to frame; give a target or a node")?;
    let room = world
        .rooms
        .first()
        .ok_or("the grid world has no rooms to frame")?;
    let sector = i64::from(world.scale.sector_editor_units());
    let origin = |axis: usize| i64::from(room.origin[axis]);
    let middle = |axis: usize, extent: usize| {
        (origin(axis) * sector + extent as i64 * sector / 2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    Ok([
        middle(0, room.width),
        world.scale.to_editor(room.y_offset) + 512,
        middle(1, room.depth),
    ])
}

/// Locate the frontend binary that owns the renderer.
pub fn find_frontend(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return path
            .is_file()
            .then(|| path.to_path_buf())
            .ok_or_else(|| format!("{} is not a file", path.display()));
    }
    // Newest first: run-fast is what `make run` builds and is usually current.
    for candidate in [
        "target/run-fast/frontend",
        "target/release/frontend",
        "target/debug/frontend",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(
        "no frontend binary found; build one with `cargo build -p frontend --profile run-fast`"
            .to_string(),
    )
}

/// Render `shot` and return PNG bytes.
pub fn render(
    frontend: &Path,
    project_dir: &Path,
    shot: Shot,
    out_dir: &Path,
) -> Result<Vec<u8>, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|error| format!("create {}: {error}", out_dir.display()))?;
    let ppm = out_dir.join("mcp-shot.ppm");
    let status = Command::new(frontend)
        .arg("dump-editor-preview")
        .arg("--project")
        .arg(project_dir)
        .arg(format!("--out={}", ppm.display()))
        .arg(format!("--yaw={}", shot.yaw_q12))
        .arg(format!("--pitch={}", shot.pitch_q12))
        .arg(format!("--radius={}", shot.radius))
        .arg(format!("--target-x={}", shot.target[0]))
        .arg(format!("--target-y={}", shot.target[1]))
        .arg(format!("--target-z={}", shot.target[2]))
        .output()
        .map_err(|error| format!("run {}: {error}", frontend.display()))?;
    if !status.status.success() {
        return Err(format!(
            "the renderer failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        ));
    }
    let raw = std::fs::read(&ppm).map_err(|error| format!("read {}: {error}", ppm.display()))?;
    let _ = std::fs::remove_file(&ppm);
    png_from_ppm(&raw)
}

/// Re-encode a binary PPM (P6) as PNG. Shared with the playtest frame dump.
pub fn png_from_ppm(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut fields = Vec::new();
    let mut index = 0usize;
    while fields.len() < 4 {
        while raw
            .get(index)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            index += 1;
        }
        if raw.get(index) == Some(&b'#') {
            while raw.get(index).is_some_and(|byte| *byte != b'\n') {
                index += 1;
            }
            continue;
        }
        let start = index;
        while raw
            .get(index)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            index += 1;
        }
        if start == index {
            return Err("truncated PPM header".to_string());
        }
        fields.push(
            std::str::from_utf8(&raw[start..index])
                .map_err(|_| "non-UTF8 PPM header".to_string())?
                .to_string(),
        );
    }
    index += 1;
    if fields[0] != "P6" {
        return Err(format!("expected a P6 PPM, got {:?}", fields[0]));
    }
    let width: u32 = fields[1].parse().map_err(|_| "bad PPM width".to_string())?;
    let height: u32 = fields[2]
        .parse()
        .map_err(|_| "bad PPM height".to_string())?;
    let wanted = width as usize * height as usize * 3;
    let pixels = raw
        .get(index..index + wanted)
        .ok_or_else(|| "PPM body is shorter than its header claims".to_string())?;
    let mut png = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(&mut png),
        pixels,
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|error| format!("encode PNG: {error}"))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_applies_defaults_and_a_minimum_distance() {
        let shot = frame([1, 2, 3], 10, None, None);
        assert_eq!((shot.yaw_q12, shot.pitch_q12), (DEFAULT_YAW, DEFAULT_PITCH));
        assert_eq!(shot.radius, MIN_RADIUS);
        let shot = frame([0; 3], 5000, Some(512), Some(300));
        assert_eq!(
            (shot.yaw_q12, shot.pitch_q12, shot.radius),
            (512, 300, 5000)
        );
    }

    #[test]
    fn the_world_center_is_inside_the_starter_room() {
        let project = ProjectDocument::starter();
        let [x, y, z] = world_center(&project, None).expect("the starter has a world");
        let world = project.active_scene().world.grid().unwrap();
        let side = world.scale.sector_editor_units() * 6;
        assert_eq!((x, z), (side / 2, side / 2));
        assert!(y > 0);
    }

    #[test]
    fn an_eye_pose_puts_the_orbit_eye_at_the_position() {
        for (position, yaw, pitch) in [
            ([1000, 500, 2000], 0u16, 0u16),
            ([-300, 900, 4000], 1024, 3900),
            ([5000, 100, -2000], 3000, 200),
        ] {
            let shot = eye_shot(position, yaw, pitch);
            let eye = eye_of(&shot);
            assert!(
                (0..3).all(|i| (eye[i] - position[i]).abs() <= 1),
                "{position:?} vs {eye:?}"
            );
        }
    }

    #[test]
    fn look_at_points_the_camera_along_the_line() {
        let from = [0, 500, 0];
        let to = [0, 500, -4000];
        // North is -Z: yaw 0, level.
        let (yaw, pitch) = heading_toward(from, to).unwrap();
        assert_eq!((yaw, pitch), (0, 0));
        // West is -X: a quarter turn of yaw.
        let (yaw, _) = heading_toward(from, [-4000, 500, 0]).unwrap();
        assert_eq!(yaw, 1024);
        // Looking up gives a small positive pitch.
        let (_, pitch) = heading_toward(from, [0, 1500, -1000]).unwrap();
        assert!(pitch > 0 && pitch < 1024, "{pitch}");
        assert!(heading_toward(from, from).is_err());
    }

    #[test]
    fn a_malformed_ppm_is_an_error_rather_than_a_panic() {
        assert!(png_from_ppm(b"P6 2 2 255\n\x00").is_err());
        assert!(png_from_ppm(b"P3 1 1 255\n\x00\x00\x00").is_err());
    }
}
