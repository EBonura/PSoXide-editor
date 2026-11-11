//! The Edit tool's decision table (REVIEW.md part b), replayed from the strip
//! button through real pointer and key events, plus the right-click menu and
//! Play from here from both menus. Each row is a function of the view; a row
//! that makes no sense in a view (the wall arrow in the Plan) says so in its
//! name list.

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, press_key, texts};
use super::tool_audit::*;
use super::tool_flows::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::WallEdge;

type Row = fn(View) -> Result<(), String>;

fn erig(v: View) -> WindowRig {
    let mut rig = v.rig(Fixture::Starter);
    v.aim_near(&mut rig, [-1, -1], [7, 6], 0.55);
    assert!(start(&mut rig, GridTool::Edit, Start::Strip));
    rig
}

fn corners(rig: &WindowRig, c: [i32; 2]) -> Option<[i32; 4]> {
    sectors(rig).rooms.iter().find_map(|r| {
        let [x, z] = r.local(c)?;
        Some(r.sector(x, z)?.ground_floor()?.heights)
    })
}

fn sel_len(rig: &WindowRig) -> usize {
    rig.ws.grid.selection.items().len()
}

fn need(ok: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(what())
    }
}

fn raise_px(v: View) -> f32 {
    if v.in_plan() {
        36.0
    } else {
        40.0
    }
}

fn r_click_selects(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let before = rig.ws.project.clone();
    v.click_cell(&mut rig, [2, 2]);
    need(sel_len(&rig) >= 1 && rig.ws.project == before, || {
        "click did not select".into()
    })?;
    let first = format!("{:?}", rig.ws.grid.selection);
    v.click_cell(&mut rig, [4, 4]);
    need(format!("{:?}", rig.ws.grid.selection) != first, || {
        "a second click kept the first".into()
    })
}

fn r_ctrl_toggles_shift_adds(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    v.click_cell(&mut rig, [1, 1]);
    let one = sel_len(&rig);
    rig.modifiers = egui::Modifiers::COMMAND;
    v.click_cell(&mut rig, [3, 4]);
    let two = sel_len(&rig);
    v.click_cell(&mut rig, [1, 1]);
    let back = sel_len(&rig);
    rig.modifiers = egui::Modifiers::NONE;
    need(two > one && back < two, || {
        format!("ctrl-click: {one} {two} {back}")
    })?;
    let before = sel_len(&rig);
    rig.modifiers = egui::Modifiers::SHIFT;
    v.click_cell(&mut rig, [4, 4]);
    rig.modifiers = egui::Modifiers::NONE;
    need(sel_len(&rig) > before, || {
        format!("shift-click range: {before} then {}", sel_len(&rig))
    })
}

fn r_floor_drag_raises_one_step_and_undoes(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let base = rig.ws.project.clone();
    let steps = history_steps(&rig);
    let a = v.cell(&rig, [2, 2]);
    rig.drag(a, a + Vec2::new(0.0, -raise_px(v)));
    let h = corners(&rig, [2, 2]).ok_or("no floor")?;
    need(h.iter().all(|c| *c > 0), || {
        format!("tile not raised: {h:?} {}", rig.ws.status)
    })?;
    need(history_steps(&rig) == steps + 1, || {
        "not one undo step".into()
    })?;
    need(sel_len(&rig) >= 1, || {
        "the raised face is not selected".into()
    })?;
    rig.ws.do_undo();
    need(rig.ws.project == base, || "undo does not restore".into())
}

fn r_unselected_press_moves_only_that(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    v.click_cell(&mut rig, [1, 1]);
    let a = v.cell(&rig, [4, 4]);
    rig.drag(a, a + Vec2::new(0.0, -raise_px(v)));
    need(corners(&rig, [4, 4]).is_some_and(|h| h[0] > 0), || {
        "the pressed tile did not move".into()
    })?;
    need(corners(&rig, [1, 1]) == Some([0; 4]), || {
        "the selected tile moved".into()
    })
}

fn r_selected_press_moves_the_selection(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let (a, b) = (v.cell(&rig, [1, 1]), v.cell(&rig, [2, 2]));
    let step = (v.cell(&rig, [2, 2]) - v.cell(&rig, [1, 1])).abs();
    let pad = Vec2::new(step.x, step.y) * 0.3;
    rig.modifiers = egui::Modifiers::SHIFT;
    rig.drag(a - pad, b + pad);
    rig.modifiers = egui::Modifiers::NONE;
    need(sel_len(&rig) >= 4, || {
        format!("box selected {}", sel_len(&rig))
    })?;
    rig.drag(a, a + Vec2::new(0.0, -raise_px(v)));
    let hs: Vec<_> = [[1, 1], [2, 1], [1, 2], [2, 2]]
        .iter()
        .map(|c| corners(&rig, *c))
        .collect();
    need(
        hs.iter().all(|h| h.is_some_and(|h| h[0] > 0)) && hs.windows(2).all(|w| w[0] == w[1]),
        || format!("{hs:?}"),
    )
}

fn r_box_from_void(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    // Pan so the void east of the room is in view.
    v.aim_near(&mut rig, [2, -1], [11, 6], 0.55);
    v.click_cell(&mut rig, [4, 2]);
    need(sel_len(&rig) >= 1, || "no selection to clear".into())?;
    let (a, b) = (v.cell(&rig, [9, 1]), v.cell(&rig, [10, 3]));
    if !v.in_plan() {
        let rect = rig.ws.last_persp_viewport_rect;
        need(
            crate::grid_tools::edit::lands_on_nothing(&rig.ws, rect, a),
            || "the spot is not void".into(),
        )?;
    }
    // A box over nothing, pressed in the void, clears the selection.
    rig.drag(a, b);
    need(sel_len(&rig) == 0, || {
        format!("box on nothing left {}", sel_len(&rig))
    })?;
    // Shift adds, plain replaces.
    let (a, b) = (v.cell(&rig, [9, 0]), v.cell(&rig, [3, 1]));
    rig.modifiers = egui::Modifiers::SHIFT;
    rig.drag(a, b);
    rig.modifiers = egui::Modifiers::NONE;
    let boxed = sel_len(&rig);
    need(boxed >= 1, || "shift box from void selected nothing".into())?;
    let (a, b) = (v.cell(&rig, [9, 5]), v.cell(&rig, [4, 4]));
    rig.drag(a, b);
    need(sel_len(&rig) >= 1, || "box replace selected nothing".into())
}

fn r_edge_tilts(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let p = mid(v.cell(&rig, [2, 2]), v.cell(&rig, [3, 2]));
    rig.drag(p, p + Vec2::new(0.0, -raise_px(v)));
    let (a, b) = (
        corners(&rig, [2, 2]).ok_or("no floor")?,
        corners(&rig, [3, 2]).ok_or("no floor")?,
    );
    need(a != [0; 4] || b != [0; 4], || {
        format!("edge drag changed nothing: {}", rig.ws.status)
    })?;
    need(
        a.contains(&0) || b.contains(&0),
        || format!("a tilt leaves corners: {a:?} {b:?}"),
    )
}

fn mid(a: Pos2, b: Pos2) -> Pos2 {
    a + (b - a) * 0.5
}

fn r_corner_lifts(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let c = [[2, 2], [3, 2], [2, 3], [3, 3]];
    let p = c
        .iter()
        .map(|c| v.cell(&rig, *c))
        .fold(Vec2::ZERO, |a, p| a + p.to_vec2())
        / 4.0;
    let p = p.to_pos2();
    rig.drag(p, p + Vec2::new(0.0, -raise_px(v)));
    let hs: Vec<_> = c.iter().map(|c| corners(&rig, *c)).collect();
    let lifted = hs
        .iter()
        .flatten()
        .flat_map(|h| h.iter())
        .filter(|h| **h > 0)
        .count();
    need((1..16).contains(&lifted), || {
        format!("corner lift: {lifted} corners moved ({})", rig.ws.status)
    })
}

fn r_double_click_selects_the_floor(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let p = v.cell(&rig, [2, 2]);
    rig.click(p);
    rig.click(p);
    need(sel_len(&rig) >= 30, || {
        format!("double click selected {}", sel_len(&rig))
    })
}

fn r_esc_restores_and_second_esc_clears(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    v.click_cell(&mut rig, [2, 2]);
    let base = rig.ws.project.clone();
    let steps = history_steps(&rig);
    let a = v.cell(&rig, [4, 4]);
    drag_hold(&mut rig, a, a + Vec2::new(0.0, -raise_px(v)));
    need(rig.ws.project != base, || "the drag did not show".into())?;
    key(&mut rig, egui::Key::Escape);
    release(&mut rig, a + Vec2::new(0.0, -raise_px(v)));
    need(
        rig.ws.project == base && history_steps(&rig) == steps,
        || "Esc did not restore".into(),
    )?;
    key(&mut rig, egui::Key::Escape);
    need(sel_len(&rig) == 0, || {
        "a second Esc did not clear the selection".into()
    })
}

fn r_typed_amount(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let a = v.cell(&rig, [2, 2]);
    drag_hold(&mut rig, a, a + Vec2::new(0.0, -20.0));
    key(&mut rig, egui::Key::Minus);
    key(&mut rig, egui::Key::Num3);
    key(&mut rig, egui::Key::Enter);
    release(&mut rig, a + Vec2::new(0.0, -20.0));
    let h = corners(&rig, [2, 2]).ok_or("no floor")?;
    need(h == [-768; 4], || {
        format!("typed -3: {h:?} {}", rig.ws.status)
    })?;
    need(sel_len(&rig) >= 1, || "the selection was deleted".into())
}

fn r_repeat(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let a = v.cell(&rig, [2, 2]);
    rig.drag(a, a + Vec2::new(0.0, -raise_px(v)));
    let first = corners(&rig, [2, 2]).ok_or("no floor")?;
    v.click_cell(&mut rig, [4, 4]);
    key(&mut rig, egui::Key::R);
    let second = corners(&rig, [4, 4]).ok_or("no floor")?;
    need(first[0] > 0 && second == first, || {
        format!("R repeat: {first:?} then {second:?}: {}", rig.ws.status)
    })?;
    // The options bar's Repeat does the same.
    v.click_cell(&mut rig, [1, 4]);
    need(option_click(&mut rig, "Repeat"), || {
        "no Repeat in the options bar".into()
    })?;
    need(corners(&rig, [1, 4]) == Some(first), || {
        "Repeat button did nothing".into()
    })
}

fn r_raise_by_apply(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    v.click_cell(&mut rig, [2, 2]);
    set_field(&mut rig, "Raise by", "2");
    need(option_click(&mut rig, "Apply"), || "no Apply".into())?;
    let h = corners(&rig, [2, 2]).ok_or("no floor")?;
    need(h == [512; 4], || {
        format!("raise by 2: {h:?} {}", rig.ws.status)
    })
}

fn r_move_handle(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let (a, b) = (v.cell(&rig, [1, 1]), v.cell(&rig, [2, 2]));
    rig.modifiers = egui::Modifiers::SHIFT;
    rig.drag(a, b);
    rig.modifiers = egui::Modifiers::NONE;
    let view = if v.in_plan() {
        crate::grid_tools::view::GridView::Top(ViewportTransform::from_focus(
            rig.ws.last_orthographic_viewport_rect,
            OrthographicView::Top,
            [rig.ws.orthographic_focus[0], rig.ws.orthographic_focus[2]],
            rig.ws.viewport_zoom,
        ))
    } else {
        crate::grid_tools::view::GridView::Persp(rig.ws.last_persp_viewport_rect)
    };
    let h = crate::grid_tools::edit::move_handle_screen(&rig.ws, view).ok_or("no move handle")?;
    let before = rig.ws.project.clone();
    for cols in [6, 7, 8, 9] {
        let target = h + (v.cell(&rig, [2 + cols, 2]) - v.cell(&rig, [2, 2]));
        rig.drag(h, target);
        if rig.ws.project != before {
            break;
        }
    }
    need(rig.ws.project != before, || {
        format!("the handle moved nothing: {}", rig.ws.status)
    })
}

fn r_tilt_selection(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let p = v.cell(&rig, [2, 2]);
    rig.click(p);
    rig.click(p);
    need(sel_len(&rig) >= 30, || "the floor is not selected".into())?;
    // The east edge of the east tile.
    let step = v.cell(&rig, [5, 2]) - v.cell(&rig, [4, 2]);
    let e = if v.in_plan() {
        // Plan: the room's outline is a wall line to push; the tilt takes an
        // edge between two tiles.
        mid(v.cell(&rig, [4, 2]), v.cell(&rig, [5, 2]))
    } else {
        v.cell(&rig, [5, 2]) + step * 0.5 - step.normalized() * 4.0
    };
    let before = rig.ws.project.clone();
    rig.drag(e, e + Vec2::new(0.0, -raise_px(v)));
    let west = corners(&rig, [0, 2]).ok_or("no floor")?;
    let east = corners(&rig, [5, 2]).ok_or("no floor")?;
    need(rig.ws.project != before, || {
        format!("tilt changed nothing: {}", rig.ws.status)
    })?;
    need(east.iter().max() != west.iter().max(), || {
        format!("not tilted: west {west:?} east {east:?}")
    })
}

fn r_backtick_cycles(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let p = v.cell(&rig, [2, 2]);
    rig.move_to(p);
    let before = format!("{}|{}", rig.ws.status, rig.ws.edit_hint());
    press_key(&mut rig, egui::Key::Backtick, false);
    let after = format!("{}|{}", rig.ws.status, rig.ws.edit_hint());
    need(before != after, || {
        format!("backtick said nothing: {after}")
    })
}

// -- walls (3D only: they need the height) --

fn north_wall_px(rig: &mut WindowRig, v: View, up: f32) -> (Pos2, Pos2) {
    v.wall(rig, [2, 0], WallEdge::North, 0.5, 0.5, 0.0, up)
}

fn r_wall_push(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let (a, _) = north_wall_px(&mut rig, v, 600.0);
    let sector_px = (v.cell(&rig, [2, 2]) - v.cell(&rig, [2, 1])).length();
    let up = if v.in_plan() { sector_px } else { 40.0 };
    rig.drag(a, a + Vec2::new(0.0, -up));
    need(has_floor_at(&rig, [2, -1]), || {
        format!("wall push grew nothing: {}", rig.ws.status)
    })
}

/// A pixel on the north wall of [2, 0], `up` editor units above its foot, with
/// the camera standing back far enough to see the whole wall and the arrow
/// above it.
fn tall_wall_px(rig: &mut WindowRig, up: f32) -> Pos2 {
    face_wall(rig, [2, 0], WallEdge::North, 0.0, 6.0, 1500.0);
    px3(rig, wall_point(rig, [2, 0], WallEdge::North, 0.5, 0.0, up)).expect("the wall is on screen")
}

fn r_wall_arrow_slide(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let a = tall_wall_px(&mut rig, 1000.0);
    rig.click(a);
    let face = rig
        .ws
        .grid
        .selection
        .items()
        .iter()
        .find_map(|e| match e {
            psxed_project::grid::edit::GridElement::Face(f) => Some(*f),
            _ => None,
        })
        .ok_or("the wall is not selected")?;
    let p = crate::grid_tools::edit::arrow_screen(&rig.ws, rig.ws.last_persp_viewport_rect, face)
        .ok_or("no arrow")?;
    let before = rig.ws.project.clone();
    rig.drag(p, p + Vec2::new(0.0, -30.0));
    need(rig.ws.project != before, || {
        format!(
            "the arrow slid nothing: {} hint {} arrow {p:?}",
            rig.ws.status,
            rig.ws.edit_hint()
        )
    })
}

fn r_wall_top_stretch(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    // Just under the top edge of the wall, which stands at 2560 editor units.
    let a = tall_wall_px(&mut rig, 2520.0);
    let before = rig.ws.project.clone();
    rig.drag(a, a + Vec2::new(0.0, 25.0));
    need(rig.ws.project != before, || {
        format!(
            "the top edge stretched nothing: {} hint {} at {a:?}",
            rig.ws.status,
            rig.ws.edit_hint()
        )
    })
}

fn r_wall_foot_stretch(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let a = tall_wall_px(&mut rig, 40.0);
    let before = rig.ws.project.clone();
    rig.drag(a, a + Vec2::new(0.0, -25.0));
    need(rig.ws.project != before, || {
        format!("the foot stretched nothing: {}", rig.ws.status)
    })
}

// -- objects --




// -- the right-click menu and Play from here --

fn click_text_near(rig: &mut WindowRig, label: &str, p: Pos2) -> bool {
    let hit = texts(rig)
        .into_iter()
        .filter(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label)
        .min_by(|a, b| {
            a.1.center()
                .distance(p)
                .total_cmp(&b.1.center().distance(p))
        });
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 3);
            true
        }
        None => false,
    }
}

fn r_context_menu(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    let p = v.cell(&rig, [2, 2]);
    rig.secondary_click(p);
    let shown = |rig: &WindowRig| {
        texts(rig)
            .into_iter()
            .any(|(t, _)| t.trim() == "Play from here")
    };
    need(shown(&rig), || "a right click opened no menu".into())?;
    need(click_text_near(&mut rig, "Select room", p), || {
        "no Select room".into()
    })?;
    need(sel_len(&rig) >= 30, || {
        format!("Select room selected {}", sel_len(&rig))
    })?;
    let q = v.cell(&rig, [4, 4]);
    rig.secondary_click(q);
    need(shown(&rig), || {
        "the second right click opened no menu".into()
    })?;
    let before_delete = rig.ws.project.clone();
    need(click_text_near(&mut rig, "Delete", q), || {
        "no Delete".into()
    })?;
    need(rig.ws.project != before_delete, || {
        format!("Delete removed nothing: {}", rig.ws.status)
    })?;
    let r = v.cell(&rig, [1, 1]);
    rig.secondary_click(r);
    need(click_text_near(&mut rig, "Play from here", r), || {
        "no Play from here in the menu".into()
    })?;
    need(rig.ws.take_playtest_request().is_some(), || {
        "the menu's Play from here asked for no cook".into()
    })?;
    need(rig.ws.status.contains("Play from here"), || {
        rig.ws.status.clone()
    })
}

fn r_play_from_the_top_bar(v: View) -> Result<(), String> {
    let mut rig = erig(v);
    // The Play button's arrow is the last thing in the bar: find "Play" and
    // click the arrow just right of it.
    let play = texts(&rig)
        .into_iter()
        .filter(|(t, r)| t.trim().ends_with("Play") && r.center().y < 40.0)
        .map(|(_, r)| r)
        .next()
        .ok_or("no Play in the top bar")?;
    let mut done = false;
    for dx in [14.0, 22.0, 30.0, 40.0] {
        rig.click(Pos2::new(play.right() + dx, play.center().y));
        if click_text_lowest(&mut rig, "Play from here") {
            done = true;
            break;
        }
    }
    need(done, || "the Play arrow offers no Play from here".into())?;
    frames(&mut rig, 3);
    need(rig.ws.take_playtest_request().is_some(), || {
        format!("no cook asked: {}", rig.ws.status)
    })
}

pub(super) fn click_text_lowest(rig: &mut WindowRig, label: &str) -> bool {
    let hit = texts(rig)
        .into_iter()
        .filter(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label)
        .max_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y));
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 3);
            true
        }
        None => false,
    }
}

/// (row, the views it is replayed in, the replay)
pub(super) const ROWS: &[(&str, &[View], Row)] = &[
    (
        "click selects, a second click replaces",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_click_selects,
    ),
    (
        "Ctrl-click toggles, Shift-click adds a range",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_ctrl_toggles_shift_adds,
    ),
    (
        "floor drag raises one tile, one undo step, face selected",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_floor_drag_raises_one_step_and_undoes,
    ),
    (
        "press on an unselected tile moves only it",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_unselected_press_moves_only_that,
    ),
    (
        "press on a selected tile moves the selection",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_selected_press_moves_the_selection,
    ),
    (
        "box select from the void, Shift adds",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_box_from_void,
    ),
    (
        "edge drag tilts",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_edge_tilts,
    ),
    (
        "corner drag lifts one corner",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_corner_lifts,
    ),
    (
        "double click selects the room floor",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_double_click_selects_the_floor,
    ),
    (
        "Esc restores the drag, a second Esc clears",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_esc_restores_and_second_esc_clears,
    ),
    (
        "typed amount -3 Enter",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_typed_amount,
    ),
    (
        "R and Repeat repeat the last action",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_repeat,
    ),
    (
        "Raise by + Apply",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_raise_by_apply,
    ),
    (
        "move handle",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_move_handle,
    ),
    (
        "tilt of a selection",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_tilt_selection,
    ),
    (
        "backtick cycles candidates",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_backtick_cycles,
    ),
    (
        "wall body push",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_wall_push,
    ),
    (
        "wall up arrow slide (3D only: needs height)",
        &[View::ThreeD, View::BothPerspPane],
        r_wall_arrow_slide,
    ),
    (
        "wall top edge stretch (3D only)",
        &[View::ThreeD, View::BothPerspPane],
        r_wall_top_stretch,
    ),
    (
        "wall foot stretch (3D only)",
        &[View::ThreeD, View::BothPerspPane],
        r_wall_foot_stretch,
    ),
    (
        "right-click menu: Select room, Delete, Play from here",
        &[
            View::ThreeD,
            View::Plan,
            View::BothPlanPane,
            View::BothPerspPane,
        ],
        r_context_menu,
    ),
    (
        "Play from here in the Play menu",
        &[View::ThreeD, View::Plan],
        r_play_from_the_top_bar,
    ),
];

#[test]
fn the_edit_tool_decision_table_replays_from_the_strip_in_every_view() {
    let mut bad = Vec::new();
    for (name, views, row) in ROWS {
        for view in views.iter() {
            if let Err(e) = row(*view) {
                bad.push(format!("{name} [{view:?}]: {e}"));
            }
        }
    }
    assert!(bad.is_empty(), "{} rows:\n{}", bad.len(), bad.join("\n"));
}
