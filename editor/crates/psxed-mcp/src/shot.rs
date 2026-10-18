//! 3D preview screenshots.
//!
//! A shot is either an orbit (a target point plus a heading and a distance) or
//! an eye pose (a position plus a heading). The renderer only has an orbit
//! camera, so an eye pose is the orbit whose target sits a fixed distance
//! ahead of the eye: the eye lands on the position (the images are
//! pixel-identical to the same view asked for as an orbit from far away; the
//! 320 unit "radius" the reply used to print was only that helper point). Rendering itself stays in the frontend binary, which owns
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
    /// World point the camera looks at (orbit shots; an eye pose keeps the
    /// point `radius` ahead of the eye here, for the reply text only).
    pub target: [i32; 3],
    /// An exact eye pose: the eye is this point and looks along
    /// `yaw_q12`/`pitch_q12`. `None` is an orbit. The renderer only has an
    /// orbit camera, so an eye pose is the orbit whose target lies
    /// `radius` ahead of the eye along the heading; its eye is this point to
    /// within a unit of rounding.
    pub eye: Option<[i32; 3]>,
}

/// What a shot leaves out of the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderOpts {
    /// Do not draw the player's model.
    pub hide_player: bool,
    /// Do not draw any character or enemy model.
    pub hide_actors: bool,
    /// Do not draw the editor's own overlays: entity bounds boxes and the
    /// light bulbs.
    pub hide_overlays: bool,
}

/// An orbit camera framing `target` from `distance` away.
pub fn frame(target: [i32; 3], distance: i32, yaw: Option<u16>, pitch: Option<u16>) -> Shot {
    Shot {
        yaw_q12: yaw.unwrap_or(DEFAULT_YAW),
        pitch_q12: pitch.unwrap_or(DEFAULT_PITCH),
        radius: distance.max(MIN_RADIUS),
        target,
        eye: None,
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
/// An eye pose is exact: the eye sits at `position` (the reply says "exact
/// eye") and looks along the heading (`look_at` is turned into a
/// heading; the heading is quantised to 1/4096 turn, 0.09 degrees, which is
/// 3 units at 2000 units out).
///
/// Heading convention: yaw 0 looks toward -Z (north), yaw 90 degrees looks
/// toward -X (west), 180 south, 270 east. `pitch_deg` is positive looking UP
/// (negative looks down). In an orbit the raw `pitch` is the other way
/// round: positive puts the eye BELOW the target, so a high orbit pitch near
/// a full turn (4096 - 320, say) looks down from above.
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
    /// Do not draw the player's model. A shot from the entrance is inside it
    /// otherwise.
    pub hide_player: Option<bool>,
    /// Do not draw any character or enemy model (the player included).
    pub hide_actors: Option<bool>,
    /// Do not draw the editor's overlays (entity bounds boxes, light bulbs).
    pub hide_overlays: Option<bool>,
    /// A clean view: no player, actors or overlays, as the level looks in
    /// play without the HUD (the preview never draws a HUD). Shorthand for
    /// the three switches above; they still win when given.
    pub clean: Option<bool>,
}

/// Which actors a shot leaves out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorHide {
    /// Only the player-controlled entity.
    Player,
    /// Every character and enemy.
    All,
}

impl RenderOpts {
    /// The actors this leaves out, if any.
    pub fn actor_hide(&self) -> Option<ActorHide> {
        if self.hide_actors {
            Some(ActorHide::All)
        } else if self.hide_player {
            Some(ActorHide::Player)
        } else {
            None
        }
    }
}

/// Take the model off the entities `hide` names, in a scratch copy of the
/// project (the renderer draws an entity through its Model Renderer
/// component, so one without a model draws nothing). Returns how many
/// entities lost their model.
pub fn without_actor_models(project: &mut ProjectDocument, hide: ActorHide) -> usize {
    use psxed_project::NodeKind;
    let mut stripped = 0;
    for scene in &mut project.scenes {
        let hosts: Vec<(psxed_project::NodeId, Vec<psxed_project::NodeId>, bool)> = scene
            .nodes()
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Entity))
            .map(|node| {
                let is_player = node.children.iter().any(|id| {
                    scene.node(*id).is_some_and(|child| {
                        matches!(
                            child.kind,
                            NodeKind::CharacterController { player: true, .. }
                        )
                    })
                });
                (node.id, node.children.clone(), is_player)
            })
            .collect();
        for (_, children, is_player) in hosts {
            if hide == ActorHide::Player && !is_player {
                continue;
            }
            let mut any = false;
            for id in children {
                if let Some(NodeKind::ModelRenderer { model, .. }) =
                    scene.node_mut(id).map(|child| &mut child.kind)
                {
                    any |= model.take().is_some();
                }
            }
            stripped += usize::from(any);
        }
    }
    stripped
}

impl ShotSpec {
    /// What this shot leaves out.
    pub fn render_opts(&self) -> RenderOpts {
        let clean = self.clean.unwrap_or(false);
        RenderOpts {
            hide_player: self.hide_player.unwrap_or(clean),
            hide_actors: self.hide_actors.unwrap_or(clean),
            hide_overlays: self.hide_overlays.unwrap_or(clean),
        }
    }
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

/// An eye pose: the orbit whose eye is `position`, looking along the
/// heading.
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
        eye: Some(position),
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

fn degrees(q12: u16) -> f32 {
    let d = f32::from(q12) / 4096.0 * 360.0;
    if d > 180.0 {
        d - 360.0
    } else {
        d
    }
}

/// A shot in words, for the reply: where the eye is and where it looks.
pub fn describe(shot: &Shot) -> String {
    let eye = eye_of(shot);
    match shot.eye {
        Some(_) => format!(
            "exact eye {eye:?}, heading yaw {:.1} deg, pitch {:+.1} deg (up positive; yaw 0 north, 90 west)",
            degrees(shot.yaw_q12),
            degrees(shot.pitch_q12)
        ),
        None => format!(
            "orbit eye {eye:?} around target {:?}, radius {}, yaw {} pitch {} (raw q12, 4096 a turn)",
            shot.target, shot.radius, shot.yaw_q12, shot.pitch_q12
        ),
    }
}

/// What a shot hides, for the reply.
pub fn describe_opts(opts: RenderOpts) -> String {
    let mut hidden = Vec::new();
    if opts.hide_actors {
        hidden.push("actors");
    } else if opts.hide_player {
        hidden.push("player");
    }
    if opts.hide_overlays {
        hidden.push("overlays");
    }
    if hidden.is_empty() {
        String::new()
    } else {
        format!("; hidden: {}", hidden.join(", "))
    }
}

/// Where the eye ends up, for the reply text.
pub fn eye_of(shot: &Shot) -> [i32; 3] {
    if let Some(eye) = shot.eye {
        return eye;
    }
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
    opts: RenderOpts,
    out_dir: &Path,
) -> Result<Vec<u8>, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|error| format!("create {}: {error}", out_dir.display()))?;
    let ppm = out_dir.join("mcp-shot.ppm");
    let mut command = Command::new(frontend);
    command
        .arg("dump-editor-preview")
        .arg("--project")
        .arg(project_dir)
        .arg(format!("--out={}", ppm.display()))
        .arg(format!("--yaw={}", shot.yaw_q12))
        .arg(format!("--pitch={}", shot.pitch_q12))
        .arg(format!("--radius={}", shot.radius))
        .arg(format!("--target-x={}", shot.target[0]))
        .arg(format!("--target-y={}", shot.target[1]))
        .arg(format!("--target-z={}", shot.target[2]));
    if opts.hide_overlays {
        command.arg("--hide-editor-overlays");
    }
    let status = command
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
            // The eye is the position, whatever the helper target is.
            assert_eq!(shot.eye, Some(position));
            assert_eq!(eye_of(&shot), position);
            // The point kept for the reply lies MIN_RADIUS ahead along the
            // heading.
            let o = orbit_offset(yaw, pitch);
            let ahead = [
                position[0] as f32 - MIN_RADIUS as f32 * o[0],
                position[1] as f32 - MIN_RADIUS as f32 * o[1],
                position[2] as f32 - MIN_RADIUS as f32 * o[2],
            ];
            assert!((0..3).all(|i| (shot.target[i] as f32 - ahead[i]).abs() <= 1.0));
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
    fn an_eye_pose_with_look_at_solves_to_the_eye_and_describes_itself() {
        let project = ProjectDocument::starter();
        let spec = ShotSpec {
            position: Some([100, 600, 5000]),
            look_at: Some([100, 600, 0]),
            ..ShotSpec::default()
        };
        let shot = solve(&project, None, &spec).unwrap();
        assert_eq!(shot.eye, Some([100, 600, 5000]));
        assert_eq!((shot.yaw_q12, shot.pitch_q12), (0, 0));
        let text = describe(&shot);
        assert!(text.contains("exact eye [100, 600, 5000]"), "{text}");
        assert!(text.contains("yaw 0.0 deg, pitch +0.0 deg"), "{text}");
        // An orbit keeps its target and radius.
        let orbit = solve(
            &project,
            None,
            &ShotSpec {
                target: Some([0, 0, 0]),
                distance: Some(4000),
                ..ShotSpec::default()
            },
        )
        .unwrap();
        assert_eq!((orbit.eye, orbit.radius), (None, 4000));
        assert!(describe(&orbit).starts_with("orbit eye"));
        // Pitch is degrees, positive up: looking down gives a negative one.
        let down = solve(
            &project,
            None,
            &ShotSpec {
                position: Some([0, 2000, 0]),
                look_at: Some([0, 0, -2000]),
                ..ShotSpec::default()
            },
        )
        .unwrap();
        assert!(
            describe(&down).contains("pitch -45.0 deg"),
            "{}",
            describe(&down)
        );
    }

    #[test]
    fn hiding_switches_compose_with_clean() {
        let none = ShotSpec::default().render_opts();
        assert_eq!(none, RenderOpts::default());
        let clean = ShotSpec {
            clean: Some(true),
            ..ShotSpec::default()
        };
        let o = clean.render_opts();
        assert!(o.hide_player && o.hide_actors && o.hide_overlays);
        // An explicit switch wins over clean.
        let keep = ShotSpec {
            clean: Some(true),
            hide_overlays: Some(false),
            ..ShotSpec::default()
        };
        assert!(!keep.render_opts().hide_overlays && keep.render_opts().hide_player);
        assert_eq!(
            describe_opts(clean.render_opts()),
            "; hidden: actors, overlays"
        );
        assert_eq!(describe_opts(RenderOpts::default()), "");
        let player = ShotSpec {
            hide_player: Some(true),
            ..ShotSpec::default()
        };
        assert_eq!(describe_opts(player.render_opts()), "; hidden: player");
    }

    #[test]
    fn a_malformed_ppm_is_an_error_rather_than_a_panic() {
        assert!(png_from_ppm(b"P6 2 2 255\n\x00").is_err());
        assert!(png_from_ppm(b"P3 1 1 255\n\x00\x00\x00").is_err());
    }
}
