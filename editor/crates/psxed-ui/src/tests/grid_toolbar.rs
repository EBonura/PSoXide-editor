//! The top of the grid editor: the top bar, the tool row, the viewport
//! header and the options bar (the layout Manny approved on 2026-10-20).
//!
//! Everything here drives the whole editor window (`WindowRig`, real fonts,
//! real pointer events), so a button that is drawn but cannot be clicked, or
//! two buttons on top of each other, fail.

use super::grid_live::WindowRig;
use super::*;

/// Captures for the review pages: `PSXED_TOOLBAR_CAPTURE=<dir>
/// PSXED_TOOLBAR_LABEL=before|after` writes the whole window of both worlds
/// at both window sizes; without the variable this does nothing.
#[test]
fn toolbar_captures() {
    let Some(dir) = std::env::var_os("PSXED_TOOLBAR_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_TOOLBAR_LABEL").unwrap_or_else(|_| "after".into());
    for (world, tag) in [(0, "dp-world"), (1, "grid-starter")] {
        for size in [Vec2::new(1456.0, 819.0), Vec2::new(1920.0, 1080.0)] {
            let mut rig = if world == 0 {
                WindowRig::with_size(size)
            } else {
                WindowRig::starter(size)
            };
            rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
            for _ in 0..4 {
                rig.frame(vec![]);
            }
            rig.capture(&dir.join(format!(
                "{tag}-{}x{}-{label}.png",
                size.x as u32, size.y as u32
            )));
        }
    }
}

// ---------------------------------------------------------------------
// Helpers

/// Every text the last frame drew, with its bounds.
fn texts(rig: &WindowRig) -> Vec<(String, Rect)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.text().to_string(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect()
}

/// The first text `label` (icon prefix ignored) inside `band`.
fn text_in(rig: &WindowRig, band: Rect, label: &str) -> Option<Rect> {
    texts(rig)
        .into_iter()
        .find(|(t, r)| {
            band.contains(r.center())
                && t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label
        })
        .map(|(_, r)| r)
}

/// The window's top bar.
fn bar(rig: &WindowRig) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(rig.size.x, ACTION_BAR_COMPACT_HEIGHT))
}

fn in_bar(rig: &WindowRig, label: &str) -> Rect {
    text_in(rig, bar(rig), label).unwrap_or_else(|| panic!("no `{label}` in the top bar"))
}

fn frames(rig: &mut WindowRig, n: usize) {
    for _ in 0..n {
        rig.frame(vec![]);
    }
}

/// Filled rectangles of the last frame inside `band`: `(rect, fill)`.
fn fills_in(rig: &WindowRig, band: Rect) -> Vec<(Rect, Color32)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Rect(r) if band.contains(r.rect.center()) => {
                Some((r.rect, r.fill))
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------
// Step 1: the top bar. One row: the project (name, unsaved dot) and the
// menus on the left, the workspaces in the middle, Undo, Redo, Build and
// Play on the right; Play is the only filled button.

const TOP_BAR_ORDER: [&str; 13] = [
    "File",
    "Edit",
    "View",
    "Tools",
    "Help",
    "World",
    "HUD",
    "Animation",
    "Material",
    "Undo",
    "Redo",
    "Build",
    "Play",
];

#[test]
fn the_top_bar_is_one_row_in_the_approved_order() {
    for width in [1280.0, 1456.0, 1920.0] {
        let rig = WindowRig::with_size(Vec2::new(width, 819.0));
        let name = rig.ws.project.name.clone();
        let mut order: Vec<(&str, Rect)> = vec![(name.as_str(), in_bar(&rig, &name))];
        for label in TOP_BAR_ORDER {
            order.push((label, in_bar(&rig, label)));
        }
        let row_y = order[0].1.center().y;
        for (label, r) in &order {
            assert!(
                (r.center().y - row_y).abs() < 4.0,
                "{width}: `{label}` is on another row: {r:?}"
            );
        }
        for pair in order.windows(2) {
            let ((a, ra), (b, rb)) = (pair[0], pair[1]);
            assert!(
                ra.max.x < rb.min.x,
                "{width}: `{a}` {ra:?} is not left of `{b}` {rb:?}"
            );
        }
    }
}

#[test]
fn the_workspaces_are_a_control_centred_in_the_bar() {
    for width in [1280.0, 1456.0, 1920.0] {
        let rig = WindowRig::with_size(Vec2::new(width, 819.0));
        let first = in_bar(&rig, "World");
        let last = in_bar(&rig, "Material");
        let middle = (first.min.x + last.max.x) / 2.0;
        assert!(
            (middle - width / 2.0).abs() < 24.0,
            "{width}: the workspaces are centred on {middle}"
        );
    }
}

#[test]
fn the_unsaved_dot_replaces_the_badge() {
    let mut rig = WindowRig::new();
    let name = rig.ws.project.name.clone();
    let dots = |rig: &WindowRig| -> Vec<Pos2> {
        rig.out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Circle(c)
                    if c.radius <= 5.0 && bar(rig).contains(c.center) =>
                {
                    Some(c.center)
                }
                _ => None,
            })
            .collect()
    };
    assert!(!rig.ws.dirty, "the fixture starts saved");
    assert!(dots(&rig).is_empty(), "a saved project shows no dot");
    rig.ws.mark_dirty();
    frames(&mut rig, 2);
    let name_rect = in_bar(&rig, &name);
    let dots = dots(&rig);
    assert_eq!(dots.len(), 1, "{dots:?}");
    assert!(
        dots[0].x > name_rect.max.x && dots[0].x < name_rect.max.x + 24.0,
        "the dot {:?} follows the name {name_rect:?}",
        dots[0]
    );
    let anywhere = texts(&rig)
        .into_iter()
        .any(|(t, _)| t.to_uppercase().contains("UNSAVED"));
    assert!(!anywhere, "the badge is gone");
}

#[test]
fn play_is_the_only_filled_button_of_the_bar() {
    let rig = WindowRig::new();
    let fills = fills_in(&rig, bar(&rig));
    let play = in_bar(&rig, "Play");
    let play_fill = fills
        .iter()
        .find(|(r, _)| r.contains(play.center()) && r.width() < 140.0)
        .map(|(_, c)| *c)
        .expect("a fill under Play");
    assert_eq!(play_fill, STUDIO_PLAY_FILL);
    for label in TOP_BAR_ORDER.iter().filter(|l| **l != "Play") {
        let at = in_bar(&rig, label).center();
        assert!(
            !fills
                .iter()
                .any(|(r, c)| *c == STUDIO_PLAY_FILL && r.contains(at)),
            "`{label}` sits on the Play colour"
        );
    }
}

#[test]
fn the_workspace_control_switches_the_workspace() {
    let mut rig = WindowRig::new();
    for (label, view) in [
        ("HUD", WorkspaceView::Ui),
        ("Animation", WorkspaceView::Animation),
        ("Material", WorkspaceView::Material),
        ("World", WorkspaceView::Room),
    ] {
        let at = in_bar(&rig, label).center();
        rig.click(at);
        frames(&mut rig, 2);
        assert_eq!(rig.ws.active_workspace, view, "{label}");
    }
}

#[test]
fn undo_redo_build_and_play_work_from_the_bar() {
    let mut rig = WindowRig::new();
    let before = rig.world().clone();
    rig.draw_floor([6, 8], [6, 9]);
    assert_ne!(*rig.world(), before);
    frames(&mut rig, 2);
    let undo = in_bar(&rig, "Undo").center();
    rig.click(undo);
    assert_eq!(*rig.world(), before, "Undo");
    frames(&mut rig, 2);
    let redo = in_bar(&rig, "Redo").center();
    rig.click(redo);
    assert_ne!(*rig.world(), before, "Redo");
    frames(&mut rig, 2);
    let build = in_bar(&rig, "Build").center();
    rig.click(build);
    assert!(matches!(
        rig.ws.pending_playtest_request,
        Some(EditorPlaytestRequest::BuildProject)
    ));
    rig.ws.pending_playtest_request = None;
    let play = in_bar(&rig, "Play").center();
    rig.click(play);
    assert!(matches!(
        rig.ws.pending_playtest_request,
        Some(EditorPlaytestRequest::Play)
    ));
}

#[test]
fn the_play_menu_keeps_its_arrow() {
    let mut rig = WindowRig::new();
    let play = in_bar(&rig, "Play");
    let fills = fills_in(&rig, bar(&rig));
    // The arrow is the filled segment right of the Play face.
    let arrow = fills
        .iter()
        .filter(|(r, c)| *c == STUDIO_PLAY_FILL && r.min.x > play.max.x)
        .map(|(r, _)| r.center())
        .next()
        .expect("an arrow segment beside Play");
    rig.click(arrow);
    frames(&mut rig, 2);
    assert!(rig.find("UI Scenes").is_some(), "the boot menu opened");
}

#[test]
fn clicking_the_project_name_renames_it() {
    let mut rig = WindowRig::new();
    let name = rig.ws.project.name.clone();
    let at = in_bar(&rig, &name).center();
    rig.click(at);
    frames(&mut rig, 2);
    assert!(rig.ws.project_name_editing);
}

#[test]
fn a_running_build_shows_in_the_bar_without_moving_anything() {
    let mut rig = WindowRig::new();
    let play_idle = in_bar(&rig, "Play");
    let ctx = rig.ctx.clone();
    let viewport = rig.viewport.clone();
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, rig.size)),
            time: Some(10.0),
            ..egui::RawInput::default()
        },
        |ctx| {
            rig.ws
                .draw(ctx, viewport.clone(), EditorPlaytestStatus::Building)
        },
    );
    rig.out = out;
    assert!(
        text_in(&rig, bar(&rig), "BUILDING").is_some(),
        "the state is shown while it is not idle"
    );
    let chip = in_bar(&rig, "BUILDING");
    assert!(
        chip.min.x > in_bar(&rig, "Material").max.x + 8.0,
        "{chip:?}"
    );
    let play = in_bar(&rig, "Rebuild & Play");
    assert!((play.max.x - play_idle.max.x).abs() < 2.0, "{play:?}");
}
