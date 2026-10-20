//! The Materials palette's preview size (lane editor-palette-zoom,
//! 2026-10-20): the - and + buttons on the "All project materials" row,
//! Ctrl/Cmd with the wheel, the sizes, and that the size is remembered.
//!
//! Everything drives the whole editor window (`WindowRig`, real fonts, real
//! pointer events at 1456 x 819).

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, texts};
use super::*;
use crate::grid_tools::materials_panel::{DEFAULT_SWATCH, SWATCH_SIZES};

const MINUS: char = '\u{e11c}';
const PLUS: char = '\u{e13d}';

/// Captures for the review pages: `PSXED_PALETTE_CAPTURE=<dir>` writes the
/// dp-world window at the smallest, the default and the largest preview
/// size, in the World and the Material tab; without it this does nothing.
#[test]
fn palette_zoom_captures() {
    let Some(dir) = std::env::var_os("PSXED_PALETTE_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_PALETTE_LABEL").unwrap_or_else(|_| "after".into());
    for (size, name) in [
        (SWATCH_SIZES[0], "small"),
        (DEFAULT_SWATCH, "default"),
        (SWATCH_SIZES[4], "large"),
    ] {
        for (view, tab) in [
            (WorkspaceView::Room, "world"),
            (WorkspaceView::Material, "material"),
        ] {
            let mut rig = WindowRig::new();
            rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
            rig.ws.active_workspace = view;
            rig.ws.grid.palette_swatch = size;
            frames(&mut rig, 4);
            rig.capture(&dir.join(format!("palette-{tab}-{name}-1456x819-{label}.png")));
        }
    }
}

// ---------------------------------------------------------------------
// Helpers

fn rig() -> WindowRig {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    frames(&mut rig, 4);
    rig
}

/// A palette with one big group of materials (a real world has hundreds), so
/// a row has something to wrap.
fn crowded() -> WindowRig {
    let mut rig = rig();
    for n in 0..60 {
        let id = rig.ws.project.add_resource(
            format!("Zoom / {n}"),
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        );
        rig.ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .palette_pins
            .push(id);
    }
    rig.ws.grid.palette_search = "Zoom".to_string();
    frames(&mut rig, 4);
    rig
}

/// The text of the "All project materials" checkbox.
fn row_label(rig: &WindowRig) -> Rect {
    texts(rig)
        .into_iter()
        .find(|(t, _)| t == "All project materials")
        .map(|(_, r)| r)
        .expect("the All project materials row")
}

/// The icon button drawing `glyph` on the same row as the checkbox.
fn row_button(rig: &WindowRig, glyph: char) -> Rect {
    let row = row_label(rig);
    texts(rig)
        .into_iter()
        .find(|(t, r)| {
            t.chars().eq([glyph])
                && (r.center().y - row.center().y).abs() < 6.0
                && r.min.x > row.max.x
        })
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("no {glyph:?} button on the All project materials row"))
}

/// Every swatch of the last frame, by the square outline each one draws.
fn swatches(rig: &WindowRig) -> Vec<Rect> {
    let size = f32::from(rig.ws.grid.palette_swatch);
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Rect(r)
                if (r.rect.width() - size).abs() < 0.01
                    && (r.rect.height() - size).abs() < 0.01 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect()
}

fn wheel(rig: &mut WindowRig, at: Pos2, dy: f32, mods: egui::Modifiers) {
    rig.move_to(at);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, rig.size)),
        time: Some(rig.time + 1.0 / 60.0),
        focused: true,
        modifiers: mods,
        events: vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, dy),
            modifiers: mods,
        }],
        ..egui::RawInput::default()
    };
    rig.time += 1.0 / 60.0;
    let ws = &mut rig.ws;
    let viewport = rig.viewport.clone();
    rig.out = rig.ctx.run(input, |ctx| {
        ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
    });
    rig.textures.apply(&rig.out.textures_delta);
    frames(rig, 2);
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers::COMMAND
}

/// A point inside the palette: on its search box.
fn inside_palette(rig: &WindowRig) -> Pos2 {
    texts(rig)
        .into_iter()
        .find(|(t, _)| t == "Search materials")
        .map(|(_, r)| r.center())
        .expect("the search box")
}

// ---------------------------------------------------------------------
// The buttons

#[test]
fn the_all_materials_row_has_minus_and_plus_at_its_right_end() {
    let rig = rig();
    let label = row_label(&rig);
    let minus = row_button(&rig, MINUS);
    let plus = row_button(&rig, PLUS);
    assert!(label.max.x < minus.min.x, "{label:?} then {minus:?}");
    assert!(
        minus.max.x < plus.min.x,
        "- is left of +: {minus:?} {plus:?}"
    );
    assert!(
        rig.size.x - plus.max.x < 40.0,
        "+ hugs the dock edge: {plus:?}"
    );
}

#[test]
fn the_buttons_say_what_they_do_when_hovered() {
    let mut rig = rig();
    for (glyph, tip) in [(MINUS, "Smaller previews"), (PLUS, "Bigger previews")] {
        // Neither button is at its limit at the default size.
        rig.ws.grid.palette_swatch = DEFAULT_SWATCH;
        frames(&mut rig, 2);
        let at = row_button(&rig, glyph).center();
        rig.move_to(at);
        frames(&mut rig, 60);
        assert!(
            texts(&rig).iter().any(|(t, _)| t == tip),
            "no `{tip}` tooltip: {:?}",
            texts(&rig)
                .iter()
                .map(|(t, _)| t.clone())
                .collect::<Vec<_>>()
        );
        rig.move_to(Pos2::new(5.0, 5.0));
        frames(&mut rig, 3);
    }
}

#[test]
fn the_default_size_is_the_one_the_palette_always_had() {
    let rig = crowded();
    assert_eq!(rig.ws.grid.palette_swatch, 44);
    assert_eq!(DEFAULT_SWATCH, 44);
    assert!(
        swatches(&rig).len() > 3,
        "swatches of 44 pt are drawn: {}",
        swatches(&rig).len()
    );
}

#[test]
fn plus_and_minus_step_through_the_sizes_and_stop_at_both_ends() {
    let mut rig = rig();
    assert!((4..=6).contains(&SWATCH_SIZES.len()));
    // Up to the biggest.
    for expected in SWATCH_SIZES.iter().skip(3) {
        let at = row_button(&rig, PLUS).center();
        rig.click(at);
        assert_eq!(rig.ws.grid.palette_swatch, *expected);
        assert!(!swatches(&rig).is_empty(), "swatches of {expected}");
    }
    // At the biggest the + does nothing.
    let at = row_button(&rig, PLUS).center();
    rig.click(at);
    assert_eq!(rig.ws.grid.palette_swatch, *SWATCH_SIZES.last().unwrap());
    // Down to the smallest.
    for expected in SWATCH_SIZES.iter().rev().skip(1) {
        let at = row_button(&rig, MINUS).center();
        rig.click(at);
        assert_eq!(rig.ws.grid.palette_swatch, *expected);
    }
    let at = row_button(&rig, MINUS).center();
    rig.click(at);
    assert_eq!(rig.ws.grid.palette_swatch, SWATCH_SIZES[0]);
}

#[test]
fn the_grid_fills_the_dock_width_at_every_size() {
    let mut rig = crowded();
    for size in SWATCH_SIZES {
        rig.ws.grid.palette_swatch = size;
        frames(&mut rig, 3);
        let plus = row_button(&rig, PLUS);
        let found = swatches(&rig);
        assert!(!found.is_empty(), "swatches at {size}");
        // The + glyph sits inside its button's padding; the button's frame
        // is the dock's content edge.
        let edge = plus.max.x + 8.0;
        let right = found.iter().map(|r| r.max.x).fold(f32::MIN, f32::max);
        assert!(
            right <= edge,
            "{size}: swatch past the dock edge {edge}: {right}"
        );
        // The last column ends within one swatch of the edge: the row wrapped
        // to the full width instead of to a narrow column.
        assert!(
            edge - right < f32::from(size) + 4.0,
            "{size}: the grid stops {} short of the dock edge",
            edge - right
        );
    }
    // The smaller the swatch, the more fit on a row.
    let per_row = |rig: &mut WindowRig, size: u16| {
        rig.ws.grid.palette_swatch = size;
        frames(rig, 3);
        let found = swatches(rig);
        let top = found.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
        found.iter().filter(|r| (r.min.y - top).abs() < 1.0).count()
    };
    let (small, big) = (per_row(&mut rig, 24), per_row(&mut rig, 96));
    assert!(small > big, "{small} per row at 24, {big} at 96");
}

// ---------------------------------------------------------------------
// The wheel

#[test]
fn ctrl_or_cmd_with_the_wheel_over_the_palette_resizes_it() {
    let mut rig = rig();
    let at = inside_palette(&rig);
    wheel(&mut rig, at, 120.0, ctrl());
    assert_eq!(rig.ws.grid.palette_swatch, 64, "wheel up grows");
    let at = inside_palette(&rig);
    wheel(&mut rig, at, -120.0, ctrl());
    assert_eq!(rig.ws.grid.palette_swatch, 44, "wheel down shrinks");
    // At the ends it stays inside the sizes.
    for _ in 0..8 {
        let at = inside_palette(&rig);
        wheel(&mut rig, at, -120.0, ctrl());
    }
    assert_eq!(rig.ws.grid.palette_swatch, SWATCH_SIZES[0]);
    for _ in 0..12 {
        let at = inside_palette(&rig);
        wheel(&mut rig, at, 120.0, ctrl());
    }
    assert_eq!(rig.ws.grid.palette_swatch, *SWATCH_SIZES.last().unwrap());
}

#[test]
fn the_wheel_alone_or_away_from_the_palette_leaves_the_size() {
    let mut rig = rig();
    let at = inside_palette(&rig);
    wheel(&mut rig, at, 120.0, egui::Modifiers::NONE);
    assert_eq!(
        rig.ws.grid.palette_swatch, DEFAULT_SWATCH,
        "a plain wheel scrolls the list"
    );
    // Over the 3D view, Ctrl+wheel is the camera's, not the palette's.
    let view = rig.ws.grid.view_rect.get().expect("the 3D view").center();
    wheel(&mut rig, view, 120.0, ctrl());
    assert_eq!(rig.ws.grid.palette_swatch, DEFAULT_SWATCH);
}

// ---------------------------------------------------------------------
// Small swatches, both docks, remembered

#[test]
fn a_small_swatch_still_names_its_material_on_hover() {
    let mut rig = crowded();
    rig.ws.grid.palette_swatch = SWATCH_SIZES[0];
    frames(&mut rig, 3);
    let first = *swatches(&rig).first().expect("a swatch");
    rig.move_to(first.center());
    frames(&mut rig, 60);
    let names: Vec<String> =
        psxed_project::grid::palette::palette_materials(&rig.ws.project, false)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
    let shown = texts(&rig);
    assert!(
        shown
            .iter()
            .any(|(t, _)| names.iter().any(|n| t.starts_with(n.as_str()))),
        "no material name tooltip over {first:?}: {:?}",
        shown.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn the_material_tab_dock_has_the_same_buttons_and_the_same_size() {
    let mut rig = rig();
    let at = row_button(&rig, PLUS).center();
    rig.click(at);
    assert_eq!(rig.ws.grid.palette_swatch, 64);
    rig.ws.active_workspace = WorkspaceView::Material;
    frames(&mut rig, 4);
    assert!(
        !swatches(&rig).is_empty(),
        "swatches of 64 in the Material tab"
    );
    let minus = row_button(&rig, MINUS).center();
    rig.click(minus);
    assert_eq!(rig.ws.grid.palette_swatch, 44, "the - in the Material tab");
    rig.ws.active_workspace = WorkspaceView::Room;
    frames(&mut rig, 4);
    assert_eq!(rig.ws.grid.palette_swatch, 44, "the World tab follows");
    assert!(!swatches(&rig).is_empty());
}

#[test]
fn the_size_is_remembered_in_the_view_file() {
    let mut rig = rig();
    let at = row_button(&rig, PLUS).center();
    rig.click(at);
    assert_eq!(rig.ws.grid.palette_swatch, 64);
    rig.ws.save_view_state().expect("the view file");
    let mut reopened = EditorWorkspace::open_directory(rig.ws.project_dir.clone()).unwrap();
    assert_eq!(reopened.grid.palette_swatch, 64);
    // A size the editor does not offer snaps to the nearest one.
    let view = psxed_project::EditorViewState {
        palette_swatch: 70,
        ..psxed_project::EditorViewState::default()
    };
    reopened.apply_editor_view_state(view);
    assert_eq!(reopened.grid.palette_swatch, 64);
}

#[test]
fn a_view_file_from_before_the_setting_opens_at_the_default() {
    let mut rig = rig();
    let file = rig
        .ws
        .project_dir
        .join(psxed_project::EDITOR_VIEW_STATE_FILE);
    std::fs::write(&file, "()").unwrap();
    let reopened = EditorWorkspace::open_directory(rig.ws.project_dir.clone()).unwrap();
    assert_eq!(reopened.grid.palette_swatch, DEFAULT_SWATCH);
    frames(&mut rig, 1);
}
