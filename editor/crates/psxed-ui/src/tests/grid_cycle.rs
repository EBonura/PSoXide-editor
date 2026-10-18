//! Clicking again on the same spot in the Top view takes the next thing
//! under the pointer (plane, floor, ceiling) and Alt no longer clicks
//! through.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{add_plane, GridElement, PlaneId};
use psxed_project::grid::GridPlane;

/// A flat plane over cell (2, 2) of a 6x6 room.
fn rig_with_plane(label: &str, ceiling: bool) -> (GridRig, PlaneId) {
    let mut rig = GridRig::top(label, flat_project(6, 6, ceiling));
    let mut plane = GridPlane::new([2560, 256, 2560], [1024, 1024]);
    plane.rotation = [0, 90, 0];
    let id = {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        add_plane(world, 0, plane).expect("room for the plane")
    };
    rig.ws.grid_changed();
    rig.tool(GridTool::Select);
    rig.pump(vec![], egui::Modifiers::NONE);
    (rig, id)
}

fn selected_cell(rig: &GridRig) -> bool {
    matches!(
        rig.ws.grid.selection.items(),
        [GridElement::Cell { cell: [2, 2], .. }]
    )
}

#[test]
fn a_repeated_click_takes_the_plane_then_the_sector_then_the_plane_again() {
    let (mut rig, id) = rig_with_plane("cycle-select", false);
    let spot = rig.cell_centre(2, 2);
    rig.click(spot);
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
    // Clicking again on the same spot goes through to the sector under it.
    rig.click(spot);
    assert!(
        rig.ws.grid.plane_sel.is_empty(),
        "{:?}",
        rig.ws.grid.plane_sel
    );
    assert!(selected_cell(&rig), "{:?}", rig.ws.grid.selection);
    // And once more wraps round to the plane.
    rig.click(spot);
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
}

#[test]
fn a_click_elsewhere_or_after_a_drag_starts_again_at_the_top() {
    let (mut rig, id) = rig_with_plane("cycle-reset", false);
    let spot = rig.cell_centre(2, 2);
    rig.click(spot);
    rig.click(spot);
    assert!(selected_cell(&rig));
    // Another spot, then back: the plane first.
    rig.click(rig.cell_centre(4, 4));
    rig.click(spot);
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
    // A drag between two clicks breaks the sequence.
    rig.click(spot);
    assert!(selected_cell(&rig));
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(1, 0));
    rig.click(spot);
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
}

#[test]
fn alt_no_longer_clicks_through_a_plane() {
    let (mut rig, id) = rig_with_plane("cycle-alt", false);
    rig.click_with(rig.cell_centre(2, 2), egui::Modifiers::ALT);
    assert_eq!(rig.ws.grid.plane_sel, vec![id]);
    assert!(!selected_cell(&rig));
}

#[test]
fn hover_shows_what_the_next_click_would_take() {
    let (mut rig, id) = rig_with_plane("cycle-hover", false);
    let spot = rig.cell_centre(2, 2);
    rig.move_to(spot, egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid.hover, crate::grid_tools::GridHover::Plane(id));
    rig.click(spot);
    rig.move_to(spot + Vec2::new(1.0, 0.0), egui::Modifiers::NONE);
    assert!(
        !matches!(rig.ws.grid.hover, crate::grid_tools::GridHover::Plane(_)),
        "{:?}",
        rig.ws.grid.hover
    );
}

#[test]
fn paint_reaches_the_ceiling_by_clicking_again_not_by_alt() {
    let mut rig = GridRig::top("cycle-paint", flat_project(4, 4, true));
    let brick = {
        let id = rig.ws.project.add_resource(
            "brick",
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        );
        rig.ws.grid.material = Some(id);
        id
    };
    rig.tool(GridTool::Paint);
    let material = |rig: &GridRig, ceiling: bool| {
        let sector = rig.room(0).sector(1, 1).unwrap();
        let surface = if ceiling {
            sector.ground_ceiling()
        } else {
            sector.ground_floor()
        };
        surface.unwrap().tri_a.material
    };
    let spot = rig.cell_centre(1, 1);
    rig.click_with(spot, egui::Modifiers::ALT);
    assert_eq!(material(&rig, false), Some(brick), "alt paints the floor");
    assert_eq!(material(&rig, true), None);
    // Clicking the same spot again goes on to the ceiling.
    rig.click(spot);
    assert_eq!(material(&rig, true), Some(brick));
}
