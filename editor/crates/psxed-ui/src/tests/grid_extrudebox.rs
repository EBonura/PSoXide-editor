//! Extrude picks with a real marquee. 2026-10-21, Manny: "when we select the
//! zone to extrude, we should have a marquee selection, the current one is
//! bad".
//!
//! Shift-drag (or a plain drag on empty space) draws a screen rectangle in
//! the 3D view and picks the faces of ONE kind inside it: the kind of the
//! face under the drag start, floors when it starts on nothing. Faces the
//! camera does not draw are never picked. Shift+Alt adds to the pick. The
//! release only picks; pressing a picked piece and dragging extrudes the set.
//!
//! The rig is the dp-world stand-in (see `grid_extrudefix`).

use super::grid_extrudefix::{
    bar, ceiling, eu, floor, idle, look_over_the_hall, look_up_at_the_ceiling, north_wall, over,
    region_len, rig, with_shift, HALL, MIXED,
};
use super::grid_live::WindowRig;
use super::*;
use crate::grid_tools::extrude_tool::ExtrudeSurface;
use crate::grid_tools::{Gesture, GridHover, GridTool};
use psxed_project::grid::edit::{FaceTarget, GridElement, SurfaceKind};
use std::collections::BTreeSet;

const SHIFT_ALT: egui::Modifiers = egui::Modifiers {
    alt: true,
    shift: true,
    ctrl: false,
    command: false,
    mac_cmd: false,
};

/// Screen point over the floor at fractional world cell `(x, z)`.
fn spot(rig: &WindowRig, x: f32, z: f32) -> Pos2 {
    spot_at(rig, x, z, 0)
}

fn spot_at(rig: &WindowRig, x: f32, z: f32, gu: i32) -> Pos2 {
    let world = rig.world();
    let at = |v: f32| world.scale.to_editor((v * 1024.0) as i32) as f32;
    rig.persp_screen([at(x), eu(rig, gu), at(z)])
        .unwrap_or_else(|| panic!("({x}, {z}) at {gu} is off screen"))
}

fn with_mods(rig: &mut WindowRig, mods: egui::Modifiers, f: impl FnOnce(&mut WindowRig)) {
    rig.modifiers = mods;
    f(rig);
    rig.modifiers = egui::Modifiers::NONE;
}

/// Press at `from` and move to `to`, leaving the button down.
fn hold(rig: &mut WindowRig, from: Pos2, to: Pos2) {
    rig.move_to(from);
    let e = rig.press(from, true);
    rig.frame(vec![e]);
    for step in 1..=6 {
        rig.move_to(from + (to - from) * (step as f32 / 6.0));
    }
}

fn let_go(rig: &mut WindowRig, at: Pos2) {
    let e = rig.press(at, false);
    rig.frame(vec![e]);
    rig.frame(vec![]);
}

fn picked_cells(rig: &WindowRig) -> BTreeSet<[i32; 2]> {
    rig.ws
        .grid
        .verb
        .region
        .as_ref()
        .map(|r| r.cells().collect())
        .unwrap_or_default()
}

fn cells(list: &[[i32; 2]]) -> BTreeSet<[i32; 2]> {
    list.iter().copied().collect()
}

/// Corners that put exactly cells 9..=10 by 3..=4 under a box.
fn four_floors(rig: &WindowRig) -> (Pos2, Pos2) {
    (spot(rig, 8.8, 3.1), spot(rig, 10.9, 4.9))
}

fn four() -> BTreeSet<[i32; 2]> {
    cells(&[[9, 3], [10, 3], [9, 4], [10, 4]])
}

fn box_of_floors(rig: &mut WindowRig, mods: egui::Modifiers) {
    let (a, b) = four_floors(rig);
    with_mods(rig, mods, |rig| rig.drag(a, b));
}

fn over_screen(rig: &WindowRig, cell: [i32; 2], gu: i32) -> Pos2 {
    rig.persp_screen(over(rig, cell, gu)).unwrap()
}

#[test]
fn a_shift_box_over_several_floors_picks_exactly_those() {
    let mut rig = rig();
    let before = rig.ws.project.clone();
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    assert_eq!(picked_cells(&rig), four(), "{}", rig.ws.status);
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
    assert!(idle(&rig));
    assert_eq!(rig.ws.project, before, "releasing the box only picks");
}

#[test]
fn a_plain_drag_from_empty_space_is_the_same_box_of_floors() {
    let mut rig = rig();
    // Above the hall's back wall there is nothing to press.
    let view = rig.ws.last_persp_viewport_rect;
    let a = Pos2::new(spot(&rig, 8.8, 3.1).x, view.min.y + 40.0);
    let b = spot(&rig, 10.9, 4.9);
    assert!(rig
        .ws
        .pick_face_3d(rig.ws.last_persp_viewport_rect, a)
        .is_none());
    rig.drag(a, b);
    let got = picked_cells(&rig);
    assert!(got.contains(&[9, 3]) && got.contains(&[10, 4]), "{got:?}");
    assert!(
        got.iter().all(|c| floor(&rig, *c).is_some()),
        "only cells with a floor: {got:?}"
    );
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
}

#[test]
fn the_box_is_drawn_and_the_faces_it_will_pick_light_up_while_dragging() {
    let mut rig = rig();
    let (a, b) = four_floors(&rig);
    with_shift(&mut rig, |rig| hold(rig, a, b));
    let Gesture::Marquee { start, current, .. } = rig.ws.grid.gesture else {
        panic!("no marquee in flight: {:?}", rig.ws.grid.gesture);
    };
    assert!((start - a).length() < 2.0 && (current - b).length() < 2.0);
    let GridHover::Elements(lit) = &rig.ws.grid.hover else {
        panic!("nothing lit: {:?}", rig.ws.grid.hover);
    };
    let lit_cells: BTreeSet<[i32; 2]> = lit
        .iter()
        .filter_map(|e| match e {
            GridElement::Face(f)
                if matches!(
                    f.target,
                    FaceTarget::Surface {
                        kind: SurfaceKind::Floor,
                        ..
                    }
                ) =>
            {
                Some(f.cell())
            }
            _ => None,
        })
        .collect();
    assert_eq!(lit_cells, four());
    assert_eq!(lit.len(), 4, "only floors light up: {lit:?}");
    assert!(
        picked_cells(&rig).is_empty(),
        "nothing is picked before the release"
    );
    let_go(&mut rig, b);
    assert_eq!(picked_cells(&rig), four());
}

#[test]
fn a_box_that_starts_on_a_wall_picks_only_walls_inside_it() {
    let mut rig = rig();
    // From the north wall down over the first rows of floor: the floors are
    // inside the box but are not walls.
    let a = north_wall(&rig, [8, -1], 2900);
    let b = spot(&rig, 12.9, 2.5);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    assert_eq!(
        rig.ws.grid.verb.surface,
        ExtrudeSurface::Wall,
        "{}",
        rig.ws.status
    );
    assert!(picked_cells(&rig).is_empty(), "no floor is picked");
    assert!(rig.ws.grid.selection.len() >= 3, "{}", rig.ws.status);
    for e in rig.ws.grid.selection.items() {
        let wall = match e {
            GridElement::Band { face, .. } | GridElement::Face(face) => matches!(
                face.target,
                FaceTarget::Derived { .. } | FaceTarget::Wall { .. }
            ),
            _ => false,
        };
        assert!(wall, "{e:?} is not a wall piece");
    }
}

#[test]
fn walls_the_camera_does_not_draw_are_not_picked() {
    let mut rig = rig();
    // A box over the whole hall, seen from outside the south: its far north
    // wall faces the camera, the near south wall shows its back.
    let a = north_wall(&rig, [8, -1], 2900);
    let b = spot_at(&rig, 13.0, 9.0, 0);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert!(rig.ws.grid.selection.len() >= 3, "{}", rig.ws.status);
    let _ = HALL;
    for e in rig.ws.grid.selection.items() {
        let (GridElement::Band { face, .. } | GridElement::Face(face)) = e else {
            panic!("{e:?}");
        };
        let edge = match face.target {
            FaceTarget::Derived { edge, .. } | FaceTarget::Wall { edge, .. } => edge,
            other => panic!("{other:?}"),
        };
        assert!(
            edge != psxed_project::grid::WallEdge::South,
            "a south wall (back to the camera) was picked: {face:?}"
        );
    }
}

#[test]
fn a_ceiling_the_camera_cannot_see_is_never_picked() {
    // From above the hall a box starting on a floor has the ceilings inside
    // it, culled: only floors come out.
    let mut rig = rig();
    let (a, b) = four_floors(&rig);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Floor);
    assert_eq!(picked_cells(&rig), four());
    // From inside, looking up: a box starting on a ceiling picks ceilings
    // and none of the floors under them.
    look_up_at_the_ceiling(&mut rig, [11, 1]);
    let a = spot_at(&rig, 10.1, 0.6, 3072);
    let b = spot_at(&rig, 12.9, 1.9, 3072);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    assert_eq!(
        rig.ws.grid.verb.surface,
        ExtrudeSurface::Ceiling,
        "{}",
        rig.ws.status
    );
    let got = picked_cells(&rig);
    assert!(got.contains(&[11, 1]), "{got:?}");
    assert!(got.iter().all(|c| ceiling(&rig, *c).is_some()), "{got:?}");
}

#[test]
fn shift_alt_adds_a_second_box_to_the_pick() {
    let mut rig = rig();
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    let (a, b) = (spot(&rig, 11.7, 1.1), spot(&rig, 13.3, 1.9));
    with_mods(&mut rig, SHIFT_ALT, |rig| rig.drag(a, b));
    let got = picked_cells(&rig);
    assert!(four().is_subset(&got), "{got:?}");
    assert!(got.len() > 4, "the second box added cells: {got:?}");
    assert!(got.contains(&[12, 1]), "{got:?}");
    // A plain Shift box replaces.
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    assert_eq!(picked_cells(&rig), four());
}

#[test]
fn a_box_of_another_kind_added_to_a_pick_is_refused_in_words() {
    let mut rig = rig();
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    let before = rig.ws.project.clone();
    let a = north_wall(&rig, [8, -1], 2900);
    let b = north_wall(&rig, [12, -1], 400);
    with_mods(&mut rig, SHIFT_ALT, |rig| rig.drag(a, b));
    assert!(rig.ws.status.contains(MIXED), "{}", rig.ws.status);
    assert_eq!(picked_cells(&rig), four(), "the floor pick stays");
    assert!(rig.ws.grid.selection.is_empty(), "no wall joined it");
    assert_eq!(rig.ws.project, before);
    assert!(idle(&rig));
}

#[test]
fn a_plain_shift_box_of_another_kind_replaces_the_pick() {
    let mut rig = rig();
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    let a = north_wall(&rig, [8, -1], 2900);
    let b = north_wall(&rig, [12, -1], 400);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert!(picked_cells(&rig).is_empty());
    assert!(!rig.ws.grid.selection.is_empty());
}

#[test]
fn after_the_release_a_press_on_a_picked_floor_extrudes_the_whole_set() {
    let mut rig = rig();
    box_of_floors(&mut rig, egui::Modifiers::SHIFT);
    assert_eq!(picked_cells(&rig), four());
    rig.drag(
        over_screen(&rig, [9, 3], 0),
        over_screen(&rig, [9, 3], 1024),
    );
    let heights: Vec<Option<i32>> = four().iter().map(|c| floor(&rig, *c)).collect();
    assert!(
        heights[0].is_some_and(|h| h > 0) && heights.iter().all(|h| *h == heights[0]),
        "{heights:?}: {}",
        rig.ws.status
    );
    assert_eq!(floor(&rig, [11, 3]), Some(0), "cells outside the box stay");
}

#[test]
fn a_boxed_set_of_walls_is_pushed_by_out() {
    let mut rig = rig();
    let a = north_wall(&rig, [8, -1], 2900);
    let b = north_wall(&rig, [10, -1], 200);
    with_shift(&mut rig, |rig| rig.drag(a, b));
    let n = rig.ws.grid.selection.len();
    assert!(n >= 2, "{}", rig.ws.status);
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    assert!(
        floor(&rig, [8, -2]).is_some() && floor(&rig, [9, -2]).is_some(),
        "{}",
        rig.ws.status
    );
}

#[test]
fn escape_while_boxing_drops_the_box_and_keeps_the_old_pick() {
    let mut rig = rig();
    rig.click(over_screen(&rig, [12, 4], 0));
    let (a, b) = four_floors(&rig);
    with_shift(&mut rig, |rig| hold(rig, a, b));
    assert!(matches!(rig.ws.grid.gesture, Gesture::Marquee { .. }));
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(idle(&rig), "{:?}", rig.ws.grid.gesture);
    assert_eq!(picked_cells(&rig), cells(&[[12, 4]]), "{}", rig.ws.status);
    assert!(matches!(rig.ws.grid.hover, GridHover::None));
}

#[test]
fn the_region_source_row_is_gone_from_extrude_but_stays_for_ramp() {
    let mut rig = rig();
    rig.click(over_screen(&rig, [9, 3], 0));
    let bar_text = bar(&rig);
    for source in ["Marquee", "Brush", "Path", "Lasso", "Region"] {
        assert!(
            !bar_text.contains(source),
            "{source} still in the Extrude bar: {bar_text}"
        );
    }
    rig.ws.set_grid_tool(GridTool::Ramp);
    rig.frame(vec![]);
    assert!(bar(&rig).contains("Brush"), "{}", bar(&rig));
}

#[test]
fn the_bar_and_the_status_teach_the_box() {
    let mut rig = rig();
    rig.frame(vec![]);
    assert!(bar(&rig).contains("box"), "{}", bar(&rig));
    let (a, b) = four_floors(&rig);
    with_shift(&mut rig, |rig| hold(rig, a, b));
    rig.frame(vec![]);
    assert!(bar(&rig).contains("Release"), "{}", bar(&rig));
    let_go(&mut rig, b);
    assert!(rig.ws.status.contains("4 floors"), "{}", rig.ws.status);
    assert!(rig.ws.status.contains("drag"), "{}", rig.ws.status);
}

#[test]
fn the_help_describes_the_box() {
    let mut rig = rig();
    rig.ws.grid.help_open = true;
    rig.frame(vec![]);
    rig.frame(vec![]);
    let text: String = super::grid_toolbar::texts(&rig)
        .into_iter()
        .map(|(t, _)| t)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("Grid editor keys"),
        "help is not open: {text}"
    );
    assert!(text.contains("Shift+Alt"), "{text}");
    assert!(!text.contains("Lasso: click corners"), "{text}");
}

#[test]
fn the_plan_still_sweeps_a_rectangle_and_shift_adds() {
    let mut rig = rig();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig.drag(rig.cell(9, 3), rig.cell(10, 4));
    assert_eq!(picked_cells(&rig), four(), "{}", rig.ws.status);
    with_shift(&mut rig, |rig| rig.drag(rig.cell(12, 1), rig.cell(13, 1)));
    assert_eq!(region_len(&rig), 6, "{}", rig.ws.status);
}

/// Captures for the review page:
/// `PSXED_EXTRUDEBOX_CAPTURE=<dir> PSXED_EXTRUDEBOX_LABEL=before|after`.
#[test]
fn extrudebox_captures() {
    use super::grid_pick_shots::{settle, wire};
    let Some(dir) = std::env::var_os("PSXED_EXTRUDEBOX_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let label = std::env::var("PSXED_EXTRUDEBOX_LABEL").unwrap_or_else(|_| "after".into());
    let shot = |rig: &mut WindowRig, name: &str| {
        settle(rig);
        wire(rig);
        rig.capture(&dir.join(format!("{name}-{label}.png")));
    };
    let mut rig = rig();
    let (a, b) = four_floors(&rig);
    with_shift(&mut rig, |rig| hold(rig, a, b));
    shot(&mut rig, "box-1-dragging-over-floors");
    let_go(&mut rig, b);
    shot(&mut rig, "box-2-picked-after-release");
    rig.drag(
        over_screen(&rig, [9, 3], 0),
        over_screen(&rig, [9, 3], 1024),
    );
    shot(&mut rig, "box-3-extruded-set");

    let mut rig = self::rig();
    look_over_the_hall(&mut rig);
    let a = north_wall(&rig, [8, -1], 2900);
    let b = north_wall(&rig, [12, -1], 400);
    with_shift(&mut rig, |rig| hold(rig, a, b));
    shot(&mut rig, "box-4-dragging-over-walls");
    let_go(&mut rig, b);
    shot(&mut rig, "box-5-walls-picked");
}
