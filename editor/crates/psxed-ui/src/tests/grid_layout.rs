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

fn material(project: &mut ProjectDocument, name: &str) -> ResourceId {
    // Level images are what the palette lists.
    project.add_resource(
        name,
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(Some(
            format!("level_textures/{}.psxt", name.replace([' ', '.', '/'], "_")),
        ))),
    )
}

/// A world with the Graybox set and `pieces` x `cells` tiles named `DP p.c`
/// (what dp-world has: 650 of them).
fn tiled_project(pieces: usize, cells: usize) -> ProjectDocument {
    let mut project = flat_project(4, 4, false);
    material(&mut project, "Graybox / Ground 64");
    for p in 0..pieces {
        for c in 0..cells {
            material(&mut project, &format!("DP {p}.{c}"));
        }
    }
    project
}

#[test]
fn palette_groups_name_the_prefix_of_a_material() {
    use crate::grid_tools::materials_panel::group_of;
    assert_eq!(group_of("DP 13.4"), "DP 13");
    assert_eq!(group_of("Graybox / Ground 64"), "Graybox");
    assert_eq!(group_of("Moss"), "Other");
    assert_eq!(group_of("Wall 2.5.1"), "Wall 2.5");
}

#[test]
fn the_palette_groups_600_materials_and_remembers_the_recent_ones() {
    let project = tiled_project(100, 6);
    let mut ws = EditorWorkspace::with_project(test_temp_dir("layout-palette"), project);
    let text = screen(&mut ws);
    assert!(text.contains("Graybox (1)"), "{text}");
    assert!(text.contains("DP 5 (6)"), "{text}");
    assert!(!text.contains("Recent"), "nothing used yet");
    // Using a material puts it first among the recent ones.
    let id = ws
        .project
        .resources
        .iter()
        .find(|r| r.name == "DP 5.2")
        .unwrap()
        .id;
    ws.choose_palette_material(id);
    assert_eq!(ws.grid.material, Some(id));
    assert_eq!(ws.grid.palette_recent.first(), Some(&id));
    let text = screen(&mut ws);
    assert!(text.contains("Recent"), "{text}");
    // A material is not listed twice among the recent ones, and the list is short.
    ws.choose_palette_material(id);
    assert_eq!(ws.grid.palette_recent.len(), 1);
    for p in 0..20 {
        let other = ws
            .project
            .resources
            .iter()
            .find(|r| r.name == format!("DP {p}.0"))
            .unwrap()
            .id;
        ws.choose_palette_material(other);
    }
    assert!(ws.grid.palette_recent.len() <= 8);
}

#[test]
fn searching_opens_the_groups_with_a_match() {
    let project = tiled_project(100, 6);
    let mut ws = EditorWorkspace::with_project(test_temp_dir("layout-palette-search"), project);
    ws.grid.palette_search = "DP 77.3".to_string();
    let text = screen(&mut ws);
    assert!(text.contains("1 of"), "the count reads the match: {text}");
    assert!(text.contains("DP 77 (1)"), "{text}");
    assert!(!text.contains("DP 5 (6)"), "other groups are filtered out");
}
