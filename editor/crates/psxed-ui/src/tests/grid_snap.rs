//! The one snap rule: Alt is the fine snap (64 units), Shift+Alt is free,
//! in every grid tool that places or moves things.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;

fn spot_at(rig: &mut GridRig, pos: Pos2, mods: egui::Modifiers) -> [f32; 2] {
    rig.tool(GridTool::Place);
    rig.move_to(pos, mods);
    match &rig.ws.grid.hover {
        crate::grid_tools::GridHover::Spot { position } => [position[0], position[2]],
        other => panic!("{other:?}"),
    }
}

#[test]
fn place_snaps_to_the_sector_centre_then_the_fine_lattice_then_not_at_all() {
    // A point 0.3 sector across a cell, in editor units (scale 1:1).
    let at = |rig: &GridRig| rig.top_at(2.0 * S + 0.3 * S, 2.0 * S + 0.71 * S);
    let mut rig = GridRig::top("snap-place", flat_project(5, 5, false));
    let pos = at(&rig);
    assert_eq!(
        spot_at(&mut rig, pos, egui::Modifiers::NONE),
        [2.5 * S, 2.5 * S]
    );
    let [x, z] = spot_at(&mut rig, pos, egui::Modifiers::ALT);
    assert_eq!((x % 64.0, z % 64.0), (0.0, 0.0), "{x} {z}");
    assert_ne!([x, z], [2.5 * S, 2.5 * S]);
    let [fx, fz] = spot_at(&mut rig, pos, egui::Modifiers::ALT | egui::Modifiers::SHIFT);
    // Free keeps what the pointer touched, to the pixel.
    assert!(
        (fx - (2.3 * S)).abs() < 12.0 && (fz - (2.71 * S)).abs() < 12.0,
        "{fx} {fz}"
    );
    assert!(fx % 64.0 != 0.0 || fz % 64.0 != 0.0, "{fx} {fz}");
}

#[test]
fn the_status_bar_and_help_say_the_same_rule() {
    use super::grid_ui::{draw_editor, full_context};
    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("snap-text"), flat_project(3, 3, false));
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.apply_grid_pose("tool=plane").unwrap();
    ws.grid.help_open = true;
    // egui sizes a new window invisibly first; the next frame shows it.
    let _ = draw_editor(&mut ws, &ctx, true);
    let output = draw_editor(&mut ws, &ctx, true);
    let text: String = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for wanted in [
        "Snap 256 (Alt 64, Shift+Alt free)",
        "Shift+Alt",
        "Reserved for the level verbs",
    ] {
        assert!(text.contains(wanted), "missing {wanted:?}");
    }
    assert!(!text.contains("Ctrl 64"));
}
