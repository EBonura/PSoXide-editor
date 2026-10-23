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

/// Instruction cap per requested poll. A generous bound, so the poll target is
/// what ends a run; a level that loads a wide resident set at start spins for
/// a long while before its first poll and needs more than the 900 000 a
/// small level gets by on.
const STEPS_PER_POLL: u64 = 2_000_000;

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
    build_disc_with(frontend, project_dir, GuestFeatures::default())
}

/// Optional measurement features for the guest build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestFeatures {
    /// `emulator-telemetry`: cycles per task per frame.
    pub telemetry: bool,
    /// `camera-pose-log`: the player's position every update.
    pub track: bool,
}

/// [`build_disc`], optionally with guest measurement features so the run can
/// report render and world cycles per frame and the player's path. They cost
/// guest RAM, so a level already near the cap may refuse to link; the error
/// says so.
pub fn build_disc_with(
    frontend: &Path,
    project_dir: &Path,
    features: GuestFeatures,
) -> Result<PathBuf, String> {
    let mut command = Command::new(frontend);
    command
        .arg("build-project-disc")
        .arg("--project")
        .arg(project_dir);
    if features.telemetry || features.track {
        let mut list = String::from("cd-stream-bench");
        if features.telemetry {
            list.push_str(" emulator-telemetry");
        }
        if features.track {
            list.push_str(" camera-pose-log");
        }
        command.env("EDITOR_PLAYTEST_FEATURES", list);
    }
    let output = command
        .output()
        .map_err(|error| format!("run {}: {error}", frontend.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the disc build failed:\n{}",
            failure_reason(&stdout, &stderr)
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

/// What went wrong in a failed build, from its combined output: the cook's and
/// the compiler's own `error` lines when there are any (they carry the reason),
/// else the tail of the log (the guest toolchain is verbose).
pub fn failure_reason(stdout: &str, stderr: &str) -> String {
    let all: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
    let errors: Vec<&str> = all
        .iter()
        .copied()
        .filter(|l| {
            let lower = l.to_ascii_lowercase();
            (lower.contains("error:") || lower.contains("error["))
                && !lower.contains("make: ***")
                && !lower.starts_with("error: make ")
        })
        .take(12)
        .collect();
    if !errors.is_empty() {
        return errors.join("\n");
    }
    all.iter()
        .rev()
        .take(12)
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
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
        format!(
            "no .cue in {}; run without skip_build first",
            baked.display()
        )
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
        .arg(format!("--steps={}", u64::from(polls) * STEPS_PER_POLL))
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

/// What a scripted route run produced.
pub struct RouteRun {
    /// Counters and notable lines.
    pub report: RunReport,
    /// The emulator's per-tick log, CSV.
    pub route_csv: String,
    /// Guest telemetry per frame, CSV, when the disc had telemetry.
    pub profile_csv: Option<String>,
    /// Captured frames: route tick and PPM path, ascending.
    pub frames: Vec<(u64, PathBuf)>,
    /// The final frame.
    pub final_ppm: PathBuf,
    /// Everything the emulator printed, for the guest's debug lines.
    pub log: String,
}

/// Replay a poll-bound tape on `cue`, capturing a frame every
/// `frame_interval` route ticks plus the final one, and the per-tick logs.
pub fn run_route(
    frontend: &Path,
    cue: &Path,
    work: &Path,
    tape: &Path,
    polls: u32,
    frame_interval: u32,
    guest_log: bool,
) -> Result<RouteRun, String> {
    let shots = work.join("frames");
    let _ = std::fs::remove_dir_all(&shots);
    std::fs::create_dir_all(&shots)
        .map_err(|error| format!("create {}: {error}", shots.display()))?;
    let final_ppm = work.join("final.ppm");
    let route_log = work.join("route.csv");
    let profile_log = work.join("profile.csv");
    let mut command = Command::new(frontend);
    command.arg("launch");
    if guest_log {
        command.arg("--guest-debug-log");
    }
    let output = command
        .arg("--path")
        .arg(cue)
        .arg("--embedded-playtest")
        .arg("--input-tape")
        .arg(tape)
        .arg(format!("--stop-at-poll={polls}"))
        .arg(format!("--steps={}", u64::from(polls) * STEPS_PER_POLL))
        .arg("--dump-hw")
        .arg(&final_ppm)
        .arg("--route-log")
        .arg(&route_log)
        .arg("--profile-log")
        .arg(&profile_log)
        .arg("--route-screenshot-dir")
        .arg(&shots)
        .arg("--route-screenshot-interval")
        .arg(frame_interval.max(1).to_string())
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
    let mut frames: Vec<(u64, PathBuf)> = std::fs::read_dir(&shots)
        .map_err(|error| format!("read {}: {error}", shots.display()))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let tick = path
                .file_stem()?
                .to_str()?
                .strip_prefix("tick-")?
                .parse::<u64>()
                .ok()?;
            Some((tick, path))
        })
        .collect();
    frames.sort();
    Ok(RouteRun {
        report: parse_report(&text),
        log: text.clone(),
        route_csv: std::fs::read_to_string(&route_log)
            .map_err(|error| format!("read {}: {error}", route_log.display()))?,
        profile_csv: std::fs::read_to_string(&profile_log).ok(),
        frames,
        final_ppm,
    })
}

/// The captured frame nearest the route tick at which port 1 completed
/// `poll` polls, with that tick.
pub fn frame_near_poll(
    route_csv: &str,
    frames: &[(u64, PathBuf)],
    poll: u64,
) -> Option<(u64, PathBuf)> {
    let mut lines = route_csv.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let tick_col = header.iter().position(|h| *h == "route_tick")?;
    let poll_col = header.iter().position(|h| *h == "port1_polls")?;
    let mut last = None;
    let mut found = None;
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        let (Some(Ok(polls)), Some(Ok(tick))) = (
            cells.get(poll_col).map(|c| c.parse::<u64>()),
            cells.get(tick_col).map(|c| c.parse::<u64>()),
        ) else {
            continue;
        };
        last = Some(tick);
        if polls >= poll {
            found = Some(tick);
            break;
        }
    }
    // A poll past the last logged row (the run stops at the first flip after
    // its target) means the end of the log.
    let tick = found.or(last)?;
    frames
        .iter()
        .min_by_key(|(t, _)| t.abs_diff(tick))
        .map(|(t, path)| (*t, path.clone()))
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
                && !line.starts_with("[guest ")
                && !line.starts_with("player fall:sample")
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
    /// Summary of a scripted route run: the counters, and a plain warning
    /// when the run ended well short of its polls.
    pub fn route_summary(&self, polls_requested: u32) -> String {
        let mut out = format!(
            "port1-polls {} of {polls_requested} requested, {} route ticks.\n",
            self.port1_polls, self.route_ticks
        );
        if self.port1_polls * 10 < u64::from(polls_requested) * 9 {
            out.push_str("The run ended well short of its route, which usually means it stalled on a screen waiting for input.\n");
        }
        for note in self.notes.iter().take(8) {
            out.push_str(note);
            out.push('\n');
        }
        out
    }

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
        assert!(
            stalled.summary(7000).contains("far short"),
            "{}",
            stalled.summary(7000)
        );
        assert!(!report.summary(7000).contains("far short"));

        // A failed build is explained by its error lines, not its log tail.
        let reason = failure_reason(
            "[cook-playtest] warning: slow\n[cook-playtest] error: Model 'X' mesh a/b.psxmdl: No such file",
            "make: *** [cook-playtest] Error 1\nerror: make cook-playtest failed: exit status: 2",
        );
        assert_eq!(
            reason,
            "[cook-playtest] error: Model 'X' mesh a/b.psxmdl: No such file"
        );
        assert!(failure_reason("a\nb", "c").ends_with("c"));

        // Missing counters must not panic or invent numbers.
        assert_eq!(parse_report("nothing useful here").port1_polls, 0);

        // A poll maps to the captured frame nearest its route tick.
        let route = "route_tick,port1_polls\n0,0\n100,40\n200,150\n300,260\n";
        let frames = vec![
            (60, PathBuf::from("a")),
            (180, PathBuf::from("b")),
            (240, PathBuf::from("c")),
        ];
        assert_eq!(frame_near_poll(route, &frames, 140).unwrap().0, 180);
        assert_eq!(frame_near_poll(route, &frames, 250).unwrap().0, 240);
        // Past the end of the log: the last frame there is.
        assert_eq!(frame_near_poll(route, &frames, 9999).unwrap().0, 240);

        // The default schedule taps through the menus rather than pressing once.
        let schedule = menu_press_schedule(1000);
        assert!(schedule.starts_with("60:cross:"), "{schedule}");
        assert!(schedule.matches("cross").count() > 3, "{schedule}");
    }
}
