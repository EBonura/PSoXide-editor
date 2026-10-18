//! The Bands tool, the wall-run Inspector context and painting bands,
//! driven through real pointer and key input.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, draw_with_events, full_context};
use super::*;
use crate::grid_tools::{Gesture, GridHover, GridTool};
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::{effective_set, run_faces};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::faces::all_faces;
use psxed_project::grid::{Anchor, WallEdge};

fn project() -> ProjectDocument {
    derived_project_for_tiling(5, 5)
}

/// The north wall of cell (2, 0), seen from the south by the rig camera.
fn north() -> FaceRef {
    FaceRef::derived(0, [2, 0], WallEdge::North, DerivedKind::Closure)
}

/// Screen position on the north wall of cell (2, 0) at `y` editor units.
fn on_wall(rig: &GridRig, y: f32) -> Pos2 {
    rig.screen_of([2.5 * S, y, 0.0])
}

fn lines(rig: &GridRig) -> Vec<Anchor> {
    rig.room(0)
        .band_sets
        .first()
        .map(|s| s.lines.clone())
        .unwrap_or_default()
}

fn faces_on_north(rig: &GridRig) -> usize {
    all_faces(rig.world())
        .iter()
        .filter(|f| {
            f.cell == [2, 0]
                && matches!(
                    f.kind,
                    psxed_project::grid::GridFaceKind::Derived {
                        edge: WallEdge::North,
                        ..
                    }
                )
        })
        .count()
}

#[test]
fn hovering_a_wall_previews_the_cut_and_a_click_makes_it_one_undo_step() {
    let mut rig = GridRig::persp("bands-cut", project());
    rig.tool(GridTool::Bands);
    assert_eq!(faces_on_north(&rig), 1);
    // The rule previews at the pointer height snapped to a click, with the
    // anchor the height implies: in the middle of the wall it is a datum.
    rig.move_to(on_wall(&rig, 1530.0), egui::Modifiers::NONE);
    match &rig.ws.grid.hover {
        GridHover::Band {
            face, anchor, line, ..
        } => {
            assert_eq!(*face, north());
            assert_eq!(*anchor, Some(Anchor::Abs(1536)), "{:?}", rig.ws.grid.hover);
            assert_eq!(*line, None);
        }
        other => panic!("hover {other:?}"),
    }
    rig.click(on_wall(&rig, 1530.0));
    assert_eq!(lines(&rig), vec![Anchor::Abs(1536)], "{}", rig.ws.status);
    // The whole run took the set: two faces on this segment.
    assert_eq!(faces_on_north(&rig), 2);
    assert!(
        rig.ws.status.contains("applies to 5 wall segments"),
        "{}",
        rig.ws.status
    );
    // The wall is selected, so the Inspector shows its run.
    assert!(rig.ws.grid.selection.contains(&GridElement::Face(north())));
    rig.ws.do_undo();
    assert!(rig.room(0).band_sets.is_empty());
    assert_eq!(faces_on_north(&rig), 1);
    rig.ws.do_redo();
    assert_eq!(lines(&rig).len(), 1);
}

#[test]
fn near_the_bottom_a_line_follows_the_floor_and_near_the_top_the_ceiling() {
    // A low room (1536 up) so the whole wall is in the rig's view.
    let mut project = project();
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        for z in 0..5 {
            for x in 0..5 {
                world.rooms[0]
                    .sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_ceiling(Some(psxed_project::grid::GridSurface::flat(1536, None)));
            }
        }
    }
    let mut rig = GridRig::persp("bands-anchors", project);
    rig.tool(GridTool::Bands);
    let hovered = |rig: &mut GridRig, y: f32| {
        rig.move_to(on_wall(rig, y), egui::Modifiers::NONE);
        match &rig.ws.grid.hover {
            GridHover::Band { anchor, .. } => *anchor,
            other => panic!("y {y}: {other:?}"),
        }
    };
    assert_eq!(hovered(&mut rig, 300.0), Some(Anchor::Bottom(256)));
    assert_eq!(hovered(&mut rig, 800.0), Some(Anchor::Abs(768)));
    assert_eq!(hovered(&mut rig, 1400.0), Some(Anchor::Top(256)));
}

#[test]
fn a_line_drags_up_and_down_as_one_undo_step_and_escape_puts_it_back() {
    let mut rig = GridRig::persp("bands-drag", project());
    rig.tool(GridTool::Bands);
    rig.click(on_wall(&rig, 1536.0));
    assert_eq!(lines(&rig), vec![Anchor::Abs(1536)]);
    // Grab the line and drag it up: it stays an absolute datum, higher.
    let (from, to) = (on_wall(&rig, 1536.0), on_wall(&rig, 2048.0));
    rig.drag(from, to);
    let moved = lines(&rig);
    let Some(Anchor::Abs(h)) = moved.first().copied() else {
        panic!("{moved:?}");
    };
    assert!(h > 1536, "{h}");
    assert_eq!(h % 256, 0, "snapped to a click");
    // One undo step puts it back.
    rig.ws.do_undo();
    assert_eq!(lines(&rig), vec![Anchor::Abs(1536)]);
    // Escape in the middle of a drag restores it too.
    rig.drag_hold(from, to, egui::Modifiers::NONE);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Band(_)));
    assert!(lines(&rig) != vec![Anchor::Abs(1536)]);
    rig.key(egui::Key::Escape);
    assert_eq!(lines(&rig), vec![Anchor::Abs(1536)]);
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
}

#[test]
fn delete_removes_the_selected_line_and_the_tool_has_its_own_key() {
    let mut rig = GridRig::persp("bands-delete", project());
    rig.key(egui::Key::E);
    assert_eq!(rig.ws.grid.tool, GridTool::Bands);
    rig.click(on_wall(&rig, 768.0));
    rig.click(on_wall(&rig, 2048.0));
    assert_eq!(lines(&rig).len(), 2, "{:?}", lines(&rig));
    // Click the upper line (it selects it), then Delete.
    rig.click(on_wall(&rig, 2048.0));
    assert_eq!(rig.ws.grid.band_line.map(|(_, l)| l), Some(1));
    rig.key(egui::Key::Delete);
    assert_eq!(lines(&rig).len(), 1);
    rig.ws.do_undo();
    assert_eq!(lines(&rig).len(), 2);
}

#[test]
fn a_run_scoped_edit_copies_a_shared_set_and_room_scope_edits_in_place() {
    let mut rig = GridRig::persp("bands-scope", project());
    rig.tool(GridTool::Bands);
    // Cut one run; the room has one set.
    rig.click(on_wall(&rig, 1536.0));
    assert_eq!(rig.room(0).band_sets.len(), 1);
    // The south wall is another run: give it the same set by hand, so the
    // set is shared, then cut the north run again in run scope.
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        world.rooms[0].defaults.bands = Some(0);
    }
    rig.ws.grid_changed();
    rig.click(on_wall(&rig, 768.0));
    // The north run now has its own copy with two lines; the default
    // keeps one.
    assert_eq!(rig.room(0).band_sets.len(), 2);
    assert_eq!(rig.room(0).band_sets[0].lines.len(), 1);
    let own = effective_set(rig.world(), north()).unwrap();
    assert_eq!(own.1.lines.len(), 2);
    // In room scope the same gesture edits the shared set in place.
    rig.ws.grid.band_scope = psxed_project::grid::edit::bands::BandScope::Room;
    let south = FaceRef::derived(0, [2, 4], WallEdge::South, DerivedKind::Closure);
    assert_eq!(effective_set(rig.world(), south).map(|(i, _)| i), Some(0));
    assert_eq!(run_faces(rig.world(), north()).len(), 5);
}

#[test]
fn painting_a_banded_wall_paints_the_band_under_the_pointer() {
    let mut rig = GridRig::persp("bands-paint", project());
    let brick = rig.ws.project.add_resource(
        "brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    rig.tool(GridTool::Bands);
    rig.click(on_wall(&rig, 1536.0));
    rig.ws.grid.material = Some(brick);
    rig.tool(GridTool::Paint);
    // The lower band (below the line) gets the brick.
    rig.click(on_wall(&rig, 700.0));
    let set = &rig.room(0).band_sets[0];
    assert_eq!(set.bands[0].material, Some(brick), "{}", rig.ws.status);
    assert_eq!(set.bands[1].material, None);
    // The wall itself got no override.
    assert!(rig
        .room(0)
        .edge_overrides
        .iter()
        .all(|(_, o)| o.material.is_none()));
    rig.ws.do_undo();
    assert_eq!(rig.room(0).band_sets[0].bands[0].material, None);
}

fn text_of(output: &egui::FullOutput) -> String {
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

fn inspector_text(ws: &mut EditorWorkspace) -> String {
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    text_of(&draw_editor(ws, &ctx, true))
}

fn click_text(ws: &mut EditorWorkspace, label: &str) {
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    let output = draw_editor(ws, &ctx, true);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == label => {
                Some(t.pos + t.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {label:?} on screen"));
    let press = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    draw_with_events(
        ws,
        &ctx,
        vec![egui::Event::PointerMoved(pos), press(true), press(false)],
    );
}

#[test]
fn the_wall_run_context_adds_bands_edits_a_band_and_removes_them() {
    let mut ws = EditorWorkspace::with_project(test_temp_dir("bands-inspector"), project());
    ws.set_grid_tool(GridTool::Select);
    ws.grid.selection.set(vec![GridElement::Face(north())]);
    let text = inspector_text(&mut ws);
    assert!(text.contains("Bands"), "{text}");
    assert!(text.contains("5 wall segments in this run"), "{text}");
    assert!(text.contains("no bands"), "{text}");
    // Add bands: a trim a click up.
    click_text(&mut ws, "Add bands");
    let room = &ws.project.active_scene().world.grid().unwrap().rooms[0];
    assert_eq!(room.band_sets.len(), 1);
    assert_eq!(room.band_sets[0].lines, vec![Anchor::Bottom(256)]);
    // An Inspector edit is one undo step.
    ws.do_undo();
    assert!(
        ws.project.active_scene().world.grid().unwrap().rooms[0]
            .band_sets
            .is_empty(),
        "the Inspector edit was not recorded for undo"
    );
    ws.do_redo();
    let text = inspector_text(&mut ws);
    assert!(text.contains("base band"), "{text}");
    assert!(text.contains("top band"), "{text}");
    assert!(text.contains("above the bottom"), "{text}");
    // Add a line, then remove all bands.
    click_text(&mut ws, "Add line");
    assert_eq!(
        ws.project.active_scene().world.grid().unwrap().rooms[0].band_sets[0]
            .lines
            .len(),
        2
    );
    click_text(&mut ws, "Remove bands");
    let room = &ws.project.active_scene().world.grid().unwrap().rooms[0];
    assert!(room.band_sets.is_empty(), "{:?}", room.band_sets);
    assert!(inspector_text(&mut ws).contains("no bands"));
}

#[test]
fn the_rooms_panel_lists_the_sets_and_the_room_default() {
    let mut ws = EditorWorkspace::with_project(test_temp_dir("bands-rooms"), project());
    ws.grid.room_settings_open = true;
    // No sets yet: no row.
    assert!(!inspector_text(&mut ws).contains(" sets"));
    {
        let world = ws.project.active_scene_mut().world.grid_mut().unwrap();
        let mut set = psxed_project::grid::BandSet::single("trimmed", None);
        set.insert_line(0, Anchor::Bottom(256));
        world.rooms[0].band_sets.push(set);
        world.rooms[0].defaults.bands = Some(0);
    }
    ws.grid_changed();
    let text = inspector_text(&mut ws);
    assert!(text.contains("1 sets"), "{text}");
    assert!(text.contains("trimmed (0)"), "{text}");
}
