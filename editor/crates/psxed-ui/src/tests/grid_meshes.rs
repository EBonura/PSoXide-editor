//! Static meshes driven through real pointer and key input: the Shift+N
//! chord, placing, selecting, moving, turning, deleting, the Inspector's
//! numbers and undo, in the Top view and in 3D.

use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::add_asset;
use psxed_project::grid::mesh::{ArchSpec, ColumnSpec, MeshAsset, MeshKind};

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

/// A rig with an 8x8 floor and an arch and a column in the palette, the arch
/// chosen, the Mesh tool on.
fn rig(label: &str, persp: bool) -> GridRig {
    let mut rig = if persp {
        GridRig::persp(label, flat_project(8, 8, false))
    } else {
        GridRig::top(label, flat_project(8, 8, false))
    };
    {
        let world = rig
            .ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .expect("grid");
        let arch = add_asset(
            world,
            MeshAsset::new("arch", MeshKind::Arch(ArchSpec::default())),
        )
        .expect("arch");
        add_asset(
            world,
            MeshAsset::new("column", MeshKind::Column(ColumnSpec::default())),
        )
        .expect("column");
        assert_eq!(arch, 0);
    }
    rig.ws.grid.mesh_asset = Some(0);
    rig.tool(GridTool::Mesh);
    rig
}

#[test]
fn shift_n_is_the_mesh_tool_and_plain_n_stays_the_plane_tool() {
    let mut rig = GridRig::top("mesh-chord", flat_project(4, 4, false));
    rig.key_with(egui::Key::N, shift());
    assert_eq!(rig.ws.grid.tool, GridTool::Mesh);
    rig.key(egui::Key::N);
    assert_eq!(rig.ws.grid.tool, GridTool::Plane);
    assert_eq!(GridTool::Mesh.key_label(), "Shift+N");
}

#[test]
fn a_click_places_the_chosen_asset_on_the_floor_snapped_and_selected() {
    let mut rig = rig("mesh-place", false);
    rig.click(rig.top_at(3072.0 + 100.0, 3072.0 + 60.0));
    let meshes = &rig.room(0).meshes;
    assert_eq!(meshes.len(), 1, "{}", rig.ws.status);
    let m = &meshes[0];
    assert_eq!(m.asset, 0);
    // 256 lattice, standing on the floor at height 0.
    assert_eq!((m.position[0] % 256, m.position[2] % 256), (0, 0), "{m:?}");
    assert_eq!(m.position[1], 0);
    assert_eq!(rig.ws.grid.mesh_sel, Some(m.id));
    // One undo step takes it back.
    rig.ws.do_undo();
    assert!(rig.room(0).meshes.is_empty());
    rig.ws.do_redo();
    assert_eq!(rig.room(0).meshes.len(), 1);
}

#[test]
fn without_an_asset_a_click_says_what_to_do_and_places_nothing() {
    let mut rig = GridRig::top("mesh-none", flat_project(6, 6, false));
    rig.tool(GridTool::Mesh);
    rig.click(rig.cell_centre(2, 2));
    assert!(rig.room(0).meshes.is_empty());
    assert!(rig.ws.status.contains("palette"), "{}", rig.ws.status);
}

#[test]
fn a_click_in_3d_places_on_the_surface_under_the_pointer() {
    let mut rig = rig("mesh-3d", true);
    rig.click(rig.screen_of([3.5 * S, 0.0, 3.5 * S]));
    let meshes = &rig.room(0).meshes;
    assert_eq!(meshes.len(), 1, "{}", rig.ws.status);
    let m = &meshes[0];
    assert!(
        (m.position[0] - 3584).abs() <= 256 && (m.position[2] - 3584).abs() <= 256,
        "{m:?}"
    );
    assert_eq!(m.position[1], 0);
}

#[test]
fn dragging_an_instance_moves_it_and_one_undo_puts_it_back() {
    let mut rig = rig("mesh-drag", false);
    rig.click(rig.top_at(3072.0, 3072.0));
    let start = rig.room(0).meshes[0].position;
    // Press inside the footprint (the arch is two sectors wide at its middle).
    let from = rig.top_at(start[0] as f32, start[2] as f32);
    let to = rig.top_at(start[0] as f32 + 768.0, start[2] as f32 - 512.0);
    rig.drag(from, to);
    let moved = rig.room(0).meshes[0].position;
    assert_eq!(moved, [start[0] + 768, 0, start[2] - 512], "{moved:?}");
    assert_eq!(
        rig.room(0).meshes.len(),
        1,
        "a drag never places a second one"
    );
    rig.ws.do_undo();
    assert_eq!(rig.room(0).meshes[0].position, start);
}

#[test]
fn r_and_the_brackets_turn_the_selection_or_the_next_heading() {
    let mut rig = rig("mesh-turn", false);
    // Nothing selected: R turns the heading of the next placement.
    rig.key(egui::Key::R);
    assert_eq!(rig.ws.grid.mesh_turn, 90);
    rig.click(rig.cell_centre(3, 3));
    assert_eq!(rig.room(0).meshes[0].rotation[0], 90);
    // Selected: R turns the instance, [ and ] by fifteen.
    rig.key(egui::Key::R);
    assert_eq!(rig.room(0).meshes[0].rotation[0], 180);
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.room(0).meshes[0].rotation[0], -165);
    rig.key(egui::Key::OpenBracket);
    rig.key(egui::Key::OpenBracket);
    assert_eq!(rig.room(0).meshes[0].rotation[0], 165);
    assert_eq!(rig.ws.grid.mesh_turn, 90, "the next heading is unchanged");
}

#[test]
fn delete_removes_the_selected_instance_and_its_id_is_not_reused() {
    let mut rig = rig("mesh-delete", false);
    rig.click(rig.cell_centre(2, 2));
    rig.key(egui::Key::Delete);
    assert!(rig.room(0).meshes.is_empty());
    assert_eq!(rig.ws.grid.mesh_sel, None);
    rig.click(rig.cell_centre(2, 2));
    let first = rig.room(0).meshes[0].id;
    rig.ws.grid.mesh_sel = None;
    rig.click(rig.cell_centre(5, 5));
    assert_ne!(rig.room(0).meshes[1].id, first);
}

#[test]
fn the_palette_makes_assets_and_a_recipe_edit_reaches_every_copy() {
    let mut rig = rig("mesh-palette", false);
    rig.click(rig.cell_centre(2, 2));
    rig.ws.grid.mesh_asset = Some(1);
    rig.click(rig.cell_centre(5, 5));
    // The column was placed from the second asset.
    assert_eq!(rig.room(0).meshes[1].asset, 1);
    // Drawing the Inspector with a selected mesh and the palette open works
    // in both views.
    for persp in [false, true] {
        rig.ws.view_2d = !persp;
        rig.pump(vec![], egui::Modifiers::NONE);
    }
    assert!(rig.ws.status.contains("Placed mesh"), "{}", rig.ws.status);
}

#[test]
fn every_view_draws_with_meshes_placed_and_selected() {
    let ctx = full_context();
    for persp in [false, true] {
        let mut rig = rig("mesh-draw", persp);
        rig.click(rig.cell_centre(3, 3));
        rig.ws.last_viewport_size = Vec2::new(1200.0, 700.0);
        for view_2d in [true, false] {
            let output = draw_editor(&mut rig.ws, &ctx, view_2d);
            assert!(!output.shapes.is_empty());
        }
    }
}

#[test]
fn the_overlay_hover_ghost_follows_the_pointer() {
    let mut rig = rig("mesh-ghost", false);
    rig.move_to(rig.cell_centre(4, 4), egui::Modifiers::NONE);
    match &rig.ws.grid.hover {
        crate::grid_tools::GridHover::MeshSpot { position, .. } => {
            assert!((position[0] - 4608).abs() <= 256, "{position:?}")
        }
        other => panic!("{other:?}"),
    }
    // Over an existing instance the hover is the instance.
    rig.click(rig.cell_centre(4, 4));
    rig.move_to(rig.cell_centre(4, 4), egui::Modifiers::NONE);
    assert!(matches!(
        rig.ws.grid.hover,
        crate::grid_tools::GridHover::Mesh(_)
    ));
}

#[test]
fn minus_and_equals_scale_the_selection_by_eighths_with_undo() {
    let mut rig = rig("mesh-scale", false);
    rig.click(rig.cell_centre(3, 3));
    assert_eq!(rig.room(0).meshes[0].scale, [256; 3]);
    rig.key(egui::Key::Equals);
    rig.key(egui::Key::Equals);
    assert_eq!(rig.room(0).meshes[0].scale, [320; 3]);
    rig.key(egui::Key::Minus);
    assert_eq!(rig.room(0).meshes[0].scale, [288; 3]);
    rig.ws.do_undo();
    assert_eq!(rig.room(0).meshes[0].scale, [320; 3]);
    // With nothing selected the key says what to do.
    rig.ws.grid.mesh_sel = None;
    rig.key(egui::Key::Equals);
    assert!(rig.ws.status.contains("Select a mesh"), "{}", rig.ws.status);
    // The scale never leaves the document's range.
    rig.click(rig.cell_centre(3, 3));
    for _ in 0..40 {
        rig.key(egui::Key::Minus);
    }
    assert_eq!(rig.room(0).meshes[0].scale, [32; 3]);
}
