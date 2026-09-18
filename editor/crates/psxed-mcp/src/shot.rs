//! Auto-framed 3D preview screenshots.
//!
//! Aiming the editor's orbit camera by hand is six numbers and no feedback,
//! and it went wrong in every session that used it: two shots in the Phase 5
//! run landed inside solid walls and came back black. The camera is not hard
//! to aim, it is just hard to aim *blind*, so this module aims it against the
//! geometry.
//!
//! The eye sits at `target + radius * [cos_p*sin_y, -sin_p, cos_p*cos_y]`
//! (`orbit_camera_position_f32` in psxed-ui). That is a ray from the target,
//! so `Brush::raycast` answers exactly the question that matters: how far can
//! the camera pull back along this heading before it is inside something.
//!
//! Rendering itself stays in the frontend binary, which owns the PSX raster
//! path and a headless wgpu device. Pulling that in would turn this crate into
//! a 75-second build for one function, so the shot is taken by running
//! `frontend dump-editor-preview` with the solved camera.

use std::path::{Path, PathBuf};
use std::process::Command;

use psxed_project::brush::Brush;
use psxed_project::ProjectDocument;

use crate::resolve_scene;

/// Turns are 4096 units, matching the editor's Q12 angle convention.
const TURN: f64 = 4096.0;
/// Keep the eye this fraction short of the first surface behind it.
const CLEARANCE: f64 = 0.82;
/// Never place the eye closer than this to its target.
const MIN_RADIUS: f64 = 320.0;

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
    /// Distance to the first surface behind the eye, or `f64::INFINITY`.
    pub clearance: f64,
}

/// Unit vector from the target towards the eye.
fn eye_direction(yaw_q12: u16, pitch_q12: u16) -> [f64; 3] {
    let yaw = f64::from(yaw_q12) / TURN * std::f64::consts::TAU;
    let pitch = f64::from(pitch_q12) / TURN * std::f64::consts::TAU;
    let (sin_y, cos_y) = yaw.sin_cos();
    let (sin_p, cos_p) = pitch.sin_cos();
    [cos_p * sin_y, -sin_p, cos_p * cos_y]
}

/// Distance from `origin` along `dir` to the nearest solid surface.
fn clearance_along(brushes: &[Brush], origin: [f64; 3], dir: [f64; 3]) -> f64 {
    brushes
        .iter()
        .filter(|brush| brush.contents.is_solid())
        .filter_map(|brush| brush.raycast(origin, dir).map(|(t, _)| t))
        .filter(|t| *t > 0.0)
        .fold(f64::INFINITY, f64::min)
}

/// Solve a camera that frames `target` without ending up inside geometry.
///
/// With no yaw or pitch given, a small sweep picks the heading with the most
/// room behind it, which is what stops the eye burying itself in a wall.
pub fn frame(
    project: &ProjectDocument,
    scene_index: Option<usize>,
    target: [i32; 3],
    desired_radius: i32,
    yaw: Option<u16>,
    pitch: Option<u16>,
) -> Result<Shot, String> {
    let index = resolve_scene(project, scene_index)?;
    let brushes = &project.scenes[index].brushes;
    let origin = target.map(f64::from);
    let desired = f64::from(desired_radius.max(1));

    let yaws: Vec<u16> = match yaw {
        Some(value) => vec![value],
        // Eight headings is enough to find an open one and cheap enough to
        // brute force; there is no cleverness worth adding here.
        None => (0..8).map(|step| step * 512).collect(),
    };
    let pitches: Vec<u16> = match pitch {
        Some(value) => vec![value],
        // Both sides of the horizon. The eye sits at `-sin(pitch)` in Y, so
        // pitches under a quarter turn put it BELOW the target and pitches
        // near a full turn put it above. A target near the floor has almost
        // no room below it, and offering only the low half is how a shot of
        // a floor-standing entity ends up underground.
        None => vec![224, 320, 448, 3648, 3776, 3872],
    };

    let mut best: Option<(Shot, f64)> = None;
    for yaw_q12 in yaws {
        for &pitch_q12 in &pitches {
            let dir = eye_direction(yaw_q12, pitch_q12);
            let clearance = clearance_along(brushes, origin, dir);
            let room = if clearance.is_finite() {
                clearance * CLEARANCE
            } else {
                desired
            };
            // Score on the room actually available, BEFORE the minimum-radius
            // floor. Scoring after it made every cramped heading tie at the
            // floor value, so the first one won and the eye went through a
            // wall. A heading with 190 units of room must lose to one with
            // 2900, even though both would be clamped to the same radius.
            let score = room.min(desired);
            let shot = Shot {
                yaw_q12,
                pitch_q12,
                radius: room.min(desired).max(MIN_RADIUS).round() as i32,
                target,
                clearance,
            };
            if best.is_none_or(|(_, current)| score > current) {
                best = Some((shot, score));
            }
        }
    }
    best.map(|(shot, _)| shot)
        .ok_or_else(|| "no camera could be solved".to_string())
}

/// Centre and half-span of a run of brushes, for framing what was just built.
pub fn brush_range_target(
    project: &ProjectDocument,
    scene_index: Option<usize>,
    first: usize,
    count: usize,
) -> Result<([i32; 3], i32), String> {
    let index = resolve_scene(project, scene_index)?;
    let brushes = &project.scenes[index].brushes;
    let end = first.saturating_add(count).min(brushes.len());
    if first >= brushes.len() || count == 0 {
        return Err(format!(
            "brushes {first}..{} are out of range: the scene has {}",
            first + count,
            brushes.len()
        ));
    }
    let (min, max) = brushes[first..end]
        .iter()
        .map(Brush::solve)
        .filter(|solved| solved.is_valid())
        .fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), solved| {
            (
                std::array::from_fn(|axis| lo[axis].min(solved.min[axis])),
                std::array::from_fn(|axis| hi[axis].max(solved.max[axis])),
            )
        });
    if min[0] > max[0] {
        return Err("those brushes enclose no volume".to_string());
    }
    let center = std::array::from_fn(|axis| ((min[axis] + max[axis]) * 0.5).round() as i32);
    let span = (0..3)
        .map(|axis| max[axis] - min[axis])
        .fold(0.0, f64::max);
    Ok((center, (span * 1.2).round().max(512.0) as i32))
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
    Err("no frontend binary found; build one with `cargo build -p frontend --profile run-fast`"
        .to_string())
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

/// Re-encode a binary PPM (P6) as PNG.
fn png_from_ppm(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut fields = Vec::new();
    let mut index = 0usize;
    while fields.len() < 4 {
        while raw.get(index).is_some_and(|byte| byte.is_ascii_whitespace()) {
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
    let height: u32 = fields[2].parse().map_err(|_| "bad PPM height".to_string())?;
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

    /// The failure this module exists to prevent: an orbit camera solved
    /// blind ends up inside a wall. Framed against a closed room, every
    /// heading must keep the eye in the open space.
    #[test]
    fn framing_keeps_the_eye_inside_the_room() {
        let mut project = ProjectDocument::default();
        project.scenes[0].brushes.clear();
        // A 4096-cube room centred on the origin, walls 256 thick.
        let shell = Brush::cuboid([-2304, -2304, -2304], [2304, 2304, 2304])
            .hollow(256)
            .expect("the shell builds");
        project.scenes[0].brushes.extend(shell);

        // Asking for a radius far larger than the room must be pulled in.
        // Not to below the room's half-extent though: along a diagonal the
        // eye can be 2693 units out and still well inside a 2048 half-room,
        // so the radius alone says nothing. Containment is the real invariant.
        let shot = frame(&project, None, [0, 0, 0], 50_000, None, None)
            .expect("a camera solves");
        assert!(
            shot.radius < 50_000,
            "radius {} was not pulled in at all",
            shot.radius
        );
        assert!(shot.clearance.is_finite(), "the walls should be hit");

        // The solved eye really is in open space: nothing contains it.
        let direction = eye_direction(shot.yaw_q12, shot.pitch_q12);
        let eye = std::array::from_fn(|axis| {
            f64::from(shot.target[axis]) + direction[axis] * f64::from(shot.radius)
        });
        assert!(
            !project.scenes[0]
                .brushes
                .iter()
                .any(|brush| crate::contains(brush, eye)),
            "the eye at {eye:?} landed inside a brush"
        );

        // With nothing in the way the requested radius survives.
        project.scenes[0].brushes.clear();
        let open = frame(&project, None, [0, 0, 0], 8000, Some(0), Some(300))
            .expect("a camera solves in an empty scene");
        assert_eq!(open.radius, 8000);
        assert!(open.clearance.is_infinite());

        // A target sitting just above the floor must not be framed from
        // underneath it: the low pitches have almost no room, so the solver
        // has to reach for a heading above the horizon.
        project.scenes[0].brushes.clear();
        project.scenes[0]
            .brushes
            .push(Brush::cuboid([-4096, -256, -4096], [4096, 0, 4096]));
        let low = frame(&project, None, [0, 64, 0], 4000, None, None)
            .expect("a camera solves above a floor");
        let direction = eye_direction(low.yaw_q12, low.pitch_q12);
        let eye_y = 64.0 + direction[1] * f64::from(low.radius);
        assert!(
            eye_y > 0.0,
            "pitch {} put the eye at y {eye_y:.0}, under the floor",
            low.pitch_q12
        );

        // A malformed PPM is an error rather than a panic.
        assert!(png_from_ppm(b"P6 2 2 255\n\x00").is_err());
        assert!(png_from_ppm(b"P3 1 1 255\n\x00\x00\x00").is_err());
    }
}
