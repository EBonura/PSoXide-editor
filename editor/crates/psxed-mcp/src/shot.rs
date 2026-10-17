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
    fn a_malformed_ppm_is_an_error_rather_than_a_panic() {
        assert!(png_from_ppm(b"P6 2 2 255\n\x00").is_err());
        assert!(png_from_ppm(b"P3 1 1 255\n\x00\x00\x00").is_err());
    }
}
