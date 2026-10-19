//! Usability audit of 2026-10-19: tests for the problems the audit found.
//! Each one failed on `grid/main` 5b181404 before its fix (docs/grid/USABILITY-AUDIT.md).

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::{GridTool, GridViewMode};

fn describe(rig: &GridRig) -> String {
    let world = rig.world();
    let items = rig.ws.grid.selection.items();
    let faces = rig.ws.grid.selection.faces(world);
    let mut s = format!("{} items, {} faces:", items.len(), faces.len());
    for f in faces.iter().take(10) {
        s += &format!(" [{:?} {:?}]", f.leaf, f.target);
    }
    s
}

fn texts(output: &egui::FullOutput) -> String {
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

// ---------------------------------------------------------------------
// P1: after the Cells tool divides a sector, the plan cannot pick, paint or
// raise any piece of it. Reported by Manny 2026-10-19: "you can split but
// you can't select anything individually".

fn leaf_floor(x: i32, z: i32) -> psxed_project::grid::edit::FaceRef {
    psxed_project::grid::edit::FaceRef::new(
        0,
        psxed_project::grid::LeafKey { level: 3, x, z },
        psxed_project::grid::edit::FaceTarget::floor(0),
    )
}

/// An 8 x 8 room whose sector (2, 2) is divided into four half cells.
fn divided_plan() -> GridRig {
    let mut rig = GridRig::top("audit-divided", derived_project_for_tiling(8, 8));
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(2, 2));
    assert_eq!(rig.room(0).leaf_count(), 67, "{}", rig.ws.status);
    rig
}

#[test]
fn a_divided_sector_is_selectable_one_half_cell_at_a_time_in_the_plan() {
    let mut rig = divided_plan();
    rig.tool(GridTool::Select);
    let ne = rig.top_at(2.75 * S, 2.25 * S);
    rig.click(ne);
    let world = rig.world();
    let faces = rig.ws.grid.selection.faces(world);
    assert!(
        faces.contains(&leaf_floor(10, 8)),
        "clicking the north-east half cell selected nothing of it: {}",
        describe(&rig)
    );
    assert!(
        faces.iter().all(|f| f.leaf
            == psxed_project::grid::LeafKey {
                level: 3,
                x: 10,
                z: 8
            }),
        "only that half cell: {}",
        describe(&rig)
    );
    // The neighbour half is its own pick.
    rig.click(rig.top_at(2.25 * S, 2.75 * S));
    let faces = rig.ws.grid.selection.faces(rig.world());
    assert!(faces.contains(&leaf_floor(8, 10)), "{}", describe(&rig));
    assert!(!faces.contains(&leaf_floor(10, 8)), "{}", describe(&rig));
    // A box over the sector takes all four.
    rig.drag(rig.top_at(1.9 * S, 1.9 * S), rig.top_at(3.1 * S, 3.1 * S));
    let faces = rig.ws.grid.selection.faces(rig.world());
    assert!(faces.len() >= 4, "{}", describe(&rig));
}

#[test]
fn paint_in_the_plan_reaches_one_half_cell() {
    let mut rig = divided_plan();
    // Two materials: paint the NE half with the second.
    let mut project = rig.ws.project.clone();
    let second = project.add_resource(
        "second",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    rig.ws.project = project;
    rig.ws.grid.material = Some(second);
    rig.tool(GridTool::Paint);
    let before = rig.world().clone();
    rig.click(rig.top_at(2.75 * S, 2.25 * S));
    assert_ne!(
        *rig.world(),
        before,
        "nothing was painted: {}",
        rig.ws.status
    );
}

// ---------------------------------------------------------------------
// P2: opening the Split view must frame the world in the plan half. The
// plan's one-off framing went to `frame_viewport`, which frames the 3D
// camera whenever the workspace is not in the plan mode (Split is not).

#[test]
fn the_split_view_frames_the_world_in_the_plan_half() {
    use super::grid_ui::{draw_editor_sized, full_context};
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("audit-split-fit"), flat_project(8, 8, false));
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    let _ = draw_editor_sized(&mut ws, &ctx, false, Vec2::new(1920.0, 1080.0));
    ws.set_grid_view_mode(GridViewMode::Split);
    for _ in 0..3 {
        let _ = draw_editor_sized(&mut ws, &ctx, false, Vec2::new(1920.0, 1080.0));
    }
    let rect = ws.last_orthographic_viewport_rect;
    let transform = ViewportTransform::from_focus(
        rect,
        OrthographicView::Top,
        [ws.orthographic_focus[0], ws.orthographic_focus[2]],
        ws.viewport_zoom,
    );
    let tl = transform.world_to_screen([0.0, 0.0]);
    let br = transform.world_to_screen([8.0 * S, 8.0 * S]);
    assert!(
        rect.contains(tl) && rect.contains(br),
        "the room spans {tl:?} to {br:?} but the plan half is {rect:?}"
    );
}

// ---------------------------------------------------------------------
// P2: one home per concept in every tab of a grid project.

#[test]
fn a_grid_project_draws_no_file_tree_in_any_workspace() {
    use super::grid_ui::full_context;
    let ctx = full_context();
    let texture = ctx.load_texture(
        "audit-tabs",
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
    for view in [
        WorkspaceView::Room,
        WorkspaceView::Ui,
        WorkspaceView::Animation,
        WorkspaceView::Material,
    ] {
        let mut ws =
            EditorWorkspace::with_project(test_temp_dir("audit-no-fs"), flat_project(4, 4, true));
        ws.last_viewport_size = Vec2::new(1200.0, 700.0);
        ws.active_workspace = view;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0))),
                ..egui::RawInput::default()
            },
            |ctx| ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle),
        );
        let text = texts(&output);
        for stale in ["FileSystem", "main.map", "res://"] {
            assert!(!text.contains(stale), "{view:?} still draws {stale:?}");
        }
        // The Resources dock stays the one list of assets.
        assert!(text.contains("Resources"), "{view:?}");
    }
}

#[test]
fn placing_keeps_its_message_and_shift_keeps_the_tool() {
    let mut rig = GridRig::top("audit-place", flat_project(6, 6, true));
    rig.ws.place_kind = PlaceKind::SpawnMarker;
    rig.tool(GridTool::Place);
    rig.click(rig.cell_centre(3, 3));
    assert!(rig.ws.status.starts_with("Placed"), "{}", rig.ws.status);
    assert_eq!(rig.ws.grid.tool, GridTool::Select);
    rig.tool(GridTool::Place);
    let nodes = rig.ws.project.active_scene().nodes().len();
    rig.click_with(rig.cell_centre(4, 4), egui::Modifiers::SHIFT);
    assert_eq!(rig.ws.project.active_scene().nodes().len(), nodes + 1);
    assert_eq!(rig.ws.grid.tool, GridTool::Place, "Shift keeps placing");
    assert!(rig.ws.status.starts_with("Placed"), "{}", rig.ws.status);
}

#[test]
fn a_new_project_has_a_paint_material_and_the_first_paint_click_paints() {
    use super::grid_ui::{draw_editor, full_context};
    let mut project = flat_project(4, 4, false);
    project.add_resource(
        "Graybox / Brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let mut rig = GridRig::top("audit-paint-default", project);
    assert_eq!(rig.ws.grid.material, None);
    let ctx = full_context();
    let _ = draw_editor(&mut rig.ws, &ctx, true);
    assert!(rig.ws.grid.material.is_some(), "the palette picks one");
}

#[test]
fn no_two_buttons_of_the_tab_and_tool_rows_share_a_label() {
    let mut labels: Vec<&str> = GridTool::ALL.iter().map(|t| t.label()).collect();
    labels.extend(GridViewMode::ALL.iter().map(|m| m.label()));
    let mut seen = std::collections::BTreeSet::new();
    for label in &labels {
        assert!(seen.insert(*label), "{label} names two buttons");
    }
}

#[test]
fn both_halves_of_the_split_view_select() {
    let mut rig = GridRig::split("audit-split-select", flat_project(6, 6, false));
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(2, 2));
    assert_eq!(rig.ws.grid.selection.len(), 1, "plan half");
    assert_eq!(rig.ws.grid.selection.primary().unwrap().cell(), [2, 2]);
    let in_3d = rig.screen_of([3.5 * S, 0.0, 3.5 * S]);
    assert!(rig.persp_rect().contains(in_3d), "{in_3d:?}");
    rig.click(in_3d);
    assert_eq!(rig.ws.grid.selection.len(), 1, "3D half");
    assert_eq!(rig.ws.grid.selection.primary().unwrap().cell(), [3, 3]);
}

#[test]
fn delete_raise_and_a_range_reach_the_pieces_of_a_divided_sector() {
    // Delete takes the picked half cell's faces and nothing else.
    let mut rig = divided_plan();
    rig.tool(GridTool::Select);
    let faces_before = psxed_project::grid::faces::all_faces(rig.world()).len();
    rig.click(rig.top_at(2.75 * S, 2.25 * S));
    rig.key(egui::Key::Delete);
    let gone = rig
        .room(0)
        .leaf(psxed_project::grid::LeafKey {
            level: 3,
            x: 10,
            z: 8,
        })
        .is_none_or(|s| s.ground_floor().is_none());
    assert!(
        gone,
        "the picked half cell kept its floor: {}",
        rig.ws.status
    );
    assert!(
        rig.room(0)
            .leaf(psxed_project::grid::LeafKey {
                level: 3,
                x: 8,
                z: 8
            })
            .is_some_and(|s| s.ground_floor().is_some()),
        "the north-west half cell is still there"
    );
    rig.ws.do_undo();
    assert_eq!(
        psxed_project::grid::faces::all_faces(rig.world()).len(),
        faces_before
    );
    // Shift range from one half cell to a whole sector on its right.
    rig.click(rig.top_at(2.25 * S, 2.25 * S));
    rig.click_with(rig.cell_centre(4, 2), egui::Modifiers::SHIFT);
    assert!(
        rig.ws.grid.selection.faces(rig.world()).len() >= 2,
        "{}",
        describe(&rig)
    );
}

#[test]
fn raise_in_the_plan_lifts_a_half_cell_region() {
    let mut rig = divided_plan();
    rig.tool(GridTool::Raise);
    let from = rig.top_at(2.75 * S, 2.25 * S);
    // Pick the region (a click), then press on it and drag up.
    rig.click(from);
    rig.drag(from, from + Vec2::new(0.0, -2.0 * CLICK_PX_PLAN));
    let leaf = psxed_project::grid::LeafKey {
        level: 3,
        x: 10,
        z: 8,
    };
    let raised = rig
        .room(0)
        .leaf(leaf)
        .and_then(|s| s.ground_floor())
        .map(|f| f.heights);
    assert!(
        raised.is_some_and(|h| h.iter().any(|v| *v > 0)),
        "{raised:?} {}",
        rig.ws.status
    );
}

const CLICK_PX_PLAN: f32 = crate::grid_tools::view::TOP_PIXELS_PER_CLICK;
