//! CLI: how full RAM, VRAM and SPU RAM are for a cooked project's guest.
//!
//! Usage:
//!   occupancy-report --map <link.map> [--exe <editor-playtest.exe>]
//!       [--manifest <generated/level_manifest.rs>] [--project <project.ron>]
//!       [--ram <ram.bin>] [--vram <vram.ppm>] [--spu <spu.bin>]
//!       [--label <text>] [--json <out.json>] [--text <out.txt>]
//!
//! The link map is what `PSOXIDE_GUEST_LINK_MAP=<path> make build-editor-playtest`
//! writes. `--project` cooks the project in memory (the same cook as
//! `cook-playtest`, nothing is written) for the texture and SFX tables. The
//! three dumps come from `frontend launch --dump-ram/--dump-vram/--dump-spu-ram`
//! at the point of interest; without them the report is link-map and cook
//! numbers only, and says so on every row.
//!
//! Exit codes: 0 success, 2 on bad arguments or an unreadable input.

use std::path::PathBuf;
use std::process::ExitCode;

use psxed_project::occupancy::{build_report, render_json, render_text, Inputs};

fn parse_args() -> Result<(Inputs, Option<PathBuf>, Option<PathBuf>), String> {
    let mut inputs = Inputs::default();
    let (mut json, mut text) = (None, None);
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = |what: &str| it.next().ok_or_else(|| format!("{flag} takes {what}"));
        match flag.as_str() {
            "--map" => inputs.map = Some(PathBuf::from(value("a path")?)),
            "--exe" => inputs.exe = Some(PathBuf::from(value("a path")?)),
            "--manifest" => inputs.manifest = Some(PathBuf::from(value("a path")?)),
            "--project" => inputs.project = Some(PathBuf::from(value("a path")?)),
            "--ram" => inputs.ram = Some(PathBuf::from(value("a path")?)),
            "--vram" => inputs.vram = Some(PathBuf::from(value("a path")?)),
            "--spu" => inputs.spu = Some(PathBuf::from(value("a path")?)),
            "--label" => inputs.label = Some(value("text")?),
            "--json" => json = Some(PathBuf::from(value("a path")?)),
            "--text" => text = Some(PathBuf::from(value("a path")?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok((inputs, json, text))
}

fn run() -> Result<(), String> {
    let (inputs, json, text_path) = parse_args()?;
    let report = build_report(&inputs).map_err(|e| e.to_string())?;
    let text = render_text(&report);
    print!("{text}");
    if let Some(path) = text_path {
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(path) = json {
        std::fs::write(&path, render_json(&report))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("[occupancy-report] {message}");
            ExitCode::from(2)
        }
    }
}
