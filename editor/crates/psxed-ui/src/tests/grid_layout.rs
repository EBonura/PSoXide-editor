//! The layout rework of a grid project: the Assets drawer, the view switch,
//! the palette, the Properties header, the Structure panel and the tool
//! groups.

use super::grid_rig::*;
use super::grid_ui::{draw_editor_sized, full_context};
use super::*;
use crate::grid_tools::{GridTool, GridViewMode};

pub(super) fn text_of(output: &egui::FullOutput) -> String {
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

pub(super) fn screen(ws: &mut EditorWorkspace) -> String {
    let ctx = full_context();
    text_of(&draw_editor_sized(
        ws,
        &ctx,
        false,
        Vec2::new(1920.0, 1080.0),
    ))
}

#[test]
fn a_grid_project_opens_with_the_assets_drawer_closed_and_place_opens_it() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-drawer"), flat_project(4, 4, true));
    assert!(!ws.resources_open, "closed at open");
    let text = screen(&mut ws);
    assert!(
        text.contains("Assets"),
        "the drawer bar names itself:\n{text}"
    );
    assert!(
        !text.contains("Filter resources"),
        "no card list while closed"
    );
    // Opening it by hand.
    ws.resources_open = true;
    let text = screen(&mut ws);
    assert!(text.contains("Filter resources"), "{text}");
    // Choosing Place for a prop that needs a model opens it.
    ws.resources_open = false;
    ws.place_kind = PlaceKind::ModelInstance;
    ws.set_grid_tool(GridTool::Place);
    assert!(ws.resources_open, "Place needs a model");
    // A light needs nothing from it.
    ws.resources_open = false;
    ws.place_kind = PlaceKind::PointLightMarker;
    ws.set_grid_tool(GridTool::Place);
    assert!(!ws.resources_open);
}

#[test]
fn the_view_switch_reads_3d_plan_both() {
    let labels: Vec<&str> = GridViewMode::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(labels, ["3D", "Plan", "Both"]);
}
