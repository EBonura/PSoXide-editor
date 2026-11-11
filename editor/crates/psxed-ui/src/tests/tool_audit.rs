//! The tool audit: every tool reachable from the strip, replayed the way a
//! person uses it, through the whole editor window and real pointer and key
//! events, starting from the strip button (or a key, or the tool search).
//!
//! `PSXED_TOOL_AUDIT=<file>` makes the matrix test write one line per
//! (project, view, tool, start, gesture) cell to that file; without the
//! variable it does nothing. The tests that stay on (see the bottom) hold the
//! rules the matrix showed the editor must keep.

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, press_key, text_in, texts};
use super::*;
use crate::grid_tools::{GridTool, CLASSIC, MORE_BUILD};
use std::fmt::Write as _;

pub(super) const AUDIT_WINDOW: Vec2 = Vec2::new(1456.0, 819.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fixture {
    /// The tracked starter project: what File > New makes.
    Starter,
    /// A copy of projects/styles-demo.
    StylesDemo,
}

impl Fixture {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Starter => "starter",
            Self::StylesDemo => "styles-demo",
        }
    }

    pub(super) fn rig(self) -> WindowRig {
        match self {
            Self::Starter => WindowRig::starter(AUDIT_WINDOW),
            Self::StylesDemo => WindowRig::from_dir(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../projects/styles-demo"),
                AUDIT_WINDOW,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Start {
    Strip,
    Key,
    Search,
}

fn band(rig: &WindowRig) -> Rect {
    Rect::from_min_max(
        Pos2::new(0.0, ACTION_BAR_COMPACT_HEIGHT),
        Pos2::new(
            rig.size.x,
            ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx),
        ),
    )
}

/// Put the editor on a neutral tool so a start has to switch.
pub(super) fn park(rig: &mut WindowRig) {
    rig.ws
        .set_grid_tool(if rig.ws.grid.tool == GridTool::Select {
            GridTool::Split
        } else {
            GridTool::Select
        });
    frames(rig, 2);
}

fn strip_click(rig: &mut WindowRig, label: &str) -> bool {
    let band = band(rig);
    let Some(r) = text_in(rig, band, label) else {
        return false;
    };
    rig.click(r.center());
    frames(rig, 2);
    true
}

/// Activate `tool` with a start a person has; false when that start does not
/// reach the tool (no key, no button).
pub(super) fn start(rig: &mut WindowRig, tool: GridTool, how: Start) -> bool {
    park(rig);
    match how {
        Start::Strip => {
            if CLASSIC.contains(&tool) || MORE_BUILD.contains(&tool) {
                let menu = if CLASSIC.contains(&tool) {
                    "Classic"
                } else {
                    "More"
                };
                let anchor = text_in(rig, band(rig), menu).map(|r| r.center());
                if !strip_click(rig, menu) {
                    return false;
                }
                let anchor = anchor.unwrap_or(Pos2::ZERO);
                // The menu opens under its button: only texts near it.
                let entry = texts(rig).into_iter().find(|(t, r)| {
                    t.contains(tool.label())
                        && t.contains(tool.key_label())
                        && t.len() > 3
                        && (r.center().x - anchor.x).abs() < 160.0
                        && r.center().y > anchor.y
                        && r.center().y < anchor.y + 260.0
                });
                match entry {
                    Some((_, r)) => {
                        rig.click(r.center());
                        frames(rig, 2);
                    }
                    None => return false,
                }
            } else if !strip_click(rig, tool.label()) {
                return false;
            }
        }
        Start::Key => {
            if !tool.has_key() {
                return false;
            }
            press_key(rig, tool.key(), tool.shift());
        }
        Start::Search => {
            press_key(rig, egui::Key::Slash, false);
            rig.frame(vec![egui::Event::Text(tool.label().to_string())]);
            frames(rig, 1);
            press_key(rig, egui::Key::Enter, false);
        }
    }
    rig.ws.grid.tool == tool
}

pub(super) fn plan(rig: &mut WindowRig) {
    rig.ws.apply_grid_pose("view=plan").unwrap();
    frames(rig, 4);
}

/// The 3D view, with the camera standing a few sectors back from the middle
/// of the floor, the way a person frames the work before using a tool.
pub(super) fn persp(rig: &mut WindowRig) {
    rig.ws.apply_grid_pose("view=3d").unwrap();
    frames(rig, 4);
    let (room, c) = floor_block(rig);
    let world = rig.world();
    let r = &world.rooms[room];
    let sector = world.scale.to_editor(1024) as f32;
    let [lx, lz] = r.local(c).unwrap();
    let h = r.sector(lx, lz).and_then(|s| s.ground_floor()).map_or(0, |f| f.heights[0]);
    let y = crate::grid_tools::view::editor_y(world, r.y_offset, h);
    let t = [(c[0] as f32 + 1.5) * sector, y, (c[1] as f32 + 1.5) * sector];
    let cam = [t[0] as i32, (t[1] + sector * 3.0) as i32, (t[2] + sector * 4.4) as i32];
    rig.ws.camera_rig.mode = ViewportCameraMode::Free;
    rig.ws.camera_rig.free_initialized = true;
    rig.ws.camera_rig.free_position = cam;
    let (yaw, pitch) =
        camera_angles_to_look_at(cam, [t[0] as i32, t[1] as i32, t[2] as i32]).unwrap();
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    frames(rig, 3);
}

/// World cells (room, cell) that have a floor, in the room with the most.
fn floor_cells(rig: &WindowRig) -> Vec<(usize, [i32; 2])> {
    let world = rig.world();
    let mut best: Vec<(usize, [i32; 2])> = Vec::new();
    for (ri, room) in world.rooms.iter().enumerate() {
        let mut cells = Vec::new();
        for z in 0..room.depth {
            for x in 0..room.width {
                if room.sector(x, z).is_some_and(|s| s.ground_floor().is_some()) {
                    cells.push((ri, [room.origin[0] + x as i32, room.origin[1] + z as i32]));
                }
            }
        }
        if cells.len() > best.len() {
            best = cells;
        }
    }
    best
}

/// A 3x2 block of floor cells near the middle of the biggest room: (room,
/// top-left cell).
pub(super) fn floor_block(rig: &WindowRig) -> (usize, [i32; 2]) {
    block(rig, 4, 3).or_else(|| block(rig, 3, 2)).expect("a 3x2 block of floor")
}

fn block(rig: &WindowRig, w: i32, h: i32) -> Option<(usize, [i32; 2])> {
    let cells = floor_cells(rig);
    let has = |room: usize, c: [i32; 2]| cells.contains(&(room, c));
    let (min_x, max_x, min_z, max_z) = cells.iter().fold(
        (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
        |a, (_, c)| (a.0.min(c[0]), a.1.max(c[0]), a.2.min(c[1]), a.3.max(c[1])),
    );
    let centre = [(min_x + max_x) / 2, (min_z + max_z) / 2];
    let mut best = None;
    let mut best_d = i32::MAX;
    for &(room, c) in &cells {
        let ok = (0..w).all(|dx| (0..h).all(|dz| has(room, [c[0] + dx, c[1] + dz])));
        let d = (c[0] - centre[0]).abs() + (c[1] - centre[1]).abs();
        if ok && d < best_d {
            best = Some((room, c));
            best_d = d;
        }
    }
    best
}

/// Plan-view pixel of the centre of cell `c`.
pub(super) fn plan_px(rig: &WindowRig, c: [i32; 2]) -> Pos2 {
    rig.cell(c[0], c[1])
}

/// A floor pixel and a second one a drag away, in the 3D view, found with
/// the editor's own picking.
pub(super) fn floor_px(rig: &WindowRig) -> Option<(Pos2, Pos2)> {
    face_px(rig, true)
}

/// A wall pixel and a second one along it.
pub(super) fn wall_px(rig: &WindowRig) -> Option<(Pos2, Pos2)> {
    face_px(rig, false)
}

fn face_px(rig: &WindowRig, floor: bool) -> Option<(Pos2, Pos2)> {
    let rect = rig.ws.last_persp_viewport_rect;
    let is = |p: Pos2| {
        rig.ws.pick_face_3d(rect, p).is_some_and(|h| {
            if floor {
                h.normal[1] > 0.8
            } else {
                h.normal[1].abs() < 0.3
            }
        })
    };
    let mut found: Vec<Pos2> = Vec::new();
    let mut y = rect.min.y + 20.0;
    while y < rect.max.y - 10.0 {
        let mut x = rect.min.x + 20.0;
        while x < rect.max.x - 10.0 {
            let p = Pos2::new(x, y);
            if is(p) {
                found.push(p);
            }
            x += 14.0;
        }
        y += 14.0;
    }
    let centre = rect.center();
    found.sort_by(|a, b| {
        a.distance(centre)
            .partial_cmp(&b.distance(centre))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for &a in &found {
        for d in [
            Vec2::new(240.0, 0.0),
            Vec2::new(-240.0, 0.0),
            Vec2::new(170.0, 0.0),
            Vec2::new(-170.0, 0.0),
            Vec2::new(110.0, 0.0),
            Vec2::new(-110.0, 0.0),
            Vec2::new(0.0, 80.0),
            Vec2::new(0.0, -80.0),
            Vec2::new(0.0, 40.0),
            Vec2::new(60.0, 30.0),
        ] {
            if is(a + d) && rect.contains(a + d) {
                return Some((a, a + d));
            }
        }
    }
    None
}

/// A drag whose end keeps the button down.
pub(super) fn drag_hold(rig: &mut WindowRig, from: Pos2, to: Pos2) {
    rig.move_to(from);
    let e = rig.press(from, true);
    rig.frame(vec![e]);
    for step in 1..=6 {
        rig.move_to(from + (to - from) * (step as f32 / 6.0));
    }
}

pub(super) fn release(rig: &mut WindowRig, at: Pos2) {
    let e = rig.press(at, false);
    rig.frame(vec![e]);
    rig.frame(vec![]);
}

/// Press at `at` and drag straight up by `px`.
pub(super) fn drag_up(rig: &mut WindowRig, at: Pos2, px: f32) {
    rig.drag(at, at + Vec2::new(0.0, -px));
}

pub(super) fn key(rig: &mut WindowRig, k: egui::Key) {
    press_key(rig, k, false);
}

pub(super) fn type_text(rig: &mut WindowRig, text: &str) {
    for c in text.chars() {
        let k = match c {
            '0' => egui::Key::Num0,
            '1' => egui::Key::Num1,
            '2' => egui::Key::Num2,
            '3' => egui::Key::Num3,
            '4' => egui::Key::Num4,
            '5' => egui::Key::Num5,
            '6' => egui::Key::Num6,
            '7' => egui::Key::Num7,
            '8' => egui::Key::Num8,
            _ => egui::Key::Num9,
        };
        press_key(rig, k, false);
    }
}

/// What the user can read after a step: the hint bar and the message.
pub(super) fn said(rig: &WindowRig) -> String {
    let hint = rig.ws.status_hint();
    let status = rig.ws.status.clone();
    let tool_hint = rig.ws.grid.tool.hint();
    let mut out = String::new();
    if hint != tool_hint && !hint.starts_with(tool_hint) {
        let _ = write!(out, "hint[{}]", hint.chars().take(110).collect::<String>());
    }
    if !status.is_empty() {
        let _ = write!(out, " status[{}]", status.chars().take(110).collect::<String>());
    }
    out
}

/// A fingerprint of everything a gesture can change besides the document:
/// the selection, the verb region and the edit tool's pick.
pub(super) fn picked(rig: &WindowRig) -> String {
    format!(
        "{:?}|{:?}|{}",
        rig.ws.grid.selection,
        rig.ws.grid.verb.region.as_ref().map(|r| r.cells().count()),
        rig.ws.grid.verb.path.len()
    )
}

/// The result of one cell of the matrix.
#[derive(Clone, Debug)]
pub(super) struct Outcome {
    pub changed: bool,
    /// After the gesture: one undo gives back the document before it.
    pub undo_restores: Option<bool>,
    /// Then one redo gives back the document after it.
    pub redo_reapplies: Option<bool>,
    pub picked_changed: bool,
    pub said: String,
    pub history_steps: usize,
}

fn history_len(rig: &WindowRig) -> usize {
    rig.ws.history.steps()
}

/// Run `f` from a clean slate and report what it did.
pub(super) fn run(
    rig: &mut WindowRig,
    base: &ProjectDocument,
    tool: GridTool,
    how: Start,
    f: impl FnOnce(&mut WindowRig),
) -> Option<Outcome> {
    rig.ws.project = base.clone();
    rig.ws.history = Default::default();
    rig.ws.grid.verb.clear();
    {
        use crate::grid_tools::verb_state::RegionSource as R;
        let v = &mut rig.ws.grid.verb;
        (v.source, v.stairs_source, v.spiral_source, v.shared_source) =
            (R::Marquee, R::Marquee, R::Marquee, R::Marquee);
    }
    rig.ws.grid.selection = Default::default();
    rig.ws.status.clear();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    frames(rig, 2);
    park(rig);
    if !start(rig, tool, how) {
        return None;
    }
    let before_doc = rig.ws.project.clone();
    let before_pick = picked(rig);
    let h0 = history_len(rig);
    f(rig);
    let changed = rig.ws.project != before_doc;
    let (mut undo_restores, mut redo_reapplies) = (None, None);
    let said_now = said(rig);
    let picked_now = picked(rig) != before_pick;
    let steps = history_len(rig) - h0;
    if changed {
        let after = rig.ws.project.clone();
        rig.ws.do_undo();
        undo_restores = Some(rig.ws.project == before_doc);
        rig.ws.do_redo();
        redo_reapplies = Some(rig.ws.project == after);
    }
    Some(Outcome {
        changed,
        undo_restores,
        redo_reapplies,
        picked_changed: picked_now,
        said: said_now,
        history_steps: steps,
    })
}

/// The texts of the options bar and the header under the strip, joined.
pub(super) fn options_line(rig: &WindowRig) -> String {
    let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
    let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
    let mut items: Vec<(String, Rect)> = texts(rig)
        .into_iter()
        .filter(|(_, r)| r.center().y > top && r.center().y < bottom && r.width() > 0.0)
        .collect();
    items.sort_by(|a, b| a.1.min.x.partial_cmp(&b.1.min.x).unwrap_or(std::cmp::Ordering::Equal));
    items
        .into_iter()
        .map(|(t, _)| t.trim().to_string())
        .collect::<Vec<_>>()
        .join(" , ")
}

/// Click the label `label` in the options bar.
pub(super) fn option_click(rig: &mut WindowRig, label: &str) -> bool {
    let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
    let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
    let band = Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(rig.size.x, bottom));
    match text_in(rig, band, label) {
        Some(r) => {
            rig.click(r.center());
            frames(rig, 2);
            true
        }
        None => false,
    }
}

/// Four floor pixels in the 3D view that make a rough rectangle, found with
/// the editor's picking.
pub(super) fn floor_quad(rig: &WindowRig) -> Option<[Pos2; 4]> {
    let rect = rig.ws.last_persp_viewport_rect;
    let is = |p: Pos2| {
        rect.contains(p)
            && rig
                .ws
                .pick_face_3d(rect, p)
                .is_some_and(|h| h.normal[1] > 0.8)
    };
    let centre = rect.center();
    for scale in [2.2_f32, 1.6, 1.0, 0.7, 0.5] {
        for oy in [0.0_f32, 40.0, -40.0, 80.0, -80.0] {
            for ox in [0.0_f32, 60.0, -60.0, 120.0, -120.0] {
                let a = centre + Vec2::new(ox, oy);
                let q = [
                    a,
                    a + Vec2::new(90.0 * scale, 0.0),
                    a + Vec2::new(90.0 * scale, 50.0 * scale),
                    a + Vec2::new(0.0, 50.0 * scale),
                ];
                if q.iter().all(|&p| is(p)) {
                    return Some(q);
                }
            }
        }
    }
    None
}

/// Screen pixel of the centre of world cell `c` of the block's room in the
/// 3D view, projected through the editor's own camera.
pub(super) fn cell_px3d(rig: &WindowRig, c: [i32; 2]) -> Option<Pos2> {
    let (room, base) = floor_block(rig);
    let world = rig.world();
    let r = &world.rooms[room];
    let sector = world.scale.to_editor(1024) as f32;
    let [lx, lz] = r.local(base).unwrap();
    let h = r.sector(lx, lz).and_then(|s| s.ground_floor()).map_or(0, |f| f.heights[0]);
    let y = crate::grid_tools::view::editor_y(world, r.y_offset, h);
    let p = [(c[0] as f32 + 0.5) * sector, y, (c[1] as f32 + 0.5) * sector];
    let camera = rig.ws.viewport_3d_camera();
    let rect = rig.ws.last_persp_viewport_rect;
    crate::grid_tools::view::project_point(camera, rect, p).map(|(p, _)| p)
}

/// Four cell centres in the plan that make a 2x2 square.
pub(super) fn plan_quad(rig: &WindowRig) -> [Pos2; 4] {
    let (_, c) = floor_block(rig);
    [
        plan_px(rig, c),
        plan_px(rig, [c[0] + 1, c[1]]),
        plan_px(rig, [c[0] + 1, c[1] + 1]),
        plan_px(rig, [c[0], c[1] + 1]),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Marquee,
    Brush,
    Path,
    Lasso,
    /// The cells picked with the Edit tool first.
    EditSelection,
    /// Four corners of a ring with Path, Enter, then the climb (Spiral).
    Ring,
    /// Path waypoints, no Enter, then the press and drag up.
    PathNoEnter,
    /// Lasso corners, no Enter, then the press and drag up.
    LassoNoEnter,
}

/// Make a region the way `source` does, in the current view, then press in
/// the middle of it and drag up for the amount. Returns false when the view
/// has no spot to do it on.
pub(super) fn region_then_amount(rig: &mut WindowRig, source: Source, in_plan: bool, tool: GridTool) -> bool {
    let (_, bc) = floor_block(rig);
    let at = |rig: &WindowRig, c: [i32; 2]| {
        if in_plan {
            Some(plan_px(rig, c))
        } else {
            cell_px3d(rig, c)
        }
    };
    let corners = [
        [bc[0], bc[1]],
        [bc[0] + 1, bc[1]],
        [bc[0] + 1, bc[1] + 1],
        [bc[0], bc[1] + 1],
    ];
    let mut quad = [Pos2::ZERO; 4];
    for (slot, c) in quad.iter_mut().zip(corners) {
        match at(rig, c) {
            Some(p) => *slot = p,
            None => return false,
        }
    }
    if tool == GridTool::Spiral && matches!(source, Source::Marquee | Source::PathNoEnter) {
        for (slot, d) in quad.iter_mut().zip([(0, 0), (3, 0), (3, 2), (0, 2)]) {
            match at(rig, [bc[0] + d.0, bc[1] + d.1]) {
                Some(p) => *slot = p,
                None => return false,
            }
        }
    }
    let mut mid = quad[0] + (quad[2] - quad[0]) * 0.5;
    if source == Source::Path {
        mid = quad[1];
    }
    match source {
        Source::Marquee => rig.drag(quad[0], quad[2]),
        Source::Brush => {
            rig.drag(quad[0], quad[1]);
            rig.drag(quad[1], quad[2]);
        }
        Source::Path => {
            for p in [quad[0], quad[1], quad[2]] {
                rig.click(p);
            }
            key(rig, egui::Key::Enter);
        }
        Source::Lasso => {
            // The shape's corners are the outer corners of a 2x2 block of
            // sectors, so it holds four sector centres.
            for (dx, dz) in [(0, 0), (2, 0), (2, 2), (0, 2)] {
                let a = at(rig, [bc[0] + dx, bc[1] + dz]);
                let b = at(rig, [bc[0] + dx - 1, bc[1] + dz - 1]);
                match (a, b) {
                    (Some(a), Some(b)) => rig.click(a + (b - a) * 0.5),
                    _ => return false,
                }
            }
            key(rig, egui::Key::Enter);
        }
        Source::PathNoEnter => {
            let pts: &[Pos2] = if tool == GridTool::Spiral {
                &quad
            } else {
                &quad[..3]
            };
            for &p in pts {
                rig.click(p);
            }
            mid = quad[1];
        }
        Source::LassoNoEnter => {
            for (dx, dz) in [(0, 0), (2, 0), (2, 2), (0, 2)] {
                let a = at(rig, [bc[0] + dx, bc[1] + dz]);
                let b = at(rig, [bc[0] + dx - 1, bc[1] + dz - 1]);
                match (a, b) {
                    (Some(a), Some(b)) => rig.click(a + (b - a) * 0.5),
                    _ => return false,
                }
            }
            mid = quad[1];
        }
        Source::Ring => {
            let c = bc;
            let mut ring = [Pos2::ZERO; 4];
            for (slot, cell) in ring.iter_mut().zip([
                c,
                [c[0] + 3, c[1]],
                [c[0] + 3, c[1] + 2],
                [c[0], c[1] + 2],
            ]) {
                match at(rig, cell) {
                    Some(p) => *slot = p,
                    None => return false,
                }
            }
            for p in ring {
                rig.click(p);
            }
            key(rig, egui::Key::Enter);
            mid = ring[1];
        }
        Source::EditSelection => {
            // Pick with the Edit tool (a Shift box), then come back.
            let before = rig.ws.grid.tool;
            let _ = before;
            if !start(rig, GridTool::Edit, Start::Strip) {
                return false;
            }
            let (Some(a), Some(b), Some(m)) = (
                at(rig, bc),
                at(rig, [bc[0] + 3, bc[1] + 2]),
                at(rig, [bc[0] + 1, bc[1] + 1]),
            ) else {
                return false;
            };
            mid = m;
            rig.modifiers = egui::Modifiers::SHIFT;
            rig.drag(a, b);
            rig.modifiers = egui::Modifiers::NONE;
            frames(rig, 2);
            if !start(rig, tool, Start::Strip) {
                return false;
            }
            eprintln!("selection after edit: {}", picked(rig));
        }
    }
    drag_up(rig, mid, 70.0);
    true
}

#[test]
fn sources_matrix() {
    let Ok(out_path) = std::env::var("PSXED_TOOL_AUDIT_SOURCES") else {
        return;
    };
    let mut out = String::new();
    for fixture in [Fixture::Starter, Fixture::StylesDemo] {
        let mut rig = fixture.rig();
        let base = rig.ws.project.clone();
        for tool in [
            GridTool::Ramp,
            GridTool::Stairs,
            GridTool::Spiral,
            GridTool::Slab,
            GridTool::Break,
        ] {
            for in_plan in [false, true] {
                for source in [
                    Source::Marquee,
                    Source::Brush,
                    Source::Path,
                    Source::Lasso,
                    Source::EditSelection,
                    Source::Ring,
                    Source::PathNoEnter,
                    Source::LassoNoEnter,
                ] {
                    if source == Source::Ring && tool != GridTool::Spiral {
                        continue;
                    }
                    let mut chosen = true;
                    let mut spot = true;
                    let res = run(&mut rig, &base, tool, Start::Strip, |rig| {
                        if in_plan {
                            plan(rig);
                        } else {
                            persp(rig);
                        }
                        let label = match (tool, source) {
                            (GridTool::Stairs, Source::Marquee) => Some("Rectangle"),
                            (_, Source::Marquee) => Some("Marquee"),
                            (_, Source::Brush) => Some("Brush"),
                            (_, Source::Path | Source::PathNoEnter) => Some("Path"),
                            (_, Source::Lasso | Source::LassoNoEnter) => Some("Lasso"),
                            (_, Source::EditSelection) => None,
                            (_, Source::Ring) => Some("Path"),
                        };
                        if let Some(label) = label {
                            chosen = option_click(rig, label);
                        }
                        if chosen {
                            spot = region_then_amount(rig, source, in_plan, tool);
                        }
                    });
                    let _ = writeln!(
                        out,
                        "{}|{}|{}|{:?}|{}",
                        fixture.name(),
                        tool.label(),
                        if in_plan { "plan" } else { "3d" },
                        source,
                        match res {
                            _ if !chosen => "no option button for this source".to_string(),
                            _ if !spot => "no floor spot found (not verified)".to_string(),
                            Some(o) => format!(
                                "doc={} steps={} {}",
                                o.changed, o.history_steps, o.said
                            ),
                            None => "not-started".to_string(),
                        }
                    );
                }
            }
        }
    }
    std::fs::write(out_path, out).unwrap();
}

#[test]
fn matrix() {
    let Ok(out_path) = std::env::var("PSXED_TOOL_AUDIT") else {
        return;
    };
    let only = std::env::var("PSXED_TOOL_AUDIT_TOOLS").ok();
    let mut out = String::new();
    for fixture in [Fixture::Starter, Fixture::StylesDemo] {
        let mut rig = fixture.rig();
        let base = rig.ws.project.clone();
        for tool in GridTool::ALL {
            if only
                .as_deref()
                .is_some_and(|o| !o.split(',').any(|n| n == tool.label()))
            {
                continue;
            }
            // How each start reaches the tool.
            for how in [Start::Strip, Start::Key, Start::Search] {
                rig.ws.project = base.clone();
                park(&mut rig);
                let ok = start(&mut rig, tool, how);
                let _ = writeln!(
                    out,
                    "{}|{}|start-{:?}|{}|{}",
                    fixture.name(),
                    tool.label(),
                    how,
                    ok,
                    said(&rig)
                );
            }
            let _ = start(&mut rig, tool, Start::Strip);
            let _ = writeln!(
                out,
                "{}|{}|info|bar={}|options={}",
                fixture.name(),
                tool.label(),
                rig.ws.status_hint().chars().take(160).collect::<String>(),
                options_line(&rig)
            );
            // The gestures, started from the strip.
            for view in ["3d", "plan"] {
                for g in GESTURES {
                    if g.view != view {
                        continue;
                    }
                    let res = run(&mut rig, &base, tool, Start::Strip, |rig| {
                        if view == "plan" {
                            plan(rig);
                        } else {
                            persp(rig);
                        }
                        (g.f)(rig);
                    });
                    let _ = match res {
                        Some(o) => writeln!(
                            out,
                            "{}|{}|{}:{}|doc={}|pick={}|steps={}|undo={:?}|redo={:?}|{}",
                            fixture.name(),
                            tool.label(),
                            view,
                            g.name,
                            o.changed,
                            o.picked_changed,
                            o.history_steps,
                            o.undo_restores,
                            o.redo_reapplies,
                            o.said
                        ),
                        None => writeln!(
                            out,
                            "{}|{}|{}:{}|not-started",
                            fixture.name(),
                            tool.label(),
                            view,
                            g.name
                        ),
                    };
                }
            }
        }
    }
    std::fs::write(out_path, out).unwrap();
}

pub(super) struct Gesture {
    pub name: &'static str,
    pub view: &'static str,
    pub f: fn(&mut WindowRig),
}

fn g_click_floor(rig: &mut WindowRig) {
    if let Some((a, _)) = floor_px(rig) {
        rig.click(a);
    }
}
fn g_drag_floor(rig: &mut WindowRig) {
    if let Some((a, b)) = floor_px(rig) {
        rig.drag(a, b);
    }
}
fn g_drag_floor_then_up(rig: &mut WindowRig) {
    if let Some((a, b)) = floor_px(rig) {
        rig.drag(a, b);
        let mid = a + (b - a) * 0.5;
        drag_up(rig, mid, 80.0);
    }
}
fn g_esc_floor(rig: &mut WindowRig) {
    if let Some((a, b)) = floor_px(rig) {
        drag_hold(rig, a, b);
        key(rig, egui::Key::Escape);
        release(rig, b);
    }
}
fn g_esc_cells(rig: &mut WindowRig) {
    let (_, c) = floor_block(rig);
    drag_hold(rig, plan_px(rig, c), plan_px(rig, [c[0] + 3, c[1] + 2]));
    key(rig, egui::Key::Escape);
    release(rig, plan_px(rig, [c[0] + 3, c[1] + 2]));
}
fn g_drag_wall(rig: &mut WindowRig) {
    if let Some((a, b)) = wall_px(rig) {
        rig.drag(a, b);
    }
}
fn g_click_cell(rig: &mut WindowRig) {
    let (_, c) = floor_block(rig);
    rig.click(plan_px(rig, [c[0] + 1, c[1]]));
}
fn g_drag_cells(rig: &mut WindowRig) {
    let (_, c) = floor_block(rig);
    rig.drag(plan_px(rig, c), plan_px(rig, [c[0] + 3, c[1] + 2]));
}
fn g_drag_line(rig: &mut WindowRig) {
    let (_, c) = floor_block(rig);
    rig.drag(plan_px(rig, [c[0], c[1] + 1]), plan_px(rig, [c[0] + 3, c[1] + 1]));
}
fn g_drag_cells_then_up(rig: &mut WindowRig) {
    let (_, c) = floor_block(rig);
    rig.drag(plan_px(rig, c), plan_px(rig, [c[0] + 3, c[1] + 2]));
    let mid = plan_px(rig, [c[0] + 1, c[1] + 1]);
    drag_up(rig, mid, 60.0);
}

const GESTURES: [Gesture; 10] = [
    Gesture { name: "click-floor", view: "3d", f: g_click_floor },
    Gesture { name: "drag-floor", view: "3d", f: g_drag_floor },
    Gesture { name: "drag-floor-then-up", view: "3d", f: g_drag_floor_then_up },
    Gesture { name: "drag-wall", view: "3d", f: g_drag_wall },
    Gesture { name: "click-cell", view: "plan", f: g_click_cell },
    Gesture { name: "drag-cells", view: "plan", f: g_drag_cells },
    Gesture { name: "drag-line", view: "plan", f: g_drag_line },
    Gesture { name: "esc-mid-drag", view: "plan", f: g_esc_cells },
    Gesture { name: "esc-mid-drag", view: "3d", f: g_esc_floor },
    Gesture { name: "drag-cells-then-up", view: "plan", f: g_drag_cells_then_up },
];


// ---------------------------------------------------------------------
// The rules the audit showed the editor must keep.

fn offered(rig: &mut WindowRig, tool: GridTool, source: Source) -> bool {
    let label = match (tool, source) {
        (_, Source::EditSelection | Source::Ring) => return true,
        (GridTool::Stairs | GridTool::Spiral, Source::Marquee) => "Rectangle",
        (_, Source::Marquee) => "Marquee",
        (_, Source::Brush) => "Brush",
        (_, Source::Path | Source::PathNoEnter) => "Path",
        (_, Source::Lasso | Source::LassoNoEnter) => "Lasso",
    };
    let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
    let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
    let band = Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(rig.size.x, bottom));
    text_in(rig, band, label).is_some()
}

/// Every region source the options bar offers, and the Edit tool's
/// selection, makes the tool's change in both views.
#[test]
fn every_region_source_does_the_tools_job() {
    let mut failures = Vec::new();
    let mut rig = Fixture::Starter.rig();
    let base = rig.ws.project.clone();
    for tool in [
        GridTool::Ramp,
        GridTool::Stairs,
        GridTool::Slab,
        GridTool::Break,
        GridTool::Spiral,
    ] {
        for in_plan in [false, true] {
            for source in [
                Source::Marquee,
                Source::Brush,
                Source::Path,
                Source::Lasso,
                Source::PathNoEnter,
                Source::LassoNoEnter,
                Source::EditSelection,
            ] {
                // The Spiral's path source is a ring with Enter.
                let source = if tool == GridTool::Spiral && source == Source::Path {
                    Source::Ring
                } else {
                    source
                };
                let mut present = true;
                let mut ran = true;
                let res = run(&mut rig, &base, tool, Start::Strip, |rig| {
                    if in_plan {
                        plan(rig);
                    } else {
                        persp(rig);
                    }
                    if source != Source::EditSelection {
                        if !offered(rig, tool, source) {
                            present = false;
                            return;
                        }
                        let _ = match (tool, source) {
                            (GridTool::Stairs | GridTool::Spiral, Source::Marquee) => {
                                option_click(rig, "Rectangle")
                            }
                            (_, Source::Marquee) => option_click(rig, "Marquee"),
                            (_, Source::Brush) => option_click(rig, "Brush"),
                            (_, Source::Path | Source::PathNoEnter | Source::Ring) => {
                                option_click(rig, "Path")
                            }
                            _ => option_click(rig, "Lasso"),
                        };
                    }
                    ran = region_then_amount(rig, source, in_plan, tool);
                });
                if !present {
                    continue;
                }
                let o = res.expect("the tool starts from the strip");
                let explained = tool == GridTool::Break && o.said.contains("nothing broke");
                if !ran || !(o.changed || explained) {
                    failures.push(format!(
                        "{} {:?} {} -> changed {} ({})",
                        tool.label(),
                        source,
                        if in_plan { "plan" } else { "3d" },
                        o.changed,
                        o.said
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The first thing a person tries with Spiral, a press-drag on the floor and
/// then a drag up, builds a spiral, and Esc and undo put it back.
#[test]
fn spiral_press_drag_on_the_floor_builds_a_spiral() {
    for in_plan in [false, true] {
        let mut rig = Fixture::Starter.rig();
        let base = rig.ws.project.clone();
        let o = run(&mut rig, &base, GridTool::Spiral, Start::Strip, |rig| {
            if in_plan {
                plan(rig);
            } else {
                persp(rig);
            }
            assert!(region_then_amount(rig, Source::Marquee, in_plan, GridTool::Spiral));
        })
        .unwrap();
        assert!(o.changed, "{}", o.said);
        assert!(o.said.contains("treads"), "{}", o.said);
        assert_eq!(o.undo_restores, Some(true));
        assert_eq!(o.redo_reapplies, Some(true));
    }
}

/// Moving from Spiral to Ramp does not carry Spiral's source along.
#[test]
fn a_tool_does_not_inherit_the_source_of_the_last_one() {
    let mut rig = Fixture::Starter.rig();
    for first in [GridTool::Spiral, GridTool::Stairs] {
        assert!(start(&mut rig, first, Start::Strip));
        assert!(option_click(&mut rig, "Path"));
        for next in [GridTool::Ramp, GridTool::Slab, GridTool::Break] {
            assert!(start(&mut rig, next, Start::Strip));
            assert_eq!(
                rig.ws.grid.verb.source,
                crate::grid_tools::verb_state::RegionSource::Marquee,
                "{} after {}",
                next.label(),
                first.label()
            );
        }
    }
}

/// No tool ends a press-drag or a click on the floor or a wall with no change
/// to the document or the selection and no message.
#[test]
fn no_tool_ends_a_gesture_silently() {
    let mut silent = Vec::new();
    for fixture in [Fixture::Starter, Fixture::StylesDemo] {
        let mut rig = fixture.rig();
        let base = rig.ws.project.clone();
        for tool in GridTool::ALL {
            for view in ["3d", "plan"] {
                for g in GESTURES.iter().filter(|g| g.view == view && !g.name.starts_with("esc")) {
                    let mut ran = true;
                    let (mut hint0, mut edit0) = (String::new(), String::new());
                    let o = run(&mut rig, &base, tool, Start::Strip, |rig| {
                        if view == "plan" {
                            plan(rig);
                        } else {
                            persp(rig);
                            ran = match g.name {
                                "drag-wall" => wall_px(rig).is_some(),
                                _ => floor_px(rig).is_some(),
                            };
                        }
                        hint0 = rig.ws.build_hint().to_string();
                        edit0 = rig.ws.edit_hint().to_string();
                        if ran {
                            (g.f)(rig);
                        }
                    })
                    .expect("every tool starts from the strip");
                    if !ran {
                        continue;
                    }
                    let status = rig.ws.status.clone();
                    let spoke = !rig.ws.build_hint().is_empty() && rig.ws.build_hint() != hint0
                        || !rig.ws.edit_hint().is_empty() && rig.ws.edit_hint() != edit0
                        || !status.is_empty()
                        && !status.starts_with("Tool:")
                        && !status.starts_with("View:");
                    if !(o.changed || o.picked_changed || spoke) {
                        silent.push(format!(
                            "{} {} {}:{} -> status {:?}",
                            fixture.name(),
                            tool.label(),
                            view,
                            g.name,
                            status
                        ));
                    }
                }
            }
        }
    }
    assert!(silent.is_empty(), "{}", silent.join("\n"));
}


/// Every catalog tool starts from its strip button, from the search, and
/// from its key when it has one (the strip's menus included).
#[test]
fn every_catalog_tool_starts_from_the_strip_the_search_and_its_key() {
    let mut rig = Fixture::Starter.rig();
    for tool in GridTool::ALL {
        for how in [Start::Strip, Start::Search, Start::Key] {
            let started = start(&mut rig, tool, how);
            if how == Start::Key && !tool.has_key() {
                continue;
            }
            assert!(started, "{} from {how:?}: {}", tool.label(), rig.ws.status);
        }
    }
}

/// Esc in the middle of a press-drag puts the document back exactly and
/// leaves no undo step, for every tool, in both views.
#[test]
fn esc_in_the_middle_of_a_drag_restores_exactly_for_every_tool() {
    let mut bad = Vec::new();
    for fixture in [Fixture::Starter, Fixture::StylesDemo] {
        let mut rig = fixture.rig();
        let base = rig.ws.project.clone();
        for tool in GridTool::ALL {
            for g in GESTURES.iter().filter(|g| g.name == "esc-mid-drag") {
                let view = g.view;
                let o = run(&mut rig, &base, tool, Start::Strip, |rig| {
                    if view == "plan" {
                        plan(rig);
                    } else {
                        persp(rig);
                    }
                    if view == "plan" || floor_px(rig).is_some() {
                        (g.f)(rig);
                    }
                })
                .unwrap();
                if o.changed || o.history_steps != 0 {
                    bad.push(format!(
                        "{} {} {}: changed {} steps {}",
                        fixture.name(),
                        tool.label(),
                        view,
                        o.changed,
                        o.history_steps
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
