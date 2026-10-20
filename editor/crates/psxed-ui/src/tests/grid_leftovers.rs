//! Three leftovers of the 10-19 usability pass: the Material tab's left dock,
//! a label that ran off the plan pane, and the Properties header of a light.

use super::grid_layout::{screen, text_of};
use super::grid_material_tab::{draw_material_tab, two_materials};
use super::grid_rig::*;
use super::grid_ui::{draw_editor_sized, full_context};
use super::*;

#[test]
fn the_material_tab_left_dock_lists_the_materials_and_not_the_scene_graph() {
    let (project, _a, b) = two_materials();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("leftover-mat-dock"), project);
    ws.grid.material = Some(b);
    let text = draw_material_tab(&mut ws);
    assert!(
        !text.contains("Scene Graph"),
        "the scene graph is drawn:\n{text}"
    );
    assert!(!text.contains("Filter scene graph"), "{text}");
    assert!(text.contains("Search materials"), "no palette:\n{text}");
    // Both materials are in it (the group is open: a world of two).
    let lists = |name: &str| text.contains(name);
    assert!(lists("Graybox"), "{text}");
}

#[test]
fn choosing_in_the_material_tab_dock_does_not_repaint_selected_faces() {
    let (project, a, b) = two_materials();
    let mut ws = EditorWorkspace::with_project(test_temp_dir("leftover-mat-pick"), project);
    ws.grid.material = Some(b);
    ws.apply_grid_pose("tool=select;select=floors:0,0:1,1")
        .unwrap();
    let before = ws.project.clone();
    ws.active_workspace = WorkspaceView::Material;
    ws.choose_palette_material(a);
    assert_eq!(ws.grid.material, Some(a));
    assert_eq!(
        ws.project.active_scene().world,
        before.active_scene().world,
        "browsing materials in the Material tab changed the world"
    );
}

/// Text shapes of a frame with the rectangle each occupies on screen.
fn text_rects(output: &egui::FullOutput) -> Vec<(String, Rect)> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn the_stairs_chip_stays_inside_the_pane_it_is_drawn_in() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("leftover-chip"), flat_project(8, 8, false));
    ws.apply_grid_pose("view=split;tool=stairs;stairs-path=7,2:7,6;verb-drag=1")
        .unwrap();
    let ctx = full_context();
    let size = Vec2::new(1920.0, 1080.0);
    let _ = draw_editor_sized(&mut ws, &ctx, false, size);
    let out = draw_editor_sized(&mut ws, &ctx, false, size);
    let plan = ws.last_orthographic_viewport_rect;
    let persp = ws.last_persp_viewport_rect;
    let chips: Vec<_> = text_rects(&out)
        .into_iter()
        .filter(|(t, _)| t.contains("steps, riser") && t.contains("limit"))
        .collect();
    assert!(
        !chips.is_empty(),
        "no stairs chip drawn:\n{}",
        text_of(&out)
    );
    for (text, rect) in chips {
        let pane = if plan.contains(rect.center()) {
            plan
        } else {
            persp
        };
        assert!(
            pane.contains_rect(rect.expand2(Vec2::new(6.0, 3.0))),
            "chip {text:?} at {rect:?} runs out of its pane {pane:?}"
        );
    }
}

#[test]
fn a_selected_light_says_light_in_the_properties_header() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("leftover-light"), flat_project(6, 6, true));
    ws.place_kind = PlaceKind::PointLightMarker;
    ws.place_at_world_hit([2.5 * S, 0.0, 2.5 * S]);
    let id = ws.selection.selected_node;
    assert_ne!(id, NodeId::ROOT, "the light is selected after placing");
    let name = ws.project.active_scene().node(id).unwrap().name.clone();
    let text = screen(&mut ws);
    assert!(
        text.contains(&format!("Light: {name}")),
        "header should read Light: {name}\n{text}"
    );
}
