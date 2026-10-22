//! Per-frame cost of a headless run, from the emulator's own logs.
//!
//! Two sources, in order of preference:
//!
//! * the guest telemetry CSV (`--profile-log`), which exists only when the
//!   disc was built with the `emulator-telemetry` feature. It carries cycles
//!   per render and per update task, so frames over the 30 fps work budget can
//!   be counted exactly;
//! * the emulator-owned route log (`--route-log`), which works for any disc
//!   and sees only when the display flips. That gives frame cadence in
//!   vblanks, not cycles spent.
//!
//! The budget figure is two vblanks, the 30 fps work budget the rest of the
//! project measures against (`psoxide-perf cortex-30fps-report`).

use std::fmt::Write as _;

/// Cycles in two vblanks, the work budget of one 30 fps frame.
pub const TWO_VBLANK_CYCLES: i64 = 1_128_960;
/// Cycles in one vblank, the time a 60 Hz simulation tick has to finish in.
pub const ONE_VBLANK_CYCLES: i64 = TWO_VBLANK_CYCLES / 2;
const NTSC_HZ: f64 = 60.0;

/// A CSV parsed to named integer columns. Empty cells read as zero.
pub struct Table {
    header: Vec<String>,
    rows: Vec<Vec<i64>>,
}

impl Table {
    /// Parse `text`. Non-numeric cells read as zero, since both logs carry a
    /// few text columns the cost maths never touches.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header: Vec<String> = lines
            .next()
            .ok_or("the log is empty")?
            .split(',')
            .map(|c| c.trim().to_string())
            .collect();
        let rows = lines
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.split(',')
                    .map(|c| c.trim().parse::<i64>().unwrap_or(0))
                    .collect()
            })
            .collect();
        Ok(Self { header, rows })
    }

    fn col(&self, name: &str) -> Option<usize> {
        self.header.iter().position(|h| h == name)
    }

    fn get(&self, row: usize, col: usize) -> i64 {
        self.rows[row].get(col).copied().unwrap_or(0)
    }

    /// Number of data rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no data rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Linear-interpolated percentile of integer samples.
pub fn percentile(values: &[i64], fraction: f64) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut ordered = values.to_vec();
    ordered.sort_unstable();
    let position = (ordered.len() - 1) as f64 * fraction;
    let lower = position as usize;
    let upper = (lower + 1).min(ordered.len() - 1);
    let weight = position - lower as f64;
    (ordered[lower] as f64 + (ordered[upper] - ordered[lower]) as f64 * weight).round() as i64
}

fn mean(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

fn thousands(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn stats(label: &str, values: &[i64]) -> String {
    format!(
        "{label}: mean {}, p90 {}, max {}",
        thousands(mean(values).round() as i64),
        thousands(percentile(values, 0.90)),
        thousands(values.iter().copied().max().unwrap_or(0))
    )
}

/// The bus cycle at which port 1 had completed `poll` polls.
fn cycle_at_poll(route: &Table, poll: u64) -> i64 {
    let (Some(cycles), Some(polls)) = (route.col("bus_cycles"), route.col("port1_polls")) else {
        return 0;
    };
    (0..route.len())
        .find(|&r| route.get(r, polls) as u64 >= poll)
        .map_or(0, |r| route.get(r, cycles))
}

/// Cadence from the route log over the polls `from_poll..`.
pub fn route_summary(route: &Table, from_poll: u64) -> Result<String, String> {
    let (Some(polls), Some(flip), Some(delta)) = (
        route.col("port1_polls"),
        route.col("display_start_changed"),
        route.col("bus_cycle_delta"),
    ) else {
        return Err(
            "the route log has no port1_polls, display_start_changed or bus_cycle_delta column"
                .to_string(),
        );
    };
    let first = (0..route.len())
        .find(|&r| route.get(r, polls) as u64 >= from_poll)
        .ok_or("the run ended before the route started")?;
    let ticks = route.len() - first;
    if ticks < 2 {
        return Err("the route window is too short to measure".to_string());
    }
    // Cycles and vblanks between presentation flips.
    let mut periods_cycles = Vec::new();
    let mut periods_ticks = Vec::new();
    let (mut acc_cycles, mut acc_ticks) = (0i64, 0i64);
    let mut flips = 0usize;
    for r in first..route.len() {
        acc_cycles += route.get(r, delta);
        acc_ticks += 1;
        if route.get(r, flip) != 0 {
            flips += 1;
            periods_cycles.push(acc_cycles);
            periods_ticks.push(acc_ticks);
            acc_cycles = 0;
            acc_ticks = 0;
        }
    }
    let mut out = format!(
        "frame cadence from the route log (no guest telemetry in this disc): {flips} presented frames over {ticks} vblanks = {:.1} fps\n",
        NTSC_HZ * flips as f64 / ticks as f64
    );
    let count = |lo: i64, hi: i64| {
        periods_ticks
            .iter()
            .filter(|&&t| t >= lo && t <= hi)
            .count()
    };
    let _ = writeln!(
        out,
        "frames by presentation period: 1 vblank {}, 2 vblanks {}, 3 vblanks {}, 4 or more {}",
        count(1, 1),
        count(2, 2),
        count(3, 3),
        count(4, i64::MAX)
    );
    let _ = writeln!(
        out,
        "{} (a 30 fps frame is 2 vblanks = {} cycles; this log cannot see work inside a period, build with telemetry: true for render and world cycles)",
        stats("presentation period cycles", &periods_cycles),
        thousands(TWO_VBLANK_CYCLES)
    );
    let over = periods_ticks.iter().filter(|&&t| t > 2).count();
    let _ = writeln!(
        out,
        "frames slower than 30 fps (more than 2 vblanks): {over} of {}",
        periods_ticks.len()
    );
    Ok(out)
}

/// Fixed simulation ticks that took longer than one vblank, over `rows`: the
/// `fixed_update_task` column when the guest recorded the task, else the
/// `update` stage (the scene's update, which is nearly all of a tick).
///
/// The playtest scheduler has no cap on the fixed ticks it runs before a
/// visual frame (`max_fixed_ticks_before_visual: 0`): a tick that outgrows a
/// vblank leaves the simulation permanently behind real time, so it runs back
/// to back and no frame is ever rendered. That is a total stall with a picture
/// frozen on screen, and this line is what names it. `None` when the log has
/// no such column.
fn tick_overruns(profile: &Table, rows: &[usize]) -> Option<String> {
    let ticks_of = |name: &str| -> Vec<i64> {
        profile.col(name).map_or_else(Vec::new, |c| {
            rows.iter()
                .map(|&r| profile.get(r, c))
                .filter(|&v| v > 0)
                .collect()
        })
    };
    let mut ticks = ticks_of("fixed_update_task");
    if ticks.is_empty() {
        ticks = ticks_of("update");
    }
    if ticks.is_empty() {
        return None;
    }
    let over = ticks.iter().filter(|&&c| c > ONE_VBLANK_CYCLES).count();
    let mut out = format!(
        "sim ticks over one vblank ({} cycles): {over} of {}, {}\n",
        thousands(ONE_VBLANK_CYCLES),
        ticks.len(),
        stats("tick cycles", &ticks)
    );
    if over * 2 > ticks.len() {
        let _ = writeln!(
            out,
            "the simulation cannot keep up with real time: ticks run back to back and starve rendering (the scheduler has no fixed-tick cap)"
        );
    }
    Some(out)
}

/// Cost from the guest telemetry CSV over the rows after `from_cycle`.
pub fn profile_summary(profile: &Table, from_cycle: i64) -> Result<String, String> {
    let (Some(start), Some(visual), Some(render), Some(update)) = (
        profile.col("start_bus_cycles"),
        profile.col("visual_frames"),
        profile.col("render"),
        profile.col("update"),
    ) else {
        return Err(
            "the profile log lacks start_bus_cycles, visual_frames, render or update columns"
                .to_string(),
        );
    };
    let world = profile.col("room");
    let rows: Vec<usize> = (0..profile.len())
        .filter(|&r| profile.get(r, start) >= from_cycle)
        .collect();
    if rows.is_empty() {
        return Err("no telemetry rows after the route started".to_string());
    }
    let visuals: Vec<usize> = rows
        .iter()
        .copied()
        .filter(|&r| profile.get(r, visual) > 0)
        .collect();
    if visuals.is_empty() {
        // The telemetry is there and says nothing was presented: that is the
        // finding (a stalled game), not a reason to fall back to the cadence
        // log, which cannot see why.
        let mut out = format!(
            "frame cost from guest telemetry: 0 presented frames over {} guest frames = 0.0 fps\n",
            rows.len()
        );
        out.push_str(&tick_overruns(profile, &rows).unwrap_or_default());
        return Ok(out);
    }
    let renders: Vec<i64> = visuals.iter().map(|&r| profile.get(r, render)).collect();
    let worlds: Option<Vec<i64>> =
        world.map(|c| visuals.iter().map(|&r| profile.get(r, c)).collect());
    // One presented frame's work: its render plus every update since the
    // previous presented frame.
    let mut work = Vec::new();
    let mut pending = 0i64;
    for &r in &rows {
        pending += profile.get(r, update);
        if profile.get(r, visual) > 0 {
            work.push(profile.get(r, render) + pending);
            pending = 0;
        }
    }
    let presented: i64 = visuals.iter().map(|&r| profile.get(r, visual)).sum();
    let over = work.iter().filter(|&&w| w > TWO_VBLANK_CYCLES).count();
    let mut out = format!(
        "frame cost from guest telemetry: {presented} presented frames over {} guest frames = {:.1} fps\n",
        rows.len(),
        NTSC_HZ * presented as f64 / rows.len() as f64
    );
    let _ = writeln!(
        out,
        "{}",
        stats("render cycles per presented frame", &renders)
    );
    if let Some(worlds) = &worlds {
        let _ = writeln!(out, "{}", stats("world (room draw) cycles", worlds));
    }
    let _ = writeln!(
        out,
        "{}",
        stats("frame work (render + updates) cycles", &work)
    );
    let _ = writeln!(
        out,
        "frames over {} cycles (the 30 fps work budget, two vblanks): {over} of {}",
        thousands(TWO_VBLANK_CYCLES),
        work.len()
    );
    out.push_str(&tick_overruns(profile, &rows).unwrap_or_default());
    Ok(out)
}

/// The cost block for a run: telemetry when the disc had it, else cadence.
pub fn summarize(
    route_csv: &str,
    profile_csv: Option<&str>,
    from_poll: u64,
) -> Result<String, String> {
    let route = Table::parse(route_csv)?;
    if let Some(text) = profile_csv {
        if let Ok(profile) = Table::parse(text) {
            if profile.len() > 30 {
                let from = cycle_at_poll(&route, from_poll);
                if let Ok(summary) = profile_summary(&profile, from) {
                    return Ok(summary);
                }
            }
        }
    }
    route_summary(&route, from_poll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_interpolate_and_thousands_group() {
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 0.5), 30);
        assert_eq!(percentile(&[10, 20], 0.9), 19);
        assert_eq!(percentile(&[], 0.9), 0);
        assert_eq!(thousands(1_128_960), "1,128,960");
        assert_eq!(thousands(12), "12");
    }

    #[test]
    fn cadence_counts_flips_and_slow_frames() {
        // 8 ticks: flips after ticks 2, 4 and 7, so periods of 3, 2 and 3 vblanks.
        let mut csv = String::from(
            "route_tick,bus_cycles,bus_cycle_delta,display_start_changed,port1_polls\n",
        );
        let flips = [0, 0, 1, 0, 1, 0, 0, 1];
        let mut cycles = 0;
        for (tick, flip) in flips.iter().enumerate() {
            cycles += 571_000;
            csv.push_str(&format!("{tick},{cycles},571000,{flip},{}\n", tick + 10));
        }
        let text = summarize(&csv, None, 10).unwrap();
        assert!(text.contains("3 presented frames over 8 vblanks"), "{text}");
        assert!(text.contains("2 vblanks 1, 3 vblanks 2"), "{text}");
        assert!(
            text.contains("slower than 30 fps (more than 2 vblanks): 2 of 3"),
            "{text}"
        );
    }

    #[test]
    fn telemetry_counts_frames_over_the_work_budget() {
        let mut csv =
            String::from("guest_frame,start_bus_cycles,visual_frames,render,update,room\n");
        for i in 0..40 {
            let render = if i == 20 { 1_300_000 } else { 400_000 };
            csv.push_str(&format!(
                "{i},{},1,{render},50000,{}\n",
                i * 600_000,
                render / 2
            ));
        }
        let route =
            "route_tick,bus_cycles,bus_cycle_delta,display_start_changed,port1_polls\n0,0,0,0,0\n";
        let text = summarize(route, Some(&csv), 0).unwrap();
        assert!(text.contains("40 presented frames"), "{text}");
        assert!(text.contains("1 of 40"), "{text}");
        assert!(text.contains("max 1,300,000"), "{text}");
    }

    fn ticks_csv(visual: i64, task: i64) -> String {
        let mut csv = String::from(
            "guest_frame,start_bus_cycles,fixed_update_task,visual_frames,render,update,room\n",
        );
        for i in 0..40 {
            csv.push_str(&format!(
                "{i},{},{task},{visual},400000,{task},0\n",
                i * 600_000
            ));
        }
        csv
    }

    const NO_ROUTE: &str =
        "route_tick,bus_cycles,bus_cycle_delta,display_start_changed,port1_polls\n0,0,0,0,0\n";

    #[test]
    fn ticks_longer_than_a_vblank_are_counted() {
        let text = summarize(NO_ROUTE, Some(&ticks_csv(1, 700_000)), 0).unwrap();
        assert!(
            text.contains("sim ticks over one vblank (564,480 cycles): 40 of 40"),
            "{text}"
        );
        assert!(text.contains("max 700,000"), "{text}");
        assert!(text.contains("cannot keep up with real time"), "{text}");
        let text = summarize(NO_ROUTE, Some(&ticks_csv(1, 120_000)), 0).unwrap();
        assert!(
            text.contains("over one vblank (564,480 cycles): 0 of 40"),
            "{text}"
        );
        assert!(!text.contains("cannot keep up"), "{text}");
    }

    #[test]
    fn the_update_stage_stands_in_when_the_task_is_not_recorded() {
        // The playtest disc leaves `fixed_update_task` at zero.
        let mut csv = String::from(
            "guest_frame,start_bus_cycles,fixed_update_task,visual_frames,render,update,room\n",
        );
        for i in 0..40 {
            let update = if i % 10 == 0 { 900_000 } else { 100_000 };
            csv.push_str(&format!("{i},{},0,1,400000,{update},0\n", i * 600_000));
        }
        let text = summarize(NO_ROUTE, Some(&csv), 0).unwrap();
        assert!(text.contains("(564,480 cycles): 4 of 40"), "{text}");
        assert!(!text.contains("cannot keep up"), "{text}");
    }

    #[test]
    fn a_stalled_run_reports_its_ticks_instead_of_a_cadence() {
        // Telemetry is present, the game ran, nothing was presented: the old
        // summary fell back to the route log and never said why.
        let text = summarize(NO_ROUTE, Some(&ticks_csv(0, 700_000)), 0).unwrap();
        assert!(
            text.contains("0 presented frames over 40 guest frames"),
            "{text}"
        );
        assert!(text.contains("40 of 40"), "{text}");
        assert!(!text.contains("no guest telemetry"), "{text}");
    }
}
