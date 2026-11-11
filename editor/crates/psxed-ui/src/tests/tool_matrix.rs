//! The tool x view matrix. Every one of the 33 tools has a canonical use,
//! written once, started from its strip button, and run in the 3D view, the
//! Plan, and both panes of the split view. The same flow, held in the middle
//! of its last gesture, is pressed Esc on. `PSXED_MATRIX_OUT=<file>` writes the
//! results as the matrix document (see `matrix_report`).

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, texts};
use super::tool_audit::*;
use super::tool_flows::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::WallEdge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Expect {
    /// The document changes.
    Doc,
    /// The selection changes.
    Selection,
    /// Nodes are added.
    Nodes,
}

/// What a flow leaves for the checks: the state just before its last gesture
/// and, when it was held, where the pointer is.
pub(super) struct Out {
    pub held: Option<Pos2>,
    pub before: ProjectDocument,
    pub sel_before: String,
    pub nodes_before: usize,
    pub steps_before: usize,
}

type Flow = fn(&mut WindowRig, View, bool) -> Result<Out, String>;

fn nodes(rig: &WindowRig) -> usize {
    rig.ws.project.active_scene().nodes().len()
}

fn mark(rig: &WindowRig) -> Out {
    Out {
        held: None,
        before: rig.ws.project.clone(),
        sel_before: format!("{:?}", rig.ws.grid.selection),
        nodes_before: nodes(rig),
        steps_before: history_steps(rig),
    }
}

/// A drag, released or held in the middle.
fn go(rig: &mut WindowRig, a: Pos2, b: Pos2, release_it: bool, out: &mut Out) {
    if release_it {
        rig.drag(a, b);
    } else {
        drag_hold(rig, a, b);
        out.held = Some(b);
    }
}

fn mid(a: Pos2, b: Pos2) -> Pos2 {
    a + (b - a) * 0.5
}

const UP: Vec2 = Vec2::new(0.0, -70.0);

fn cells_up_by(
    rig: &mut WindowRig,
    v: View,
    lo: [i32; 2],
    hi: [i32; 2],
    up: f32,
    r: bool,
) -> Result<Out, String> {
    v.drag_cells(rig, lo, hi);
    let m = mid(v.cell(rig, lo), v.cell(rig, hi));
    let mut out = mark(rig);
    go(rig, m, m + Vec2::new(0.0, -up), r, &mut out);
    Ok(out)
}

fn cells_up(rig: &mut WindowRig, v: View, lo: [i32; 2], hi: [i32; 2], r: bool) -> Result<Out, String> {
    cells_up_by(rig, v, lo, hi, 70.0, r)
}

/// A climb a person makes in the 3D view is a few pixels: the pointer rate
/// there is much higher than the Plan's 18 pixels to the click.
fn small_climb(v: View) -> f32 {
    match v {
        View::ThreeD => 14.0,
        View::BothPerspPane => 8.0,
        _ => 70.0,
    }
}

fn f_stairs(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    cells_up_by(rig, v, [1, 1], [2, 2], small_climb(v), r)
}

fn f_break(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    cells_up_by(rig, v, [0, 0], [4, 4], 120.0, r)
}

fn f_edit(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let a = v.cell(rig, [2, 2]);
    let mut out = mark(rig);
    go(rig, a, a + Vec2::new(0.0, -60.0), r, &mut out);
    Ok(out)
}

fn f_box(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [0, 0]), v.cell(rig, [2, 2]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_height(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    f_edit(rig, v, r)
}

fn f_extrude(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    cells_up(rig, v, [1, 1], [2, 2], r)
}

fn f_spiral(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    cells_up(rig, v, [1, 1], [4, 3], r)
}

fn f_room(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [10, 0]), v.cell(rig, [13, 3]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_corridor(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let mut out = mark(rig);
    for c in [[5, 2], [8, 2], [8, 1]] {
        v.click_cell(rig, c);
    }
    if r {
        key(rig, egui::Key::Enter);
    } else {
        out.held = Some(Pos2::ZERO);
    }
    Ok(out)
}

fn f_bridge(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let mut out = mark(rig);
    v.click_cell(rig, [5, 2]);
    if r {
        v.click_cell(rig, [9, 2]);
    } else {
        out.held = Some(Pos2::ZERO);
    }
    Ok(out)
}

fn f_wall_tool_run(rig: &mut WindowRig, v: View, r: bool, up: f32, base: f32) -> Result<Out, String> {
    let _ = (up, base);
    // Opening, Door and Bands act on a wall.
    let (a, b) = v.wall(rig, [2, 0], WallEdge::North, 0.375, 0.625, 0.0, 300.0);
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_opening(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    f_wall_tool_run(rig, v, r, 300.0, 0.0)
}

fn f_bands(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, _) = v.wall(rig, [2, 0], WallEdge::North, 0.5, 0.5, 0.0, 500.0);
    let mut out = mark(rig);
    if r {
        rig.click(a);
    } else {
        out.held = Some(a);
        drag_hold(rig, a, a + Vec2::new(0.0, 0.0));
    }
    Ok(out)
}

fn f_line(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [1, 2]), v.cell(rig, [4, 2]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_building(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    f_room(rig, v, r)
}

fn f_cathedral(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [8, -1]), v.cell(rig, [19, 5]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_region_drag(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [1, 1]), v.cell(rig, [4, 4]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_small_drag(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [1, 1]), v.cell(rig, [2, 2]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_floor(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let (a, b) = (v.cell(rig, [7, 1]), v.cell(rig, [8, 2]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_ceiling(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    // A floor with no ceiling over it, then the ceiling.
    if !start(rig, GridTool::Floor, Start::Strip) {
        return Err("Floor did not start".into());
    }
    let (a, b) = (v.cell(rig, [7, 1]), v.cell(rig, [8, 2]));
    rig.drag(a, b);
    if !start(rig, GridTool::Ceiling, Start::Strip) {
        return Err("Ceiling did not start".into());
    }
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_wall(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    // Along the line between the rows z=2 and z=3.
    let a = mid(v.cell(rig, [1, 2]), v.cell(rig, [1, 3]));
    let b = mid(v.cell(rig, [4, 2]), v.cell(rig, [4, 3]));
    let mut out = mark(rig);
    go(rig, a, b, r, &mut out);
    Ok(out)
}

fn f_click(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    let a = v.cell(rig, [2, 2]);
    let mut out = mark(rig);
    if r {
        rig.click(a);
    } else {
        // A click has no middle: the press is held, then Esc.
        drag_hold(rig, a, a + Vec2::new(0.5, 0.0));
        out.held = Some(a);
    }
    Ok(out)
}

fn f_lever(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    // A lever waits for its door: a door first.
    if !start(rig, GridTool::Door, Start::Strip) {
        return Err("Door did not start".into());
    }
    let (a, b) = v.wall(rig, [2, 0], WallEdge::North, 0.375, 0.625, 0.0, 300.0);
    rig.drag(a, b);
    v.aim(rig, [-1, -3], [22, 8]);
    if !start(rig, GridTool::Lever, Start::Strip) {
        return Err("Lever did not start".into());
    }
    let mut out = mark(rig);
    let p = v.cell(rig, [4, 3]);
    if r {
        rig.click(p);
        v.click_cell(rig, [2, 0]);
    } else {
        rig.click(p);
        out.held = Some(Pos2::ZERO);
    }
    Ok(out)
}

/// The Library drawer: the closed bar's button, then a tab.
pub(super) fn lib_open(rig: &mut WindowRig, tab: &str) -> bool {
    let lowest = |rig: &WindowRig, label: &str| {
        texts(rig)
            .into_iter()
            .filter(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label)
            .max_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y))
            .map(|(_, r)| r)
    };
    if lowest(rig, tab).is_none() {
        let Some(r) = lowest(rig, "Library") else { return false };
        rig.click(r.center());
        frames(rig, 3);
    }
    let Some(r) = lowest(rig, tab) else { return false };
    rig.click(r.center());
    frames(rig, 3);
    true
}

/// Click the first Library row that carries `icon`.
pub(super) fn lib_row(rig: &mut WindowRig, icon: char) -> bool {
    let hit = texts(rig)
        .into_iter()
        .filter(|(t, r)| t.contains(icon) && r.center().y > rig.size.y * 0.5)
        .min_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y));
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 3);
            true
        }
        None => false,
    }
}

/// Click a material swatch of the Library's Materials tab: the thumbnails sit
/// between the tabs and the count line.
pub(super) fn lib_swatch(rig: &mut WindowRig, n: usize) -> bool {
    let tabs = texts(rig)
        .into_iter()
        .filter(|(t, _)| t.trim() == "Materials")
        .max_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y))
        .map(|(_, r)| r);
    let Some(tabs) = tabs else { return false };
    let p = Pos2::new(tabs.left() + 30.0 + 62.0 * n as f32, tabs.bottom() + 80.0);
    rig.click(p);
    frames(rig, 2);
    true
}

fn f_paint(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    if !lib_open(rig, "Materials") {
        return Err("no Library".into());
    }
    let mut armed = false;
    for n in 1..6 {
        let _ = start(rig, GridTool::Select, Start::Strip);
        if lib_swatch(rig, n) && rig.ws.grid.tool == GridTool::Paint {
            armed = true;
            break;
        }
    }
    if !armed {
        return Err("a material click did not arm Paint".into());
    }
    let a = v.cell(rig, [2, 2]);
    let mut out = mark(rig);
    if r {
        rig.click(a);
    } else {
        drag_hold(rig, a, a + Vec2::new(30.0, 0.0));
        out.held = Some(a);
    }
    Ok(out)
}

fn f_mesh(rig: &mut WindowRig, v: View, r: bool) -> Result<Out, String> {
    // An arch makes a mesh the world can place copies of.
    if !start(rig, GridTool::Arcade, Start::Strip) {
        return Err("Arcade did not start".into());
    }
    let (a, b) = (v.cell(rig, [1, 2]), v.cell(rig, [4, 2]));
    rig.drag(a, b);
    if !lib_open(rig, "Meshes") {
        return Err("no Library".into());
    }
    if !lib_row(rig, crate::icons::CUBOID) {
        return Err("the Meshes tab lists no mesh".into());
    }
    if rig.ws.grid.tool != GridTool::Mesh {
        return Err("a mesh click did not arm Mesh".into());
    }
    let p = v.cell(rig, [2, 4]);
    let mut out = mark(rig);
    if r {
        rig.click(p);
    } else {
        drag_hold(rig, p, p + Vec2::new(0.5, 0.0));
        out.held = Some(p);
    }
    Ok(out)
}

pub(super) const FLOWS: [(GridTool, Expect, Flow); 33] = [
    (GridTool::Edit, Expect::Doc, f_edit),
    (GridTool::Select, Expect::Selection, f_box),
    (GridTool::Height, Expect::Doc, f_height),
    (GridTool::Extrude, Expect::Doc, f_extrude),
    (GridTool::Room, Expect::Doc, f_room),
    (GridTool::Corridor, Expect::Doc, f_corridor),
    (GridTool::Opening, Expect::Doc, f_opening),
    (GridTool::Arcade, Expect::Doc, f_line),
    (GridTool::Colonnade, Expect::Doc, f_line),
    (GridTool::Bridge, Expect::Doc, f_bridge),
    (GridTool::Building, Expect::Doc, f_building),
    (GridTool::Cathedral, Expect::Doc, f_cathedral),
    (GridTool::Stairs, Expect::Doc, f_stairs),
    (GridTool::Ramp, Expect::Doc, f_extrude),
    (GridTool::Spiral, Expect::Doc, f_spiral),
    (GridTool::Slab, Expect::Doc, f_stairs),
    (GridTool::Floor, Expect::Doc, f_floor),
    (GridTool::Ceiling, Expect::Doc, f_ceiling),
    (GridTool::Wall, Expect::Doc, f_wall),
    (GridTool::Plane, Expect::Doc, f_click),
    (GridTool::Mesh, Expect::Doc, f_mesh),
    (GridTool::Sculpt, Expect::Doc, f_region_drag),
    (GridTool::Break, Expect::Doc, f_break),
    (GridTool::Terrain, Expect::Doc, f_region_drag),
    (GridTool::Ruin, Expect::Doc, f_region_drag),
    (GridTool::Cells, Expect::Doc, f_small_drag),
    (GridTool::Split, Expect::Doc, f_click),
    (GridTool::Paint, Expect::Doc, f_paint),
    (GridTool::Bands, Expect::Doc, f_bands),
    (GridTool::Place, Expect::Nodes, f_click),
    (GridTool::Door, Expect::Doc, f_opening),
    (GridTool::Lever, Expect::Nodes, f_lever),
    (GridTool::Checkpoint, Expect::Nodes, f_click),
];

/// Click flows have no middle; these end at once and Esc is not applicable.
fn instant(tool: GridTool) -> bool {
    matches!(
        tool,
        GridTool::Plane
            | GridTool::Split
            | GridTool::Paint
            | GridTool::Bands
            | GridTool::Place
            | GridTool::Lever
            | GridTool::Checkpoint
            | GridTool::Mesh
            | GridTool::Bridge
    )
}

fn fresh(view: View, tool: GridTool) -> Result<WindowRig, String> {
    let mut rig = view.rig(Fixture::Starter);
    view.aim(&mut rig, [-1, -3], [22, 8]);
    if !start(&mut rig, tool, Start::Strip) {
        return Err(format!("{} did not start from the strip", tool.label()));
    }
    Ok(rig)
}

/// Run one cell: the flow released, and the effect it was to have.
pub(super) fn run_cell(tool: GridTool, expect: Expect, flow: Flow, view: View) -> Result<String, String> {
    let mut rig = fresh(view, tool)?;
    // The Bands tool cuts at a height, which the Plan does not have: there it
    // selects the run and says to cut in the 3D view.
    let expect = if tool == GridTool::Bands && view.in_plan() { Expect::Selection } else { expect };
    let out = flow(&mut rig, view, true)?;
    let ok = match expect {
        Expect::Doc => rig.ws.project != out.before,
        Expect::Selection => format!("{:?}", rig.ws.grid.selection) != out.sel_before,
        Expect::Nodes => nodes(&rig) > out.nodes_before,
    };
    if !ok {
        return Err(format!("{expect:?} did not change: {}", rig.ws.status));
    }
    if expect == Expect::Doc && history_steps(&rig) <= out.steps_before {
        return Err("no undo step".into());
    }
    if expect != Expect::Selection && rig.ws.project != out.before {
        let after = rig.ws.project.clone();
        let mut undone = 0;
        while history_steps(&rig) > out.steps_before {
            rig.ws.do_undo();
            undone += 1;
        }
        if rig.ws.project != out.before {
            return Err(format!("{undone} undo step(s) do not restore the document"));
        }
        for _ in 0..undone {
            rig.ws.do_redo();
        }
        if rig.ws.project != after {
            return Err("redo does not reapply".into());
        }
    }
    Ok(rig.ws.status.chars().take(60).collect())
}

/// The same flow held in the middle of its last gesture, then Esc: the
/// document is as it was and no undo step is left. `Ok(None)` for a click.
pub(super) fn esc_cell(tool: GridTool, flow: Flow, view: View) -> Result<Option<()>, String> {
    if instant(tool) {
        return Ok(None);
    }
    let mut rig = fresh(view, tool)?;
    let out = flow(&mut rig, view, false)?;
    key(&mut rig, egui::Key::Escape);
    if let Some(p) = out.held {
        if p != Pos2::ZERO {
            release(&mut rig, p);
        }
    }
    if rig.ws.project != out.before {
        return Err(format!("the document changed: {}", rig.ws.status));
    }
    if history_steps(&rig) != out.steps_before {
        return Err("an undo step was left".into());
    }
    Ok(Some(()))
}

#[test]
fn every_tool_does_its_job_in_every_view() {
    let mut bad = Vec::new();
    for (tool, expect, flow) in FLOWS {
        for view in View::ALL {
            if let Err(e) = run_cell(tool, expect, flow, view) {
                bad.push(format!("{} {:?}: {e}", tool.label(), view));
            }
        }
    }
    assert!(bad.is_empty(), "{} cells:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn esc_in_the_middle_of_every_gesture_changes_nothing_in_every_view() {
    let mut bad = Vec::new();
    for (tool, _, flow) in FLOWS {
        for view in View::ALL {
            if let Err(e) = esc_cell(tool, flow, view) {
                bad.push(format!("{} {:?}: {e}", tool.label(), view));
            }
        }
    }
    assert!(bad.is_empty(), "{} cells:\n{}", bad.len(), bad.join("\n"));
}

/// Every tool exactly once in the table.
#[test]
fn the_matrix_names_every_tool_once() {
    for tool in GridTool::ALL {
        assert_eq!(FLOWS.iter().filter(|f| f.0 == tool).count(), 1, "{}", tool.label());
    }
}
