//! CLI: cook the stress world for a run of seeds and judge each against the
//! design gates (M8). A generated world is only a test if it is not picked
//! because it passes, so this reports every seed: which gates it passes, which
//! it fails and by how much, and how much pool each one would have needed.
//!
//! Usage:
//!   stream-world-sweep [--config CONFIG.ron] [--first SEED] [--count N]
//!                      [--reclaim BYTES] [--pools R1,R2,...]
//!
//! The gates are the default design parameters; only the cut target and the
//! pool the generator names are applied (the same file the generator writes as
//! `stream-cook.ron`). `--reclaim` sets the RAM scenario (bytes the arena
//! hands back); the table at the end re-judges the pool gate at other
//! reclaim values without re-cooking.

use std::path::PathBuf;
use std::process::ExitCode;

use psxed_project::brush_region::{PartitionParams, RAM_BUDGET};
use psxed_project::brush_world::stream_cook::{cook_project_gated, CookedWorld, StreamPvs};
use psxed_project::brush_world::BrushWorldCookMode;
use psxed_project::stream_world::{generate, load_donor, StreamWorldConfig};

struct Row {
    seed: u64,
    regions: usize,
    payload: u64,
    max_closure: usize,
    failures: Vec<&'static str>,
    others_ok: bool,
    rho: f64,
    rho_limit: f64,
    peak: u64,
    pool_available: u64,
    skeleton: u64,
    ball_regions: usize,
    cook_seconds: f64,
    over_budget: usize,
}

fn main() -> ExitCode {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let projects = manifest.join("../../projects");
    let mut config_path = projects.join("stream-world.config.ron");
    let mut first = 1u64;
    let mut count = 50u64;
    let mut reclaim: Option<u32> = None;
    let mut pools: Vec<u32> = vec![0, 50_000, 100_000, 150_000, 200_000, 300_000, 400_000];
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config_path = args.next().map(PathBuf::from).unwrap_or(config_path),
            "--first" => first = args.next().and_then(|v| v.parse().ok()).unwrap_or(first),
            "--count" => count = args.next().and_then(|v| v.parse().ok()).unwrap_or(count),
            "--reclaim" => reclaim = args.next().and_then(|v| v.parse().ok()),
            "--pools" => {
                pools = args
                    .next()
                    .map(|v| v.split(',').filter_map(|p| p.parse().ok()).collect())
                    .unwrap_or(pools)
            }
            other => {
                eprintln!("[sweep] unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let mut base = match std::fs::read_to_string(&config_path) {
        Ok(text) => StreamWorldConfig::from_ron_str(&text).unwrap_or_else(|e| panic!("{e}")),
        Err(_) => StreamWorldConfig::default(),
    };
    if let Some(bytes) = reclaim {
        base.arena_reclaim_bytes = bytes;
    }
    let donor = load_donor(&projects).unwrap_or_else(|e| panic!("donor: {e}"));
    let root = projects.join("graybox-reach");
    println!(
        "# stress world sweep: grid {}x{}, module {} units, cut target {} B, detail {} B, door degree <= {}, seeds {}..{}",
        base.grid[0], base.grid[1], base.module_units, base.region_target_bytes,
        base.detail_bytes, base.max_door_degree, first, first + count - 1
    );
    let plan = base.pool_plan();
    println!(
        "# {}",
        plan.describe(base.region_target_bytes)
            .replace('\n', "\n# ")
    );
    println!("seed regions payload_KB max|V| ball  rho/limit(B/u)  peak/avail(KB)  failures");

    let mut rows: Vec<Row> = Vec::new();
    for seed in first..first + count {
        let config = StreamWorldConfig {
            seed,
            ..base.clone()
        };
        let world = match generate(&config, &donor) {
            Ok(world) => world,
            Err(error) => {
                println!("{seed} generator error: {error}");
                continue;
            }
        };
        let mut params = PartitionParams::default();
        world.overrides.apply(&mut params);
        let started = std::time::Instant::now();
        let cooked = cook_project_gated(
            &world.project,
            &root,
            BrushWorldCookMode::Draft,
            [0; 3],
            &params,
            StreamPvs::Clustered {
                reach: params.vis_distance,
            },
        );
        let cooked = match cooked {
            Ok(cooked) => cooked,
            Err(error) => {
                println!("{seed} cook error: {error}");
                continue;
            }
        };
        let (Some(measured), CookedWorld::Streamed(streamed)) = (&cooked.measured, &cooked.world)
        else {
            println!("{seed} cooked as one region");
            continue;
        };
        let gate = &measured.gate;
        let failures: Vec<&'static str> = {
            let mut kinds: Vec<&'static str> = gate.failures.iter().map(|f| f.kind()).collect();
            kinds.sort_unstable();
            kinds.dedup();
            kinds
        };
        let peak = gate.pool.as_ref().map_or(0, |p| p.bytes);
        let row = Row {
            seed,
            regions: measured.regions.len(),
            payload: streamed.payloads.iter().map(|p| p.len() as u64).sum(),
            max_closure: gate.max_closure,
            others_ok: failures.iter().all(|k| *k == "pool"),
            failures,
            rho: gate.rho_worst(),
            rho_limit: gate.rho_gate_limit,
            peak,
            pool_available: gate.pool_available,
            skeleton: gate.skeleton_bytes,
            ball_regions: gate.pool.as_ref().map_or(0, |p| p.lead_regions),
            cook_seconds: started.elapsed().as_secs_f64(),
            over_budget: world.stats.over_budget_modules,
        };
        println!(
            "{:>4} {:>7} {:>10.0} {:>5} {:>4}  {:>6.1}/{:<6.1}  {:>6.0}/{:<6.0}  {}{}",
            row.seed,
            row.regions,
            row.payload as f64 / 1024.0,
            row.max_closure,
            row.ball_regions,
            row.rho,
            row.rho_limit,
            row.peak as f64 / 1024.0,
            row.pool_available as f64 / 1024.0,
            if row.failures.is_empty() {
                "PASS".to_string()
            } else {
                row.failures.join("+")
            },
            if row.over_budget > 0 {
                format!("  ({} modules over the generator budget)", row.over_budget)
            } else {
                String::new()
            },
        );
        rows.push(row);
    }
    if rows.is_empty() {
        return ExitCode::from(1);
    }

    let n = rows.len();
    let passed = rows.iter().filter(|r| r.failures.is_empty()).count();
    println!("\n# {passed} of {n} seeds pass every gate at this pool");
    let mut by_kind: std::collections::BTreeMap<&str, usize> = Default::default();
    for row in &rows {
        for kind in &row.failures {
            *by_kind.entry(kind).or_default() += 1;
        }
    }
    for (kind, seeds) in &by_kind {
        println!("#   {seeds} of {n} fail the {kind} gate");
    }
    let mut peaks: Vec<u64> = rows.iter().map(|r| r.peak + r.skeleton).collect();
    peaks.sort_unstable();
    let pct = |p: usize| peaks[(peaks.len() - 1) * p / 100];
    println!(
        "# pool a seed needs (peak requirement + skeleton), KB: min {:.0}, median {:.0}, p90 {:.0}, max {:.0}",
        peaks[0] as f64 / 1024.0,
        pct(50) as f64 / 1024.0,
        pct(90) as f64 / 1024.0,
        *peaks.last().unwrap() as f64 / 1024.0
    );
    let mut rho: Vec<f64> = rows.iter().map(|r| r.rho / r.rho_limit).collect();
    rho.sort_by(f64::total_cmp);
    println!(
        "# rho / limit: min {:.2}, median {:.2}, max {:.2}; seeds under 1.0: {}",
        rho[0],
        rho[rho.len() / 2],
        rho[rho.len() - 1],
        rho.iter().filter(|r| **r <= 1.0).count()
    );
    let mean_seconds = rows.iter().map(|r| r.cook_seconds).sum::<f64>() / n as f64;
    println!("# mean partition + cook time {mean_seconds:.1}s per seed");
    println!("\n# pass rate if the arena handed back R bytes (the other gates as measured):");
    println!("# reclaim_KB pool_KB pass_rate");
    for reclaim in pools {
        let pool = u64::from(RAM_BUDGET.world_pool_bytes()) + u64::from(reclaim);
        let ok = rows
            .iter()
            .filter(|r| r.others_ok && r.peak + r.skeleton <= pool)
            .count();
        println!(
            "# {:>10.0} {:>7.0} {:>4}/{} = {:.0}%",
            f64::from(reclaim) / 1024.0,
            pool as f64 / 1024.0,
            ok,
            n,
            100.0 * ok as f64 / n as f64
        );
    }
    ExitCode::SUCCESS
}
