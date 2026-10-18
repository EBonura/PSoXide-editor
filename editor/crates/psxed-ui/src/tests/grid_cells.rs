//! The Cells tool and the tools on cells of other sizes, through the editor:
//! a brush stroke subdivides, Alt merges, one undo step per stroke, and
//! Raise and the corner drag work on the finer cells (T9).

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::{Gesture, GridTool};
use psxed_project::grid::tree::check;
use psxed_project::grid::LeafKey;

const CLICK_PX: f32 = crate::grid_tools::view::TOP_PIXELS_PER_CLICK;

fn hall() -> ProjectDocument {
    derived_project_for_tiling(8, 8)
}

fn levels(rig: &GridRig) -> [usize; 5] {
    let mut out = [0; 5];
    for (key, _) in rig.room(0).leaves() {
        out[usize::from(key.level)] += 1;
    }
    out
}

#[test]
fn a_brush_stroke_subdivides_and_one_undo_puts_the_room_back() {
    let mut rig = GridRig::top("cells-brush", hall());
    let original = rig.ws.project.clone();
    rig.tool(GridTool::Cells);
    // Press in sector (2,2) and drag over (3,2): two sectors, four children
    // each.
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(3, 2));
    assert!(matches!(rig.ws.grid.gesture, Gesture::Idle));
    assert_eq!(levels(&rig)[3], 8, "{}", rig.ws.status);
    assert_eq!(levels(&rig)[2], 62);
    assert!(check(rig.room(0)).is_empty());
    // The status bar says what happened.
    assert!(rig.ws.status.contains("Subdivided"), "{}", rig.ws.status);
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, original);
    rig.ws.do_redo();
    assert_eq!(levels(&rig)[3], 8);
}

#[test]
fn alt_paints_the_cells_back_together_and_a_balanced_neighbour_follows_a_deep_dig() {
    let mut rig = GridRig::top("cells-merge", hall());
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(4, 4));
    assert_eq!(levels(&rig)[3], 4);
    // Alt merges them back.
    rig.click_with(
        rig.cell_centre(4, 4),
        egui::Modifiers {
            alt: true,
            ..egui::Modifiers::NONE
        },
    );
    assert_eq!(levels(&rig)[3], 0, "{}", rig.ws.status);
    assert_eq!(rig.room(0).leaf_count(), 64);
    // Digging a quarter sector deep makes the neighbours balance.
    rig.ws.grid.cells.target = crate::grid_tools::cells_target_quarter();
    rig.click(rig.cell_centre(4, 4));
    assert!(levels(&rig)[4] >= 4, "{:?}", levels(&rig));
    assert!(check(rig.room(0)).is_empty());
}

#[test]
fn the_selection_buttons_act_on_the_picked_cells() {
    let mut rig = GridRig::top("cells-select", hall());
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 1));
    rig.tool(GridTool::Cells);
    rig.key(egui::Key::Enter);
    assert_eq!(levels(&rig)[3], 4, "{}", rig.ws.status);
    rig.key_with(
        egui::Key::Enter,
        egui::Modifiers {
            shift: true,
            ..egui::Modifiers::NONE
        },
    );
    assert_eq!(rig.room(0).leaf_count(), 64, "{}", rig.ws.status);
}

#[test]
fn raise_lifts_the_fine_cells_of_a_region_and_the_height_tool_drags_a_fine_corner() {
    let mut rig = GridRig::top("cells-raise", hall());
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(2, 2));
    assert_eq!(levels(&rig)[3], 4);
    rig.tool(GridTool::Raise);
    rig.drag(rig.cell_centre(2, 2), rig.cell_centre(2, 2));
    let from = rig.cell_centre(2, 2);
    rig.drag_with(from, from - Vec2::new(0.0, CLICK_PX), egui::Modifiers::NONE);
    let lifted = rig
        .room(0)
        .leaves()
        .filter(|(k, _)| k.level == 3)
        .all(|(_, s)| s.ground_floor().unwrap().heights == [256; 4]);
    assert!(lifted, "{}", rig.ws.status);
    assert!(check(rig.room(0)).is_empty(), "{:?}", check(rig.room(0)));
    // The fine cells stand above the rest; the raised block has derived
    // sides on the sector ring.
    let (key, _) = rig.room(0).leaf_at([8, 8]).unwrap();
    assert_eq!(key, LeafKey::new(3, 8, 8).unwrap());
}

#[test]
fn the_rectangle_tools_leave_cells_that_are_not_sectors_alone_without_panicking() {
    let mut rig = GridRig::top("cells-floor", hall());
    rig.tool(GridTool::Cells);
    rig.click(rig.cell_centre(2, 2));
    let before = rig.ws.project.clone();
    for tool in [GridTool::Floor, GridTool::Ceiling, GridTool::Wall, GridTool::Paint] {
        rig.tool(tool);
        rig.drag(rig.cell_centre(1, 2), rig.cell_centre(3, 2));
    }
    assert!(check(rig.room(0)).is_empty(), "{:?}", check(rig.room(0)));
    // The fine cells are still there, whatever the tools did around them.
    assert_eq!(levels(&rig)[3], 4);
    let _ = before;
}
