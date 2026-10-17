//! Material inspection.
//!
//! The project stores a `psxt_path` and no dimensions, so texture tiling was
//! being reasoned about from an assumed 64x64. The real size is in the cooked
//! `.psxt` header.

use std::fmt::Write as _;
use std::path::Path;

use psxed_project::{ProjectDocument, ResourceData};

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
