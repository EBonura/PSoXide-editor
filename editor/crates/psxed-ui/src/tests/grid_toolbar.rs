//! The top of the grid editor: the top bar, the tool row, the viewport
//! header and the options bar (the layout Manny approved on 2026-10-20).
//!
//! Everything here drives the whole editor window (`WindowRig`, real fonts,
//! real pointer events), so a button that is drawn but cannot be clicked, or
//! two buttons on top of each other, fail.

use super::grid_live::WindowRig;
use super::*;
use crate::grid_tools::GridTool;

/// Captures for the review pages: `PSXED_TOOLBAR_CAPTURE=<dir>
/// PSXED_TOOLBAR_LABEL=before|after` writes the whole window of both worlds
/// at both window sizes; without the variable this does nothing.
#[test]
fn toolbar_captures() {
    let Some(dir) = std::env::var_os("PSXED_TOOLBAR_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_TOOLBAR_LABEL").unwrap_or_else(|_| "after".into());
    for (world, tag) in [(0, "dp-world"), (1, "grid-starter")] {
        for size in [Vec2::new(1456.0, 819.0), Vec2::new(1920.0, 1080.0)] {
            let mut rig = if world == 0 {
                WindowRig::with_size(size)
            } else {
                WindowRig::starter(size)
            };
            rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
            for _ in 0..4 {
                rig.frame(vec![]);
            }
            rig.capture(&dir.join(format!(
                "{tag}-{}x{}-{label}.png",
                size.x as u32, size.y as u32
            )));
        }
    }
    // The three menus of the tool row, open.
    for (arrow_of, name) in [
        ("Ceiling", "create"),
        ("Raise", "shape"),
        ("Place", "objects"),
    ] {
        let mut rig = WindowRig::new();
        rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
        frames(&mut rig, 4);
        open_arrow(&mut rig, arrow_of);
        rig.capture(&dir.join(format!("menu-{name}-1456x819-{label}.png")));
    }
}

// ---------------------------------------------------------------------
// Helpers

/// Every text the last frame drew, with its bounds.
fn texts(rig: &WindowRig) -> Vec<(String, Rect)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.text().to_string(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect()
}

/// The first text `label` (icon prefix ignored) inside `band`.
fn text_in(rig: &WindowRig, band: Rect, label: &str) -> Option<Rect> {
    texts(rig)
        .into_iter()
        .find(|(t, r)| {
            band.contains(r.center())
                && t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label
        })
        .map(|(_, r)| r)
}

/// The window's top bar.
fn bar(rig: &WindowRig) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(rig.size.x, ACTION_BAR_COMPACT_HEIGHT))
}

fn in_bar(rig: &WindowRig, label: &str) -> Rect {
    text_in(rig, bar(rig), label).unwrap_or_else(|| panic!("no `{label}` in the top bar"))
}

fn frames(rig: &mut WindowRig, n: usize) {
    for _ in 0..n {
        rig.frame(vec![]);
    }
}

/// Filled rectangles of the last frame inside `band`: `(rect, fill)`.
fn fills_in(rig: &WindowRig, band: Rect) -> Vec<(Rect, Color32)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Rect(r) if band.contains(r.rect.center()) => {
                Some((r.rect, r.fill))
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------
// Step 1: the top bar. One row: the project (name, unsaved dot) and the
// menus on the left, the workspaces in the middle, Undo, Redo, Build and
// Play on the right; Play is the only filled button.

const TOP_BAR_ORDER: [&str; 13] = [
    "File",
    "Edit",
    "View",
    "Tools",
    "Help",
    "World",
    "HUD",
    "Animation",
    "Material",
    "Undo",
    "Redo",
    "Build",
    "Play",
];

#[test]
fn the_top_bar_is_one_row_in_the_approved_order() {
    for width in [1280.0, 1456.0, 1920.0] {
        let rig = WindowRig::with_size(Vec2::new(width, 819.0));
        let name = rig.ws.project.name.clone();
        let mut order: Vec<(&str, Rect)> = vec![(name.as_str(), in_bar(&rig, &name))];
        for label in TOP_BAR_ORDER {
            order.push((label, in_bar(&rig, label)));
        }
        let row_y = order[0].1.center().y;
        for (label, r) in &order {
            assert!(
                (r.center().y - row_y).abs() < 4.0,
                "{width}: `{label}` is on another row: {r:?}"
            );
        }
        for pair in order.windows(2) {
            let ((a, ra), (b, rb)) = (pair[0], pair[1]);
            assert!(
                ra.max.x < rb.min.x,
                "{width}: `{a}` {ra:?} is not left of `{b}` {rb:?}"
            );
        }
    }
}

#[test]
fn the_workspaces_are_a_control_centred_in_the_bar() {
    for width in [1280.0, 1456.0, 1920.0] {
        let rig = WindowRig::with_size(Vec2::new(width, 819.0));
        let first = in_bar(&rig, "World");
        let last = in_bar(&rig, "Material");
        let middle = (first.min.x + last.max.x) / 2.0;
        assert!(
            (middle - width / 2.0).abs() < 24.0,
            "{width}: the workspaces are centred on {middle}"
        );
    }
}

#[test]
fn the_unsaved_dot_replaces_the_badge() {
    let mut rig = WindowRig::new();
    let name = rig.ws.project.name.clone();
    let dots = |rig: &WindowRig| -> Vec<Pos2> {
        rig.out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Circle(c)
                    if c.radius <= 5.0 && bar(rig).contains(c.center) =>
                {
                    Some(c.center)
                }
                _ => None,
            })
            .collect()
    };
    assert!(!rig.ws.dirty, "the fixture starts saved");
    assert!(dots(&rig).is_empty(), "a saved project shows no dot");
    rig.ws.mark_dirty();
    frames(&mut rig, 2);
    let name_rect = in_bar(&rig, &name);
    let dots = dots(&rig);
    assert_eq!(dots.len(), 1, "{dots:?}");
    assert!(
        dots[0].x > name_rect.max.x && dots[0].x < name_rect.max.x + 24.0,
        "the dot {:?} follows the name {name_rect:?}",
        dots[0]
    );
    let anywhere = texts(&rig)
        .into_iter()
        .any(|(t, _)| t.to_uppercase().contains("UNSAVED"));
    assert!(!anywhere, "the badge is gone");
}

#[test]
fn play_is_the_only_filled_button_of_the_bar() {
    let rig = WindowRig::new();
    let fills = fills_in(&rig, bar(&rig));
    let play = in_bar(&rig, "Play");
    let play_fill = fills
        .iter()
        .find(|(r, _)| r.contains(play.center()) && r.width() < 140.0)
        .map(|(_, c)| *c)
        .expect("a fill under Play");
    assert_eq!(play_fill, STUDIO_PLAY_FILL);
    for label in TOP_BAR_ORDER.iter().filter(|l| **l != "Play") {
        let at = in_bar(&rig, label).center();
        assert!(
            !fills
                .iter()
                .any(|(r, c)| *c == STUDIO_PLAY_FILL && r.contains(at)),
            "`{label}` sits on the Play colour"
        );
    }
}

#[test]
fn the_workspace_control_switches_the_workspace() {
    let mut rig = WindowRig::new();
    for (label, view) in [
        ("HUD", WorkspaceView::Ui),
        ("Animation", WorkspaceView::Animation),
        ("Material", WorkspaceView::Material),
        ("World", WorkspaceView::Room),
    ] {
        let at = in_bar(&rig, label).center();
        rig.click(at);
        frames(&mut rig, 2);
        assert_eq!(rig.ws.active_workspace, view, "{label}");
    }
}

#[test]
fn undo_redo_build_and_play_work_from_the_bar() {
    let mut rig = WindowRig::new();
    let before = rig.world().clone();
    rig.draw_floor([6, 8], [6, 9]);
    assert_ne!(*rig.world(), before);
    frames(&mut rig, 2);
    let undo = in_bar(&rig, "Undo").center();
    rig.click(undo);
    assert_eq!(*rig.world(), before, "Undo");
    frames(&mut rig, 2);
    let redo = in_bar(&rig, "Redo").center();
    rig.click(redo);
    assert_ne!(*rig.world(), before, "Redo");
    frames(&mut rig, 2);
    let build = in_bar(&rig, "Build").center();
    rig.click(build);
    assert!(matches!(
        rig.ws.pending_playtest_request,
        Some(EditorPlaytestRequest::BuildProject)
    ));
    rig.ws.pending_playtest_request = None;
    let play = in_bar(&rig, "Play").center();
    rig.click(play);
    assert!(matches!(
        rig.ws.pending_playtest_request,
        Some(EditorPlaytestRequest::Play)
    ));
}

#[test]
fn the_play_menu_keeps_its_arrow() {
    let mut rig = WindowRig::new();
    let play = in_bar(&rig, "Play");
    let fills = fills_in(&rig, bar(&rig));
    // The arrow is the filled segment right of the Play face.
    let arrow = fills
        .iter()
        .filter(|(r, c)| *c == STUDIO_PLAY_FILL && r.min.x > play.max.x)
        .map(|(r, _)| r.center())
        .next()
        .expect("an arrow segment beside Play");
    rig.click(arrow);
    frames(&mut rig, 2);
    assert!(rig.find("UI Scenes").is_some(), "the boot menu opened");
}

#[test]
fn clicking_the_project_name_renames_it() {
    let mut rig = WindowRig::new();
    let name = rig.ws.project.name.clone();
    let at = in_bar(&rig, &name).center();
    rig.click(at);
    frames(&mut rig, 2);
    assert!(rig.ws.project_name_editing);
}

#[test]
fn a_running_build_shows_in_the_bar_without_moving_anything() {
    let mut rig = WindowRig::new();
    let play_idle = in_bar(&rig, "Play");
    let ctx = rig.ctx.clone();
    let viewport = rig.viewport.clone();
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, rig.size)),
            time: Some(10.0),
            ..egui::RawInput::default()
        },
        |ctx| {
            rig.ws
                .draw(ctx, viewport.clone(), EditorPlaytestStatus::Building)
        },
    );
    rig.out = out;
    assert!(
        text_in(&rig, bar(&rig), "BUILDING").is_some(),
        "the state is shown while it is not idle"
    );
    let chip = in_bar(&rig, "BUILDING");
    assert!(
        chip.min.x > in_bar(&rig, "Material").max.x + 8.0,
        "{chip:?}"
    );
    let play = in_bar(&rig, "Rebuild & Play");
    assert!((play.max.x - play_idle.max.x).abs() < 2.0, "{play:?}");
}

// ---------------------------------------------------------------------
// Step 2: the tool row. One line under the top bar, never wrapping:
// Select and Height, CREATE (Floor, Wall, Ceiling and a menu with Plane and
// Mesh), SHAPE (a split button for the last used shape tool), SURFACE
// (Paint, Bands), OBJECTS (a Place split button with Light, Spawn, Prop,
// Entity). Each tool shows its key next to its name.

/// The strip between the top bar and the docks.
fn tool_band(rig: &WindowRig) -> Rect {
    Rect::from_min_max(
        Pos2::new(0.0, ACTION_BAR_COMPACT_HEIGHT),
        Pos2::new(rig.size.x, ACTION_BAR_COMPACT_HEIGHT + TOOL_ROW_HEIGHT),
    )
}

fn in_row(rig: &WindowRig, label: &str) -> Rect {
    text_in(rig, tool_band(rig), label).unwrap_or_else(|| panic!("no `{label}` in the tool row"))
}

fn row_click(rig: &mut WindowRig, label: &str) {
    let at = in_row(rig, label).center();
    rig.click(at);
    frames(rig, 2);
}

/// A menu item: the text `label` just below the tool row (the scene tree and
/// the inspector can say "Entity" or "Raise" too, lower down).
fn menu_item(rig: &WindowRig, label: &str) -> Option<Rect> {
    let below = Rect::from_min_max(
        Pos2::new(0.0, tool_band(rig).max.y),
        Pos2::new(rig.size.x, tool_band(rig).max.y + 260.0),
    );
    text_in(rig, below, label)
}

/// The arrow (a chevron glyph) of the split button or menu right after `label`.
fn arrow_after(rig: &WindowRig, label: &str) -> Pos2 {
    let name = in_row(rig, label);
    texts(rig)
        .into_iter()
        .filter(|(t, r)| {
            t.trim() == icons::CHEVRON_DOWN.to_string()
                && tool_band(rig).contains(r.center())
                && r.min.x > name.max.x
        })
        .map(|(_, r)| r)
        .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .unwrap_or_else(|| panic!("no arrow after `{label}`"))
        .center()
}

fn open_arrow(rig: &mut WindowRig, label: &str) {
    let at = arrow_after(rig, label);
    rig.click(at);
    frames(rig, 2);
}

/// A key press with the frames a real key takes.
fn press_key(rig: &mut WindowRig, key: egui::Key, shift: bool) {
    let modifiers = if shift {
        egui::Modifiers::SHIFT
    } else {
        egui::Modifiers::NONE
    };
    for pressed in [true, false] {
        rig.frame(vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }]);
    }
    frames(rig, 2);
}

const ROW_TOOLS: [(&str, GridTool); 8] = [
    ("Select", GridTool::Select),
    ("Height", GridTool::Height),
    ("Floor", GridTool::Floor),
    ("Wall", GridTool::Wall),
    ("Ceiling", GridTool::Ceiling),
    ("Paint", GridTool::Paint),
    ("Bands", GridTool::Bands),
    ("Place", GridTool::Place),
];

#[test]
fn the_tool_row_is_one_line_at_every_window_width() {
    for width in [1280.0, 1456.0, 1920.0] {
        let rig = WindowRig::with_size(Vec2::new(width, 819.0));
        let mut seen: Vec<(&str, Rect)> = Vec::new();
        for label in [
            "Select", "Height", "Floor", "Wall", "Ceiling", "Raise", "Paint", "Bands", "Place",
        ] {
            seen.push((label, in_row(&rig, label)));
        }
        let y = seen[0].1.center().y;
        for (label, r) in &seen {
            assert!(
                (r.center().y - y).abs() < 4.0,
                "{width}: `{label}` wrapped to another line: {r:?}"
            );
        }
        for pair in seen.windows(2) {
            assert!(
                pair[0].1.max.x < pair[1].1.min.x,
                "{width}: `{}` is not left of `{}`",
                pair[0].0,
                pair[1].0
            );
        }
        // The last control still fits the window.
        let last = arrow_after(&rig, "Place");
        assert!(last.x < width - 6.0, "{width}: the row is cut at {last:?}");
        // Under the bar, above every dock.
        let structure = text_in(
            &rig,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(300.0, rig.size.y)),
            "Structure",
        )
        .unwrap();
        assert!(structure.min.y > tool_band(&rig).max.y, "{width}");
    }
}

#[test]
fn the_groups_are_labelled_but_select_and_height_are_not() {
    let rig = WindowRig::new();
    let all: Vec<String> = texts(&rig)
        .into_iter()
        .filter(|(_, r)| tool_band(&rig).contains(r.center()))
        .map(|(t, _)| t)
        .collect();
    for label in ["CREATE", "SHAPE", "SURFACE", "OBJECTS"] {
        assert!(all.iter().any(|t| t == label), "no {label}: {all:?}");
    }
    for gone in ["PICK", "DRAW", "FINISH", "BUILD"] {
        assert!(!all.iter().any(|t| t == gone), "{gone} is still there");
    }
    // Nothing sits left of Select: it opens the row.
    let select = in_row(&rig, "Select");
    assert!(
        !texts(&rig)
            .iter()
            .any(|(_, r)| tool_band(&rig).contains(r.center()) && r.max.x < select.min.x),
        "something precedes Select"
    );
    let order: Vec<f32> = ["CREATE", "SHAPE", "SURFACE", "OBJECTS"]
        .iter()
        .map(|l| in_row(&rig, l).min.x)
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}

#[test]
fn every_visible_tool_shows_its_key_beside_its_name() {
    let rig = WindowRig::new();
    for (label, tool) in ROW_TOOLS {
        let name = in_row(&rig, label);
        let key = tool.key_label();
        let beside = texts(&rig).into_iter().any(|(t, r)| {
            t == key
                && (r.center().y - name.center().y).abs() < 5.0
                && r.min.x >= name.max.x
                && r.min.x < name.max.x + 16.0
        });
        assert!(beside, "`{label}` shows no `{key}` beside it");
    }
    let raise = in_row(&rig, "Raise");
    assert!(texts(&rig)
        .iter()
        .any(|(t, r)| t == "U" && r.min.x >= raise.max.x && r.min.x < raise.max.x + 16.0));
}

#[test]
fn each_visible_tool_button_selects_its_tool() {
    let mut rig = WindowRig::new();
    for (label, tool) in ROW_TOOLS {
        // Start from another tool each time.
        rig.ws.set_grid_tool(if tool == GridTool::Select {
            GridTool::Floor
        } else {
            GridTool::Select
        });
        frames(&mut rig, 2);
        row_click(&mut rig, label);
        assert_eq!(rig.ws.grid.tool, tool, "{label}");
    }
}

#[test]
fn the_create_menu_holds_plane_and_mesh() {
    let mut rig = WindowRig::new();
    assert!(
        text_in(&rig, tool_band(&rig), "Plane").is_none(),
        "Plane is behind the arrow"
    );
    open_arrow(&mut rig, "Ceiling");
    for item in ["Plane", "Mesh"] {
        assert!(menu_item(&rig, item).is_some(), "{item} is in the menu");
    }
    let plane = menu_item(&rig, "Plane").unwrap().center();
    rig.click(plane);
    frames(&mut rig, 2);
    assert_eq!(rig.ws.grid.tool, GridTool::Plane);
    assert!(menu_item(&rig, "Mesh").is_none(), "the menu closed");
    open_arrow(&mut rig, "Ceiling");
    let mesh = menu_item(&rig, "Mesh").unwrap().center();
    rig.click(mesh);
    frames(&mut rig, 2);
    assert_eq!(rig.ws.grid.tool, GridTool::Mesh);
}

const SHAPES: [(&str, GridTool); 9] = [
    ("Raise", GridTool::Raise),
    ("Ramp", GridTool::Ramp),
    ("Stairs", GridTool::Stairs),
    ("Spiral", GridTool::Spiral),
    ("Break", GridTool::Break),
    ("Rough", GridTool::Rough),
    ("Cells", GridTool::Cells),
    ("Slab", GridTool::Slab),
    ("Diagonal", GridTool::Split),
];

#[test]
fn the_shape_button_uses_the_last_shape_tool_and_the_arrow_lists_all_nine() {
    let mut rig = WindowRig::new();
    // The face shows Raise to begin with; clicking it uses it.
    row_click(&mut rig, "Raise");
    assert_eq!(rig.ws.grid.tool, GridTool::Raise);
    // The arrow lists every shape tool with its key.
    open_arrow(&mut rig, "Raise");
    for (label, tool) in SHAPES {
        assert!(
            menu_item(&rig, label).is_some(),
            "no {label} in the menu, below the row"
        );
        assert!(
            texts(&rig).iter().any(|(t, _)| t == tool.key_label()),
            "no key `{}` for {label}",
            tool.key_label()
        );
    }
    // Picking one uses it, closes the menu, and the face now shows it.
    let stairs = menu_item(&rig, "Stairs").unwrap().center();
    rig.click(stairs);
    frames(&mut rig, 2);
    assert_eq!(rig.ws.grid.tool, GridTool::Stairs);
    assert!(
        text_in(&rig, tool_band(&rig), "Stairs").is_some(),
        "the face shows the last used shape tool"
    );
    assert!(text_in(&rig, tool_band(&rig), "Raise").is_none());
    // Another tool does not change it; clicking the face returns to it.
    row_click(&mut rig, "Floor");
    assert_eq!(rig.ws.grid.tool, GridTool::Floor);
    assert!(text_in(&rig, tool_band(&rig), "Stairs").is_some());
    row_click(&mut rig, "Stairs");
    assert_eq!(rig.ws.grid.tool, GridTool::Stairs);
}

#[test]
fn a_shape_key_sets_the_face_too_and_diagonal_is_a_shape() {
    let mut rig = WindowRig::new();
    press_key(&mut rig, egui::Key::X, false);
    assert_eq!(rig.ws.grid.tool, GridTool::Break);
    assert!(text_in(&rig, tool_band(&rig), "Break").is_some());
    press_key(&mut rig, egui::Key::D, false);
    assert_eq!(rig.ws.grid.tool, GridTool::Split);
    assert!(
        text_in(&rig, tool_band(&rig), "Diagonal").is_some(),
        "Diagonal moved from FINISH to SHAPE"
    );
    let shape = in_row(&rig, "SHAPE");
    let surface = in_row(&rig, "SURFACE");
    let diagonal = in_row(&rig, "Diagonal");
    assert!(diagonal.min.x > shape.max.x && diagonal.max.x < surface.min.x);
}

#[test]
fn changing_the_shape_tool_does_not_move_the_rest_of_the_row() {
    let mut rig = WindowRig::new();
    let places = |rig: &WindowRig| -> Vec<Rect> {
        ["SURFACE", "Paint", "Bands", "OBJECTS", "Place"]
            .iter()
            .map(|l| in_row(rig, l))
            .collect()
    };
    let before = places(&rig);
    for (_, tool) in SHAPES {
        rig.ws.set_grid_tool(tool);
        frames(&mut rig, 2);
        assert_eq!(places(&rig), before, "{tool:?} moved the row");
    }
}

#[test]
fn the_place_menu_lists_light_spawn_prop_and_entity() {
    let mut rig = WindowRig::new();
    for (label, kind) in [
        ("Light", PlaceKind::PointLightMarker),
        ("Spawn", PlaceKind::SpawnMarker),
        ("Prop", PlaceKind::ModelInstance),
        ("Entity", PlaceKind::Character),
    ] {
        rig.ws.set_grid_tool(GridTool::Select);
        frames(&mut rig, 2);
        open_arrow(&mut rig, "Place");
        for name in ["Light", "Spawn", "Prop", "Entity"] {
            assert!(
                menu_item(&rig, name).is_some(),
                "{name} is in the Place menu"
            );
        }
        let item = menu_item(&rig, label).unwrap().center();
        rig.click(item);
        frames(&mut rig, 2);
        assert_eq!(rig.ws.grid.tool, GridTool::Place, "{label}");
        assert_eq!(rig.ws.place_kind, kind, "{label}");
    }
}

#[test]
fn every_tool_key_still_works() {
    let mut rig = WindowRig::new();
    for tool in GridTool::ALL {
        // From a tool the key does not belong to.
        rig.ws.set_grid_tool(if tool == GridTool::Select {
            GridTool::Floor
        } else {
            GridTool::Select
        });
        frames(&mut rig, 2);
        press_key(&mut rig, tool.key(), tool.shift());
        assert_eq!(rig.ws.grid.tool, tool, "{}", tool.label());
    }
}
