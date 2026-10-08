//! Replay a seed set of combat duels and write one aggregate report.
//!
//! ```text
//! duel-batch --project editor/projects/graybox-reach --frontend target/release/frontend \
//!     --seeds 1-20 --parallel 2 --out <dir> [--scenario graybox|heavy] [--polls 11400] \
//!     [--skip-build] [--label name]
//! ```
use psxed_mcp::duel_batch::{self, BatchOptions, Scenario};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: duel-batch --project <dir> --frontend <bin> --out <dir> [--seeds 1-20] \
         [--parallel 1|2] [--scenario graybox|heavy] [--polls N] [--skip-build] [--label name]"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut project, mut frontend, mut out) = (None, None, None);
    let mut seeds = "1-20".to_string();
    let (mut parallel, mut polls, mut skip_build) = (2usize, 11400u32, false);
    let (mut scenario, mut label) = ("graybox".to_string(), String::new());
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--project" => project = Some(PathBuf::from(value())),
            "--frontend" => frontend = Some(PathBuf::from(value())),
            "--out" => out = Some(PathBuf::from(value())),
            "--seeds" => seeds = value(),
            "--parallel" => parallel = value().parse().unwrap_or_else(|_| usage()),
            "--polls" => polls = value().parse().unwrap_or_else(|_| usage()),
            "--scenario" => scenario = value(),
            "--label" => label = value(),
            "--skip-build" => skip_build = true,
            _ => usage(),
        }
    }
    let (Some(project), Some(frontend), Some(out)) = (project, frontend, out) else {
        usage()
    };
    let options = BatchOptions {
        frontend,
        project,
        scenario: Scenario::parse(&scenario).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        }),
        seeds: duel_batch::parse_seeds(&seeds).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        }),
        parallel,
        polls: polls.clamp(900, 12000),
        skip_build,
        out,
        label,
    };
    match duel_batch::execute(&options) {
        Ok(report) => print!("{}", duel_batch::markdown(&report)),
        Err(error) => {
            eprintln!("batch failed: {error}");
            std::process::exit(1);
        }
    }
}
