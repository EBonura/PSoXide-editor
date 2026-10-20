//! The Sculpt tool through real pointer input: the brush in both views, one
//! undo step a stroke, exact cancel, Shift, the modes, the detail option,
//! the face budget readout and the walkability overlay.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::sculpt_tool::Readout;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::edit::sculpt::{Detail, SculptMode, SculptReport};
use psxed_project::grid::limits::MAX_ROOM_FACES;

const SHIFT: egui::Modifiers = egui::Modifiers::SHIFT;

/// The highest and lowest floor corner of room 0.
fn range(rig: &GridRig) -> (i32, i32) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for (_, s) in rig.room(0).leaves() {
        for h in s.ground_floor().unwrap().heights {
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    (lo, hi)
}

fn rig_top() -> GridRig {
    let mut rig = GridRig::top("sculpt-top", flat_project(10, 10, false));
    rig.tool(GridTool::Sculpt);
    rig
}

/// Press at `from`, move through `to` and back a few times so the brush
/// dabs along the path, and keep the button down.
fn scrub(rig: &mut GridRig, from: Pos2, to: Pos2, mods: egui::Modifiers) {
    rig.move_to(from, mods);
    rig.button(from, true, mods);
    for pass in 0..3 {
        let (a, b) = if pass % 2 == 0 {
            (from, to)
        } else {
            (to, from)
        };
        for step in 1..=16 {
            rig.move_to(a + (b - a) * (step as f32 / 16.0), mods);
        }
    }
}

#[test]
fn a_stroke_raises_the_ground_under_the_brush_in_one_undo_step() {
    let mut rig = rig_top();
    let original = rig.ws.project.clone();
    let (from, to) = (rig.cell_centre(4, 5), rig.cell_centre(6, 5));
    scrub(&mut rig, from, to, egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Sculpt(_)));
    let (lo, hi) = range(&rig);
    assert_eq!(lo, 0);
    assert!(hi >= 64, "{}", rig.ws.status);
    assert!(
        rig.ws.status.starts_with("Sculpt Raise/Lower: Faces"),
        "{}",
        rig.ws.status
    );
    rig.release(to, egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    let sculpted = rig.ws.project.clone();
    assert_ne!(sculpted, original);
    // One undo step for the whole stroke.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(rig.ws.project, sculpted);
}

#[test]
fn escape_puts_the_document_back_exactly_and_shift_digs() {
    let mut rig = rig_top();
    let original = rig.ws.project.clone();
    let (from, to) = (rig.cell_centre(4, 5), rig.cell_centre(6, 5));
    scrub(&mut rig, from, to, egui::Modifiers::NONE);
    assert_ne!(rig.ws.project, original);
    rig.key(egui::Key::Escape);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(rig.ws.project, original);
    assert_eq!(
        rig.ws.project.to_ron_string().unwrap(),
        original.to_ron_string().unwrap()
    );
    // Undo has nothing of the cancelled stroke.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    // Shift lowers.
    scrub(&mut rig, from, to, SHIFT);
    rig.release(to, SHIFT);
    let (lo, hi) = range(&rig);
    assert!(lo <= -64 && hi == 0, "{lo} {hi}: {}", rig.ws.status);
}

#[test]
fn the_3d_view_sculpts_the_ground_under_the_pointer() {
    let mut rig = GridRig::persp("sculpt-3d", flat_project(10, 10, false));
    rig.tool(GridTool::Sculpt);
    let original = rig.ws.project.clone();
    let at = |rig: &GridRig, x: f32, z: f32| rig.screen_of([x * S, 0.0, z * S]);
    let (from, to) = (at(&rig, 4.5, 5.0), at(&rig, 6.5, 5.0));
    // The ring follows the pointer on the surface while hovering.
    rig.move_to(from, egui::Modifiers::NONE);
    let ghost = rig.ws.grid.sculpt.ghost.expect("a brush ring");
    assert!((ghost.at[0] - 18.0).abs() < 1.5 && (ghost.at[1] - 20.0).abs() < 1.5);
    scrub(&mut rig, from, to, egui::Modifiers::NONE);
    assert_ne!(rig.ws.project, original, "{}", rig.ws.status);
    rig.release(to, egui::Modifiers::NONE);
    assert!(range(&rig).1 >= 64);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn ramp_and_dunes_follow_the_drag() {
    let mut rig = rig_top();
    rig.ws.grid.sculpt.params.mode = SculptMode::Ramp;
    rig.ws.grid.sculpt.params.rise = 1024;
    let (a, b) = (rig.top_at(2.0 * S, 5.0 * S), rig.top_at(8.0 * S, 5.0 * S));
    rig.drag(a, b);
    let (lo, hi) = range(&rig);
    assert_eq!(lo, 0);
    assert_eq!(hi, 1024, "{}", rig.ws.status);
    rig.ws.do_undo();
    // Dunes: crests and troughs along the drag.
    let mut dunes = rig_top();
    dunes.ws.grid.sculpt.params.mode = SculptMode::Dunes;
    dunes.ws.grid.sculpt.params.wavelength = 12.0;
    dunes.ws.grid.sculpt.params.dune_height = 256;
    let (a, b) = (
        dunes.top_at(1.0 * S, 5.0 * S),
        dunes.top_at(9.0 * S, 5.0 * S),
    );
    dunes.drag(a, b);
    let (lo, hi) = range(&dunes);
    assert!(lo < 0 && hi > 0, "{lo} {hi}: {}", dunes.ws.status);
}

#[test]
fn auto_detail_divides_cells_and_the_readout_counts_the_faces() {
    let mut rig = rig_top();
    rig.ws.grid.sculpt.params.detail = Detail::Auto;
    rig.ws.grid.sculpt.params.radius = 3.0;
    rig.ws.grid.sculpt.params.strength = 1.0;
    let leaves = rig.room(0).leaf_count();
    let at = rig.cell_centre(5, 5);
    scrub(
        &mut rig,
        at,
        at + Vec2::new(30.0, 0.0),
        egui::Modifiers::NONE,
    );
    rig.release(at, egui::Modifiers::NONE);
    assert!(rig.room(0).leaf_count() > leaves, "{}", rig.ws.status);
    let readout = rig.ws.grid.sculpt.last.expect("a readout");
    assert!(readout.faces[1] > readout.faces[0], "{readout:?}");
    assert!(
        readout.faces_text().contains('+'),
        "{}",
        readout.faces_text()
    );
    assert!(rig.ws.status.contains("cells divided"), "{}", rig.ws.status);
}

#[test]
fn the_face_budget_warns_orange_at_85_percent_and_red_at_the_cap() {
    let at = |faces: usize| Readout {
        room: 0,
        faces: [0, faces],
        report: SculptReport::default(),
    };
    assert_eq!(at(1000).warning(), None);
    // The strip's budget chip turns at the same whole percent.
    let from = (0..MAX_ROOM_FACES)
        .find(|n| n * 100 / MAX_ROOM_FACES >= 85)
        .unwrap();
    assert_eq!(at(from - 1).warning(), None);
    assert_eq!(at(from).warning(), Some(false));
    assert_eq!(at(MAX_ROOM_FACES).warning(), Some(true));
    assert!(at(14000).faces_text().contains(&MAX_ROOM_FACES.to_string()));
}

#[test]
fn the_walkability_overlay_is_on_while_sculpting_and_the_keys_work() {
    let mut rig = GridRig::top("sculpt-keys", flat_project(10, 10, false));
    assert!(!rig.ws.walk_overlay_on());
    rig.key(egui::Key::S);
    assert_eq!(rig.ws.grid.tool, GridTool::Sculpt);
    assert!(rig.ws.walk_overlay_on());
    let radius = rig.ws.grid.sculpt.params.radius;
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.ws.grid.sculpt.params.radius, radius + 1.0);
    rig.key(egui::Key::OpenBracket);
    rig.key(egui::Key::OpenBracket);
    assert_eq!(rig.ws.grid.sculpt.params.radius, radius - 1.0);
    rig.key(egui::Key::Tab);
    assert_eq!(rig.ws.grid.sculpt.params.mode, SculptMode::Smooth);
    rig.key(egui::Key::V);
    assert!(!rig.ws.walk_overlay_on());
    assert!(rig.ws.apply_grid_pose("tool=sculpt").is_ok());
    assert_eq!(rig.ws.grid.tool, GridTool::Sculpt);
}

#[test]
fn the_option_row_lists_the_modes_detail_and_the_face_count() {
    use super::grid_ui::{draw_editor, full_context};
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("sculpt-options"), flat_project(10, 10, false));
    ws.last_viewport_size = Vec2::new(1400.0, 700.0);
    ws.set_grid_tool(GridTool::Sculpt);
    let output = draw_editor(&mut ws, &ctx, true);
    let text: String = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Raise/Lower",
        "Smooth",
        "Flatten",
        "Ramp",
        "Dunes",
        "Detail Sector",
        "falloff Smooth",
        "Floor",
    ] {
        assert!(text.contains(needle), "{needle}:\n{text}");
    }
}
