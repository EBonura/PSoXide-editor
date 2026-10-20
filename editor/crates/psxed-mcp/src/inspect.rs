//! Material inspection.
//!
//! The project stores a `psxt_path` and no dimensions, so texture tiling was
//! being reasoned about from an assumed 64x64. The real size is in the cooked
//! `.psxt` header.

use std::fmt::Write as _;
use std::path::Path;

use psxed_project::face_uv::UvMode;
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

/// What one tile of a material covers under the grid mapping, in words.
///
/// The grid renderer lays a texture by the face's mode: `World` spends a
/// fixed number of texels per sector (the world's own setting, 64 for a new
/// world so a 64 texel tile is one sector, 32 for an older one), `Fit` puts a
/// fixed number of tiles across each face. The material's own default
/// (`grid_uv`) applies to faces that inherit.
fn grid_tile(
    material: &psxed_project::MaterialResource,
    texels: [u16; 2],
    sector_editor: Option<i32>,
    world_uv: UvMode,
) -> String {
    let mode =
        psxed_project::grid::uv::resolve(UvMode::Inherit, Some(material.grid_uv), world_uv);
    match mode {
        UvMode::World { texels_per_sector } => {
            let sectors = |texels: u16, density: u16| f64::from(texels) / f64::from(density.max(1));
            let (h, v) = (
                sectors(texels[0], texels_per_sector[0]),
                sectors(texels[1], texels_per_sector[1]),
            );
            let n = |value: f64| {
                if (value - value.round()).abs() < 0.005 {
                    format!("{}", value.round() as i64)
                } else {
                    format!("{value:.2}")
                }
            };
            let editor = sector_editor
                .map(|s| {
                    format!(
                        " = {} x {} editor units",
                        (h * f64::from(s)).round() as i64,
                        (v * f64::from(s)).round() as i64
                    )
                })
                .unwrap_or_default();
            format!(
                "world {} x {} texels/sector: a tile is {} x {} sectors{editor}",
                texels_per_sector[0],
                texels_per_sector[1],
                n(h),
                n(v)
            )
        }
        UvMode::Fit { repeats_q4 } => format!(
            "fit {} x {} tiles per face",
            f64::from(repeats_q4[0]) / 16.0,
            f64::from(repeats_q4[1]) / 16.0
        ),
        UvMode::Inherit => "-".to_string(),
    }
}

/// Every material with its texel size, depth, blend mode and tint, and what
/// one tile covers under the grid mapping.
///
/// Size is the point. Without it, how often a texture repeats over a surface
/// is guesswork. A grid world lays a texture by sector (the world's texels per
/// sector, 64 for a new world, unless a face or the material says otherwise), a brush world by units (16
/// per texel); the column follows the project's own kind of world.
pub fn materials(project: &ProjectDocument, project_root: &Path) -> String {
    materials_filtered(project, project_root, None)
}

/// [`materials`], listing only the resource ids in `only` when given.
pub fn materials_filtered(
    project: &ProjectDocument,
    project_root: &Path,
    only: Option<&std::collections::HashSet<u64>>,
) -> String {
    let grid = project.active_scene().world.grid();
    let sector_editor = grid.map(|world| world.scale.sector_editor_units());
    let world_uv = grid.map_or(UvMode::LEGACY, |world| world.default_uv());
    let mut out = String::from("# Materials\n\n");
    out.push_str(if grid.is_some() {
        "| name | id | texels | grid tile (default mapping) | depth | blend | tint |\n"
    } else {
        "| name | id | texels | tile repeat | depth | blend | tint |\n"
    });
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    let mut listed = 0usize;
    for resource in &project.resources {
        let ResourceData::Material(material) = &resource.data else {
            continue;
        };
        if only.is_some_and(|ids| !ids.contains(&resource.id.raw())) {
            continue;
        }
        listed += 1;
        // Texel size and depth: a baked generated texture is known from its
        // recipe, an image from its `.psxt` header.
        let dimensions = if material.texture_mode == psxed_project::MaterialTextureMode::Generated {
            let size = psxed_project::normalize_generated_texture_size(material.generated.size);
            Some((size, size, 4u8))
        } else {
            material
                .psxt_path
                .as_deref()
                .and_then(|raw| resolve_texture(project_root, raw))
                .and_then(|path| psxt_dimensions(&path))
        };
        let (size, repeat, depth) = dimensions.map_or_else(
            || ("flat".to_string(), "-".to_string(), "-".to_string()),
            |(width, height, depth)| {
                let repeat = if grid.is_some() {
                    grid_tile(material, [width, height], sector_editor, world_uv)
                } else {
                    format!(
                        "{}x{}",
                        u32::from(width) * crate::UNITS_PER_TEXEL as u32,
                        u32::from(height) * crate::UNITS_PER_TEXEL as u32
                    )
                };
                (format!("{width}x{height}"), repeat, format!("{depth}bpp"))
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
    if let Some(world) = grid {
        let world_texels = world.texels_per_sector.get();
        let _ = writeln!(
            out,
            "\n{listed} materials. `grid tile` is what one full texture covers with the default \
             mapping (World, {world_texels} texels per sector, the world's setting, unless the \
             material's own tiling default says otherwise): a face can override it with \
             grid_tiling (World density, Fit N x M, or Fit across a whole rectangle), a \
             material's default is grid_material_tiling."
        );
    } else {
        let _ = writeln!(
            out,
            "\n{listed} materials. `tile repeat` is how many authored world units one full \
             texture covers at identity UV scale, at {} units per texel.",
            crate::UNITS_PER_TEXEL
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::face_uv::UvMode;

    fn row<'a>(table: &'a str, name: &str) -> &'a str {
        table
            .lines()
            .find(|l| l.starts_with(&format!("| {name} |")))
            .unwrap_or_else(|| panic!("no row for {name}: {table}"))
    }

    #[test]
    fn a_grid_project_reports_the_grid_mapping_not_the_brush_era_figure() {
        let project = crate::grid::test_support::new_grid_project();
        let table = materials(&project, Path::new("/nonexistent"));
        assert!(table.contains("grid tile (default mapping)"), "{table}");
        assert!(!table.contains("tile repeat"), "{table}");
        // A generated material: 64 texels, 32 per sector, so 2 sectors
        // (1280 editor units each at the new-project scale).
        let stone = row(&table, "Graybox / Warm Stone 64");
        assert!(stone.contains("| 64x64 |"), "{stone}");
        assert!(
            stone.contains(
                "world 32 x 32 texels/sector: a tile is 2 x 2 sectors = 2560 x 2560 editor units"
            ),
            "{stone}"
        );
        assert!(stone.contains("| 4bpp |"), "{stone}");
        // The brush-era 1024 figure is gone.
        assert!(!table.contains("1024x1024"), "{table}");
    }

    #[test]
    fn a_material_default_changes_the_figure() {
        let mut project = crate::grid::test_support::new_grid_project();
        let id = project
            .resources
            .iter()
            .find(|r| r.name == "Graybox / Moss 64")
            .map(|r| r.id)
            .unwrap();
        let ResourceData::Material(m) = &mut project.resource_mut(id).unwrap().data else {
            panic!("a material");
        };
        m.grid_uv = UvMode::World {
            texels_per_sector: [64, 64],
        };
        let table = materials(&project, Path::new("/nonexistent"));
        assert!(
            row(&table, "Graybox / Moss 64").contains("a tile is 1 x 1 sectors = 1280 x 1280"),
            "{table}"
        );
        m_fit(&mut project, id);
        let table = materials(&project, Path::new("/nonexistent"));
        assert!(
            row(&table, "Graybox / Moss 64").contains("fit 2 x 2 tiles per face"),
            "{table}"
        );
    }

    fn m_fit(project: &mut ProjectDocument, id: psxed_project::ResourceId) {
        let ResourceData::Material(m) = &mut project.resource_mut(id).unwrap().data else {
            panic!("a material");
        };
        m.grid_uv = UvMode::fit(2, 2);
    }
}
