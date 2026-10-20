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

/// What the editor says at 1920 x 1080 after a frame to settle (egui draws a
/// new window from its second frame).
pub(super) fn screen(ws: &mut EditorWorkspace) -> String {
    screen_sized(ws, Vec2::new(1920.0, 1080.0))
}

/// [`screen`] on a taller or wider screen (the key page needs height: rows
/// below the screen are not drawn).
pub(super) fn screen_sized(ws: &mut EditorWorkspace, size: Vec2) -> String {
    let ctx = full_context();
    let _ = draw_editor_sized(ws, &ctx, false, size);
    text_of(&draw_editor_sized(ws, &ctx, false, size))
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

#[test]
fn properties_names_what_is_edited_and_nothing_selected_is_a_hint() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-properties"), flat_project(4, 4, true));
    // Nothing selected: a hint, not the World form.
    let text = screen(&mut ws);
    assert!(text.contains("Properties"), "{text}");
    assert!(text.contains("Nothing selected"), "{text}");
    for form in ["Physics", "Far Vista", "Sky Texture"] {
        assert!(!text.contains(form), "the World form is drawn: {form}");
    }
    // The World entry of the Structure panel opens the form.
    ws.select_world();
    let text = screen(&mut ws);
    assert!(text.contains("Physics"), "{text}");
    assert!(!text.contains("Nothing selected"), "{text}");
    // Faces name themselves.
    ws.clear_node_selection_state();
    ws.apply_grid_pose("tool=select;select=floors:0,0:1,1")
        .unwrap();
    let text = screen(&mut ws);
    assert!(text.contains("4 faces"), "{text}");
    assert!(!text.contains("Physics"), "{text}");
    assert!(!text.contains("Nothing selected"), "{text}");
}

#[test]
fn levels_and_rooms_live_in_one_structure_panel() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-structure"), flat_project(4, 4, true));
    let text = screen(&mut ws);
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{text}"))
    };
    assert!(at("Structure") < at("Storeys"), "Structure heads the panel");
    assert!(at("Storeys") < at("Rooms"));
    assert!(at("Rooms") < at("Objects"));
    assert_eq!(text.matches("Structure").count(), 1, "{text}");
}

#[test]
fn the_tools_sit_in_labelled_groups_on_one_row() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-tools"), flat_project(4, 4, true));
    let ctx = full_context();
    let output = draw_editor_sized(&mut ws, &ctx, false, Vec2::new(1920.0, 1080.0));
    let y_of = |label: &str| {
        let mut found = Vec::new();
        for shape in &output.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                // The row is the strip under the top bar; the palette says Paint too.
                if t.galley.text() == label && t.pos.x < 1600.0 && t.pos.y < 100.0 {
                    found.push(t.pos.y);
                }
            }
        }
        assert_eq!(found.len(), 1, "{label}: {found:?}");
        found[0]
    };
    for group in ["CREATE", "SHAPE", "SURFACE", "OBJECTS"] {
        y_of(group);
    }
    let row = y_of("Select");
    for tool in [
        "Height", "Floor", "Ceiling", "Wall", "Extrude", "Paint", "Bands", "Place",
    ] {
        assert!((y_of(tool) - row).abs() < 4.0, "{tool} is on the row");
    }
    // The order of GridTool::ALL is the order of the groups.
    let groups: Vec<_> = GridTool::ALL.iter().map(|t| t.group().label()).collect();
    let mut seen = Vec::new();
    for g in groups {
        if seen.last() != Some(&g) {
            assert!(!seen.contains(&g), "{g} is split in two");
            seen.push(g);
        }
    }
    assert_eq!(seen, ["PICK", "CREATE", "SHAPE", "SURFACE", "OBJECTS"]);
}

fn font_size_of(output: &egui::FullOutput, needle: &str) -> Option<f32> {
    output.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text().contains(needle) => t
            .galley
            .job
            .sections
            .first()
            .map(|sec| sec.format.font_id.size),
        _ => None,
    })
}

#[test]
fn the_status_bar_separates_the_message_the_hints_and_the_budget() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-status"), flat_project(4, 4, true));
    ws.status = "Placed a spawn".to_string();
    let ctx = full_context();
    let output = draw_editor_sized(&mut ws, &ctx, false, Vec2::new(1920.0, 1080.0));
    let text = text_of(&output);
    // Three parts, each in its own label and readable.
    assert!(text.contains("Placed a spawn"));
    assert!(text.contains("Shift range"), "hints");
    assert!(text.contains("Room hall"), "{text}");
    assert!(text.contains("16 sectors"), "{text}");
    assert!(text.contains("32 / 16383 faces"), "{text}");
    for needle in [
        "Placed a spawn",
        "Shift range",
        "Room hall",
        "32 / 16383 faces",
    ] {
        let size = font_size_of(&output, needle).expect(needle);
        assert!(size >= 12.0, "{needle} is {size} px");
    }
    // The old run-together budget line is gone.
    assert!(!text.contains("busiest room"), "{text}");
}

#[test]
fn frame_camera_overlays_and_walk_are_labelled_and_debug_is_not_in_the_main_bar() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-labels"), flat_project(4, 4, true));
    let text = screen(&mut ws);
    for label in ["Frame", "Camera", "Overlays", "Walk"] {
        assert!(text.contains(label), "{label:?} is a button label:\n{text}");
    }
    // Keys, Sizes and Perf moved into Help and the Overlays menu.
    for gone in ["Keys", "Sizes", "Perf"] {
        assert!(!text.contains(gone), "{gone:?} is in the header:\n{text}");
    }
    // Debug moved into the Tools menu.
    assert!(
        !text.contains("  Debug"),
        "Debug is still in the main bar:\n{text}"
    );
}

#[test]
fn link_move_and_global_show_only_when_they_have_something_to_act_on() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-link"), flat_project(4, 4, true));
    ws.set_grid_tool(GridTool::Select);
    let text = screen(&mut ws);
    for label in ["Link", "  Move", "  Global"] {
        assert!(
            !text.contains(label),
            "{label:?} with nothing selected:\n{text}"
        );
    }
    // A corner selected: Link has something to do.
    ws.apply_grid_pose("select=vertex:1,1,0").unwrap();
    let text = screen(&mut ws);
    assert!(text.contains("Link"), "{text}");
    assert!(!text.contains("  Move"), "{text}");
    // The Height tool always has corners to weld.
    ws.apply_grid_pose("select=cells:0,0:0,0").unwrap();
    ws.grid.selection.clear();
    ws.set_grid_tool(GridTool::Height);
    assert!(screen(&mut ws).contains("Link"));
    // A node selected: the gizmo controls.
    ws.set_grid_tool(GridTool::Select);
    ws.place_kind = PlaceKind::PointLightMarker;
    ws.place_at_world_hit([1.5 * S, 0.0, 1.5 * S]);
    let text = screen(&mut ws);
    assert!(
        text.contains("  Move") && text.contains("  Global"),
        "{text}"
    );
}

#[test]
fn the_keys_page_is_searchable() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-keys"), flat_project(4, 4, true));
    ws.grid.help_open = true;
    let text = screen_sized(&mut ws, Vec2::new(1920.0, 2400.0));
    assert!(text.contains("Search keys"), "{text}");
    assert!(
        text.contains("Spiral (Shift+T)") && text.contains("Upper floor (Shift+Q) and storeys")
    );
    // A query keeps the sections with a row that matches, and only those rows.
    ws.grid.help_search = "cutaway".to_string();
    let text = screen_sized(&mut ws, Vec2::new(1920.0, 2400.0));
    assert!(text.contains("Upper floor (Shift+Q) and storeys"), "{text}");
    assert!(!text.contains("Spiral (Shift+T)"), "{text}");
    assert!(!text.contains("Wall (W)"), "{text}");
    // A section whose title matches shows all its rows.
    ws.grid.help_search = "spiral".to_string();
    let text = screen_sized(&mut ws, Vec2::new(1920.0, 2400.0));
    assert!(text.contains("Spiral (Shift+T)"), "{text}");
    ws.grid.help_search = "zzzz".to_string();
    assert!(screen_sized(&mut ws, Vec2::new(1920.0, 2400.0)).contains("No key matches"));
}
