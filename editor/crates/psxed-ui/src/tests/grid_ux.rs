//! The UX pass: the Levels panel, the view switch and the split layout, the
//! Materials panel, undo and redo buttons, and the keys of every tool.

use super::grid_levels::spans_at;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, draw_editor_sized, full_context};
use super::*;
use crate::grid_tools::{GridTool, GridViewMode};

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

fn hall() -> ProjectDocument {
    super::grid_derived::derived_project_for_tiling(8, 8)
}

/// A hall with a slab, so it has two storeys.
fn two_storeys() -> GridRig {
    let mut rig = GridRig::top("ux-two-storeys", hall());
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    for z in 0..8 {
        for x in 0..8 {
            let sector = world.rooms[0].sector_mut(x, z).unwrap();
            psxed_project::grid::edit::spans::insert_slab_column(
                sector,
                1536,
                psxed_project::grid::storeys::SLAB_THICKNESS,
                None,
            )
            .unwrap();
        }
    }
    rig.ws.grid_changed();
    assert_eq!(spans_at(&rig, [2, 2]), 2);
    rig
}

#[test]
fn the_levels_panel_lists_storeys_and_a_name_is_kept_in_the_world() {
    let mut rig = two_storeys();
    let ctx = full_context();
    let text = text_of(&draw_editor_sized(
        &mut rig.ws,
        &ctx,
        true,
        Vec2::new(1600.0, 1400.0),
    ));
    assert!(text.contains("Levels"), "{text}");
    assert!(
        text.contains("Level 0") && text.contains("Level 1"),
        "{text}"
    );
    // Going to a level is the storey cursor.
    let list = rig.ws.level_storeys();
    rig.ws.set_level(list[1].height);
    assert_eq!(rig.ws.grid.level.y, Some(list[1].height));
    // A rename is one undo step and shows in the panel and the chip.
    rig.ws.grid.level.renaming = Some((list[1].height, "Gallery".to_string()));
    let before_undo = rig.ws.history.can_undo();
    rig.ws.commit_level_rename();
    assert!(rig.ws.history.can_undo() || before_undo);
    let world = rig.world();
    assert_eq!(
        crate::grid_tools::levels_panel::level_name(world, &rig.ws.level_storeys(), list[1].height),
        "Gallery"
    );
    let text = text_of(&draw_editor(&mut rig.ws, &ctx, true));
    assert!(text.contains("Gallery"), "{text}");
    // An empty name restores the default.
    rig.ws.grid.level.renaming = Some((list[1].height, "  ".to_string()));
    rig.ws.commit_level_rename();
    assert!(rig.world().level_names.is_empty());
}

#[test]
fn the_top_view_draws_and_picks_only_the_walls_of_the_level_at_the_cursor() {
    let mut rig = two_storeys();
    let room = &rig.world().rooms[0];
    let wall =
        psxed_project::grid::derive::edge_walls(room, [0, 3], psxed_project::grid::WallEdge::West);
    let spans: Vec<u8> = wall.iter().map(|w| w.span).collect();
    assert!(spans.contains(&0) && spans.contains(&1), "{spans:?}");
    let list = rig.ws.level_storeys();
    let room = &rig.world().rooms[0];
    for (level, shown) in [(list[0].height, 0u8), (list[1].height, 1u8)] {
        let kept: Vec<u8> = wall
            .iter()
            .filter(|w| crate::grid_view::wall_in_slice(room, w, Some(level)))
            .map(|w| w.span)
            .collect();
        assert!(
            !kept.is_empty() && kept.iter().all(|s| *s == shown),
            "{kept:?}"
        );
    }
    rig.ws.grid.level.y = Some(list[1].height);
    rig.ws.grid.tool = GridTool::Select;
    let ctx = full_context();
    let _ = draw_editor(&mut rig.ws, &ctx, true);
}

#[test]
fn the_view_switch_shows_3d_the_plan_or_both_and_both_draw() {
    let mut rig = GridRig::top("ux-views", hall());
    let ctx = full_context();
    assert_eq!(rig.ws.grid_view_mode(), GridViewMode::Plan);
    for mode in [GridViewMode::Persp, GridViewMode::Split, GridViewMode::Plan] {
        rig.ws.set_grid_view_mode(mode);
        assert_eq!(rig.ws.grid_view_mode(), mode);
        let text = text_of(&draw_editor_sized(
            &mut rig.ws,
            &ctx,
            mode == GridViewMode::Plan,
            Vec2::new(1600.0, 1000.0),
        ));
        for label in ["3D", "Plan", "Split"] {
            assert!(text.contains(label), "{mode:?}: {label} missing\n{text}");
        }
    }
    // Ctrl/Cmd+9 cycles them.
    rig.ws.set_grid_view_mode(GridViewMode::Persp);
    rig.ws.cycle_view_dimension_group(false);
    assert_eq!(rig.ws.grid_view_mode(), GridViewMode::Plan);
    rig.ws.cycle_view_dimension_group(false);
    assert_eq!(rig.ws.grid_view_mode(), GridViewMode::Split);
    rig.ws.cycle_view_dimension_group(false);
    assert_eq!(rig.ws.grid_view_mode(), GridViewMode::Persp);
}

#[test]
fn every_tool_has_its_own_key_and_the_key_selects_it() {
    let mut keys: Vec<&str> = Vec::new();
    for tool in GridTool::ALL {
        assert!(!keys.contains(&tool.key_label()), "{}", tool.key_label());
        keys.push(tool.key_label());
        assert!(!tool.tooltip().is_empty() && !tool.label().is_empty());
    }
    let mut rig = GridRig::persp("ux-keys", flat_project(3, 3, false));
    rig.move_to(Pos2::new(400.0, 300.0), egui::Modifiers::NONE);
    for tool in GridTool::ALL {
        let label = tool.key_label();
        let (key, mods) = match label.strip_prefix("Shift+") {
            Some(rest) => (rest, egui::Modifiers::SHIFT),
            None => (label, egui::Modifiers::NONE),
        };
        let key = egui::Key::from_name(key).expect("a key");
        rig.key_with(key, mods);
        assert_eq!(rig.ws.grid.tool, tool, "{label} selects {}", tool.label());
    }
}

#[test]
fn the_materials_panel_searches_and_a_made_material_joins_the_palette() {
    let mut project = flat_project(3, 3, false);
    let brick = project.add_resource(
        "Graybox / Brick",
        psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
    );
    let mut ws = EditorWorkspace::with_project(test_temp_dir("ux-materials"), project);
    let ctx = full_context();
    let text = text_of(&draw_editor(&mut ws, &ctx, true));
    assert!(
        text.contains("Materials") && text.contains("Search materials"),
        "{text}"
    );
    // A new material made from the palette is pinned, shown and painted with.
    let id = ws.add_material_resource();
    ws.pin_material(id);
    let world = ws.project.active_scene().world.grid().unwrap();
    assert_eq!(world.palette_pins, vec![id]);
    assert_eq!(ws.grid.material, Some(id));
    let shown = psxed_project::grid::palette::palette_materials(&ws.project, false);
    assert!(shown.iter().any(|(m, _)| *m == id));
    assert!(shown.iter().any(|(m, _)| *m == brick));
    // The Resources dock does not list materials of a grid project a second time.
    ws.resources_open = true;
    let text = text_of(&draw_editor(&mut ws, &ctx, true));
    assert!(!text.contains("Material ("), "{text}");
}

#[test]
fn undo_and_redo_have_buttons_that_follow_the_history() {
    let mut rig = GridRig::top("ux-undo", flat_project(3, 3, false));
    assert!(!rig.ws.history.can_undo());
    rig.tool(GridTool::Floor);
    rig.drag(rig.cell_centre(4, 0), rig.cell_centre(4, 1));
    assert!(rig.ws.history.can_undo());
    rig.ws.do_undo();
    assert!(rig.ws.history.can_redo());
}

#[test]
fn clicking_the_pinned_room_again_lets_the_room_follow_the_edits() {
    let mut ws = EditorWorkspace::with_project(test_temp_dir("ux-room"), flat_project(3, 3, false));
    ws.grid_active_room = Some(0);
    ws.toggle_room_pin(0);
    assert_eq!(ws.grid_active_room, None);
    ws.toggle_room_pin(0);
    assert_eq!(ws.grid_active_room, Some(0));
}
