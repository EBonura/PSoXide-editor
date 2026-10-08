//! World-streaming partition queries (design 2026-10-08, section 7.3).
//!
//! Pure functions of a project, like the rest of this crate: they run the
//! host partitioner in `psxed_project::brush_region` and answer one question
//! each. A partition is deterministic and takes seconds on a large world, so
//! every call recomputes; there is no cache to go stale after an edit.

use std::fmt::Write as _;
use std::path::Path;

use psxed_project::brush_region::{
    partition, CookOverrides, PartitionInput, PartitionParams, StreamReport,
};
use psxed_project::ProjectDocument;

/// Authored units per engine unit; the partitioner works in engine units.
const DIVISOR: f64 = 16.0;

/// What to ask of the partition.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionQuery {
    /// The full cook report, as text or JSON.
    Report {
        /// JSON instead of text.
        json: bool,
    },
    /// Which region holds an authored point, with its payload and neighbours.
    RegionAt {
        /// Authored x, y, z.
        point: [f64; 3],
    },
    /// What a region sees and what it needs resident.
    Closure {
        /// Region id from the report.
        region: u32,
    },
    /// The disc layout cost and seek-class histogram.
    LayoutCost,
}

/// Answer `query`. `pool_bytes` overrides the page pool the gates judge
/// against; the report states any override in effect.
pub fn regions(
    project: &ProjectDocument,
    project_root: &Path,
    query: &RegionQuery,
    pool_bytes: Option<u32>,
) -> Result<String, String> {
    let mut params = PartitionParams::default();
    CookOverrides {
        pool_bytes,
        ..CookOverrides::default()
    }
    .apply(&mut params);
    let input = PartitionInput::from_project(project, project_root).map_err(|e| e.to_string())?;
    let part = partition(&input, &params);
    let report = StreamReport::build(&project.name, &input, &part);
    let mut out = String::new();
    match query {
        RegionQuery::Report { json } => {
            out = if *json { report.json } else { report.text };
        }
        RegionQuery::RegionAt { point } => {
            let engine = point.map(|v| v / DIVISOR);
            let id = part.region_at(engine);
            let r = &part.regions[id as usize];
            let _ = writeln!(
                out,
                "region {id}: {} B ({} sectors), {} faces, {} leaves, {} clipnodes, bounds {:?} to {:?} (engine units)",
                r.counts.bytes(),
                r.counts.sectors(),
                r.counts.faces,
                r.counts.leaves,
                r.counts.clip_nodes,
                r.bounds.min,
                r.bounds.max
            );
            let _ = writeln!(
                out,
                "neighbours over open apertures: {:?}",
                part.graph.adjacency[id as usize]
                    .iter()
                    .map(|&(n, _)| n)
                    .collect::<Vec<_>>()
            );
        }
        RegionQuery::Closure { region } => {
            let id = *region as usize;
            if id >= part.regions.len() {
                return Err(format!(
                    "region {region} does not exist (0..{})",
                    part.regions.len()
                ));
            }
            let _ = writeln!(out, "region {region} sees {:?}", part.closure.visible[id]);
            let _ = writeln!(
                out,
                "needs {} regions resident; hook landings pulled in: {:?}",
                part.closure.need[id].count(),
                part.closure.hook_pulls[id]
            );
            let _ = writeln!(
                out,
                "PVS row width {} bytes of {} (estimate: sampled line of sight)",
                (part.closure.visible[id].len() as u32 * params.caps.leaves).div_ceil(8),
                psx_bsp_row_limit()
            );
        }
        RegionQuery::LayoutCost => {
            let l = &part.layout;
            let _ = writeln!(
                out,
                "layout {} sectors, weighted distance {} (cut-tree order {}); edges by sector gap: <=16 {}, <=128 {}, <=512 {}, more {}",
                l.total_sectors, l.cost, l.baseline_cost, l.class_counts[0], l.class_counts[1], l.class_counts[2], l.class_counts[3]
            );
        }
    }
    Ok(out)
}

fn psx_bsp_row_limit() -> usize {
    1024
}
