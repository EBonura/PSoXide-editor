//! Three findings from Manny's test of the grid editor (2026-10-20):
//! the pieces of a banded wall could not be selected, hovering reached
//! faces the camera cannot see, and Paint kept the face selection.
//!
//! Everything runs through real pointer input in the 3D rig.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, FaceTarget, GridElement, SurfaceKind};
use psxed_project::{MaterialResource, ResourceData, ResourceId};

fn brick(rig: &mut GridRig) -> ResourceId {
    rig.ws.project.add_resource(
        "brick",
        ResourceData::Material(MaterialResource::opaque(None)),
    )
}

/// Screen position on the north wall of cell (2, 0) at `y` editor units.
fn on_wall(rig: &GridRig, y: f32) -> Pos2 {
    rig.screen_of([2.5 * S, y, 0.0])
}

/// A 5 x 5 derived room, 1536 high (so the whole wall is in the rig's
/// view), whose north wall run is cut at 768: a bottom band (0..768) and a
/// top band (768..1536).
fn banded() -> GridRig {
    let mut project = derived_project_for_tiling(5, 5);
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
    let mut rig = GridRig::persp("pick-bands", project);
    rig.tool(GridTool::Bands);
    rig.click(on_wall(&rig, 768.0));
    assert_eq!(rig.room(0).band_sets.len(), 1, "{}", rig.ws.status);
    rig
}

fn band_materials(rig: &GridRig) -> Vec<Option<ResourceId>> {
    rig.room(0).band_sets[0]
        .bands
        .iter()
        .map(|b| b.material)
        .collect()
}

// ---------------------------------------------------------------------
// 1. Bands can be selected, one at a time.

#[test]
fn a_click_selects_the_band_under_the_pointer_and_a_material_lands_on_it_alone() {
    let mut rig = banded();
    let brick = brick(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 300.0));
    rig.ws.choose_palette_material(brick);
    assert_eq!(
        band_materials(&rig),
        vec![Some(brick), None],
        "{}",
        rig.ws.status
    );
    // The upper band is its own pick, with its own material.
    let other = rig.ws.project.add_resource(
        "moss",
        ResourceData::Material(MaterialResource::opaque(None)),
    );
    rig.click(on_wall(&rig, 1200.0));
    rig.ws.choose_palette_material(other);
    assert_eq!(
        band_materials(&rig),
        vec![Some(brick), Some(other)],
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_marquee_over_one_band_picks_that_band_only() {
    let mut rig = banded();
    let brick = brick(&mut rig);
    rig.tool(GridTool::Select);
    let (a, b) = (
        rig.screen_of([2.2 * S, 100.0, 0.0]),
        rig.screen_of([2.8 * S, 600.0, 0.0]),
    );
    rig.drag(a, b);
    rig.ws.choose_palette_material(brick);
    assert_eq!(
        band_materials(&rig),
        vec![Some(brick), None],
        "{}",
        rig.ws.status
    );
}

#[test]
fn hovering_a_banded_wall_names_the_band_under_the_pointer() {
    let mut rig = banded();
    rig.tool(GridTool::Select);
    let north = FaceRef::derived(
        0,
        [2, 0],
        psxed_project::grid::WallEdge::North,
        psxed_project::grid::derive::DerivedKind::Closure,
    );
    for (y, band) in [(300.0, 0u8), (1200.0, 1u8)] {
        rig.move_to(on_wall(&rig, y), egui::Modifiers::NONE);
        match &rig.ws.grid.hover {
            crate::grid_tools::GridHover::Elements(list) => {
                assert_eq!(list.as_slice(), &[GridElement::Band { face: north, band }]);
            }
            other => panic!("y {y}: {other:?}"),
        }
    }
}

#[test]
fn a_band_selection_shows_in_the_inspector_as_its_wall() {
    let mut rig = banded();
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 1200.0));
    let world = rig.world();
    let faces = rig.ws.grid.selection.faces(world);
    assert_eq!(faces.len(), 1);
    assert!(rig.ws.grid.selection.whole_faces(world).is_empty());
    assert_eq!(rig.ws.grid.selection.bands(world).len(), 1);
}

// ---------------------------------------------------------------------
// 2. Faces the camera does not draw are not hoverable.

fn is_ceiling(face: &FaceRef) -> bool {
    matches!(
        face.target,
        FaceTarget::Surface {
            kind: SurfaceKind::Ceiling,
            ..
        }
    )
}

fn names_a_ceiling(element: &GridElement) -> bool {
    match element {
        GridElement::Face(f) | GridElement::Triangle { face: f, .. } => is_ceiling(f),
        GridElement::Edge(e) => is_ceiling(&e.face),
        GridElement::Vertex(v) => is_ceiling(&v.face),
        GridElement::Band { face, .. } => is_ceiling(face),
        GridElement::Cell { .. } => false,
    }
}

#[test]
fn from_above_the_ceiling_edges_and_corners_are_not_hoverable() {
    // The rig camera is above the room: the ceiling is culled.
    let mut rig = GridRig::persp("pick-ceiling", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Select);
    let mut tried = 0;
    for z in 0..=5 {
        for x in 0..=5 {
            let at = rig.screen_of([x as f32 * S, 3072.0, z as f32 * S]);
            rig.move_to(at, egui::Modifiers::NONE);
            if let crate::grid_tools::GridHover::Elements(list) = &rig.ws.grid.hover {
                assert!(
                    !list.iter().any(names_a_ceiling),
                    "corner ({x}, {z}) hovered {list:?}"
                );
            }
            tried += 1;
        }
    }
    assert_eq!(tried, 36);
}

#[test]
fn from_above_a_floor_corner_is_still_hoverable() {
    let mut rig = GridRig::persp("pick-floor", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Select);
    rig.move_to(
        rig.screen_of([2.0 * S, 0.0, 2.0 * S]),
        egui::Modifiers::NONE,
    );
    match &rig.ws.grid.hover {
        crate::grid_tools::GridHover::Elements(list) => {
            assert!(
                list.iter().any(|e| matches!(e, GridElement::Vertex(_))),
                "{list:?}"
            );
            assert!(!list.iter().any(names_a_ceiling), "{list:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_marquee_over_the_room_from_above_takes_no_ceiling() {
    let mut rig = GridRig::persp("pick-marquee", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Select);
    rig.drag(
        Pos2::new(RIG_VIEWPORT.min.x + 4.0, RIG_VIEWPORT.min.y + 4.0),
        Pos2::new(RIG_VIEWPORT.max.x - 4.0, RIG_VIEWPORT.max.y - 4.0),
    );
    let items = rig.ws.grid.selection.items().to_vec();
    assert!(!items.is_empty(), "the marquee took nothing");
    assert!(!items.iter().any(names_a_ceiling), "{items:?}");
}

#[test]
fn a_double_sided_wall_still_picks_from_behind() {
    use psxed_project::grid::overrides::ClosureFacing;
    // The south wall is seen from outside by the rig camera.
    let south = |rig: &mut GridRig| {
        let at = rig.screen_of([2.5 * S, 700.0, 5.0 * S]);
        rig.move_to(at, egui::Modifiers::NONE);
        match &rig.ws.grid.hover {
            crate::grid_tools::GridHover::Elements(list) => list.clone(),
            _ => Vec::new(),
        }
    };
    let walls = |list: &[GridElement]| {
        list.iter()
            .filter(|e| match e {
                GridElement::Face(f) => matches!(f.target, FaceTarget::Derived { .. }),
                _ => false,
            })
            .count()
    };
    let mut rig = GridRig::persp("pick-back-front", derived_project_for_tiling(5, 5));
    rig.tool(GridTool::Select);
    assert_eq!(
        walls(&south(&mut rig)),
        0,
        "a one-sided wall is seen through"
    );
    let mut project = derived_project_for_tiling(5, 5);
    project.active_scene_mut().world.grid_mut().unwrap().rooms[0]
        .defaults
        .closure_facing = ClosureFacing::Both;
    let mut rig = GridRig::persp("pick-back-both", project);
    rig.tool(GridTool::Select);
    assert_eq!(walls(&south(&mut rig)), 1, "a double-sided wall picks");
}

// ---------------------------------------------------------------------
// 3. Paint does not keep the selection.

fn floors() -> (GridRig, ResourceId) {
    let mut rig = GridRig::top("pick-paint", flat_project(4, 1, false));
    let moss = brick(&mut rig);
    rig.tool(GridTool::Select);
    rig.click(rig.cell_centre(1, 0));
    rig.click_with(rig.cell_centre(2, 0), egui::Modifiers::COMMAND);
    assert_eq!(rig.ws.grid.selection.len(), 2);
    (rig, moss)
}

fn floor_material(rig: &GridRig, x: usize) -> Option<ResourceId> {
    rig.room(0)
        .sector(x, 0)
        .unwrap()
        .ground_floor()
        .unwrap()
        .tri_a
        .material
}

#[test]
fn entering_paint_clears_the_selection() {
    let (mut rig, _) = floors();
    rig.tool(GridTool::Paint);
    assert!(rig.ws.grid.selection.is_empty());
}

#[test]
fn a_material_chosen_in_paint_changes_the_brush_only() {
    let (mut rig, moss) = floors();
    rig.tool(GridTool::Paint);
    // Selected some other way while Paint is active (select all, say).
    rig.ws.grid.selection.set([
        GridElement::Face(FaceRef::floor(0, [1, 0])),
        GridElement::Face(FaceRef::floor(0, [2, 0])),
    ]);
    rig.ws.choose_palette_material(moss);
    assert_eq!(rig.ws.grid.material, Some(moss));
    assert_eq!(floor_material(&rig, 1), None);
    assert_eq!(floor_material(&rig, 2), None);
}

#[test]
fn a_material_chosen_in_select_still_goes_on_the_selection() {
    let (mut rig, moss) = floors();
    rig.ws.choose_palette_material(moss);
    assert_eq!(floor_material(&rig, 1), Some(moss));
    assert_eq!(floor_material(&rig, 2), Some(moss));
    assert_eq!(floor_material(&rig, 0), None);
}
