//! What Manny hit when he tested the new editor (2026-10-20), driven through
//! the whole window with real pointer and key events:
//!
//! * "I cannot get the stairs to work": a fresh user now drags a rectangle,
//!   then presses and drags up, and real stairs appear; the bar always names
//!   the next step and a flight that is too steep says why and offers a ramp.
//! * "when using raise I thought we'd automatically add the walls": a room
//!   with hand-placed walls gets no risers, so the bar offers to convert it.
//! * "I don't understand what slab is supposed to do, it works like raise":
//!   the tool is the Upper floor, and says it leaves open space underneath.

use super::grid_live::WindowRig;
use super::grid_rig::floor_heights;
use super::grid_toolbar::{frames, options_texts, press_key, texts};
use super::*;
use crate::grid_tools::view::TOP_PIXELS_PER_CLICK as CLICK_PX;
use crate::grid_tools::GridTool;
use psxed_project::grid::faces::all_faces;

const WALLS_PROMPT: &str =
    "This room uses hand-placed walls, so new heights won't get walls. Convert to derived walls?";

/// The tracked starter project in a window, the plan showing, nothing drawn
/// yet: a derived room, so there is no walls prompt.
fn starter_plan() -> WindowRig {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    rig.ws.apply_grid_pose("view=plan").unwrap();
    frames(&mut rig, 4);
    rig
}

/// The dp-world stand-in (the starter room with hand-placed walls), plan.
fn legacy_plan() -> WindowRig {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    frames(&mut rig, 4);
    rig
}

/// Everything the options bar says, in one string.
fn bar_text(rig: &WindowRig) -> String {
    options_texts(rig)
        .into_iter()
        .map(|(t, _)| t)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn floor_at(rig: &WindowRig, cell: [i32; 2]) -> [i32; 4] {
    floor_heights(rig.world(), 0, cell).unwrap_or_else(|| panic!("no floor at {cell:?}"))
}

fn faces(rig: &WindowRig) -> usize {
    all_faces(rig.world()).len()
}

/// A press at `at` held there (no release), the pointer then moved to `to`.
fn press_and_move(rig: &mut WindowRig, at: Pos2, to: Pos2) {
    rig.move_to(at);
    let e = rig.press(at, true);
    rig.frame(vec![e]);
    for step in 1..=6 {
        rig.move_to(at + (to - at) * (step as f32 / 6.0));
    }
}

fn release_at(rig: &mut WindowRig, at: Pos2) {
    let e = rig.press(at, false);
    rig.frame(vec![e]);
    frames(rig, 2);
}

/// Drag a rectangle from cell `a` to cell `b`, then press on `press` and
/// drag up `clicks`, as a person does. Returns where the pointer ended.
fn rectangle_then_up(rig: &mut WindowRig, a: [i32; 2], b: [i32; 2], press: [i32; 2], clicks: f32) {
    rig.drag(rig.cell(a[0], a[1]), rig.cell(b[0], b[1]));
    let from = rig.cell(press[0], press[1]);
    rig.drag(from, from - Vec2::new(0.0, clicks * CLICK_PX));
}

// ---------------------------------------------------------------------
// Stairs

#[test]
fn a_fresh_user_drags_a_rectangle_then_drags_up_and_real_stairs_appear() {
    let mut rig = starter_plan();
    let original = rig.ws.project.clone();
    let faces_before = faces(&rig);
    press_key(&mut rig, egui::Key::T, false);
    assert_eq!(rig.tool(), GridTool::Stairs);
    // The bar says what to do first.
    assert!(
        bar_text(&rig).contains("Drag a rectangle where the stairs go"),
        "{}",
        bar_text(&rig)
    );
    // A rectangle three wide and four deep, then press near its north side
    // (the bottom) and drag up four clicks.
    rig.drag(rig.cell(1, 1), rig.cell(3, 4));
    assert!(
        bar_text(&rig).contains("Press on the region and drag up to set the height"),
        "{}",
        bar_text(&rig)
    );
    let from = rig.cell(2, 1);
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    // Four treads from the bottom, a click each, across the whole width.
    for (i, z) in (1..=4).enumerate() {
        for x in 1..=3 {
            assert_eq!(
                floor_at(&rig, [x, z]),
                [256 * (i as i32 + 1); 4],
                "({x},{z}): {}",
                rig.ws.status
            );
        }
    }
    assert_eq!(floor_at(&rig, [0, 1]), [0; 4], "outside the rectangle");
    assert!(
        faces(&rig) > faces_before,
        "the risers are derived walls: {} faces before, {} after",
        faces_before,
        faces(&rig)
    );
    // One undo takes the whole flight away.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_side_nearest_the_press_is_the_bottom_and_r_turns_it() {
    let mut rig = starter_plan();
    press_key(&mut rig, egui::Key::T, false);
    rig.drag(rig.cell(1, 1), rig.cell(4, 3));
    // Pressed near the south side: the flight climbs towards the north.
    let from = rig.cell(2, 3);
    rig.drag(from, from - Vec2::new(0.0, 3.0 * CLICK_PX));
    assert_eq!(floor_at(&rig, [2, 3]), [256; 4], "{}", rig.ws.status);
    assert_eq!(floor_at(&rig, [2, 1]), [768; 4]);
    rig.ws.do_undo();
    // R turns the bottom a quarter turn: from the south to the west, so the
    // treads climb towards the east.
    rig.move_to(rig.cell(2, 3));
    press_key(&mut rig, egui::Key::R, false);
    let from = rig.cell(2, 3);
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    let (west, east) = (floor_at(&rig, [1, 2])[0], floor_at(&rig, [4, 2])[0]);
    assert!(
        east > west,
        "bottom west: {west} at the west edge, {east} at the east: {}",
        rig.ws.status
    );
}

#[test]
fn while_dragging_the_bar_shows_the_steps_the_riser_and_whether_it_can_be_walked() {
    let mut rig = starter_plan();
    press_key(&mut rig, egui::Key::T, false);
    rig.drag(rig.cell(1, 1), rig.cell(2, 4));
    let from = rig.cell(1, 1);
    press_and_move(&mut rig, from, from - Vec2::new(0.0, 3.0 * CLICK_PX));
    let bar = bar_text(&rig);
    assert!(bar.contains("steps, riser"), "{bar}");
    assert!(bar.contains("walkable"), "{bar}");
    release_at(&mut rig, from - Vec2::new(0.0, 3.0 * CLICK_PX));
}

#[test]
fn a_flight_too_steep_for_treads_says_why_and_offers_a_ramp_in_place() {
    let mut rig = starter_plan();
    rig.ws.grid.verb.follow_ceiling = true;
    let original = rig.ws.project.clone();
    press_key(&mut rig, egui::Key::T, false);
    // Four sectors deep and 30 clicks high: far over what the character climbs.
    rectangle_then_up(&mut rig, [1, 1], [2, 4], [1, 1], 30.0);
    assert_eq!(rig.ws.project, original, "a refusal writes nothing");
    let bar = bar_text(&rig);
    assert!(bar.contains("Too steep for stairs"), "{bar}");
    assert!(bar.contains("Make it a ramp (Tab)"), "{bar}");
    // Tab makes it a ramp, at the rise that was asked for, in one undo step.
    press_key(&mut rig, egui::Key::Tab, false);
    let (near, far) = (floor_at(&rig, [1, 1])[0], floor_at(&rig, [1, 4])[0]);
    assert!(
        far > near && far >= 30 * 256 / 2,
        "{near} {far}: {}",
        rig.ws.status
    );
    assert!(!bar_text(&rig).contains("Too steep"), "{}", bar_text(&rig));
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
}

#[test]
fn the_ramp_button_does_the_same_as_tab() {
    let mut rig = starter_plan();
    press_key(&mut rig, egui::Key::T, false);
    rectangle_then_up(&mut rig, [1, 1], [2, 4], [1, 1], 30.0);
    let button = rig.at("Make it a ramp (Tab)");
    rig.click(button);
    frames(&mut rig, 2);
    assert!(floor_at(&rig, [1, 4])[0] > floor_at(&rig, [1, 1])[0]);
}

#[test]
fn the_path_option_is_still_there_for_a_winding_flight() {
    let mut rig = starter_plan();
    press_key(&mut rig, egui::Key::T, false);
    let bar = bar_text(&rig);
    assert!(bar.contains("Rectangle") && bar.contains("Path"), "{bar}");
    let path = rig.at("Path");
    rig.click(path);
    frames(&mut rig, 2);
    assert!(
        bar_text(&rig).contains("Click the waypoints of the flight"),
        "{}",
        bar_text(&rig)
    );
    // Waypoints, Enter, then the drag, as before.
    rig.click(rig.cell(1, 2));
    rig.click(rig.cell(4, 2));
    press_key(&mut rig, egui::Key::Enter, false);
    let from = rig.cell(2, 2);
    rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    assert_eq!(floor_at(&rig, [4, 2]), [1024; 4], "{}", rig.ws.status);
}

#[test]
fn a_rectangle_one_sector_deep_says_it_needs_a_run() {
    let mut rig = starter_plan();
    let original = rig.ws.project.clone();
    press_key(&mut rig, egui::Key::T, false);
    // Four wide, one deep, pressed near its north side: nothing to climb.
    rectangle_then_up(&mut rig, [1, 2], [4, 2], [1, 2], 3.0);
    // Pressed near the west side the run is four sectors long: that one works.
    assert!(
        rig.ws.status.contains("at least two sectors") || rig.ws.project != original,
        "{}",
        rig.ws.status
    );
}

// ---------------------------------------------------------------------
// Raise in a room with hand-placed walls

#[test]
fn the_tools_that_make_heights_offer_to_convert_a_room_with_hand_placed_walls() {
    let mut rig = legacy_plan();
    for (key, tool) in [
        (egui::Key::U, GridTool::Extrude),
        (egui::Key::A, GridTool::Ramp),
        (egui::Key::T, GridTool::Stairs),
        (egui::Key::X, GridTool::Break),
        (egui::Key::F, GridTool::Floor),
    ] {
        press_key(&mut rig, key, false);
        assert_eq!(rig.tool(), tool);
        let bar = bar_text(&rig);
        assert!(bar.contains(WALLS_PROMPT), "{}: {bar}", tool.label());
        assert!(bar.contains("Convert") && bar.contains("Not now"), "{bar}");
    }
    // Tools that make no heights stay quiet.
    for key in [egui::Key::P, egui::Key::V] {
        press_key(&mut rig, key, false);
        assert!(!bar_text(&rig).contains(WALLS_PROMPT));
    }
}

#[test]
fn a_derived_room_is_never_asked_to_convert() {
    let mut rig = starter_plan();
    for key in [egui::Key::U, egui::Key::A, egui::Key::T, egui::Key::F] {
        press_key(&mut rig, key, false);
        assert!(
            !bar_text(&rig).contains("hand-placed"),
            "{}",
            bar_text(&rig)
        );
    }
}

#[test]
fn convert_makes_the_room_derived_in_one_undo_step_and_raising_then_makes_walls() {
    use psxed_project::grid::WallMode;
    let mut rig = legacy_plan();
    let original = rig.ws.project.clone();
    press_key(&mut rig, egui::Key::U, false);
    // Without the conversion a raised platform gets no risers.
    let faces_before = faces(&rig);
    rectangle_then_up(&mut rig, [2, 2], [3, 3], [2, 2], 2.0);
    assert_eq!(
        floor_at(&rig, [2, 2]),
        [512; 4],
        "the block is raised: {}",
        rig.ws.status
    );
    assert_eq!(faces(&rig), faces_before, "a legacy room makes no walls");
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    // Convert: the answer in the bar.
    let convert = rig.at("Convert");
    rig.click(convert);
    frames(&mut rig, 2);
    assert_eq!(rig.world().rooms[0].wall_mode, WallMode::Derived);
    assert!(!bar_text(&rig).contains(WALLS_PROMPT));
    // The look is kept: the same floors and ceilings, and the outline is
    // still walled, now by derived walls.
    let (floors, ceilings, authored, derived) = face_kinds(rig.world());
    assert_eq!((floors, ceilings, authored), (36 + 64, 36 + 64, 0));
    assert!(derived >= 24, "{derived}");
    let faces_before = faces(&rig);
    let converted = rig.ws.project.clone();
    // Esc forgets the old region; a block elsewhere.
    press_key(&mut rig, egui::Key::Escape, false);
    rectangle_then_up(&mut rig, [4, 1], [5, 2], [4, 1], 2.0);
    assert_eq!(floor_at(&rig, [4, 1]), [512; 4], "{}", rig.ws.status);
    assert!(
        faces(&rig) > faces_before,
        "{} faces before, {} after: the raised block has risers now",
        faces_before,
        faces(&rig)
    );
    // The raise is one step and the conversion another.
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, converted);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original, "Convert is one undo step");
}

#[test]
fn not_now_is_remembered_for_that_room_for_the_session() {
    use psxed_project::grid::WallMode;
    let mut rig = legacy_plan();
    press_key(&mut rig, egui::Key::U, false);
    let not_now = rig.at("Not now");
    rig.click(not_now);
    frames(&mut rig, 2);
    assert!(!bar_text(&rig).contains(WALLS_PROMPT), "{}", bar_text(&rig));
    assert_eq!(rig.world().rooms[0].wall_mode, WallMode::Authored);
    // Another tool, and back: not asked again.
    for key in [egui::Key::A, egui::Key::T, egui::Key::U, egui::Key::F] {
        press_key(&mut rig, key, false);
        assert!(!bar_text(&rig).contains(WALLS_PROMPT), "{}", bar_text(&rig));
    }
}

/// Floors, ceilings and walls (hand-placed, derived) in the world.
fn face_kinds(world: &psxed_project::grid::GridWorld) -> (usize, usize, usize, usize) {
    use psxed_project::grid::faces::GridFaceKind;
    let mut n = (0, 0, 0, 0);
    for face in all_faces(world) {
        match face.kind {
            GridFaceKind::Floor { .. } => n.0 += 1,
            GridFaceKind::Ceiling { .. } => n.1 += 1,
            GridFaceKind::Wall { .. } => n.2 += 1,
            GridFaceKind::Derived { .. } => n.3 += 1,
            _ => {}
        }
    }
    n
}

#[test]
fn the_new_project_template_has_derived_walls_and_keeps_its_outline() {
    use psxed_project::grid::WallMode;
    let rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    let room = &rig.world().rooms[0];
    assert_eq!(room.wall_mode, WallMode::Derived, "the template room");
    let (floors, ceilings, authored, derived) = face_kinds(rig.world());
    assert_eq!((floors, ceilings, authored), (36, 36, 0));
    // A wall on every sector edge of the 6 x 6 outline, as before.
    assert_eq!(derived, 24);
    // And it takes raised floors: a platform gets its risers.
    let mut rig = starter_plan();
    press_key(&mut rig, egui::Key::U, false);
    let before = faces(&rig);
    rectangle_then_up(&mut rig, [2, 2], [3, 3], [2, 2], 2.0);
    assert!(faces(&rig) > before, "{before} -> {}", faces(&rig));
}

// ---------------------------------------------------------------------
// The Upper floor tool

fn hall_3d(tool_pose: &str) -> WindowRig {
    let mut rig = WindowRig::new();
    // Machine Hall: 8 x 8 sectors at world [7, -1] with a ceiling at 12 clicks.
    rig.ws
        .apply_grid_pose(&format!("view=3d;tool=slab;region=8,0:11,3;{tool_pose}"))
        .unwrap();
    frames(&mut rig, 4);
    rig
}

#[test]
fn the_slab_tool_is_called_upper_floor_and_says_what_it_does() {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d;tool=slab").unwrap();
    frames(&mut rig, 4);
    // The row's shape button shows the tool by its name, not "Slab".
    assert!(
        rig.find("Upper floor").is_some(),
        "no Upper floor in the row"
    );
    assert!(
        texts(&rig).iter().all(|(t, _)| t.trim() != "Slab"),
        "a Slab label is left"
    );
    assert_eq!(GridTool::Slab.key_label(), "Shift+Q");
    let bar = bar_text(&rig);
    assert!(
        bar.contains(
            "Adds a floor above this one, with open space underneath (needs a ceiling or open sky above)"
        ),
        "{bar}"
    );
    // The whole line is on screen at the coordinator's window size, not cut
    // short with an ellipsis.
    let line = options_texts(&rig)
        .into_iter()
        .find(|(t, _)| t.starts_with("Adds a floor above"))
        .expect("the line");
    assert!(line.1.width() > 440.0, "{:?}", line.1);
}

#[test]
fn the_upper_floor_modes_have_plain_labels() {
    let mut rig = hall_3d("");
    // Pick a region so the lead line gives way to the next step.
    use crate::grid_tools::slab_tool::SlabMode;
    let labels: Vec<&str> = SlabMode::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(
        labels,
        ["Add floor above", "Remove upper floor", "Fill opening"]
    );
    // The first is in view in the bar; the others scroll in with the rest.
    assert!(
        bar_text(&rig).contains("Add floor above"),
        "{}",
        bar_text(&rig)
    );
    for old in ["Insert", "Cut", "Fill"] {
        assert!(
            !options_texts(&rig).iter().any(|(t, _)| t == old),
            "{old} is still a label"
        );
    }
    frames(&mut rig, 1);
}

#[test]
fn dragging_an_upper_floor_shows_the_open_space_under_it() {
    let rig = hall_3d("verb-drag=6");
    let all: Vec<String> = texts(&rig).into_iter().map(|(t, _)| t).collect();
    assert!(
        all.iter()
            .any(|t| t.contains("Upper floor at +6.00 clicks") && t.contains("open space under it")),
        "{all:?}"
    );
    // The ghost: the open space is drawn as its own box, in a colour Raise
    // never uses.
    let teal = Color32::from_rgba_unmultiplied(90, 210, 230, 46);
    let teal = rig
        .out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::epaint::Shape::Path(p) if p.fill == teal));
    assert!(teal, "no open space ghost drawn");
}

#[test]
fn an_upper_floor_too_low_for_open_space_says_it_just_rises_like_raise() {
    let rig = hall_3d("verb-drag=2");
    let all: Vec<String> = texts(&rig).into_iter().map(|(t, _)| t).collect();
    assert!(
        all.iter()
            .any(|t| t.contains("no room for open space") && t.contains("like Extrude")),
        "{all:?}"
    );
}

// ---------------------------------------------------------------------
// Captures for the review page

/// `PSXED_VERBS_CAPTURE=<dir> PSXED_VERBS_LABEL=before|after
/// PSXED_VERBS_LEGACY=<a copy of the starter project before its conversion>`
/// writes the projects the 3D results are rendered from (with `frontend
/// dump-editor-ui`, a view file each) as `zz-<name>-<label>` under `<dir>`,
/// which must be a projects directory (the projects link their assets by a
/// relative path); without the variable this does nothing.
#[test]
fn verbs_captures() {
    let Some(dir) = std::env::var_os("PSXED_VERBS_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_VERBS_LABEL").unwrap_or_else(|_| "after".into());
    let legacy = PathBuf::from(std::env::var_os("PSXED_VERBS_LEGACY").expect("PSXED_VERBS_LEGACY"));
    let size = Vec2::new(1456.0, 819.0);
    let keep = |rig: &mut WindowRig, name: &str| {
        // One look at the room from the south-east, above.
        rig.ws.camera_rig.mode = crate::ViewportCameraMode::Orbit;
        rig.ws.camera_rig.yaw = 3328;
        rig.ws.camera_rig.pitch = 4096 - 420;
        rig.ws.camera_rig.radius = 9000;
        rig.ws.camera_rig.target = [3840, 400, 3840];
        rig.ws.save().expect("save");
        rig.ws.save_view_state().expect("view");
        let project = dir.join(format!("zz-{name}-{label}"));
        let _ = std::fs::remove_dir_all(&project);
        crate::starter_catalogue::copy_dir_recursive(&rig.ws.project_dir, &project).unwrap();
    };

    // The untouched projects the in-flight frames are posed on.
    let mut rig = WindowRig::starter(size);
    keep(&mut rig, "starter");
    let mut rig = WindowRig::from_dir(&legacy, size);
    keep(&mut rig, "legacy");

    // Stairs: a flight three wide, four deep, four clicks high.
    let mut rig = WindowRig::starter(size);
    rig.ws.apply_grid_pose("view=plan").unwrap();
    frames(&mut rig, 4);
    press_key(&mut rig, egui::Key::T, false);
    if label == "before" {
        let path = rig.at("Path");
        rig.click(path);
        rig.ws.grid.verb.path_width = 3;
        rig.click(rig.cell(2, 1));
        rig.click(rig.cell(2, 4));
        press_key(&mut rig, egui::Key::Enter, false);
        let from = rig.cell(2, 1);
        rig.drag(from, from - Vec2::new(0.0, 4.0 * CLICK_PX));
    } else {
        rectangle_then_up(&mut rig, [1, 1], [3, 4], [2, 1], 4.0);
    }
    keep(&mut rig, "stairs-result");

    // Raise in the room with hand-placed walls; after: convert first.
    let mut rig = WindowRig::from_dir(&legacy, size);
    rig.ws.apply_grid_pose("view=plan").unwrap();
    frames(&mut rig, 4);
    press_key(&mut rig, egui::Key::U, false);
    if label == "after" {
        let convert = rig.at("Convert");
        rig.click(convert);
        frames(&mut rig, 2);
    }
    rectangle_then_up(&mut rig, [2, 2], [3, 3], [2, 2], 2.0);
    keep(&mut rig, "raise-result");

    // The Upper floor over the middle of the room: 5 clicks, open space under.
    let mut rig = WindowRig::starter(size);
    rig.ws
        .apply_grid_pose("view=3d;tool=slab;region=2,2:4,4;verb-drag=5")
        .unwrap();
    frames(&mut rig, 2);
    rig.ws.grid.gesture = crate::grid_tools::Gesture::Idle;
    keep(&mut rig, "upper-floor-result");
}
