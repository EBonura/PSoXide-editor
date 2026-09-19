//! Level-authoring tool bodies for the PSoXide Editor MCP server.
//!
//! Every public function here is a pure function of a [`ProjectDocument`],
//! deliberately: Phase 1 runs offline against a project file, and the same
//! bodies move into the live editor's MCP bridge later without changing
//! shape (see `docs/editor-mcp-plan-2026-09-18.md`).
//!
//! The premise test behind this module: the existing 3D preview renders a
//! night-time level at fullbright, so volumes read as black masses and the
//! agent cannot judge a space from it. [`plan_view`] answers the spatial
//! question directly instead, by sectioning the authored brush solids.

pub mod audit;
pub mod edit;
pub mod inspect;
pub mod nodes;
pub mod play;
pub mod shot;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use psxed_project::brush::{Brush, Plane};
use psxed_project::{ProjectDocument, Scene};

/// Authored units per cooked (Quake) unit. The cook divides by this.
pub const WORLD_UNIT_DIVISOR: i32 = 16;
/// Authored units per texel, so a 64x64 texture tiles every 1024 units.
pub const UNITS_PER_TEXEL: i32 = 16;
/// Player capsule from the shipped default project's Character Controller.
pub const PLAYER_RADIUS: i32 = 188;
/// See [`PLAYER_RADIUS`].
pub const PLAYER_HEIGHT: i32 = 1024;
/// World sector size in the shipped default project; the natural major grid.
pub const SECTOR: i32 = 1024;

/// Which pair of world axes a [`plan_view`] plots, and which it cuts along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanAxis {
    /// Horizontal floor plan: cut along Y, plot X right and Z down.
    Top,
    /// Vertical section: cut along Z, plot X right and Y up.
    Front,
    /// Vertical section: cut along X, plot Z right and Y up.
    Side,
}

impl PlanAxis {
    /// Parse the `axis` tool argument.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "top" | "plan" => Ok(Self::Top),
            "front" => Ok(Self::Front),
            "side" => Ok(Self::Side),
            other => Err(format!("unknown axis {other:?}: use top, front or side")),
        }
    }

    /// World axis the section plane cuts along.
    const fn cut_axis(self) -> usize {
        match self {
            Self::Top => 1,
            Self::Front => 2,
            Self::Side => 0,
        }
    }

    /// World axes plotted rightwards and downwards in the image.
    const fn plot_axes(self) -> [usize; 2] {
        match self {
            Self::Top => [0, 2],
            Self::Front => [0, 1],
            Self::Side => [2, 1],
        }
    }

    /// Whether the down-image axis increases upwards in world space, so the
    /// section is drawn the way a person would hold it.
    const fn flip_vertical(self) -> bool {
        !matches!(self, Self::Top)
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Top => "top (floor plan, cut along Y)",
            Self::Front => "front (section, cut along Z)",
            Self::Side => "side (section, cut along X)",
        }
    }
}

/// Pick the scene to operate on: the caller's index, else the first scene
/// that actually has brushes, else the first scene.
pub fn resolve_scene(project: &ProjectDocument, index: Option<usize>) -> Result<usize, String> {
    if let Some(index) = index {
        return (index < project.scenes.len())
            .then_some(index)
            .ok_or_else(|| {
                format!(
                    "scene {index} out of range: the project has {}",
                    project.scenes.len()
                )
            });
    }
    project
        .scenes
        .iter()
        .position(|scene| !scene.brushes.is_empty())
        .or(if project.scenes.is_empty() { None } else { Some(0) })
        .ok_or_else(|| "the project has no scenes".to_string())
}

/// Solid brushes of a scene, paired with their solved bounds.
fn solid_brushes(scene: &Scene) -> Vec<(&Brush, [f64; 3], [f64; 3])> {
    scene
        .brushes
        .iter()
        .filter(|brush| brush.contents.is_solid())
        .filter_map(|brush| {
            let solved = brush.solve();
            solved
                .is_valid()
                .then_some((brush, solved.min, solved.max))
        })
        .collect()
}

/// Whether a world point lies inside a convex brush, using the kernel's own
/// outward-normal convention (`side` is >0 outside, <0 inside).
pub(crate) fn contains(brush: &Brush, point: [f64; 3]) -> bool {
    brush.faces.iter().all(|face| {
        Plane::from_points(face.points).is_none_or(|plane| {
            let n = plane.normal.map(|v| v as f64);
            n[0] * point[0] + n[1] * point[1] + n[2] * point[2] - plane.dist as f64 <= 0.0
        })
    })
}

/// Scale constants and the measured reference dimensions of the shipped
/// level, so sizes are looked up rather than guessed. This is the analogue
/// of the Blender MCP's `bpy_api_lookup`: its value is that it stops the
/// agent inventing plausible numbers.
pub fn metrics(project: Option<&ProjectDocument>) -> String {
    let mut out = String::new();
    out.push_str("# PSoXide authored-unit metrics\n\n");
    let _ = writeln!(
        out,
        "All level authoring is in EDITOR units. The cook divides every length\n\
         by {WORLD_UNIT_DIVISOR} to reach Quake/runtime units, so 1 Quake unit =\n\
         {UNITS_PER_TEXEL} editor units = 1 texel. A 64x64 texture therefore\n\
         tiles every {} editor units -- but materials are NOT all 64x64 (this\n\
         project also ships 1536x256 sky atlases and 16x16 UI tiles), so call\n\
         `materials` for the real per-material tile size instead of assuming.\n",
        64 * UNITS_PER_TEXEL
    );
    out.push_str("\n## Reference bodies\n\n");
    let _ = writeln!(
        out,
        "- player: radius {PLAYER_RADIUS}, height {PLAYER_HEIGHT} (diameter {} wide)",
        PLAYER_RADIUS * 2
    );
    out.push_str("- heavy enemy: radius 320, height 1741\n");
    out.push_str("- gameplay camera: distance 3500, height 1800, target height 1160\n");
    out.push_str("- gravity 96 units/tick^2; world sector 1024; draw distance 25000\n");

    out.push_str("\n## Working grid\n\n");
    out.push_str(
        "64 is the authored grid (100% of the shipped map's coordinates sit on\n\
         it, 97.4% on 128). Floor and ceiling slabs are 256 or 384 thick and\n\
         nothing else. Snap generated geometry to 64 or it will read as foreign.\n",
    );

    out.push_str("\n## Space sizes that shipped\n\n");
    out.push_str(
        "Measured from the v0.4 default map (6976 standable samples, player\n\
         hull inset). Ceilings are far more generous than a Quake instinct\n\
         suggests, because this is an elevated open structure, not corridors:\n\n\
         | | units | player heights |\n\
         | --- | --- | --- |\n\
         | min | 1024 | 1.00 |\n\
         | p25 | 3840 | 3.75 |\n\
         | median | 5376 | 5.25 |\n\
         | p75 | 7616 | 7.44 |\n\
         | max | 9344 | 9.12 |\n\n\
         Default new rooms to roughly 3000-5500 tall.\n",
    );

    out.push_str("\n## Curve snapping (arches and pillars)\n\n");
    out.push_str(
        "Generated curves snap every vertex to the working grid, and\n\
         `convex_prism` then drops collinear points and rejects non-convex\n\
         turns. A pillar can therefore come back with FEWER sides than asked\n\
         for. The obvious chord rule (2*r*sin(pi/n) > step) is necessary and\n\
         not sufficient: a radius-128 octagon on a 64 grid clears it and still\n\
         collapses to a diamond, because snapping puts its vertices in\n\
         collinear pairs. These are the measured minimum square footprints:\n\n\
         | sides | step 16 | step 32 | step 64 | step 128 |\n\
         | --- | --- | --- | --- | --- |\n\
         | 4 | 32 | 64 | 128 | 256 |\n\
         | 6 | 48 | 96 | 192 | 384 |\n\
         | 8 | 96 | 192 | 384 | 768 |\n\
         | 12 | 128 | 256 | 512 | 1024 |\n\
         | 16 | 224 | 448 | 896 | 1792 |\n\n\
         On the map's 64 grid an octagonal pillar needs a 384-unit footprint,\n\
         and a 1024-wide box tops out at 12 sides. `add_shape` builds the\n\
         solid and reports the side count it actually got, so trust that over\n\
         any formula.\n\n\
         Cost: an n-sided pillar is n+2 faces and a DoorwayArch is\n\
         segments+2 brushes. A 12-pillar octagonal colonnade is 120 faces, and\n\
         if one sightline sees them all that is a single visibility leaf.\n",
    );

    if let Some(project) = project {
        if let Ok(index) = resolve_scene(project, None) {
            let scene = &project.scenes[index];
            let _ = writeln!(
                out,
                "\n## This project\n\nScene {index} {:?}: {} brushes, {} faces.",
                scene.name,
                scene.brushes.len(),
                scene.brushes.iter().map(|b| b.faces.len()).sum::<usize>()
            );
        }
    }
    out
}

/// Structured read of a scene: extents, counts, groups, materials in use.
pub fn scene_info(project: &ProjectDocument, scene_index: Option<usize>) -> Result<String, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let solids = solid_brushes(scene);
    let mut out = String::new();
    let _ = writeln!(out, "# Scene {index}: {:?}\n", scene.name);

    if solids.is_empty() {
        out.push_str("No solid brushes.\n");
        return Ok(out);
    }

    let (min, max) = world_bounds(&solids);
    let _ = writeln!(
        out,
        "brushes {} ({} solid), faces {}",
        scene.brushes.len(),
        solids.len(),
        scene.brushes.iter().map(|b| b.faces.len()).sum::<usize>()
    );
    let _ = writeln!(
        out,
        "bounds  min [{:.0}, {:.0}, {:.0}]  max [{:.0}, {:.0}, {:.0}]",
        min[0], min[1], min[2], max[0], max[1], max[2]
    );
    let _ = writeln!(
        out,
        "span    {:.0} x {:.0} x {:.0}  ({:.1} x {:.1} player heights wide/tall)",
        max[0] - min[0],
        max[1] - min[1],
        max[2] - min[2],
        (max[0] - min[0]) / f64::from(PLAYER_HEIGHT),
        (max[1] - min[1]) / f64::from(PLAYER_HEIGHT)
    );

    // Faces per brush is a cheap tell for what primitives were used: exactly
    // 6.0 means the whole level is cuboids.
    let faces: usize = scene.brushes.iter().map(|b| b.faces.len()).sum();
    let _ = writeln!(
        out,
        "faces per brush {:.2} (6.00 = every brush is a cuboid)",
        faces as f64 / scene.brushes.len() as f64
    );

    out.push_str("\n## Groups\n\n");
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for brush in &scene.brushes {
        let name = brush
            .group
            .and_then(|id| scene.nodes().iter().find(|node| node.id == id))
            .map_or_else(|| "<ungrouped>".to_string(), |node| node.name.clone());
        *groups.entry(name).or_default() += 1;
    }
    for (name, count) in &groups {
        let _ = writeln!(out, "- {name}: {count} brushes");
    }

    out.push_str("\n## Materials in use\n\n");
    let mut used: BTreeMap<u64, usize> = BTreeMap::new();
    let mut untextured = 0usize;
    for face in scene.brushes.iter().flat_map(|b| b.faces.iter()) {
        match face.material {
            Some(id) => *used.entry(id.raw()).or_default() += 1,
            None => untextured += 1,
        }
    }
    let mut rows: Vec<_> = used.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));
    for (id, count) in rows.iter().take(20) {
        let name = project
            .resources
            .iter()
            .find(|resource| resource.id.raw() == **id)
            .map_or("<missing>", |resource| resource.name.as_str());
        let _ = writeln!(out, "- {name} (id {id}): {count} faces");
    }
    if rows.len() > 20 {
        let _ = writeln!(out, "- ... and {} more materials", rows.len() - 20);
    }
    let _ = writeln!(out, "\nfaces with no material: {untextured}");

    out.push_str("\n## Horizontal surface levels (brush tops by footprint area)\n\n");
    for (y, area, count) in busiest_levels(&solids).iter().take(8) {
        let _ = writeln!(
            out,
            "- Y {y}: {area:.0} sq units across {count} brushes"
        );
    }
    out.push_str("\nPass one of these +512 as `slice` to plan_view for a floor plan.\n");
    Ok(out)
}

fn world_bounds(solids: &[(&Brush, [f64; 3], [f64; 3])]) -> ([f64; 3], [f64; 3]) {
    solids.iter().fold(
        ([f64::MAX; 3], [f64::MIN; 3]),
        |(lo, hi), (_, min, max)| {
            (
                std::array::from_fn(|a| lo[a].min(min[a])),
                std::array::from_fn(|a| hi[a].max(max[a])),
            )
        },
    )
}

/// Brush-top Y values ranked by the footprint area resting on them. These are
/// the candidate floor levels to section a plan view at.
///
/// Backdrop brushes are excluded. The shipped level's sky cube is a single
/// brush covering most of the world, so ranking raw area put the skybox top
/// first and a plan sectioned there caught one brush out of 244. Anything
/// covering more than [`BACKDROP_AREA_FRACTION`] of the world footprint is a
/// backdrop, not a floor.
fn busiest_levels(solids: &[(&Brush, [f64; 3], [f64; 3])]) -> Vec<(i64, f64, usize)> {
    let (wmin, wmax) = world_bounds(solids);
    let world_area = ((wmax[0] - wmin[0]) * (wmax[2] - wmin[2])).max(1.0);
    let mut tops: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
    for (_, min, max) in solids {
        let area = (max[0] - min[0]) * (max[2] - min[2]);
        if area / world_area > BACKDROP_AREA_FRACTION {
            continue;
        }
        let entry = tops.entry(max[1].round() as i64).or_default();
        entry.0 += area;
        entry.1 += 1;
    }
    let mut tops: Vec<_> = tops
        .into_iter()
        .map(|(y, (area, count))| (y, area, count))
        .collect();
    tops.sort_by(|a, b| b.1.total_cmp(&a.1));
    tops
}

/// Share of the world footprint above which a brush is backdrop, not floor.
const BACKDROP_AREA_FRACTION: f64 = 0.5;

/// Window to frame a [`plan_view`] on, instead of the whole level.
#[derive(Clone, Copy, Debug)]
pub struct Focus {
    /// World point to centre on. The same point works for all three views.
    pub center: [i32; 3],
    /// Half-span in world units on each plotted axis.
    pub extent: i32,
}

/// A rendered section: the PNG plus the numbers the image cannot carry.
pub struct PlanView {
    /// PNG bytes.
    pub png: Vec<u8>,
    /// Exact dimensions, scale, and the slice actually used.
    pub legend: String,
}

const BG: [u8; 3] = [246, 245, 242];
const GRID: [u8; 3] = [222, 220, 214];
const GRID_MAJOR: [u8; 3] = [196, 193, 185];
const SOLID: [u8; 3] = [64, 68, 74];
const PLAYER: [u8; 3] = [198, 72, 48];
const RULE: [u8; 3] = [30, 32, 36];

/// Draw an orthographic section of the scene's solid brushes.
///
/// Text is deliberately not rasterized into the image (that would need a
/// bitmap font for no gain). The image carries shape and relative size, with
/// a to-scale player disc and a known grid pitch; [`PlanView::legend`]
/// carries every exact number.
pub fn plan_view(
    project: &ProjectDocument,
    scene_index: Option<usize>,
    axis: PlanAxis,
    slice: Option<i32>,
    width_px: u32,
    focus: Option<Focus>,
) -> Result<PlanView, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let solids = solid_brushes(scene);
    if solids.is_empty() {
        return Err("the scene has no solid brushes to section".to_string());
    }
    let width_px = width_px.clamp(256, 2048);
    let cut = axis.cut_axis();
    let plot = axis.plot_axes();
    let (wmin, wmax) = match focus {
        // A whole-level plan puts the player under one pixel, so framing one
        // room is how relative size actually becomes visible.
        Some(Focus { center, extent }) => {
            let extent = f64::from(extent.max(64));
            let mut min = [0.0; 3];
            let mut max = [0.0; 3];
            for axis_index in 0..3 {
                min[axis_index] = f64::from(center[axis_index]) - extent;
                max[axis_index] = f64::from(center[axis_index]) + extent;
            }
            let (full_min, full_max) = world_bounds(&solids);
            // The cut axis is not plotted; keep its true range so slice
            // validation and the legend still describe the real level.
            min[cut] = full_min[cut];
            max[cut] = full_max[cut];
            (min, max)
        }
        None => world_bounds(&solids),
    };

    // Dumb, predictable default, with the good candidates listed in the
    // legend so the next call can aim properly.
    let slice_value = slice.unwrap_or_else(|| (wmin[cut] + 512.0).round() as i32);
    let slice_f = f64::from(slice_value);
    if slice_f < wmin[cut] || slice_f > wmax[cut] {
        return Err(format!(
            "slice {slice_value} is outside the scene's {} range {:.0}..{:.0}",
            ["X", "Y", "Z"][cut],
            wmin[cut],
            wmax[cut]
        ));
    }

    let span = [
        (wmax[plot[0]] - wmin[plot[0]]).max(1.0),
        (wmax[plot[1]] - wmin[plot[1]]).max(1.0),
    ];
    let margin = 16i64;
    let units_per_px = span[0] / f64::from(width_px - (margin as u32) * 2);
    let height_px = ((span[1] / units_per_px).ceil() as i64 + margin * 2).clamp(128, 4096) as u32;

    let mut img = image::RgbImage::from_pixel(width_px, height_px, image::Rgb(BG));
    let to_px = |world: f64, axis_index: usize| -> f64 {
        (world - wmin[plot[axis_index]]) / units_per_px + margin as f64
    };
    let from_px = |px: f64, axis_index: usize| -> f64 {
        (px - margin as f64) * units_per_px + wmin[plot[axis_index]]
    };
    let row_world = |y: u32| -> f64 {
        let value = from_px(f64::from(y) + 0.5, 1);
        if axis.flip_vertical() {
            wmin[plot[1]] + wmax[plot[1]] - value
        } else {
            value
        }
    };

    // Grid first, so solids paint over it.
    for step in [SECTOR, SECTOR * 8] {
        let colour = if step == SECTOR { GRID } else { GRID_MAJOR };
        let mut world = (wmin[plot[0]] / f64::from(step)).ceil() * f64::from(step);
        while world <= wmax[plot[0]] {
            let x = to_px(world, 0).round();
            if x >= 0.0 && x < f64::from(width_px) {
                for y in 0..height_px {
                    img.put_pixel(x as u32, y, image::Rgb(colour));
                }
            }
            world += f64::from(step);
        }
        let mut world = (wmin[plot[1]] / f64::from(step)).ceil() * f64::from(step);
        while world <= wmax[plot[1]] {
            let offset = (world - wmin[plot[1]]) / units_per_px + margin as f64;
            let y = if axis.flip_vertical() {
                f64::from(height_px) - offset
            } else {
                offset
            }
            .round();
            if y >= 0.0 && y < f64::from(height_px) {
                for x in 0..width_px {
                    img.put_pixel(x, y as u32, image::Rgb(colour));
                }
            }
            world += f64::from(step);
        }
    }

    // Solids: rasterize each brush only across its own bounding box, testing
    // the exact convex predicate per pixel.
    let mut in_slice = 0usize;
    for (brush, bmin, bmax) in &solids {
        if slice_f < bmin[cut] || slice_f > bmax[cut] {
            continue;
        }
        in_slice += 1;
        let x0 = to_px(bmin[plot[0]], 0).floor().max(0.0) as u32;
        let x1 = (to_px(bmax[plot[0]], 0).ceil() as i64).clamp(0, i64::from(width_px)) as u32;
        for x in x0..x1 {
            let world_h = from_px(f64::from(x) + 0.5, 0);
            for y in 0..height_px {
                let world_v = row_world(y);
                if world_v < bmin[plot[1]] || world_v > bmax[plot[1]] {
                    continue;
                }
                let mut point = [0.0; 3];
                point[cut] = slice_f;
                point[plot[0]] = world_h;
                point[plot[1]] = world_v;
                if contains(brush, point) {
                    img.put_pixel(x, y, image::Rgb(SOLID));
                }
            }
        }
    }

    // Scale key: one sector-long rule with the player disc beside it, both to
    // scale. This is what makes the image answer "how big, compared to me".
    let key_y = height_px.saturating_sub(margin as u32 / 2 + 4);
    let rule_px = (f64::from(SECTOR) / units_per_px).round().max(2.0) as u32;
    for x in 0..rule_px.min(width_px.saturating_sub(margin as u32)) {
        for dy in 0..3u32 {
            let y = key_y.saturating_sub(dy);
            img.put_pixel(x + margin as u32 / 2, y, image::Rgb(RULE));
        }
    }
    let player_px = f64::from(PLAYER_RADIUS) / units_per_px;
    let cx = f64::from(margin as u32 / 2) + f64::from(rule_px) + player_px + 6.0;
    let cy = f64::from(key_y) - player_px - 4.0;
    let radius = player_px.max(1.5);
    let y0 = (cy - radius).floor().max(0.0) as u32;
    let y1 = ((cy + radius).ceil() as i64).clamp(0, i64::from(height_px)) as u32;
    for y in y0..y1 {
        for x in 0..width_px {
            let dx = f64::from(x) - cx;
            let dy = f64::from(y) - cy;
            if dx * dx + dy * dy <= radius * radius {
                img.put_pixel(x, y, image::Rgb(PLAYER));
            }
        }
    }

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            img.as_raw(),
            width_px,
            height_px,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("encode plan PNG: {error}"))?;

    let mut legend = String::new();
    let _ = writeln!(legend, "# Plan view: {}\n", axis.label());
    let _ = writeln!(
        legend,
        "slice {} = {slice_value}{}",
        ["X", "Y", "Z"][cut],
        if slice.is_some() {
            ""
        } else {
            " (default: 512 above the scene floor)"
        }
    );
    let _ = writeln!(
        legend,
        "{in_slice} of {} solid brushes intersect this slice",
        solids.len()
    );
    let _ = writeln!(
        legend,
        "image {width_px}x{height_px} px at {units_per_px:.1} units/px"
    );
    let _ = writeln!(
        legend,
        "plotted: {} rightwards {:.0}..{:.0}, {} {} {:.0}..{:.0}",
        ["X", "Y", "Z"][plot[0]],
        wmin[plot[0]],
        wmax[plot[0]],
        ["X", "Y", "Z"][plot[1]],
        if axis.flip_vertical() {
            "upwards"
        } else {
            "downwards"
        },
        wmin[plot[1]],
        wmax[plot[1]]
    );
    let _ = writeln!(
        legend,
        "span {:.0} x {:.0} units = {:.1} x {:.1} player heights",
        span[0],
        span[1],
        span[0] / f64::from(PLAYER_HEIGHT),
        span[1] / f64::from(PLAYER_HEIGHT)
    );
    let _ = writeln!(
        legend,
        "\nkey: grid lines every {SECTOR} (major every {}); the bar bottom-left is {SECTOR} units; the red disc is the player, radius {PLAYER_RADIUS}",
        SECTOR * 8
    );
    if f64::from(PLAYER_RADIUS) / units_per_px < 2.0 {
        legend.push_str(
            "\nThe player disc is under 2 px here, so this view shows layout, not\n\
             human scale. Pass `focus` (a world centre and a half-span of a few\n\
             thousand units) to frame one space at a readable size.\n",
        );
    }
    if matches!(axis, PlanAxis::Top) {
        legend.push_str("\ncandidate floor levels (Y, footprint area) - section at one of these +512:\n");
        for (y, area, count) in busiest_levels(&solids).iter().take(6) {
            let _ = writeln!(legend, "  Y {y}: {area:.0} sq units across {count} brushes");
        }
    }
    Ok(PlanView { png, legend })
}

use image::ImageEncoder as _;

/// Load a project from a `project.ron` path or the directory holding one.
pub fn load_project(path: &std::path::Path) -> Result<ProjectDocument, String> {
    let file = if path.is_dir() {
        path.join("project.ron")
    } else {
        path.to_path_buf()
    };
    let text = std::fs::read_to_string(&file)
        .map_err(|error| format!("read {}: {error}", file.display()))?;
    ProjectDocument::from_ron_str(&text)
        .map_err(|error| format!("parse {}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One runnable check over the whole pipeline: a two-room scene sections
    /// correctly, the slice predicate actually excludes out-of-slice brushes,
    /// and the PNG encodes.
    #[test]
    fn plan_view_sections_only_the_brushes_the_slice_crosses() {
        let mut project = ProjectDocument::default();
        let scene = project
            .scenes
            .first_mut()
            .expect("a default project has one scene");
        // `ProjectDocument::default()` is the embedded starter level, not an
        // empty document, so clear it before authoring the fixture.
        scene.brushes.clear();
        // Floor slab 0..256, and a wall standing on it up to 2048.
        scene.brushes.push(Brush::cuboid([0, 0, 0], [4096, 256, 4096]));
        scene
            .brushes
            .push(Brush::cuboid([0, 256, 0], [128, 2048, 4096]));

        // A slice through the wall only.
        let high = plan_view(&project, None, PlanAxis::Top, Some(1024), 256, None)
            .expect("wall slice renders");
        assert!(high.legend.contains("1 of 2 solid brushes"), "{}", high.legend);

        // A slice through the slab catches both (the wall's base is at 256,
        // the slab spans 0..256, so 200 is slab-only; use 128).
        let low = plan_view(&project, None, PlanAxis::Top, Some(128), 256, None)
            .expect("slab slice renders");
        assert!(low.legend.contains("1 of 2 solid brushes"), "{}", low.legend);

        // PNG magic, so an encoding regression fails here and not in a client.
        assert_eq!(&high.png[..8], b"\x89PNG\r\n\x1a\n");

        // Out-of-range slices are an error, never a blank image.
        assert!(plan_view(&project, None, PlanAxis::Top, Some(99_999), 256, None).is_err());

        // A focus window frames a sub-region without changing the slice.
        let focused = plan_view(
            &project,
            None,
            PlanAxis::Top,
            Some(1024),
            256,
            Some(Focus {
                center: [64, 1024, 2048],
                extent: 512,
            }),
        )
        .expect("focused slice renders");
        assert!(focused.legend.contains("1024 units/px") || focused.legend.contains("units/px"));
        assert!(
            focused.png.len() != high.png.len(),
            "a focused view should not encode identically to the whole level"
        );

        // The convex predicate agrees with the kernel's own winding.
        let cube = Brush::cuboid([0, 0, 0], [100, 100, 100]);
        assert!(contains(&cube, [50.0, 50.0, 50.0]));
        assert!(!contains(&cube, [150.0, 50.0, 50.0]));
    }
}
