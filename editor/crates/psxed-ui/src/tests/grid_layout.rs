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

#[test]
fn properties_names_what_is_edited_and_nothing_selected_is_a_hint() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-properties"), flat_project(4, 4, true));
    // Nothing selected: a hint and a way to the World settings, not the form.
    let text = screen(&mut ws);
    assert!(text.contains("Properties"), "{text}");
    assert!(text.contains("Nothing selected"), "{text}");
    assert!(text.contains("World settings"), "{text}");
    for form in ["Physics", "Far Vista", "Sky Texture"] {
        assert!(!text.contains(form), "the World form is drawn: {form}");
    }
    // The World row of the Scene Graph (or the button) opens the form.
    ws.replace_node_selection(NodeId::ROOT);
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
    assert!(at("Structure") < at("Levels"), "Structure heads the panel");
    assert!(at("Levels") < at("Rooms"));
    assert!(at("Rooms") < at("Scene Graph"));
    assert_eq!(text.matches("Structure").count(), 1, "{text}");
}

#[test]
fn many_lights_are_one_group_in_the_scene_graph() {
    use crate::scene_tree::{group_light_runs, SceneEntry};
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-lights"), flat_project(6, 6, true));
    ws.place_kind = PlaceKind::PointLightMarker;
    for x in 0..5 {
        ws.place_at_world_hit([(x as f32 + 0.5) * S, 0.0, 2.5 * S]);
    }
    ws.clear_node_selection_state();
    let rows = ws.project.active_scene().hierarchy_rows();
    let refs: Vec<&psxed_project::NodeRow> = rows.iter().collect();
    let lights: Vec<NodeId> = rows
        .iter()
        .filter(|r| r.kind == "Point Light")
        .map(|r| r.id)
        .collect();
    assert_eq!(lights.len(), 5);
    let count = |entries: &[SceneEntry<'_>]| {
        entries
            .iter()
            .filter(|e| matches!(e, SceneEntry::Row(r) if r.kind == "Point Light"))
            .count()
    };
    let none = HashSet::new();
    // Closed: one Lights row and no light rows.
    let closed = group_light_runs(&refs, &none, &none, NodeId::ROOT);
    assert_eq!(count(&closed), 0);
    assert!(closed.iter().any(|e| matches!(
        e,
        SceneEntry::Lights {
            count: 5,
            open: false,
            ..
        }
    )));
    let text = screen(&mut ws);
    assert!(text.contains("Lights (5)"), "{text}");
    // Open by listing the group.
    let opened: HashSet<NodeId> = lights.iter().copied().collect();
    assert_eq!(
        count(&group_light_runs(&refs, &opened, &none, NodeId::ROOT)),
        5
    );
    // A selected light opens its group by itself.
    let selected: HashSet<NodeId> = [lights[2]].into_iter().collect();
    assert_eq!(
        count(&group_light_runs(&refs, &none, &selected, lights[2])),
        5
    );
    // Two lights are not worth a group.
    assert_eq!(
        count(&group_light_runs(
            &refs[..refs.len() - 3],
            &none,
            &none,
            NodeId::ROOT
        )),
        2
    );
}

#[test]
fn the_tools_sit_in_four_labelled_groups_on_two_rows() {
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("layout-tools"), flat_project(4, 4, true));
    let ctx = full_context();
    let output = draw_editor_sized(&mut ws, &ctx, false, Vec2::new(1920.0, 1080.0));
    // A tool button's text is its icon, two spaces and the label.
    let y_of = |label: &str| {
        let mut found = Vec::new();
        for shape in &output.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                let text = t.galley.text();
                if text == label
                    || (text.ends_with(&format!("  {label}"))
                        && text.chars().count() == label.chars().count() + 3)
                {
                    // The strip is the top of the centre; the palette says Paint too.
                    if t.pos.x < 1600.0 {
                        found.push(t.pos.y);
                    }
                }
            }
        }
        assert_eq!(found.len(), 1, "{label}: {found:?}");
        found[0]
    };
    for group in ["PICK", "DRAW", "FINISH", "BUILD"] {
        y_of(group);
    }
    let first_row = y_of("Select");
    for tool in [
        "Height", "Floor", "Ceiling", "Wall", "Plane", "Mesh", "Paint", "Diagonal", "Bands",
        "Place",
    ] {
        assert!(
            (y_of(tool) - first_row).abs() < 4.0,
            "{tool} is on the first row"
        );
    }
    let second_row = y_of("Raise");
    assert!(second_row > first_row + 10.0, "Build is on its own row");
    for tool in [
        "Ramp", "Stairs", "Spiral", "Break", "Rough", "Cells", "Slab",
    ] {
        assert!(
            (y_of(tool) - second_row).abs() < 4.0,
            "{tool} is on the second row"
        );
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
    assert_eq!(seen, ["PICK", "DRAW", "FINISH", "BUILD"]);
}
