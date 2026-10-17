//! The Break verb in the editor: pick a region, press X, then press on it
//! and drag up for the intensity (0 to 100). The faces re-derive live under
//! the pointer; Tab cycles what is broken (walls and floor, walls, floor,
//! ceiling, all), Shift+X draws the next seed, Enter or the release commits
//! as one undo step, Esc puts the document back exactly.
//!
//! The break itself is `BreakDrag`: every apply restores its snapshot and
//! plans from absolutes, so the drag can go back and forth and the same seed
//! always gives the same result. Nothing but ordinary grid data is written.

use super::verb_state::{VerbDrag, VerbGesture};
use super::*;

use psxed_project::grid::edit::verbs::break_verb::{
    BreakDrag, BreakParams, BreakReport, BreakSnap, BreakStyle, BreakTargets, DEFAULT_INTENSITY,
};
use psxed_project::grid::faces::face_count;
use psxed_project::grid::walk::{self, WalkLimits};
use psxed_project::grid::GridWorld;

/// Pixels of upward travel for one percent of intensity.
const PIXELS_PER_PERCENT: f32 = 2.0;

/// The target sets Tab cycles through, with their chip labels.
pub(crate) const TARGET_SETS: [(&str, BreakTargets); 5] = [
    (
        "Walls+floor",
        BreakTargets {
            walls: true,
            floor: true,
            ceiling: false,
        },
    ),
    (
        "Walls",
        BreakTargets {
            walls: true,
            floor: false,
            ceiling: false,
        },
    ),
    (
        "Floor",
        BreakTargets {
            walls: false,
            floor: true,
            ceiling: false,
        },
    ),
    (
        "Ceiling",
        BreakTargets {
            walls: false,
            floor: false,
            ceiling: true,
        },
    ),
    (
        "All",
        BreakTargets {
            walls: true,
            floor: true,
            ceiling: true,
        },
    ),
];

/// What the option row keeps between breaks.
#[derive(Clone, Debug)]
pub(crate) struct BreakOptions {
    /// Index into [`TARGET_SETS`].
    pub targets: usize,
    /// Hold every change to the walkability mask.
    pub keep_walkable: bool,
    /// The step changes are quantised to.
    pub snap: BreakSnap,
    /// The largest vertical change at intensity 100, fine steps.
    pub amplitude: i32,
    /// A cracked triangle takes the material of the floor it borders.
    pub mix_materials: bool,
    /// The rough level (0 to 3) given to broken sectors.
    pub jitter: u8,
    /// The seed of the next break; Shift+X draws the next one.
    pub seed: u32,
    /// The typed intensity of the option row's Apply.
    pub typed_intensity: u32,
}

impl Default for BreakOptions {
    fn default() -> Self {
        Self {
            targets: 0,
            keep_walkable: true,
            snap: BreakSnap::Fine,
            amplitude: 4,
            mix_materials: false,
            jitter: 0,
            seed: 1,
            typed_intensity: DEFAULT_INTENSITY,
        }
    }
}

impl BreakOptions {
    /// Set every option to a named look and its intensity.
    pub(crate) fn apply_style(&mut self, style: BreakStyle) {
        let look = style.params();
        self.targets = TARGET_SETS
            .iter()
            .position(|(_, t)| *t == look.targets)
            .unwrap_or(0);
        self.keep_walkable = look.keep_walkable;
        self.snap = look.snap;
        self.amplitude = look.amplitude;
        self.mix_materials = look.mix_materials;
        self.jitter = look.jitter;
        self.typed_intensity = style.intensity();
    }

    /// The label of the current target set.
    pub(crate) fn targets_label(&self) -> &'static str {
        TARGET_SETS[self.targets % TARGET_SETS.len()].0
    }
}

/// A break in flight.
#[derive(Clone, Debug)]
pub(crate) struct BreakLive {
    drag: BreakDrag,
    /// The document before the gesture, for the exact cancel.
    pub(super) start: Box<ProjectDocument>,
    /// Whether the undo step has been recorded (at the first change).
    recorded: bool,
    faces_before: usize,
    /// The seed in use (Shift+X changes it mid-drag).
    pub seed: u32,
    /// What the last apply reported.
    pub report: Option<BreakReport>,
}

impl BreakLive {
    /// The document to record for undo, the first time this is asked.
    pub(super) fn take_record(&mut self) -> Option<ProjectDocument> {
        (!std::mem::replace(&mut self.recorded, true)).then(|| (*self.start).clone())
    }

    /// Whether the gesture has changed the document.
    pub(super) fn recorded(&self) -> bool {
        self.recorded
    }
}

/// The verb's parameters from the options and the world.
fn params(ws: &EditorWorkspace, world: &GridWorld) -> BreakParams {
    let o = &ws.grid.verb.brk;
    BreakParams {
        targets: TARGET_SETS[o.targets % TARGET_SETS.len()].1,
        amplitude: o.amplitude.clamp(1, 32),
        snap: o.snap,
        keep_walkable: o.keep_walkable,
        spawns: walk::player_spawns(&ws.project, world),
        mix_materials: o.mix_materials,
        jitter: o.jitter.min(3),
        limits: WalkLimits::DEFAULT,
    }
}

/// A break on the region with the current options, not yet applied.
fn start(ws: &EditorWorkspace) -> Option<BreakLive> {
    let region = ws.grid.verb.region.clone()?;
    let world = ws.project.active_scene().world.grid()?;
    let drag = BreakDrag::begin(world, &region, params(ws, world))?;
    Some(BreakLive {
        drag,
        start: Box::new(ws.project.clone()),
        recorded: false,
        faces_before: face_count(world),
        seed: ws.grid.verb.brk.seed,
        report: None,
    })
}

/// Press on the region: start the intensity drag.
pub(super) fn begin(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    _room: usize,
    _cell: [i32; 2],
) -> bool {
    let Some(live) = start(ws) else {
        ws.status = "Nothing to break: no sector of the region has a floor or ceiling".to_string();
        return false;
    };
    ws.status = format!(
        "Break seed {}: drag up for the intensity, Tab changes the target, Shift+X re-rolls",
        live.seed
    );
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag: VerbDrag::Break(Box::new(live)),
        press_y: ptr.at_press().pos.y,
        pixels_per_unit: PIXELS_PER_PERCENT,
        amount: 0,
        before: None,
        max_rise: None,
    }));
    true
}

/// The intensity the pointer is at.
pub(super) fn intensity_at(g: &VerbGesture, ptr: &GridPointer) -> i32 {
    (((g.press_y - ptr.pos.y) / g.pixels_per_unit.max(1e-4)).round() as i32).clamp(0, 100)
}

/// A break in flight at `intensity` (poses, tests).
pub(crate) fn pose_drag(
    ws: &mut EditorWorkspace,
    intensity: i32,
    seed: Option<u32>,
) -> Result<(), String> {
    if let Some(seed) = seed {
        ws.grid.verb.brk.seed = seed;
    }
    let mut live = start(ws).ok_or("no region with a floor: use region=x0,z0:x1,z1 first")?;
    live.seed = ws.grid.verb.brk.seed;
    let mut gesture = VerbGesture {
        drag: VerbDrag::Break(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: PIXELS_PER_PERCENT,
        amount: intensity.clamp(0, 100),
        before: None,
        max_rise: None,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
    ws.grid.gesture = Gesture::Verb(Box::new(gesture));
    Ok(())
}

/// Apply the gesture's intensity to the live document. Returns the status
/// line and the Inspector's longer readout.
pub(super) fn apply(
    world: &mut GridWorld,
    live: &mut BreakLive,
    intensity: i32,
) -> (String, String) {
    let report = live
        .drag
        .apply(world, intensity.clamp(0, 100) as u32, live.seed);
    let faces_after = face_count(world);
    let line = status_line(&report, live.faces_before, faces_after);
    let detail = detail(&report, &line);
    live.report = Some(report);
    (line, detail)
}

/// "Break seed 7, 60 percent: +37 faces, 12 cracks, 4 bites, 4 refused
/// (would break the route)".
pub(crate) fn status_line(report: &BreakReport, faces_before: usize, faces_after: usize) -> String {
    let delta = faces_after as i64 - faces_before as i64;
    let mut parts = vec![format!("{delta:+} faces")];
    for (count, what) in [
        (report.crumbled, "crumbled lips"),
        (report.tilted, "tilted corners"),
        (report.cracked, "cracks"),
        (report.bites, "bites"),
        (report.tops, "ragged tops"),
        (report.stubs, "stubs"),
        (report.ceiling, "ceiling changes"),
        (report.gaps, "gaps"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {what}"));
        }
    }
    if report.rejected > 0 {
        parts.push(format!(
            "{} refused (would break the route)",
            report.rejected
        ));
    }
    format!(
        "Break seed {}, {} percent: {}",
        report.seed,
        report.intensity,
        parts.join(", ")
    )
}

/// The status line plus the walkability delta and the first reasons things
/// were refused, for the Inspector.
fn detail(report: &BreakReport, line: &str) -> String {
    let mut text = line.to_string();
    if let Some([before, after]) = report.walk {
        text += &format!(
            "\nunreachable {} to {}, steps over the limit {} to {}, steep {} to {}",
            before.unreachable,
            after.unreachable,
            before.steps,
            after.steps,
            before.steep,
            after.steep
        );
    }
    if report.shrunk > 0 {
        text += &format!("\n{} changes went through at a smaller size", report.shrunk);
    }
    for reason in report.rejects.iter().take(3) {
        text += &format!("\nrefused: {reason}");
    }
    text
}

/// The text over a break being dragged.
pub(super) fn chip(live: &BreakLive, intensity: i32, targets: &str) -> String {
    format!("Break {intensity}%  seed {}  {targets}", live.seed)
}

/// Tab: the next target set, live when a drag is in flight.
pub(super) fn cycle_targets(ws: &mut EditorWorkspace) {
    let next = (ws.grid.verb.brk.targets + 1) % TARGET_SETS.len();
    ws.grid.verb.brk.targets = next;
    let label = TARGET_SETS[next].0;
    refresh_live(ws);
    if !matches!(ws.grid.gesture, Gesture::Verb(_)) {
        ws.status = format!("Break targets: {label}");
    }
}

/// Shift+X: the next seed, live when a drag is in flight.
pub(super) fn reroll(ws: &mut EditorWorkspace) {
    ws.grid.verb.brk.seed = ws.grid.verb.brk.seed.wrapping_add(1);
    let seed = ws.grid.verb.brk.seed;
    if !refresh_live(ws) {
        ws.status = format!("Break seed {seed}");
    }
}

/// Re-apply the break in flight with the current options. Returns whether
/// there was one.
fn refresh_live(ws: &mut EditorWorkspace) -> bool {
    let Gesture::Verb(mut g) = std::mem::take(&mut ws.grid.gesture) else {
        return false;
    };
    let seed = ws.grid.verb.brk.seed;
    let mut live = false;
    if let Some(world) = ws.project.active_scene().world.grid() {
        let fresh = params(ws, world);
        if let VerbDrag::Break(b) = &mut g.drag {
            b.drag.set_params(fresh);
            b.seed = seed;
            live = true;
        }
    }
    if live {
        super::verb_tool::apply_amount(ws, &mut g);
    }
    ws.grid.gesture = Gesture::Verb(g);
    live
}

/// The option row's typed intensity, applied as one undo step.
pub(super) fn apply_typed(ws: &mut EditorWorkspace) {
    if ws.grid.verb.region.is_none() {
        ws.status = "Pick a region first".to_string();
        return;
    }
    let intensity = ws.grid.verb.brk.typed_intensity.min(100) as i32;
    let Some(live) = start(ws) else {
        ws.status = "Nothing to break: no sector of the region has a floor or ceiling".to_string();
        return;
    };
    let mut gesture = VerbGesture {
        drag: VerbDrag::Break(Box::new(live)),
        press_y: 0.0,
        pixels_per_unit: PIXELS_PER_PERCENT,
        amount: intensity,
        before: None,
        max_rise: None,
    };
    super::verb_tool::apply_amount(ws, &mut gesture);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_line_counts_what_the_break_did() {
        let report = BreakReport {
            seed: 7,
            intensity: 60,
            cracked: 12,
            bites: 4,
            rejected: 4,
            ..BreakReport::default()
        };
        assert_eq!(
            status_line(&report, 100, 137),
            "Break seed 7, 60 percent: +37 faces, 12 cracks, 4 bites, 4 refused (would break the route)"
        );
        let nothing = BreakReport {
            seed: 1,
            intensity: 0,
            ..BreakReport::default()
        };
        assert_eq!(
            status_line(&nothing, 5, 5),
            "Break seed 1, 0 percent: +0 faces"
        );
    }

    #[test]
    fn tab_walks_the_five_target_sets_and_wraps() {
        assert_eq!(TARGET_SETS.len(), 5);
        let all = TARGET_SETS[4].1;
        assert!(all.walls && all.floor && all.ceiling);
        assert_eq!(BreakOptions::default().targets_label(), "Walls+floor");
    }
}
