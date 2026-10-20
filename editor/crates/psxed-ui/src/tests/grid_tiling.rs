//! The Tiling section of the Inspector, drawn through the whole editor.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, draw_with_events, full_context};
use super::*;
use psxed_project::face_uv::{FaceUv, UvMode};
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::{edit_uv, FaceRef, GridElement, UvOp};
use psxed_project::grid::WallEdge;

fn text_of(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn inspector_text(ws: &mut EditorWorkspace) -> String {
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    text_of(&draw_editor(ws, &ctx, true))
}

fn workspace(label: &str) -> EditorWorkspace {
    let mut ws = EditorWorkspace::with_project(test_temp_dir(label), flat_project(5, 5, true));
    ws.apply_grid_pose("tool=select;select=floors:2,2:2,2")
        .unwrap();
    ws
}

fn set_floor_uv(ws: &mut EditorWorkspace, mode: UvMode) {
    let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
    edit_uv(world, FaceRef::floor(0, [2, 2]), None, UvOp::SetMode(mode));
    ws.grid_changed();
}

#[test]
fn a_default_face_reports_the_legacy_tiling() {
    let mut ws = workspace("tiling-default");
    let text = inspector_text(&mut ws);
    assert!(text.contains("Tiling"), "{text}");
    assert!(
        text.contains("effective: 0.5 x 0.5 tiles on this face"),
        "{text}"
    );
    for label in ["Default", "World", "Fit", "Turn", "Flip", "Offset"] {
        assert!(text.contains(label), "{label} missing:\n{text}");
    }
    assert!(!text.contains("cuts this face"), "{text}");
}

#[test]
fn fit_reports_tiles_and_the_cook_cut() {
    let mut ws = workspace("tiling-fit");
    set_floor_uv(&mut ws, UvMode::fit(1, 1));
    let text = inspector_text(&mut ws);
    assert!(text.contains("effective: 1 x 1 on this face"), "{text}");
    assert!(!text.contains("cuts this face"), "{text}");
    // A primitive repeats its texture up to four times per axis through the
    // texture window, so 2x2 and 4x4 are not cut.
    set_floor_uv(&mut ws, UvMode::fit(2, 2));
    let text = inspector_text(&mut ws);
    assert!(text.contains("effective: 2 x 2 on this face"), "{text}");
    assert!(!text.contains("cuts this face"), "{text}");
    set_floor_uv(&mut ws, UvMode::fit(4, 4));
    let text = inspector_text(&mut ws);
    assert!(text.contains("effective: 4 x 4 on this face"), "{text}");
    assert!(!text.contains("cuts this face"), "{text}");
    set_floor_uv(&mut ws, UvMode::fit(8, 8));
    let text = inspector_text(&mut ws);
    assert!(
        text.contains("cuts this face into 4 primitives (+3)"),
        "{text}"
    );
}

#[test]
fn a_derived_wall_reports_its_whole_run() {
    let mut ws = EditorWorkspace::with_project(
        test_temp_dir("tiling-run"),
        derived_project_for_tiling(5, 5),
    );
    let face = FaceRef::derived(0, [2, 0], WallEdge::North, DerivedKind::Closure);
    ws.grid.selection.set(vec![GridElement::Face(face)]);
    let text = inspector_text(&mut ws);
    // Five sectors long (2.5 tiles at half a tile per sector), three tall.
    assert!(
        text.contains("effective: 2.5 x 1.5 tiles on this face"),
        "{text}"
    );
    assert!(text.contains("derived"), "{text}");
    // Fit counts across the whole run, not per sector.
    {
        let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
        edit_uv(world, face, None, UvOp::SetMode(UvMode::fit(8, 1)));
    }
    ws.grid_changed();
    let text = inspector_text(&mut ws);
    assert!(text.contains("effective: 8 x 1 on this face"), "{text}");
    assert!(text.contains("2 primitives (+1)"), "{text}");
}

#[test]
fn planes_keep_tile_and_stretch_and_report_them() {
    let mut ws = workspace("tiling-plane");
    ws.apply_grid_pose("tool=plane").unwrap();
    {
        let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
        let plane = psxed_project::grid::GridPlane::new([2560, 256, 2560], [2048, 1024]);
        let id = psxed_project::grid::edit::add_plane(world, 0, plane).unwrap();
        ws.grid_changed();
        ws.select_planes(vec![id]);
    }
    let text = inspector_text(&mut ws);
    assert!(text.contains("Tile") && text.contains("Stretch"), "{text}");
    assert!(
        text.contains("effective: 1 x 0.5 tiles on this face"),
        "{text}"
    );
}

#[test]
fn the_fine_controls_stay_beside_the_model() {
    let mut ws = workspace("tiling-fine");
    {
        let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
        let face = FaceRef::floor(0, [2, 2]);
        edit_uv(world, face, None, UvOp::SetMode(UvMode::fit(4, 4)));
        edit_uv(world, face, None, UvOp::Turn(true));
        edit_uv(world, face, None, UvOp::ToggleFlip(0));
        edit_uv(world, face, None, UvOp::SetOffset([20, 0]));
    }
    ws.grid_changed();
    let text = inspector_text(&mut ws);
    assert!(text.contains("90 deg"), "{text}");
    // Offset 20 texels makes a 4x4 face cross a fifth tile: one more cut.
    assert!(text.contains("primitives"), "{text}");
    let uv: FaceUv = psxed_project::grid::edit::uv_of(
        ws.project.active_scene().world.grid().unwrap(),
        FaceRef::floor(0, [2, 2]),
        None,
    )
    .unwrap();
    assert_eq!((uv.turns, uv.flip), (1, [true, false]));
}

/// Click the widget whose label is `label` in the drawn editor. Labels like
/// "World" also name other things on screen; `row_of` picks the one on the
/// same row as another label (the Tiling mode buttons share a row).
fn click_label(ws: &mut EditorWorkspace, label: &str) {
    click_label_in_row(ws, label, None);
}

fn click_label_in_row(ws: &mut EditorWorkspace, label: &str, row_of: Option<&str>) {
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    let output = draw_editor(ws, &ctx, true);
    let texts: Vec<(Pos2, String)> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.pos + t.galley.size() / 2.0, t.galley.text().to_string()))
            }
            _ => None,
        })
        .collect();
    let row_y = row_of.map(|anchor| {
        texts
            .iter()
            .find(|(_, text)| text == anchor)
            .unwrap_or_else(|| panic!("no {anchor:?} on screen"))
            .0
            .y
    });
    let pos = texts
        .iter()
        // Below the top bar and the tool row, whose keys ("H") are labels too.
        .find(|(p, text)| {
            text == label && p.y > 90.0 && row_y.is_none_or(|y| (p.y - y).abs() < 3.0)
        })
        .unwrap_or_else(|| panic!("no {label:?} on screen"))
        .0;
    let press = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    draw_with_events(
        ws,
        &ctx,
        vec![egui::Event::PointerMoved(pos), press(true), press(false)],
    );
}

fn floor_uv(ws: &EditorWorkspace) -> FaceUv {
    psxed_project::grid::edit::uv_of(
        ws.project.active_scene().world.grid().unwrap(),
        FaceRef::floor(0, [2, 2]),
        None,
    )
    .unwrap()
}

#[test]
fn clicking_the_tiling_buttons_edits_the_selected_face() {
    let mut ws = workspace("tiling-click");
    // Default -> Fit, then a preset, a turn and a flip, each one click.
    click_label(&mut ws, "Fit");
    assert_eq!(floor_uv(&ws).mode, UvMode::fit(1, 1));
    click_label(&mut ws, "4x4");
    assert_eq!(floor_uv(&ws).mode, UvMode::fit(4, 4));
    click_label(&mut ws, ">");
    assert_eq!(floor_uv(&ws).turns, 1);
    click_label(&mut ws, "<");
    click_label(&mut ws, "<");
    assert_eq!(floor_uv(&ws).turns, 3);
    click_label(&mut ws, "H");
    assert_eq!(floor_uv(&ws).flip, [true, false]);
    // World: a plain click switches the mode back to the legacy density.
    click_label_in_row(&mut ws, "World", Some("Fit"));
    assert_eq!(floor_uv(&ws).mode, UvMode::LEGACY);
    // Reset puts the whole placement back.
    click_label(&mut ws, "Reset");
    assert!(floor_uv(&ws).is_identity());
}

#[test]
fn a_multi_face_selection_fits_across_the_whole_rectangle() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("tiling-across"), flat_project(6, 4, true));
    ws.apply_grid_pose("tool=select;select=floors:1,1:4,2")
        .unwrap();
    let text = inspector_text(&mut ws);
    assert!(text.contains("Fit across the selection"), "{text}");
    assert!(text.contains("Fit across 8 faces"), "{text}");
    // A single face has no patch to fit across.
    let mut single = workspace("tiling-across-single");
    assert!(!inspector_text(&mut single).contains("Fit across the selection"));
    click_label(&mut ws, "Fit across 8 faces");
    let world = ws.project.active_scene().world.grid().unwrap();
    let uv = |cell: [i32; 2]| {
        psxed_project::grid::edit::uv_of(world, FaceRef::floor(0, cell), None).unwrap()
    };
    // The default is 2 x 2 across the 4 x 2 patch: every face is fitted,
    // each widened by what the patch reaches past it.
    for z in 1..=2 {
        for x in 1..=4 {
            let u = uv([x, z]);
            assert_eq!(u.mode, UvMode::fit(2, 2), "cell {x},{z}");
            let want = [
                [(x - 1) * 1024, (z - 1) * 1024],
                [(4 - x) * 1024, (2 - z) * 1024],
            ];
            assert_eq!(u.grow, want, "cell {x},{z}");
        }
    }
    // A face outside the selection is untouched.
    assert!(uv([0, 0]).is_identity());
    // The face the Inspector shows reads back as part of the patch, and the
    // whole thing is one undo step.
    let text = inspector_text(&mut ws);
    assert!(text.contains("part of 2 x 2 across its patch"), "{text}");
    ws.do_undo();
    let world = ws.project.active_scene().world.grid().unwrap();
    assert!(
        psxed_project::grid::edit::uv_of(world, FaceRef::floor(0, [2, 2]), None)
            .unwrap()
            .is_identity()
    );
}
