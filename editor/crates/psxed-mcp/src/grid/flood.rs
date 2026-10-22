//! Cells and flood reads: how a world cuts into streamed cells, and what a
//! budgeted portal flood reaches from a camera.
//!
//! Both cook the staged world in memory (no sky, no disc) and read the
//! cooked bytes the guest reads, with `psx_grid::flood`, the code the guest
//! runs. `rechunk_hex` previews the same world cut into hexagons without
//! touching the document.

use std::fmt::Write as _;
use std::path::Path;

use psxed_project::grid::census::{
    flood_page_budget, flood_report, resident_radii, sample_cameras, CellCensus, Graph, LevelBlobs,
    Sample, Sweep, NO_PAGE_BUDGET,
};
use psxed_project::grid::cook::GUEST_POOL_CEILING_KIB;
use psxed_project::grid::edit::chunks::{rechunk_hex, HexLayout};
use psxed_project::grid::GridWorld;
use psxed_project::playtest::MaterialBinding;
use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

/// Seed of the camera samples, so two runs measure the same cameras.
const SAMPLE_SEED: u64 = 12345;
/// Resident radii the sweep tabulates when none are asked for, sectors.
const DEFAULT_RESIDENT_RADII: [f64; 5] = [12.0, 14.0, 16.0, 20.0, 27.0];

/// The census of a world's cells.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CellsReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Read the world cut into hexagonal rooms of this size (centre to
    /// corner, sectors) instead of as authored. Nothing is changed.
    pub rechunk_hex: Option<f64>,
    /// Also flood from a spread of camera positions (a few per cell, fixed
    /// seed) and report what the flood reaches against the level's own ring
    /// rule. Takes a few seconds on a big world.
    pub sweep: Option<bool>,
    /// Bind every material as a 64 by 64 stand-in instead of the project's
    /// textures: the binding the flood design note was measured with, so its
    /// tables can be reproduced. The default reads the real cook.
    pub stand_in_materials: Option<bool>,
    /// Pool ceiling the sweep floods to, KiB. Defaults to what the guest can
    /// link.
    pub ceiling_kib: Option<usize>,
    /// With `sweep`: resident radii (sectors) to size the pool for.
    pub resident_radii: Option<Vec<f64>>,
}

/// A flood from one camera.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FloodReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Read the world cut into hexagonal rooms of this size instead of as
    /// authored. Nothing is changed.
    pub rechunk_hex: Option<f64>,
    /// The camera's WORLD sector `[x, z]`; the camera stands at its centre,
    /// at the height of the middle of the room that owns it.
    pub at: [i32; 2],
    /// Heading in degrees, 0 towards +X, 90 towards +Z. Default 0.
    pub yaw_degrees: Option<f64>,
    /// Draw radius, sectors. Defaults to the world's flood setting.
    pub near: Option<u32>,
    /// Resident radius, sectors. Defaults to the world's flood setting.
    pub res: Option<u32>,
    /// Most pages the resident set may hold. Defaults to the pool the guest
    /// can link, less the largest cell.
    pub budget_pages: Option<u32>,
}

/// The world to read: as authored, or cut into hexagons.
fn world_for(world: &GridWorld, rechunk: Option<f64>) -> Result<GridWorld, String> {
    let mut world = world.clone();
    if let Some(size) = rechunk {
        rechunk_hex(&mut world, HexLayout { size }).map_err(|e| e.to_string())?;
    }
    Ok(world)
}

fn blobs_of(
    project: &ProjectDocument,
    root: &Path,
    world: &GridWorld,
    binding: MaterialBinding,
) -> Result<LevelBlobs, String> {
    let cooked = psxed_project::playtest::cook_level_bytes(project, world, root, binding)
        .map_err(|e| format!("the cook refuses this world: {e}"))?;
    Ok(LevelBlobs::new(&cooked.core, &cooked.chunks))
}

/// `grid_cells`.
pub fn cells(
    project: &ProjectDocument,
    root: &Path,
    world: &GridWorld,
    req: &CellsReq,
) -> Result<String, String> {
    let world = world_for(world, req.rechunk_hex)?;
    let binding = if req.stand_in_materials.unwrap_or(false) {
        MaterialBinding::StandIn
    } else {
        MaterialBinding::Project
    };
    let blobs = blobs_of(project, root, &world, binding)?;
    let graph = Graph::new(&blobs);
    let census = CellCensus::of(&blobs, &graph);
    let mut out = String::new();
    if let Some(size) = req.rechunk_hex {
        let _ = writeln!(
            out,
            "(re-cut into hexagons of size {size}; the document is unchanged)"
        );
    }
    out.push_str(&census.report());
    if req.sweep.unwrap_or(false) {
        let ceiling = req.ceiling_kib.unwrap_or(GUEST_POOL_CEILING_KIB);
        let view = blobs.view();
        let samples = sample_cameras(&view, SAMPLE_SEED);
        out.push_str(&Sweep::run(&blobs, &graph, &samples, ceiling).report());
        let radii = req
            .resident_radii
            .clone()
            .unwrap_or_else(|| DEFAULT_RESIDENT_RADII.to_vec());
        let _ = writeln!(
            out,
            "pool when the flood also stops at a resident radius (two cameras per cell):"
        );
        for row in resident_radii(&blobs, ceiling, &radii) {
            let _ = writeln!(
                out,
                "  {:>4.0} sectors: cells mean {:.1}, pages mean {:.1}, peak {} pages, pool {} KiB (peak plus the largest cell), relaxations mean {:.0}",
                row.res_sectors, row.cells_mean, row.pages_mean, row.peak_pages, row.pool_kib, row.relaxations_mean
            );
        }
    }
    Ok(out)
}

/// `grid_flood`.
pub fn flood(
    project: &ProjectDocument,
    root: &Path,
    world: &GridWorld,
    req: &FloodReq,
) -> Result<String, String> {
    let world = world_for(world, req.rechunk_hex)?;
    let blobs = blobs_of(project, root, &world, MaterialBinding::Project)?;
    let graph = Graph::new(&blobs);
    let level = blobs.view();
    let s = level.sector_units() as i32;
    let Some(&room) = graph.owner.get(&(req.at[0], req.at[1])) else {
        return Err(format!(
            "no room owns world sector [{}, {}]; grid_rooms and grid_plan show where the world is",
            req.at[0], req.at[1]
        ));
    };
    let record = &level.rooms[usize::from(room)];
    let mid = i32::from(record.min_y) / 2 + i32::from(record.max_y) / 2 + record.origin[1];
    let sample = Sample {
        room: usize::from(room),
        at: [req.at[0] * s + s / 2, mid, req.at[1] * s + s / 2],
        yaw: req.yaw_degrees.unwrap_or(0.0).to_radians(),
    };
    let settings = world.stream.flood;
    let near = req.near.unwrap_or(u32::from(settings.near_radius));
    let res = req.res.unwrap_or(u32::from(settings.res_radius));
    if res < near {
        return Err(format!(
            "the resident radius ({res}) is below the draw radius ({near})"
        ));
    }
    let budget = req
        .budget_pages
        .unwrap_or_else(|| flood_page_budget(&blobs, GUEST_POOL_CEILING_KIB) as u32);
    let budget = if budget == 0 { NO_PAGE_BUDGET } else { budget };
    Ok(flood_report(&blobs, &graph, &sample, near, res, budget))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::test_support::fixture;

    fn root() -> std::path::PathBuf {
        psxed_project::projects_dir().join(psxed_project::grid::starter::STARTER_BASE)
    }

    #[test]
    fn the_census_reads_the_starter_and_floods_it_whole() {
        let (project, world) = fixture();
        let req = CellsReq {
            scene: None,
            rechunk_hex: None,
            sweep: Some(true),
            stand_in_materials: None,
            ceiling_kib: None,
            resident_radii: Some(vec![8.0]),
        };
        let text = cells(&project, &root(), &world, &req).unwrap();
        assert!(text.contains("cells 1,"), "{text}");
        assert!(text.contains("flood: resident cells"), "{text}");
        assert!(text.contains("whole level reached in 6 of them"), "{text}");
    }

    #[test]
    fn a_flood_from_a_sector_reports_its_cells_and_refuses_an_empty_one() {
        let (project, world) = fixture();
        let req = FloodReq {
            scene: None,
            rechunk_hex: None,
            at: [0, 0],
            yaw_degrees: Some(90.0),
            near: Some(4),
            res: Some(6),
            budget_pages: None,
        };
        let text = flood(&project, &root(), &world, &req).unwrap();
        assert!(text.contains("resident: 1 cells"), "{text}");
        let far = FloodReq {
            at: [500, 500],
            ..req
        };
        assert!(flood(&project, &root(), &world, &far).is_err());
        let backwards = FloodReq {
            near: Some(8),
            res: Some(4),
            at: [0, 0],
            ..far
        };
        assert!(flood(&project, &root(), &world, &backwards).is_err());
    }
}
