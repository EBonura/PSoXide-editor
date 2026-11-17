//! Run the combat duel over several seeds and print one baseline table.
//!
//! The `combat_duel` MCP tool runs one seed per call. This builds the project
//! disc once (or reuses the last one), hashes it, replays every seed against
//! that same disc and writes `baseline.json` next to the per-seed artifacts,
//! so a later change can be compared against exactly what ran before.
//!
//! ```text
//! cargo run --release -p psxed-mcp --example duel-baseline -- \
//!     --frontend target/release/psoxide-emulator \
//!     --project projects/graybox-reach --seeds 1,2,3,4,5,6,7,8
//! ```
//!
//! Flags: `--frontend PATH` (default: the usual `target/*` search), `--project DIR`
//! (default `projects/graybox-reach`), `--seeds A,B,C` (1..255, not 128), `--polls N`
//! (default 11400, the 180 second limit), `--out DIR`, `--skip-build` (reuse the
//! newest disc under `<project>/baked`), `--jobs N` (seeds run at the same time; default
//! the performance cores minus one, `--jobs 1` is the sequential run).
//!
//! Seeds are independent: each boots the same read-only disc in its own emulator process and
//! writes only under its own `seed-N` directory. With `--jobs` above one the seeds finish in any
//! order, but the table rows, `baseline.json` runs and every per-seed file are the same as the
//! sequential run, and rows print in seed order.
//!
//! The disc is built with the same guest features `combat_duel` uses
//! (`cd-stream-bench host-tty`), so the duel logs exist.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use psxed_mcp::{duel, play, shot};
use serde_json::{json, Value};

fn sha256(path: &std::path::Path) -> String {
    Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

/// Performance cores minus one (at least one): the default `--jobs`. Reads the Apple silicon
/// performance-core count and falls back to the logical CPU count elsewhere.
fn default_jobs() -> usize {
    let performance_cores = Command::new("sysctl")
        .args(["-n", "hw.perflevel0.logicalcpu"])
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<usize>()
                .ok()
        });
    performance_cores
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .map_or(1, |cores| cores.saturating_sub(1).max(1))
}

/// One table row, the same text the sequential run printed.
fn table_row(seed: u8, report: &Value) -> String {
    let damage = &report["damage_taken_player_enemy"];
    let pair = |key: &str| format!("{},{}", report[key][0], report[key][1]);
    format!(
        "{seed} {} {} {} {} {} {} {} {}",
        report["outcome"].as_str().unwrap_or("?"),
        report["end_tick"],
        damage[0],
        damage[1],
        pair("stance_switches_player_enemy"),
        pair("parries_player_enemy"),
        pair("crits_player_enemy"),
        pair("knockdowns_player_enemy"),
    )
}

fn main() {
    let mut frontend: Option<PathBuf> = None;
    let mut project = PathBuf::from("projects/graybox-reach");
    let mut seeds: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let mut polls = 11400u32;
    let mut out: Option<PathBuf> = None;
    let mut skip_build = false;
    let mut jobs = default_jobs();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .unwrap_or_else(|| panic!("{name} needs a value"))
        };
        match flag.as_str() {
            "--frontend" => frontend = Some(PathBuf::from(value("--frontend"))),
            "--project" => project = PathBuf::from(value("--project")),
            "--seeds" => {
                seeds = value("--seeds")
                    .split(',')
                    .map(|s| s.trim().parse().expect("seed must be 1..255"))
                    .collect();
            }
            "--polls" => polls = value("--polls").parse().expect("polls must be a number"),
            "--out" => out = Some(PathBuf::from(value("--out"))),
            "--skip-build" => skip_build = true,
            "--jobs" => jobs = value("--jobs").parse().expect("jobs must be a number"),
            other => panic!("unknown flag {other}"),
        }
    }
    assert!(jobs >= 1, "--jobs must be at least 1");
    assert!(
        seeds.iter().all(|s| *s != 0 && *s != 128),
        "seeds are 1..255 except 128"
    );
    let frontend = shot::find_frontend(frontend.as_deref()).expect("frontend binary");
    let cue = if skip_build {
        play::last_cue(&project)
    } else {
        play::build_disc(&frontend, &project)
    }
    .expect("project disc");
    let bin = cue.with_extension("bin");
    let disc_sha = sha256(&bin);
    eprintln!("disc {} sha256 {disc_sha}", cue.display());
    let out = out.unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        project
            .join("../../validation/combat-duels")
            .join(format!("baseline-{stamp}"))
    });
    println!(
        "seed outcome end_tick player_taken enemy_taken switches(p,e) parries(p,e) crits(p,e) knockdowns(p,e)"
    );
    let reports = run_seeds(&frontend, &cue, &out, &seeds, polls, jobs);
    let runs: Vec<Value> = reports.iter().map(compact).collect();
    let baseline = json!({
        "project": project,
        "disc_cue": cue,
        "disc_bin_sha256": disc_sha,
        "frontend": frontend,
        "frontend_sha256": sha256(&frontend),
        "polls": polls,
        "runs": runs,
    });
    std::fs::create_dir_all(&out).expect("create output directory");
    let path = out.join("baseline.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&baseline).unwrap()).expect("write baseline");
    eprintln!("baseline -> {}", path.display());
    // A seed that could not run is a failed gate, not a row to skim past.
    if runs.iter().any(|run| run["outcome"] == "run_failed") {
        eprintln!("duel-baseline: at least one seed failed to run");
        std::process::exit(1);
    }
}

/// Run every seed, up to `jobs` at a time, and return the reports in seed order. Each row prints
/// as soon as it and every row before it are done, so the output order never depends on which
/// emulator finishes first.
fn run_seeds(
    frontend: &std::path::Path,
    cue: &std::path::Path,
    out: &std::path::Path,
    seeds: &[u8],
    polls: u32,
    jobs: usize,
) -> Vec<Value> {
    struct Done {
        reports: Vec<Option<Value>>,
        printed: usize,
    }
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Done {
        reports: vec![None; seeds.len()],
        printed: 0,
    });
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(seeds.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&seed) = seeds.get(index) else {
                    return;
                };
                let report = duel::run(
                    frontend,
                    cue,
                    &out.join(format!("seed-{seed}")),
                    seed,
                    polls,
                )
                .unwrap_or_else(
                    |error| json!({"seed": seed, "outcome": "run_failed", "error": error}),
                );
                let mut done = done.lock().unwrap();
                done.reports[index] = Some(report);
                while let Some(Some(ready)) = done.reports.get(done.printed) {
                    println!("{}", table_row(seeds[done.printed], ready));
                    done.printed += 1;
                }
            });
        }
    });
    done.into_inner()
        .unwrap()
        .reports
        .into_iter()
        .map(|report| report.expect("every seed ran"))
        .collect()
}

/// The numbers worth comparing across builds; the full trace stays in each
/// seed's `report.json`.
fn compact(report: &Value) -> Value {
    let pick = |key: &str| report.get(key).cloned().unwrap_or(Value::Null);
    json!({
        "seed": pick("seed"),
        "outcome": pick("outcome"),
        "end_tick": pick("end_tick"),
        "completed_by_death": pick("completed_by_death"),
        "damage_taken_player_enemy": pick("damage_taken_player_enemy"),
        "damage_events_player_enemy": pick("damage_events_player_enemy"),
        "stance_switches_player_enemy": pick("stance_switches_player_enemy"),
        "stance_ticks_player_enemy": pick("stance_ticks_player_enemy"),
        "player_accepted_light_heavy_ranged": pick("player_accepted_light_heavy_ranged"),
        "enemy_windups_light_heavy_ranged": pick("enemy_windups_light_heavy_ranged"),
        "projectiles_emitted_player_enemy": pick("projectiles_emitted_player_enemy"),
        "energy_min_player_enemy": pick("energy_min_player_enemy"),
        "energy_end_player_enemy": pick("energy_end_player_enemy"),
        "parries_player_enemy": pick("parries_player_enemy"),
        "crits_player_enemy": pick("crits_player_enemy"),
        "knockdowns_player_enemy": pick("knockdowns_player_enemy"),
        "combat_events": pick("combat_events"),
        "warnings": pick("warnings"),
        "artifacts": pick("artifacts"),
    })
}
