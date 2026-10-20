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

/// A scene shaped like the default project's: a player and enemies, beacons,
/// and lights in runs that an actor in between breaks up.
pub(super) fn populate(ws: &mut EditorWorkspace) -> Vec<(&'static str, NodeId)> {
    let mut named = Vec::new();
    let sector = 1280.0;
    let at = |x: f32, z: f32| [x * sector, 0.0, z * sector];
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
            at(2.5, 2.5),
            vec![model(), animator(), controller(true), camera(), equipment()],
        ),
    };
    named.push(("actor", aletha));
    for (i, x) in [3.5f32, 4.5, 5.5].into_iter().enumerate() {
        add_actor(
            ws,
            &format!("Intake Custodian {}", i + 1),
            at(x, 1.5),
            vec![model(), animator(), controller(false)],
        );
    }
    add_actor(
        ws,
        "Heavy Enemy",
        at(4.5, 3.5),
        vec![model(), animator(), controller(false)],
    );
    let mut name = "Point Light".to_string();
    for i in 0..6 {
        let light = add_light(ws, &name, at(1.5 + i as f32 * 0.7, 4.5));
        if i == 2 {
            named.push(("light", light));
        }
        name.push_str(" Copy");
    }
    add_actor(ws, "Point of Interest", at(1.5, 1.5), vec![beacon()]);
    for i in 0..4 {
        add_light(ws, &name, at(1.0 + i as f32 * 0.9, 0.5));
        name.push_str(" Copy");
    }
    add_actor(ws, "Point of Interest", at(5.5, 4.5), vec![beacon()]);
    ws.project
        .active_scene_mut()
        .add_node(NodeId::ROOT, "GroupS", NodeKind::Group);
    for i in 0..5 {
        add_light(ws, &name, at(0.5 + i as f32 * 1.1, 5.0));
        name.push_str(" Copy");
    }
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
        let light = named[1].1;
        rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
        rig.ws.clear_node_selection_state();
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-objects-list-{label}.png")));
        rig.ws.replace_node_selection(actor);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-actor-selected-{label}.png")));
        rig.ws.replace_node_selection(light);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-light-selected-{label}.png")));
        rig.ws.clear_node_selection_state();
        select_room_for_capture(&mut rig.ws);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-room-selected-{label}.png")));
        open_world_for_capture(&mut rig.ws);
        frames(&mut rig, 4);
        rig.capture(&dir.join(format!("{tag}-world-settings-{label}.png")));
    }
}

fn select_room_for_capture(ws: &mut EditorWorkspace) {
    // BEFORE: rooms are not selectable; their settings open in the dock.
    ws.grid.room_settings_open = true;
}

fn open_world_for_capture(ws: &mut EditorWorkspace) {
    // BEFORE: the World row of the Scene Graph.
    ws.grid.room_settings_open = false;
    ws.replace_node_selection(NodeId::ROOT);
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
