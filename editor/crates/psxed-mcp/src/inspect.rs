//! Per-brush and per-material inspection.
//!
//! `scene_info` aggregates and `plan_view` draws silhouettes, but nothing said
//! what a single brush actually *is*. Every index-addressed call (`array`,
//! `set_material`, `carve`, `delete`) was therefore a guess, which is the
//! Blender MCP's `get_object_info` gap.
//!
//! Materials had the same problem one level down: the project stores a
//! `psxt_path` and no dimensions, so texture tiling was being reasoned about
//! from an assumed 64x64. The real size is in the cooked `.psxt` header.

use std::fmt::Write as _;
use std::path::Path;

use psxed_project::brush::Plane;
use psxed_project::{ProjectDocument, ResourceData};

use crate::resolve_scene;

/// Name the axis a face points along, or say it is off-axis.
fn normal_label(plane: &Plane) -> String {
    let n = plane.normal;
    let zeroes = n.iter().filter(|value| **value == 0).count();
    if zeroes == 2 {
        let axis = (0..3).find(|axis| n[*axis] != 0).unwrap_or(0);
        let sign = if n[axis] > 0 { '+' } else { '-' };
        let name = ["X", "Y", "Z"][axis];
        let role = match (axis, n[axis] > 0) {
            (1, true) => " (up, a floor surface)",
            (1, false) => " (down, a ceiling surface)",
            _ => "",
        };
        format!("{sign}{name}{role}")
    } else {
        format!("off-axis {n:?}")
    }
}

/// Full detail for one brush: what it is, how big, and what is on each face.
pub fn brush_info(
    project: &ProjectDocument,
    scene_index: Option<usize>,
    index: usize,
) -> Result<String, String> {
    let scene_index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[scene_index];
    let brush = scene.brushes.get(index).ok_or_else(|| {
        format!(
            "brush {index} is out of range: scene {scene_index} has {}",
            scene.brushes.len()
        )
    })?;
    let solved = brush.solve();
    let mut out = format!("# Brush {index} of scene {scene_index}\n\n");
    if !solved.is_valid() {
        out.push_str("DEGENERATE: fewer than four faces survived, so it encloses no volume.\n");
        return Ok(out);
    }
    let size: [f64; 3] = std::array::from_fn(|axis| solved.max[axis] - solved.min[axis]);
    let _ = writeln!(out, "contents: {}", brush.contents.label());
    let _ = writeln!(
        out,
        "bounds: [{:.0}, {:.0}, {:.0}] .. [{:.0}, {:.0}, {:.0}]",
        solved.min[0], solved.min[1], solved.min[2], solved.max[0], solved.max[1], solved.max[2]
    );
    let _ = writeln!(
        out,
        "size: {:.0} x {:.0} x {:.0}  ({:.2} player heights tall, {:.1} x {:.1} player widths)",
        size[0],
        size[1],
        size[2],
        size[1] / f64::from(crate::PLAYER_HEIGHT),
        size[0] / f64::from(crate::PLAYER_RADIUS * 2),
        size[2] / f64::from(crate::PLAYER_RADIUS * 2)
    );
    if let Some(group) = brush
        .group
        .and_then(|id| scene.nodes().iter().find(|node| node.id == id))
    {
        let _ = writeln!(out, "group: {:?}", group.name);
    }
    let _ = writeln!(
        out,
        "faces: {} ({} survive solving)\n",
        brush.faces.len(),
        solved.polygons.iter().flatten().count()
    );

    out.push_str("| face | normal | material | UV offset | rot | scale% |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for (position, face) in brush.faces.iter().enumerate() {
        let normal = Plane::from_points(face.points)
            .map_or_else(|| "degenerate".to_string(), |plane| normal_label(&plane));
        let material = face
            .material
            .and_then(|id| project.resources.iter().find(|resource| resource.id == id))
            .map_or_else(|| "<none>".to_string(), |resource| resource.name.clone());
        let uv = face.uv;
        let _ = writeln!(
            out,
            "| {position} | {normal} | {material} | [{}, {}] | {} | [{}, {}] |",
            uv.offset_texels[0],
            uv.offset_texels[1],
            uv.rotation_deg,
            // Widen first: scale_q8 is i16 and a 200% scale is 512, so
            // `512 * 100` overflows it and panics the tool handler, which
            // leaves the client waiting for a reply that never comes.
            i32::from(uv.scale_q8[0]) * 100 / 256,
            i32::from(uv.scale_q8[1]) * 100 / 256
        );
    }
    out.push_str("\nFace indices address set_face_uv; the whole brush addresses array, set_material, carve and delete.\n");
    Ok(out)
}

/// Texel dimensions and depth of a cooked `.psxt`, read from its header.
///
/// Layout is the shared 12-byte `AssetHeader` followed by the 16-byte
/// `TextureHeader`, whose first fields are depth, one pad byte, then width
/// and height as little-endian u16.
fn psxt_dimensions(path: &Path) -> Option<(u16, u16, u8)> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; psxed_format::AssetHeader::SIZE + 6];
    file.read_exact(&mut head).ok()?;
    if head[..4] != psxed_format::texture::MAGIC {
        return None;
    }
    let base = psxed_format::AssetHeader::SIZE;
    let depth = head[base];
    let width = u16::from_le_bytes([head[base + 2], head[base + 3]]);
    let height = u16::from_le_bytes([head[base + 4], head[base + 5]]);
    Some((width, height, depth))
}

/// Resolve a project-relative texture path the way the cook does.
fn resolve_texture(project_root: &Path, raw: &str) -> Option<std::path::PathBuf> {
    let direct = Path::new(raw);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let relative = project_root.join(raw);
    if relative.is_file() {
        return Some(relative);
    }
    // The starter ships repo-root-relative paths, so try the cwd last.
    let cwd = std::env::current_dir().ok()?.join(raw);
    cwd.is_file().then_some(cwd)
}

/// Every material with its texel size, depth, blend mode and tint.
///
/// Size is the point. Without it, how often a texture repeats over a surface
/// is guesswork: at 16 authored units per texel a 64x64 tile repeats every
/// 1024 units and a 128x128 every 2048, and nothing in the project file says
/// which one a material is.
pub fn materials(project: &ProjectDocument, project_root: &Path) -> String {
    let mut out = String::from("# Materials\n\n");
    out.push_str("| name | id | texels | tile repeat | depth | blend | tint |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    let mut listed = 0usize;
    for resource in &project.resources {
        let ResourceData::Material(material) = &resource.data else {
            continue;
        };
        listed += 1;
        let (size, repeat, depth) = material
            .psxt_path
            .as_deref()
            .and_then(|raw| resolve_texture(project_root, raw))
            .and_then(|path| psxt_dimensions(&path))
            .map_or_else(
                || ("flat".to_string(), "-".to_string(), "-".to_string()),
                |(width, height, depth)| {
                    (
                        format!("{width}x{height}"),
                        format!(
                            "{}x{}",
                            u32::from(width) * crate::UNITS_PER_TEXEL as u32,
                            u32::from(height) * crate::UNITS_PER_TEXEL as u32
                        ),
                        format!("{depth}bpp"),
                    )
                },
            );
        let _ = writeln!(
            out,
            "| {} | {} | {size} | {repeat} | {depth} | {:?} | {:?} |",
            resource.name,
            resource.id.raw(),
            material.blend_mode,
            material.tint
        );
    }
    let _ = writeln!(
        out,
        "\n{listed} materials. `tile repeat` is how many authored world units one \
         full texture covers at identity UV scale, at {} units per texel.",
        crate::UNITS_PER_TEXEL
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::brush::Brush;

    /// A brush report has to name the surface an agent is about to retexture,
    /// which means getting the up/down faces right.
    #[test]
    fn brush_info_labels_faces_and_reports_size() {
        let mut project = ProjectDocument::default();
        project.scenes[0].brushes.clear();
        project.scenes[0]
            .brushes
            .push(Brush::cuboid([0, 0, 0], [1024, 512, 2048]));

        let report = brush_info(&project, None, 0).expect("the brush reports");
        assert!(report.contains("1024 x 512 x 2048"), "{report}");
        assert!(report.contains("up, a floor surface"), "{report}");
        assert!(report.contains("down, a ceiling surface"), "{report}");
        assert!(report.contains("<none>"), "an untextured face must say so");
        // Identity UVs read as 100%, not as a raw Q8 256.
        assert!(report.contains("| 100, 100 |") || report.contains("[100, 100]"), "{report}");

        // A scale above ~128% used to overflow the i16 while being formatted.
        project.scenes[0].brushes[0].faces[0].uv.scale_q8 = [512, 4096];
        let scaled = brush_info(&project, None, 0).expect("a wide scale still reports");
        assert!(scaled.contains("[200, 1600]"), "{scaled}");

        // Out of range is an error carrying the real count.
        let error = brush_info(&project, None, 9).expect_err("out of range");
        assert!(error.contains("has 1"), "{error}");

        // A degenerate brush says so rather than reporting nonsense bounds.
        project.scenes[0].brushes.push(Brush::default());
        let degenerate = brush_info(&project, None, 1).expect("it still reports");
        assert!(degenerate.contains("DEGENERATE"), "{degenerate}");
    }
}
