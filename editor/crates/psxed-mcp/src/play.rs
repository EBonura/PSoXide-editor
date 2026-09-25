//! Build the project to a real PS1 disc and run it in the emulator.
//!
//! Cooking proves a level is *valid*; it says nothing about whether it runs.
//! This closes that gap, and it is the one thing the TrenchBroom MCP has
//! (`compile_map`, `launch_map`) that the audit could not cover.
//!
//! Two facts decide the shape of this module, both learned the hard way:
//!
//! - **Every project boots to a menu.** A run with no input sits on the title
//!   screen forever and looks like a hang. Worse, there is a second splash
//!   mid-load that also waits on CROSS. The default press schedule taps CROSS
//!   repeatedly so a caller who just wants to see the level does not have to
//!   know the flow.
//! - **`port1-polls` is the health signal.** A poll is one simulation tick, so
//!   a run that reports a few hundred polls never reached gameplay whatever
//!   else it printed. It is reported first for that reason.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Route ticks between scheduled CROSS taps in the default schedule.
const MENU_TAP_INTERVAL: u32 = 240;
/// How long each tap is held, in route ticks. Four is the documented floor
/// for the guest to poll the pad at least once; twelve survives a slow frame.
const MENU_TAP_HOLD: u32 = 12;

/// What a headless run reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// Completed port-1 controller polls, one per simulation tick.
    pub port1_polls: u64,
    /// Emulator route ticks.
    pub route_ticks: u64,
    /// Notable lines the guest or emulator printed.
    pub notes: Vec<String>,
}

/// A CROSS tap every `MENU_TAP_INTERVAL` ticks up to `until`.
///
/// Crude on purpose. The flow has a title menu, a multi-page world message
/// and a mid-load splash, all gated on CROSS at times that shift with load
/// speed, so tapping through beats encoding a brittle schedule.
pub fn menu_press_schedule(until: u32) -> String {
    (60..until.max(120))
        .step_by(MENU_TAP_INTERVAL as usize)
        .map(|tick| format!("{tick}:cross:{MENU_TAP_HOLD}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Cook the project and export a CUE/BIN disc. Returns the CUE path.
pub fn build_disc(frontend: &Path, project_dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new(frontend)
        .arg("build-project-disc")
        .arg("--project")
        .arg(project_dir)
        .output()
        .map_err(|error| format!("run {}: {error}", frontend.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // The guest toolchain is verbose; the last lines carry the reason.
        let tail: Vec<_> = stderr.lines().rev().take(12).collect();
        return Err(format!(
            "the disc build failed:\n{}",
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.ends_with(".cue"))
        .map(PathBuf::from)
        .ok_or_else(|| "the build printed no CUE path".to_string())
}

/// The most recently built disc for a project.
pub fn last_cue(project_dir: &Path) -> Result<PathBuf, String> {
    let baked = project_dir.join("baked");
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&baked)
        .map_err(|error| format!("read {}: {error}. Build a disc first.", baked.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "cue") {
            let stamp = entry
                .metadata()
                .and_then(|data| data.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if newest.as_ref().is_none_or(|(best, _)| stamp > *best) {
                newest = Some((stamp, path));
            }
        }
    }
    newest.map(|(_, path)| path).ok_or_else(|| {
        format!("no .cue in {}; run without skip_build first", baked.display())
    })
}

/// Boot `cue` headlessly, capturing the final frame as a PPM at `dump`.
pub fn run_disc(
    frontend: &Path,
    cue: &Path,
    dump: &Path,
    polls: u32,
    press: &str,
    hold_forward: bool,
) -> Result<RunReport, String> {
    if let Some(parent) = dump.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut command = Command::new(frontend);
    command
        .arg("launch")
        .arg("--path")
        .arg(cue)
        .arg("--embedded-playtest")
        .arg(format!("--stop-at-poll={polls}"))
        // Instructions, not ticks: a generous cap so the poll target is what
        // actually ends the run.
        .arg(format!("--steps={}", u64::from(polls) * 900_000))
        .arg("--dump-hw")
        .arg(dump);
    if !press.is_empty() {
        command.arg(format!("--press={press}"));
    }
    if hold_forward {
        command.arg("--hold-forward");
    }
    let output = command
        .output()
        .map_err(|error| format!("run {}: {error}", frontend.display()))?;
    if !output.status.success() {
        return Err(format!(
            "the emulator run failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(parse_report(&text))
}

/// Pull the run's counters and anything worth surfacing out of its output.
fn parse_report(text: &str) -> RunReport {
    let field = |name: &str| -> u64 {
        text.split_whitespace()
            .find_map(|token| token.strip_prefix(name)?.parse().ok())
            .unwrap_or(0)
    };
    let notes = text
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("route-ticks")
                && !line.starts_with("tick=")
                && !line.starts_with("[cli]")
        })
        .map(str::to_string)
        .collect();
    RunReport {
        port1_polls: field("port1-polls="),
        route_ticks: field("route-ticks="),
        notes,
    }
}

impl RunReport {
    /// Human summary, leading with the figure that says whether the run got
    /// anywhere at all.
    pub fn summary(&self, polls_requested: u32) -> String {
        let mut out = format!(
            "port1-polls {} of {polls_requested} requested, {} route ticks.\n",
            self.port1_polls, self.route_ticks
        );
        if self.port1_polls < u64::from(polls_requested) / 2 {
            out.push_str(
                "That is far short of the request, which normally means the run stalled on a \
                 screen waiting for input rather than crashing.\n",
            );
        } else if self.port1_polls < 3000 {
            out.push_str(
                "Under about 3000 polls the run is usually still in the menu and loading flow; \
                 ask for more to reach gameplay.\n",
            );
        }
        for note in self.notes.iter().take(8) {
            out.push_str(note);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parsing the run's counters is the whole contract with the emulator
    /// CLI, and `port1-polls` is the number a caller acts on.
    #[test]
    fn report_parses_counters_and_flags_a_short_run() {
        let text = "[cli] mounted cue-backed disc foo.cue\n\
                    combat music:on\n\
                    tick=1821011893  cycles=4181404298  pc=0x8001d024\n\
                    route-ticks=7316  port1-polls=7000\n";
        let report = parse_report(text);
        assert_eq!(report.port1_polls, 7000);
        assert_eq!(report.route_ticks, 7316);
        // Guest chatter is kept, emulator bookkeeping is not.
        assert!(report.notes.iter().any(|note| note == "combat music:on"));
        assert!(!report.notes.iter().any(|note| note.starts_with("[cli]")));

        // A run that never left the menu says so rather than looking healthy.
        let stalled = parse_report("route-ticks=903  port1-polls=900\n");
        assert!(stalled.summary(7000).contains("far short"), "{}", stalled.summary(7000));
        assert!(!report.summary(7000).contains("far short"));

        // Missing counters must not panic or invent numbers.
        assert_eq!(parse_report("nothing useful here").port1_polls, 0);

        // The default schedule taps through the menus rather than pressing once.
        let schedule = menu_press_schedule(1000);
        assert!(schedule.starts_with("60:cross:"), "{schedule}");
        assert!(schedule.matches("cross").count() > 3, "{schedule}");
    }
}
