//! The Objects panel, selection sync and the Inspector of an actor, a light,
//! a room and the World (lane editor-objects, 2026-10-20).
//!
//! Everything drives the whole editor window (`WindowRig`, real fonts, real
//! pointer events at 1456 x 819).

use super::grid_live::WindowRig;
use super::*;

fn frames(rig: &mut WindowRig, n: usize) {
    for _ in 0..n {
        rig.frame(vec![]);
    }
}

/// An Entity with the given part nodes under it, at `at`.
fn add_actor(
    ws: &mut EditorWorkspace,
    name: &str,
    at: [f32; 3],
    parts: Vec<(&str, NodeKind)>,
) -> NodeId {
    let scene = ws.project.active_scene_mut();
    let id = scene.add_node(NodeId::ROOT, name, NodeKind::Entity);
    scene.node_mut(id).unwrap().transform.translation = at;
    for (part_name, kind) in parts {
        scene.add_node(id, part_name, kind);
    }
    id
}

fn model() -> (&'static str, NodeKind) {
    (
        "Model Renderer",
        NodeKind::ModelRenderer {
            model: None,
            material: None,
            visual_offset: [0; 3],
            visual_scale_q8: psxed_project::MODEL_SCALE_ONE_Q8,
        },
    )
}

fn animator() -> (&'static str, NodeKind) {
    (
        "Animator",
        NodeKind::Animator {
            clip: None,
            action_clips: Vec::new(),
            autoplay: true,
            pose_frame: 0,
        },
    )
}

fn controller(player: bool) -> (&'static str, NodeKind) {
    (
        "Character Controller",
        NodeKind::CharacterController {
            loadout: None,
            character: None,
            settings: None,
            player,
        },
    )
}

fn camera() -> (&'static str, NodeKind) {
    (
        "Camera",
        NodeKind::Camera {
            settings: psxed_project::WorldCameraSettings::default(),
        },
    )
}

fn equipment() -> (&'static str, NodeKind) {
    (
        "Equipment",
        NodeKind::Equipment {
            weapon: None,
            character_socket: "right_hand_grip".to_string(),
            weapon_grip: "grip".to_string(),
        },
    )
}

fn beacon() -> (&'static str, NodeKind) {
    (
        "Point of Interest",
        NodeKind::PointOfInterest {
            pages: psxed_project::default_point_of_interest_pages(),
            pages_it: Vec::new(),
            prompt: psxed_project::default_point_of_interest_prompt(),
            radius: psxed_project::default_point_of_interest_radius(),
            marker_height: psxed_project::default_point_of_interest_marker_height(),
            repeatable: true,
            persistence_id: String::new(),
            reward: None,
            enabled: true,
        },
    )
}

fn add_light(ws: &mut EditorWorkspace, name: &str, at: [f32; 3]) -> NodeId {
    let scene = ws.project.active_scene_mut();
    let id = scene.add_node(
        NodeId::ROOT,
        name,
        NodeKind::PointLight {
            color: [255, 240, 200],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    scene.node_mut(id).unwrap().transform.translation = at;
    id
}

/// Editor-unit position of the centre of world cell `(cx, cz)`.
fn cell_centre(cx: i32, cz: i32) -> [f32; 3] {
    [(cx as f32 + 0.5) * 1280.0, 0.0, (cz as f32 + 0.5) * 1280.0]
}

/// A scene shaped like the default project's: a player and enemies, beacons,
/// and lights in runs that an actor in between breaks up. Everything stands
/// in its own cell of the starter room, so a click in the plan finds one.
/// Returns the player, the third light and the last light.
pub(super) fn populate(ws: &mut EditorWorkspace) -> Vec<(&'static str, NodeId)> {
    let mut named = Vec::new();
    // The starter project already has its player.
    let aletha = match ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find(|n| n.name == "Aletha (Player)")
        .map(|n| n.id)
    {
        Some(id) => id,
        None => add_actor(
            ws,
            "Aletha (Player)",
            cell_centre(3, 3),
            vec![model(), animator(), controller(true), camera(), equipment()],
        ),
    };
    named.push(("actor", aletha));
    for (i, cx) in [3, 4, 5].into_iter().enumerate() {
        add_actor(
            ws,
            &format!("Intake Custodian {}", i + 1),
            cell_centre(cx, 4),
            vec![model(), animator(), controller(false)],
        );
    }
    add_actor(
        ws,
        "Heavy Enemy",
        cell_centre(4, 5),
        vec![model(), animator(), controller(false)],
    );
    let mut name = "Point Light".to_string();
    let mut lights = Vec::new();
    let mut light_at = |ws: &mut EditorWorkspace, name: &str, i: i32| {
        let id = add_light(ws, name, cell_centre(i % 6, 1 + i / 6));
        lights.push(id);
        id
    };
    for i in 0..6 {
        light_at(ws, &name, i);
        name.push_str(" Copy");
    }
    add_actor(ws, "Point of Interest", cell_centre(4, 0), vec![beacon()]);
    for i in 6..10 {
        light_at(ws, &name, i);
        name.push_str(" Copy");
    }
    add_actor(ws, "Point of Interest", cell_centre(5, 0), vec![beacon()]);
    ws.project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "GroupS", NodeKind::Group);
    for i in 10..15 {
        light_at(ws, &name, i);
        name.push_str(" Copy");
    }
    named.push(("light", lights[2]));
    named.push(("last light", *lights.last().unwrap()));
    named
}

/// The five reviewed states of one world, written as PNGs.
/// `PSXED_OBJECTS_CAPTURE=<dir> PSXED_OBJECTS_LABEL=before|after`.
#[test]
fn objects_captures() {
    let Some(dir) = std::env::var_os("PSXED_OBJECTS_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_OBJECTS_LABEL").unwrap_or_else(|_| "after".into());
    let size = Vec2::new(1456.0, 819.0);
    for (world, tag) in [(0, "dp-world"), (1, "grid-starter")] {
        let mut rig = if world == 0 {
            WindowRig::with_size(size)
        } else {
            WindowRig::starter(size)
        };
        let named = populate(&mut rig.ws);
        let actor = named[0].1;
        let light = named[2].1;
        rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
        rig.ws.clear_node_selection_state();
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-objects-list-{label}.png")));
        rig.ws.replace_node_selection(actor);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-actor-selected-{label}.png")));
        rig.ws.clear_node_selection_state();
        select_room_for_capture(&mut rig.ws);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-room-selected-{label}.png")));
        open_world_for_capture(&mut rig.ws);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-world-settings-{label}.png")));
        rig.ws.replace_node_selection(light);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-light-selected-{label}.png")));
    }
}

fn select_room_for_capture(ws: &mut EditorWorkspace) {
    ws.select_room(0);
}

fn open_world_for_capture(ws: &mut EditorWorkspace) {
    ws.select_world();
}

// ---------------------------------------------------------------------
// What the window shows.

/// Every text of the last frame inside `area`: its string, its bounds and the
/// clip rectangle it was drawn through.
fn texts_in(rig: &WindowRig, area: Rect) -> Vec<(String, Rect, Rect)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                let bounds = t.visual_bounding_rect();
                area.contains(bounds.center()).then(|| {
                    (
                        t.galley
                            .text()
                            .trim_start_matches(|c: char| !c.is_alphanumeric())
                            .trim()
                            .to_string(),
                        bounds,
                        s.clip_rect,
                    )
                })
            }
            _ => None,
        })
        .collect()
}

/// The left dock of the 1456 x 819 window.
const DOCK: fn() -> Rect = || Rect::from_min_max(Pos2::new(0.0, 80.0), Pos2::new(280.0, 819.0));

fn dock_texts(rig: &WindowRig) -> Vec<String> {
    texts_in(rig, DOCK()).into_iter().map(|t| t.0).collect()
}

fn populated() -> (WindowRig, Vec<(&'static str, NodeId)>) {
    let mut rig = WindowRig::new();
    let named = populate(&mut rig.ws);
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    rig.ws.clear_node_selection_state();
    frames(&mut rig, 4);
    (rig, named)
}

#[test]
fn an_actor_is_one_row_and_its_parts_are_not_rows() {
    let (rig, _) = populated();
    let texts = dock_texts(&rig);
    assert!(texts.iter().any(|t| t == "Aletha (Player)"), "{texts:?}");
    for part in [
        "Model Renderer",
        "Animator",
        "Character Controller",
        "Equipment",
    ] {
        assert!(
            !texts.iter().any(|t| t == part),
            "{part} is a row of the dock: {texts:?}"
        );
    }
}

#[test]
fn every_light_is_in_one_group_however_the_siblings_are_ordered() {
    let (rig, _) = populated();
    let headers = dock_texts(&rig)
        .into_iter()
        .filter(|t| t == "Lights")
        .count();
    assert_eq!(headers, 1, "{:?}", dock_texts(&rig));
}

#[test]
fn a_row_shows_its_whole_name() {
    let (rig, _) = populated();
    for name in ["Aletha (Player)", "Intake Custodian 3", "Heavy Enemy"] {
        let (_, bounds, clip) = texts_in(&rig, DOCK())
            .into_iter()
            .find(|t| t.0 == name)
            .unwrap_or_else(|| panic!("no row {name}: {:?}", dock_texts(&rig)));
        assert!(
            bounds.right() <= clip.right() + 0.5,
            "{name} is cut at {} (drawn to {})",
            clip.right(),
            bounds.right()
        );
    }
}

#[test]
fn a_placed_or_duplicated_light_gets_a_clean_name() {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    rig.ws.place_kind = PlaceKind::PointLightMarker;
    let mut ids = Vec::new();
    for i in 0..2 {
        rig.ws
            .place_at_world_hit([(i as f32 + 1.5) * 1280.0, 0.0, 2.5 * 1280.0]);
        ids.push(rig.ws.selection.selected_node);
    }
    let name =
        |ws: &EditorWorkspace, id: NodeId| ws.project.active_scene().node(id).unwrap().name.clone();
    // The starter has one light already.
    assert_eq!(name(&rig.ws, ids[0]), "Light 2");
    assert_eq!(name(&rig.ws, ids[1]), "Light 3");
    rig.ws.replace_node_selection(ids[1]);
    rig.ws.duplicate_selected();
    assert_eq!(name(&rig.ws, rig.ws.selection.selected_node), "Light 4");
    // Nothing that already exists is renamed.
    assert!(rig
        .ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .any(|n| n.name == "Starter Light"));
}

#[test]
fn every_kind_has_its_own_icon_and_none_is_the_generic_dot() {
    let icons: Vec<char> = ObjectKind::ALL.iter().map(|k| k.icon()).collect();
    assert!(!icons.contains(&icons::CIRCLE_DOT));
    for (i, a) in icons.iter().enumerate() {
        for b in &icons[i + 1..] {
            assert_ne!(a, b, "two kinds share an icon");
        }
    }
}

#[test]
fn objects_are_listed_by_group_in_order_and_empty_groups_are_hidden() {
    let (rig, _) = populated();
    let rows = object_rows(rig.ws.project.active_scene());
    let groups: Vec<&str> = ObjectGroup::ALL
        .iter()
        .filter(|g| !rows_of_group(&rows, **g).is_empty())
        .map(|g| g.label())
        .collect();
    assert_eq!(groups, ["Actors", "Lights", "Interactions", "Other"]);
    let texts = dock_texts(&rig);
    for hidden in ["Props", "Logic"] {
        assert!(!texts.iter().any(|t| t == hidden), "{hidden}: {texts:?}");
    }
    let y = |label: &str| {
        texts_in(&rig, DOCK())
            .into_iter()
            .find(|t| t.0 == label)
            .map(|t| t.1.center().y)
    };
    assert!(y("Actors").unwrap() < y("Lights").unwrap());
}

#[test]
fn a_part_that_is_not_under_an_actor_still_has_a_row() {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    let (name, kind) = animator();
    let stray = rig
        .ws
        .project
        .active_scene_mut()
        .add_node(NodeId::ROOT, name, kind);
    let rows = object_rows(rig.ws.project.active_scene());
    assert!(rows.iter().any(|r| r.id == stray), "nothing may vanish");
}

#[test]
fn the_tree_view_keeps_a_folders_contents_and_the_kind_view_lists_them_by_kind() {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    let folder = rig
        .ws
        .project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "Crypt", NodeKind::Group);
    let light = add_light(&mut rig.ws, "Torch", [1280.0, 0.0, 1280.0]);
    rig.ws
        .project
        .active_scene_mut()
        .move_node(light, folder, 0);
    frames(&mut rig, 3);
    // By kind: the torch is a light wherever it sits.
    let rows = object_rows(rig.ws.project.active_scene());
    let torch = rows.iter().find(|r| r.id == light).unwrap();
    assert_eq!(torch.group(), ObjectGroup::Lights);
    assert_eq!(rows.iter().find(|r| r.id == folder).unwrap().child_count, 1);
    // As a tree: indented under its folder.
    assert_eq!(torch.depth, 1);
    let shown = tree_display_rows(&rows, &HashSet::new());
    assert!(shown.iter().any(|r| r.id == light));
    let collapsed: HashSet<NodeId> = [folder].into_iter().collect();
    assert!(!tree_display_rows(&rows, &collapsed)
        .iter()
        .any(|r| r.id == light));
}

// ---------------------------------------------------------------------
// Selection sync.

/// The Inspector column of the 1456 x 819 window.
const INSPECTOR: fn() -> Rect =
    || Rect::from_min_max(Pos2::new(1140.0, 80.0), Pos2::new(1456.0, 819.0));

fn inspector_texts(rig: &WindowRig) -> Vec<String> {
    texts_in(rig, INSPECTOR())
        .into_iter()
        .map(|t| t.0)
        .collect()
}

/// Whether the row drawn with `label` in the dock is the highlighted one.
fn row_is_highlighted(rig: &WindowRig, label: &str) -> bool {
    let Some((_, bounds, _)) = texts_in(rig, DOCK()).into_iter().find(|t| t.0 == label) else {
        return false;
    };
    rig.out.shapes.iter().any(|s| match &s.shape {
        egui::epaint::Shape::Rect(r) => {
            (r.fill == STUDIO_SELECTION || r.fill == STUDIO_SELECTION_HOVER)
                && r.rect.width() > 60.0
                && r.rect.contains(bounds.center())
        }
        _ => false,
    })
}

/// A row of the dock is on screen: drawn, and inside the dock's list.
fn row_is_visible(rig: &WindowRig, label: &str) -> bool {
    texts_in(rig, DOCK())
        .iter()
        .any(|t| t.0 == label && t.1.top() >= 395.0 && t.1.bottom() <= 780.0)
}

fn plan_rig() -> (WindowRig, Vec<(&'static str, NodeId)>) {
    let (mut rig, named) = populated();
    // The starter project hides its lights in the viewport.
    rig.ws.show_lights = true;
    rig.ws.apply_grid_pose("view=plan;tool=select").unwrap();
    frames(&mut rig, 4);
    (rig, named)
}

fn node_name(rig: &WindowRig, id: NodeId) -> String {
    rig.ws.project.active_scene().node(id).unwrap().name.clone()
}

#[test]
fn clicking_an_object_in_the_plan_selects_its_row_and_scrolls_it_into_view() {
    let (mut rig, named) = plan_rig();
    let far = named[2].1;
    let name = node_name(&rig, far);
    // Down the list, out of the Objects panel's first screen.
    assert!(
        !row_is_visible(&rig, &name),
        "the fixture is too short to scroll"
    );
    // Light 15 stands in cell (2, 3).
    rig.click(rig.cell(2, 3));
    frames(&mut rig, 4);
    assert_eq!(rig.ws.selection.selected_node, far, "{}", rig.ws.status);
    assert!(
        row_is_visible(&rig, &name),
        "{name} is not on screen: {:?}",
        dock_texts(&rig)
    );
    assert!(row_is_highlighted(&rig, &name));
    assert!(
        inspector_texts(&rig)
            .iter()
            .any(|t| t.starts_with("Light:")),
        "{:?}",
        inspector_texts(&rig)
    );
}

#[test]
fn a_selection_made_in_the_3d_view_reveals_the_row_in_a_folded_group() {
    let (mut rig, named) = populated();
    let light = named[2].1;
    let name = node_name(&rig, light);
    // The author folded Lights; the 3D click goes through here.
    rig.ws.collapsed_object_groups.insert(ObjectGroup::Lights);
    frames(&mut rig, 3);
    assert!(!dock_texts(&rig).contains(&name));
    rig.ws
        .select_node_with_modifiers(light, egui::Modifiers::NONE);
    frames(&mut rig, 4);
    assert!(!rig
        .ws
        .collapsed_object_groups
        .contains(&ObjectGroup::Lights));
    assert!(row_is_visible(&rig, &name), "{:?}", dock_texts(&rig));
    assert!(row_is_highlighted(&rig, &name));
}

#[test]
fn selecting_an_actors_part_highlights_the_actors_row() {
    let (mut rig, named) = populated();
    let actor = named[0].1;
    let part = actor_parts(rig.ws.project.active_scene(), actor)[0];
    rig.ws.replace_node_selection(part);
    frames(&mut rig, 4);
    assert!(row_is_highlighted(&rig, "Aletha (Player)"));
}

#[test]
fn clicking_a_row_selects_it_and_highlights_it_in_the_viewport() {
    let (mut rig, named) = plan_rig();
    let actor_cell = rig.cell(3, 3);
    let _ = (named, actor_cell);
    // A light in the first screen of the list.
    let name = {
        let rows = object_rows(rig.ws.project.active_scene());
        rows.iter()
            .find(|r| r.kind == ObjectKind::Light)
            .unwrap()
            .name
            .clone()
    };
    let at = rig.at(&name);
    rig.click(at);
    frames(&mut rig, 3);
    let id = rig.ws.selection.selected_node;
    assert_eq!(node_name(&rig, id), name);
    // The plan draws the selected marker with the selection outline.
    let node = rig.ws.project.active_scene().node(id).unwrap();
    let t = node.transform.translation;
    let centre = ViewportTransform::from_focus(
        rig.ws.last_orthographic_viewport_rect,
        OrthographicView::Top,
        [rig.ws.orthographic_focus[0], rig.ws.orthographic_focus[2]],
        rig.ws.viewport_zoom,
    )
    .world_to_screen([t[0], t[2]]);
    let outline = selected_stroke(true).color;
    let outlined = rig.out.shapes.iter().any(|s| match &s.shape {
        egui::epaint::Shape::Circle(c) => {
            c.stroke.color == outline && c.center.distance(centre) < 14.0
        }
        _ => false,
    });
    assert!(outlined, "the light is not outlined in the plan");
}

// ---------------------------------------------------------------------
// Rooms.

/// A click on the face of room `room`'s cell `(cx, cz)` in the plan.
fn click_face(rig: &mut WindowRig, cx: i32, cz: i32) {
    rig.ws.apply_grid_pose("view=plan;tool=select").unwrap();
    frames(rig, 3);
    rig.click(rig.cell(cx, cz));
    frames(rig, 3);
}

/// Machine Hall is the second room: cells 7..14 by -1..6.
const HALL_CELL: (i32, i32) = (10, 2);

#[test]
fn clicking_a_face_selects_the_face_and_highlights_its_room_in_structure() {
    let mut rig = WindowRig::new();
    frames(&mut rig, 3);
    click_face(&mut rig, HALL_CELL.0, HALL_CELL.1);
    assert!(!rig.ws.grid.selection.is_empty(), "{}", rig.ws.status);
    let shown = inspector_texts(&rig);
    assert!(shown.iter().any(|t| t.contains("face")), "{shown:?}");
    assert!(
        row_is_highlighted(&rig, "Machine Hall  (64)"),
        "{:?}",
        dock_texts(&rig)
    );
    assert!(!row_is_highlighted(&rig, "Starter room  (36)"));
    // A face in the other room moves the highlight.
    rig.click(rig.cell(2, 2));
    frames(&mut rig, 3);
    assert!(row_is_highlighted(&rig, "Starter room  (36)"));
    assert!(!row_is_highlighted(&rig, "Machine Hall  (64)"));
}

#[test]
fn choosing_a_room_in_structure_shows_the_rooms_inspector() {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    frames(&mut rig, 3);
    let hall = rig.at("Machine Hall  (64)");
    rig.click(hall);
    frames(&mut rig, 3);
    assert_eq!(rig.ws.selection.selected_room, Some(1));
    assert!(row_is_highlighted(&rig, "Machine Hall  (64)"));
    let shown = inspector_texts(&rig);
    assert!(shown.iter().any(|t| t == "Room: Machine Hall"), "{shown:?}");
    for field in ["Raise", "Ambient", "Fog"] {
        assert!(
            shown.iter().any(|t| t == field),
            "{field} missing: {shown:?}"
        );
    }
    // The settings are no longer a section of the dock.
    assert!(
        !dock_texts(&rig)
            .iter()
            .any(|t| t.starts_with("Settings of")),
        "{:?}",
        dock_texts(&rig)
    );
    // Picking a face leaves the room.
    click_face(&mut rig, 2, 2);
    assert_eq!(rig.ws.selection.selected_room, None);
    assert!(inspector_texts(&rig).iter().any(|t| t.contains("face")));
}

#[test]
fn the_pin_still_says_where_new_geometry_goes_and_a_row_click_no_longer_pins() {
    let mut rig = WindowRig::new();
    frames(&mut rig, 3);
    let hall = rig.at("Machine Hall  (64)");
    rig.click(hall);
    frames(&mut rig, 2);
    assert_eq!(rig.ws.grid_active_room, None, "choosing is not pinning");
    rig.ws.toggle_room_pin(1);
    assert_eq!(rig.ws.grid_active_room, Some(1));
}

// ---------------------------------------------------------------------
// The Inspector of an actor.

fn has_text(texts: &[String], label: &str) -> bool {
    texts.iter().any(|t| t == label)
}

#[test]
fn an_actors_parts_are_sections_of_its_inspector_not_child_rows() {
    let (mut rig, named) = populated();
    rig.ws.replace_node_selection(named[0].1);
    frames(&mut rig, 4);
    let shown = inspector_texts(&rig);
    for section in ["Character", "Model", "Animation", "Camera", "Equipment"] {
        assert!(
            has_text(&shown, section),
            "{section} is not a section: {shown:?}"
        );
    }
    assert!(!has_text(&shown, "Components"), "{shown:?}");
    assert!(
        shown.iter().any(|t| t.contains("Add part")),
        "no way to add a part: {shown:?}"
    );
    // The dock still has the one row.
    assert!(!dock_texts(&rig).iter().any(|t| t == "Model Renderer"));
}

#[test]
fn selecting_a_part_shows_its_actor_with_that_part_open() {
    let (mut rig, named) = populated();
    let actor = named[0].1;
    let animator = actor_parts(rig.ws.project.active_scene(), actor)
        .into_iter()
        .find(|id| {
            matches!(
                rig.ws.project.active_scene().node(*id).unwrap().kind,
                NodeKind::Animator { .. }
            )
        })
        .unwrap();
    rig.ws.replace_node_selection(animator);
    frames(&mut rig, 5);
    let shown = inspector_texts(&rig);
    assert!(has_text(&shown, "Aletha (Player)"), "{shown:?}");
    // The Animator's own fields are in view: its section is open.
    assert!(shown.iter().any(|t| t.contains("Autoplay")), "{shown:?}");
}

#[test]
fn add_part_puts_a_new_section_in_the_actors_inspector() {
    let (mut rig, _) = populated();
    let actor = rig
        .ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find(|n| n.name == "Intake Custodian 1")
        .unwrap()
        .id;
    rig.ws.replace_node_selection(actor);
    frames(&mut rig, 4);
    assert!(!has_text(&inspector_texts(&rig), "Camera"));
    let add = rig
        .out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text().contains("Add part") => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("an Add part button");
    rig.click(add);
    frames(&mut rig, 3);
    // The menu opens under the button; the viewport header has a Camera too.
    let below = Rect::from_min_max(Pos2::new(add.x - 150.0, add.y), Pos2::new(1456.0, 819.0));
    let camera = texts_in(&rig, below)
        .into_iter()
        .find(|t| t.0 == "Camera")
        .expect("a Camera item in the Add part menu")
        .1
        .center();
    rig.click(camera);
    frames(&mut rig, 4);
    let scene = rig.ws.project.active_scene();
    assert!(
        actor_parts(scene, actor)
            .iter()
            .any(|id| matches!(scene.node(*id).unwrap().kind, NodeKind::Camera { .. })),
        "{}",
        rig.ws.status
    );
    assert!(has_text(&inspector_texts(&rig), "Camera"));
}

#[test]
fn an_interaction_is_one_section_with_a_kind_and_switching_keeps_what_they_share() {
    let (mut rig, _) = populated();
    let beacon = rig
        .ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find(|n| n.name == "Point of Interest")
        .unwrap()
        .id;
    rig.ws.replace_node_selection(beacon);
    frames(&mut rig, 4);
    let shown = inspector_texts(&rig);
    assert!(has_text(&shown, "Interaction"), "{shown:?}");
    assert!(
        has_text(&shown, "Point of interest"),
        "the kind combo: {shown:?}"
    );
    // The conversion is a pure function of the part.
    let part = interaction_part(rig.ws.project.active_scene(), beacon).unwrap();
    let mut kind = rig
        .ws
        .project
        .active_scene()
        .node(part)
        .unwrap()
        .kind
        .clone();
    let NodeKind::PointOfInterest {
        prompt,
        radius,
        enabled,
        ..
    } = &mut kind
    else {
        panic!("a beacon holds a Point of Interest");
    };
    *prompt = "TALK".to_string();
    *radius = 700;
    *enabled = false;
    let message = convert_interaction(&kind, InteractionKind::Message).unwrap();
    assert!(matches!(
        &message,
        NodeKind::Interactable { prompt, radius: 700, enabled: false, kind: InteractableKind::Message { .. } } if prompt == "TALK"
    ));
    let back = convert_interaction(&message, InteractionKind::PointOfInterest).unwrap();
    assert!(matches!(
        &back,
        NodeKind::PointOfInterest { prompt, radius: 700, enabled: false, .. } if prompt == "TALK"
    ));
    let checkpoint = convert_interaction(&message, InteractionKind::Checkpoint).unwrap();
    assert!(matches!(
        &checkpoint,
        NodeKind::Interactable { prompt, radius: 700, enabled: false, kind: InteractableKind::Checkpoint { .. } } if prompt == "TALK"
    ));
    assert_eq!(
        convert_interaction(&kind, InteractionKind::PointOfInterest),
        None,
        "already that kind"
    );
}

// ---------------------------------------------------------------------
// The World.

fn dock_count(rig: &WindowRig, label: &str) -> usize {
    dock_texts(rig).iter().filter(|t| *t == label).count()
}

#[test]
fn the_world_has_one_entry_in_structure_and_it_opens_the_world_settings() {
    let (mut rig, _) = populated();
    assert_eq!(dock_count(&rig, "World"), 1, "{:?}", dock_texts(&rig));
    // The tab of the top bar is also called World, and is not in the dock.
    let entry = texts_in(&rig, DOCK())
        .into_iter()
        .find(|t| t.0 == "World")
        .unwrap()
        .1
        .center();
    rig.click(entry);
    frames(&mut rig, 4);
    assert!(rig.ws.selection.selected_nodes.contains(&NodeId::ROOT));
    assert!(row_is_highlighted(&rig, "World"));
    let shown = inspector_texts(&rig);
    for section in ["Physics", "Sky", "Shading", "Streaming", "Default camera"] {
        assert!(has_text(&shown, section), "{section} missing: {shown:?}");
    }
    // The World is not a row of the Objects list, nor is it offered twice.
    assert_eq!(dock_count(&rig, "World"), 1);
    assert!(!shown.iter().any(|t| t == "World settings"), "{shown:?}");
}

#[test]
fn nothing_selected_does_not_offer_a_second_way_to_the_world() {
    let (rig, _) = populated();
    let shown = inspector_texts(&rig);
    assert!(has_text(&shown, "Properties"), "{shown:?}");
    assert!(
        !shown.iter().any(|t| t.contains("World settings")),
        "the Inspector button is the duplicate: {shown:?}"
    );
}

#[test]
fn shading_belongs_to_the_world_and_not_to_a_room() {
    let mut rig = WindowRig::new();
    frames(&mut rig, 3);
    rig.ws.select_room(0);
    frames(&mut rig, 4);
    assert!(
        !has_text(&inspector_texts(&rig), "Shading"),
        "{:?}",
        inspector_texts(&rig)
    );
    rig.ws.select_world();
    frames(&mut rig, 4);
    assert!(has_text(&inspector_texts(&rig), "Shading"));
}

#[test]
fn the_world_settings_hold_the_stream_radii_and_the_default_camera() {
    let mut rig = WindowRig::new();
    rig.ws.select_world();
    frames(&mut rig, 4);
    // Open the two sections.
    for section in ["Streaming", "Default camera"] {
        let at = rig.at(section);
        rig.click(at);
        frames(&mut rig, 3);
    }
    let shown = inspector_texts(&rig);
    for field in ["Draw far", "Active", "Lead"] {
        assert!(has_text(&shown, field), "{field}: {shown:?}");
    }
    assert!(
        shown.iter().any(|t| t.contains("no Camera part")),
        "the camera says when it applies: {shown:?}"
    );
    // A radius is edited by dragging its value.
    let before = rig.world().stream;
    let at = rig.at("Draw far");
    let value = at + Vec2::new(90.0, 0.0);
    rig.drag(value, value + Vec2::new(24.0, 0.0));
    frames(&mut rig, 3);
    assert_ne!(rig.world().stream, before, "{}", rig.ws.status);
}

#[test]
fn levels_are_storeys() {
    let rig = WindowRig::new();
    let texts = dock_texts(&rig);
    assert!(has_text(&texts, "Storeys"), "{texts:?}");
    assert!(has_text(&texts, "All storeys"), "{texts:?}");
    assert!(!has_text(&texts, "Levels"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("Storey 0")), "{texts:?}");
}

// ---------------------------------------------------------------------
// Menus.

#[test]
fn spawn_points_and_arch_props_are_not_placeable_but_existing_ones_still_list() {
    let labels: Vec<&str> = PlaceKind::ALL.iter().map(|k| k.label()).collect();
    for gone in ["Spawn", "Player Spawn", "Arch Prop"] {
        assert!(!labels.contains(&gone), "{gone} is in Place: {labels:?}");
    }
    for kept in [
        "Prop",
        "Character",
        "Image Prop",
        "Box Prop",
        "Cylinder Prop",
        "Point Light",
    ] {
        assert!(labels.contains(&kept), "{kept} left Place: {labels:?}");
    }
    // Old projects still carry them: they load, list and cook as before.
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    let spawn = rig.ws.project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Start",
        NodeKind::SpawnPoint {
            player: false,
            character: None,
        },
    );
    let arch = rig.ws.project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Old arch",
        NodeKind::ArchProp {
            materials: [None; psxed_project::ARCH_PROP_MATERIAL_COUNT],
            uvs: [UvTransform::IDENTITY; psxed_project::ARCH_PROP_MATERIAL_COUNT],
            geometry: psxed_project::ArchPropGeometry::default(),
            collision_enabled: false,
        },
    );
    let rows = object_rows(rig.ws.project.active_scene());
    assert_eq!(
        rows.iter().find(|r| r.id == spawn).unwrap().kind,
        ObjectKind::SpawnPoint
    );
    assert_eq!(
        rows.iter().find(|r| r.id == arch).unwrap().kind,
        ObjectKind::ArchProp
    );
}

#[test]
fn collider_node3d_and_mesh_instance_are_in_no_menu() {
    for (label, kind) in scene_graph_addable_kinds() {
        assert!(
            !matches!(
                kind,
                NodeKind::Collider { .. }
                    | NodeKind::Node3D
                    | NodeKind::MeshInstance { .. }
                    | NodeKind::Node
            ),
            "{label} is offered"
        );
    }
    let (rig, named) = populated();
    for (shown, node_label, _) in
        crate::workspace::actor_inspector::addable_parts(rig.ws.project.active_scene(), named[0].1)
    {
        assert_ne!(node_label, "Collider", "{shown}");
        assert_ne!(node_label, "Interactable", "{shown}");
    }
}

#[test]
fn the_objects_menu_says_actor_where_it_said_entity() {
    let mut rig = WindowRig::new();
    frames(&mut rig, 3);
    let place = rig.at("Place");
    let arrow = Pos2::new(place.x + 40.0, place.y);
    rig.click(arrow);
    frames(&mut rig, 3);
    let texts: Vec<String> = texts_in(
        &rig,
        Rect::from_min_max(Pos2::new(700.0, 60.0), Pos2::new(1100.0, 200.0)),
    )
    .into_iter()
    .map(|t| t.0)
    .collect();
    for item in ["Light", "Prop", "Actor"] {
        assert!(texts.iter().any(|t| t == item), "{item}: {texts:?}");
    }
    for gone in ["Entity", "Spawn"] {
        assert!(!texts.iter().any(|t| t == gone), "{gone}: {texts:?}");
    }
}
