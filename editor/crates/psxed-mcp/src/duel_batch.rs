//! Many seeded duels, one aggregate report.
//!
//! A single duel is a sample of one. Balance questions ("does a light hit
//! still stun-lock the enemy?") need the same encounter replayed over a seed
//! set, with the build and the inputs recorded so a later run can be compared
//! against it. This module owns that: scenario selection, a bounded worker
//! pool over the emulator, the aggregate, and the contract that makes two
//! aggregates comparable.

use crate::duel;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Never run more emulators than this at once.
pub const MAX_PARALLEL: usize = 2;
/// Seeds are one stick byte: 1..=255, and 128 is the physical default.
pub const RESERVED_SEED: u8 = 128;

/// Which encounter to replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// The project exactly as authored: one light enemy.
    Graybox,
    /// The same arena with the Heavy Enemy resource standing in for the light
    /// enemy. Derived at run time; the authored layout is never edited.
    Heavy,
}

impl Scenario {
    /// Parse a scenario name as given on a command line or tool call.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "graybox" | "light" => Ok(Self::Graybox),
            "heavy" => Ok(Self::Heavy),
            other => Err(format!("unknown scenario {other:?}; use graybox or heavy")),
        }
    }

    /// Stable name used in reports and directory names.
    pub fn name(self) -> &'static str {
        match self {
            Self::Graybox => "graybox",
            Self::Heavy => "heavy",
        }
    }

    /// Project directory to build for this scenario. The Heavy scenario is a
    /// sibling project under the same parent, because project assets reach
    /// shared libraries through `../` paths.
    pub fn prepare(self, project_root: &Path) -> Result<PathBuf, String> {
        match self {
            Self::Graybox => Ok(project_root.to_path_buf()),
            Self::Heavy => {
                // The asset link below must not depend on the caller's working directory.
                let project_root = &std::fs::canonicalize(project_root).map_err(|e| e.to_string())?;
                let parent = project_root
                    .parent()
                    .ok_or_else(|| format!("{} has no parent", project_root.display()))?;
                let name = project_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or("project directory name is not UTF-8")?;
                let derived = parent.join(format!("zz-duel-heavy-{name}"));
                let source = std::fs::read_to_string(project_root.join("project.ron"))
                    .map_err(|e| format!("read project.ron: {e}"))?;
                let patched = heavy_project_source(&source)?;
                std::fs::create_dir_all(&derived).map_err(|e| e.to_string())?;
                std::fs::write(derived.join("project.ron"), patched).map_err(|e| e.to_string())?;
                let assets = derived.join("assets");
                if std::fs::symlink_metadata(&assets).is_err() {
                    std::os::unix::fs::symlink(project_root.join("assets"), &assets)
                        .map_err(|e| format!("link assets: {e}"))?;
                }
                Ok(derived)
            }
        }
    }
}

/// Swap the single light enemy for the Heavy Enemy resource.
///
/// The rewrite is textual and anchored: each anchor must occur exactly once,
/// so a project that has drifted fails loudly rather than yielding a scenario
/// that quietly is not the heavy one. Placement values (size, spacing,
/// timings, poise 50, health 200 per channel) are the ones the Cortex
/// Ignition 0.5 project uses for its placed Heavy Enemy, with the training
/// flag added so the duel bot can target it. Chosen starting points for a
/// comparison scenario, not tuned numbers.
pub fn heavy_project_source(source: &str) -> Result<String, String> {
    const MODEL_FROM: &str = "kind: ModelRenderer(model: Some((26)), material: None, visual_offset: (0, 0, 0), visual_scale_q8: 417)";
    const MODEL_TO: &str = "kind: ModelRenderer(model: Some((97)), material: None, visual_offset: (0, 0, 0), visual_scale_q8: 416)";
    const BODY_FROM: &str = "character: Some((113)), loadout: Some(0), settings: (radius: 226, height: 1229, walk_speed: 32,";
    const BODY_TO: &str = "character: Some((95)), loadout: None, settings: (radius: 320, height: 1741, walk_speed: 48,";
    const ENEMY_START: &str = "enemy: Some((tactical: true, training: true, aggro_radius: 6400,";
    const ENEMY_END: &str = "soul_value: 50))";
    const ENEMY_TO: &str = "enemy: Some((tactical: false, training: true, aggro_radius: 6144, patrol_offset: (0, 0, 0), patrol_wait_ticks: 60, reaction_ticks: 72, preferred_distance: 1024, spacing_tolerance: 192, spacing_speed_percent: 100, decision_interval_ticks: 24, circle_chance: 20, attack_priority: 6, attack_cooldown_ticks: 36, group_attack_delay_ticks: 24, windup_ticks: 24, recovery_ticks: 36, poise: 50, touch_damage: 10, max_health: 200, max_health_secondary: 200, soul_value: 50))";
    let once = |text: &str, needle: &str| -> Result<usize, String> {
        let mut found = text.match_indices(needle);
        let first = found
            .next()
            .ok_or_else(|| format!("anchor missing: {needle}"))?;
        if found.next().is_some() {
            return Err(format!("anchor is not unique: {needle}"));
        }
        Ok(first.0)
    };
    let mut text = source.replace(MODEL_FROM, MODEL_TO);
    once(&text, MODEL_TO)?;
    once(&text, BODY_FROM)?;
    text = text.replace(BODY_FROM, BODY_TO);
    let start = once(&text, ENEMY_START)?;
    let end = text[start..]
        .find(ENEMY_END)
        .ok_or("enemy settings end anchor missing")?
        + start
        + ENEMY_END.len();
    text.replace_range(start..end, ENEMY_TO);
    Ok(text)
}

/// Seeds to run: `1..=count` without the reserved value.
pub fn default_seeds(count: usize) -> Vec<u8> {
    (1..=255u8)
        .filter(|s| *s != RESERVED_SEED)
        .take(count)
        .collect()
}

/// Parse `1-20`, `3,5,9` or `1-4,9`. Rejects 0 and the reserved seed.
pub fn parse_seeds(spec: &str) -> Result<Vec<u8>, String> {
    let mut seeds = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: u8 = a.parse().map_err(|_| format!("bad seed {part:?}"))?;
        let b: u8 = b.parse().map_err(|_| format!("bad seed {part:?}"))?;
        if a == 0 || b < a {
            return Err(format!("bad seed range {part:?}"));
        }
        for seed in a..=b {
            if seed == RESERVED_SEED {
                return Err(format!("seed {RESERVED_SEED} is reserved; split the range"));
            }
            if !seeds.contains(&seed) {
                seeds.push(seed);
            }
        }
    }
    if seeds.is_empty() {
        return Err("no seeds".into());
    }
    Ok(seeds)
}

/// Statistics over one metric.
fn stats(values: &mut [f64]) -> Value {
    values.sort_by(|a, b| a.total_cmp(b));
    let n = values.len();
    let sum: f64 = values.iter().sum();
    let median = if n == 0 {
        0.0
    } else if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    };
    let round = |v: f64| (v * 100.0).round() / 100.0;
    json!({
        "n": n,
        "sum": round(sum),
        "mean": round(if n == 0 { 0.0 } else { sum / n as f64 }),
        "median": round(median),
        "min": round(values.first().copied().unwrap_or(0.0)),
        "max": round(values.last().copied().unwrap_or(0.0)),
    })
}

/// Walk a metrics object, calling `visit(path, value)` for every number.
/// Strings and nulls are skipped, so a missing measurement never counts as 0.
fn numeric_leaves(prefix: &str, value: &Value, visit: &mut dyn FnMut(&str, f64)) {
    match value {
        Value::Number(n) => {
            if let Some(v) = n.as_f64() {
                visit(prefix, v);
            }
        }
        Value::Object(map) => {
            for (key, inner) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                numeric_leaves(&path, inner, visit);
            }
        }
        _ => {}
    }
}

/// Combine per-duel reports into one aggregate. `runs` pairs each seed with
/// its duel report (the value `duel::summarize` returns) or the reason the run
/// failed to produce one.
pub fn aggregate(runs: &[(u8, Result<Value, String>)]) -> Value {
    let mut outcomes: Map<String, Value> = Map::new();
    let mut per_seed = Vec::new();
    let mut failures = Vec::new();
    let mut columns: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut ordered: Vec<_> = runs.iter().collect();
    ordered.sort_by_key(|(seed, _)| *seed);
    for (seed, run) in ordered {
        match run {
            Err(reason) => failures.push(json!({"seed": seed, "reason": reason})),
            Ok(report) => {
                let metrics = report.get("metrics").cloned().unwrap_or(Value::Null);
                let outcome = metrics["outcome"].as_str().unwrap_or("incomplete");
                let count = outcomes.get(outcome).and_then(Value::as_u64).unwrap_or(0);
                outcomes.insert(outcome.to_string(), json!(count + 1));
                numeric_leaves("", &metrics, &mut |path, v| {
                    columns.entry(path.to_string()).or_default().push(v);
                });
                let mut row = metrics.clone();
                if let Some(map) = row.as_object_mut() {
                    map.remove("definitions");
                    map.insert("seed".into(), json!(seed));
                }
                per_seed.push(row);
            }
        }
    }
    let metrics: Map<String, Value> = columns
        .into_iter()
        .map(|(path, mut values)| (path, stats(&mut values)))
        .collect();
    json!({
        "runs": per_seed.len(),
        "failed_runs": failures.len(),
        "outcomes": outcomes,
        "metrics": metrics,
        "per_seed": per_seed,
        "failures": failures,
    })
}

/// A short markdown rendering of an aggregate and its contract.
pub fn markdown(report: &Value) -> String {
    let agg = &report["aggregate"];
    let contract = &report["contract"];
    let mut out = String::new();
    out.push_str(&format!(
        "# Combat duel batch: {}\n\n",
        contract["label"].as_str().unwrap_or("unlabelled")
    ));
    out.push_str(&format!(
        "- scenario `{}`, {} runs ({} failed), seeds {}\n",
        contract["scenario"].as_str().unwrap_or("?"),
        agg["runs"],
        agg["failed_runs"],
        contract["seeds"]
    ));
    out.push_str(&format!(
        "- source `{}`, dirty {}, polls {}, disc sha256 `{}`\n",
        contract["source_rev"].as_str().unwrap_or("?"),
        contract["dirty"],
        contract["polls"],
        contract["disc_sha256"].as_str().unwrap_or("?")
    ));
    out.push_str(&format!("- outcomes {}\n\n", agg["outcomes"]));
    out.push_str("| metric | mean | median | min | max | sum |\n|---|---:|---:|---:|---:|---:|\n");
    if let Some(metrics) = agg["metrics"].as_object() {
        for (path, s) in metrics {
            out.push_str(&format!(
                "| {path} | {} | {} | {} | {} | {} |\n",
                s["mean"], s["median"], s["min"], s["max"], s["sum"]
            ));
        }
    }
    out.push_str("\n| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |\n|---:|---|---:|---:|---:|---:|---:|\n");
    if let Some(rows) = agg["per_seed"].as_array() {
        for r in rows {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                r["seed"],
                r["outcome"].as_str().unwrap_or("?"),
                r["duration_ticks"],
                r["player_hp_end"],
                r["enemy_hp_end"],
                r["poise_breaks_inflicted"],
                r["poise_breaks_suffered"]
            ));
        }
    }
    if let Some(failures) = agg["failures"].as_array().filter(|f| !f.is_empty()) {
        out.push_str("\nFailures:\n");
        for f in failures {
            out.push_str(&format!("- seed {}: {}\n", f["seed"], f["reason"]));
        }
    }
    out
}

/// What to run.
pub struct BatchOptions {
    /// Renderer binary that builds discs and runs the emulator.
    pub frontend: PathBuf,
    /// Authored project directory (Graybox Reach).
    pub project: PathBuf,
    /// Encounter to replay.
    pub scenario: Scenario,
    /// Seeds, 1..=255 excluding 128.
    pub seeds: Vec<u8>,
    /// Emulators to run at once, at most [`MAX_PARALLEL`].
    pub parallel: usize,
    /// Total input polls per duel, including loading.
    pub polls: u32,
    /// Reuse the last built disc instead of building.
    pub skip_build: bool,
    /// Directory for per-seed artifacts and the aggregate files.
    pub out: PathBuf,
    /// Free-text name recorded in the contract.
    pub label: String,
}

fn command_output(program: &str, args: &[&str], dir: &Path) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn sha256(path: &Path) -> Option<String> {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| {
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .flatten()
}

/// Everything needed to decide whether two aggregates are comparable.
fn contract(options: &BatchOptions, project: &Path, cue: &Path) -> Value {
    let dir = project;
    let status = command_output("git", &["status", "--porcelain"], dir).unwrap_or_default();
    let dirty_paths: Vec<&str> = status.lines().take(40).collect();
    let diff = command_output("git", &["diff", "HEAD"], dir).unwrap_or_default();
    let env_flags: Map<String, Value> = [
        "EDITOR_PLAYTEST_FEATURES",
        "EDITOR_PLAYTEST_CARGO_FEATURE_FLAGS",
        "EDITOR_PLAYTEST_PSX_BUILD_FLAGS",
        "PSOXIDE_GUEST_STAGE_ROOT",
        "PSOXIDE_GUEST_EXTRA_RUSTFLAGS",
    ]
    .iter()
    .map(|k| (k.to_string(), json!(std::env::var(k).ok())))
    .collect();
    json!({
        "label": options.label,
        "scenario": options.scenario.name(),
        "project": project,
        "source_rev": command_output("git", &["rev-parse", "HEAD"], dir),
        "source_branch": command_output("git", &["rev-parse", "--abbrev-ref", "HEAD"], dir),
        "dirty": !status.is_empty(),
        "dirty_paths": dirty_paths,
        "dirty_diff_bytes": diff.len(),
        "seeds": options.seeds,
        "polls": options.polls,
        "parallel": options.parallel,
        "skipped_build": options.skip_build,
        "disc": cue,
        "disc_sha256": cue.with_extension("bin").exists().then(|| sha256(&cue.with_extension("bin"))).flatten(),
        "frontend": options.frontend,
        "frontend_sha256": sha256(&options.frontend),
        "build_flags": env_flags,
        "build_flag_defaults": "Makefile: EDITOR_PLAYTEST_FEATURES ?= cd-stream-bench; release profile; guest built from the canonical stage with the SDK delay-slot flags",
        "launch": "frontend launch --embedded-playtest --input-tape <pad_poll tape> --stop-at-poll <polls> --steps 10000000000",
        "rustc": command_output("rustc", &["-V"], dir),
        "started_unix_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
    })
}

/// Build (unless skipped), replay every seed, write `batch.json` and
/// `batch.md` into `options.out`, and return the full report.
pub fn execute(options: &BatchOptions) -> Result<Value, String> {
    if options.seeds.is_empty() {
        return Err("no seeds".into());
    }
    if let Some(bad) = options
        .seeds
        .iter()
        .find(|s| **s == 0 || **s == RESERVED_SEED)
    {
        return Err(format!("seed {bad} is not allowed"));
    }
    let parallel = options.parallel.clamp(1, MAX_PARALLEL);
    let project = options.scenario.prepare(&options.project)?;
    let cue = if options.skip_build {
        crate::play::last_cue(&project)?
    } else {
        crate::play::build_disc(&options.frontend, &project)?
    };
    std::fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(u8, Result<Value, String>)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..parallel {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(&seed) = options.seeds.get(index) else {
                    break;
                };
                let dir = options.out.join(format!("seed-{seed}"));
                let run = duel::run(&options.frontend, &cue, &dir, seed, options.polls);
                results.lock().unwrap().push((seed, run));
            });
        }
    });
    let runs = results.into_inner().unwrap();
    let report = json!({
        "contract": contract(options, &project, &cue),
        "aggregate": aggregate(&runs),
    });
    std::fs::write(
        options.out.join("batch.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(options.out.join("batch.md"), markdown(&report)).map_err(|e| e.to_string())?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        seed: u8,
        outcome: &str,
        ticks: u32,
        shots: u32,
        nulls: bool,
    ) -> (u8, Result<Value, String>) {
        let mut metrics = json!({
            "outcome": outcome,
            "duration_ticks": ticks,
            "hits_on_enemy": {"light": 2, "shot": shots},
            "definitions": "ignored",
        });
        if nulls {
            metrics["energy"] = json!({"player_spent": null});
        } else {
            metrics["energy"] = json!({"player_spent": 40});
        }
        (seed, Ok(json!({"metrics": metrics})))
    }

    #[test]
    fn aggregates_counts_means_medians_and_skips_missing_measurements() {
        let runs = vec![
            run(3, "player_won", 3000, 4, false),
            run(1, "enemy_won", 1000, 0, true),
            run(2, "player_won", 2000, 2, false),
            (4, Err("emulator failed".to_string())),
        ];
        let agg = aggregate(&runs);
        assert_eq!(agg["runs"], 3);
        assert_eq!(agg["failed_runs"], 1);
        assert_eq!(agg["outcomes"], json!({"player_won": 2, "enemy_won": 1}));
        let ticks = &agg["metrics"]["duration_ticks"];
        assert_eq!(ticks["n"], 3);
        assert_eq!(ticks["mean"], 2000.0);
        assert_eq!(ticks["median"], 2000.0);
        assert_eq!(ticks["min"], 1000.0);
        assert_eq!(ticks["max"], 3000.0);
        // A null is a missing measurement, not a zero.
        assert_eq!(agg["metrics"]["energy.player_spent"]["n"], 2);
        assert_eq!(agg["metrics"]["hits_on_enemy.shot"]["sum"], 6.0);
        // Text fields never become columns, and rows are ordered by seed.
        assert!(agg["metrics"].get("definitions").is_none());
        let seeds: Vec<_> = agg["per_seed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["seed"].clone())
            .collect();
        assert_eq!(seeds, vec![json!(1), json!(2), json!(3)]);
        assert_eq!(agg["failures"][0]["seed"], 4);
    }

    #[test]
    fn even_counts_take_the_midpoint_median() {
        let agg = aggregate(&[
            run(1, "player_won", 10, 0, false),
            run(2, "player_won", 20, 0, false),
        ]);
        assert_eq!(agg["metrics"]["duration_ticks"]["median"], 15.0);
    }

    #[test]
    fn an_empty_batch_aggregates_to_zero_runs() {
        let agg = aggregate(&[]);
        assert_eq!(agg["runs"], 0);
        assert_eq!(agg["outcomes"], json!({}));
    }

    #[test]
    fn markdown_names_the_contract_and_every_seed() {
        let report = json!({
            "contract": {"label": "baseline", "scenario": "graybox", "seeds": [1, 2], "source_rev": "abc", "dirty": false, "polls": 11400, "disc_sha256": "ff"},
            "aggregate": aggregate(&[run(1, "player_won", 10, 1, false), run(2, "enemy_won", 20, 2, false)]),
        });
        let md = markdown(&report);
        assert!(md.contains("baseline"));
        assert!(md.contains("| duration_ticks | 15.0 |"));
        assert!(md.contains("| 1 | player_won | 10 |"));
        assert!(md.contains("| 2 | enemy_won | 20 |"));
    }

    #[test]
    fn seed_lists_expand_and_reject_reserved_values() {
        assert_eq!(parse_seeds("1-3,9").unwrap(), vec![1, 2, 3, 9]);
        assert_eq!(parse_seeds("5,5,6").unwrap(), vec![5, 6]);
        assert!(parse_seeds("0").is_err());
        assert!(parse_seeds("127-129").is_err());
        assert!(parse_seeds("").is_err());
        let seeds = default_seeds(130);
        assert_eq!(seeds.len(), 130);
        assert!(!seeds.contains(&128));
        assert_eq!(&seeds[..3], &[1, 2, 3]);
    }

    #[test]
    fn scenario_names_round_trip() {
        assert_eq!(Scenario::parse("heavy").unwrap(), Scenario::Heavy);
        assert_eq!(Scenario::parse("graybox").unwrap().name(), "graybox");
        assert!(Scenario::parse("boss").is_err());
    }

    #[test]
    fn heavy_scenario_rewrites_exactly_one_enemy() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/graybox-reach/project.ron");
        let Ok(source) = std::fs::read_to_string(&path) else {
            return;
        };
        let patched = heavy_project_source(&source).expect("anchors still match the project");
        assert_ne!(patched, source);
        assert!(patched.contains("character: Some((95)), loadout: None"));
        assert!(!patched.contains("character: Some((113))"));
        let document =
            psxed_project::ProjectDocument::from_ron_str(&patched).expect("patched project parses");
        assert_eq!(
            document.name,
            psxed_project::ProjectDocument::from_ron_str(&source)
                .unwrap()
                .name
        );
        // A second pass finds no light enemy left to replace.
        assert!(heavy_project_source(&patched).is_err());
    }
}
