//! Manny's findings from the test of grid/main 2db980f5 (2026-10-20): Paint
//! on a wall painted "the whole horizontal wall, but only on some walls",
//! and a split of a wall could not be taken back.
//!
//! The cause of the first: the material of a band lives in the band set,
//! and a set belongs to a run of collinear wall segments, so painting one
//! band recoloured that band along the whole run. A wall with no bands keeps
//! its material per segment and never did it, which is why only some walls
//! showed it. Everything runs through real pointer input in the 3D rig.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::{edit_bands, effective_set, BandScope};
use psxed_project::grid::edit::{material_of, FaceRef};
use psxed_project::grid::{GridWall, WallEdge};
use psxed_project::{MaterialResource, ResourceData, ResourceId};

fn paint_with(rig: &mut GridRig, name: &str) -> ResourceId {
    let id = rig
        .ws
        .project
        .add_resource(name, ResourceData::Material(MaterialResource::opaque(None)));
    rig.ws.grid.material = Some(id);
    id
}

/// The north wall of cell (x, 0) in a derived room.
fn derived_north(x: i32) -> FaceRef {
    FaceRef::derived(0, [x, 0], WallEdge::North, DerivedKind::Closure)
}

/// The authored north wall of cell (x, 0).
fn authored_north(x: i32) -> FaceRef {
    FaceRef::wall(0, [x, 0], WallEdge::North, 0)
}

/// Screen position on the north wall of cell `x` at `y` editor units.
fn on_wall(rig: &GridRig, x: i32, y: f32) -> Pos2 {
    rig.screen_of([(x as f32 + 0.5) * S, y, 0.0])
}

/// A 5 x 5 room 1536 high whose north wall run is cut at 768.
fn derived_room() -> ProjectDocument {
    let mut project = derived_project_for_tiling(5, 5);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    for z in 0..5 {
        for x in 0..5 {
            world.rooms[0]
                .sector_or_insert(x, z)
                .unwrap()
                .set_ground_ceiling(Some(psxed_project::grid::GridSurface::flat(1536, None)));
        }
    }
    project
}

/// The same room built of authored walls on the north edge.
fn authored_room() -> ProjectDocument {
    let mut project = flat_project(5, 5, true);
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    for x in 0..5 {
        world.rooms[0]
            .sector_or_insert(x, 0)
            .unwrap()
            .walls_mut(WallEdge::North)
            .push(GridWall::span(0, 1536, None));
    }
    project
}

fn cut(rig: &mut GridRig, face: FaceRef) {
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    assert!(edit_bands(world, face, BandScope::Run, |set| set
        .insert_line(0, psxed_project::grid::Anchor::Abs(768))));
    rig.ws.grid_changed();
}

/// The material of band `band` of `face`, as the wall draws it.
fn band_material(rig: &GridRig, face: FaceRef, band: usize) -> Option<ResourceId> {
    effective_set(rig.world(), face).and_then(|(_, set)| set.bands[band].material)
}

/// The five segments of the run, band `band` of each.
fn run_bands(rig: &GridRig, face: impl Fn(i32) -> FaceRef, band: usize) -> Vec<Option<ResourceId>> {
    (0..5).map(|x| band_material(rig, face(x), band)).collect()
}

// ---------------------------------------------------------------------
// Paint paints the piece under the brush: one segment, one band.

#[test]
fn painting_a_band_of_a_derived_wall_paints_that_segment_only() {
    let mut rig = GridRig::persp("wallfix-derived-band", derived_room());
    cut(&mut rig, derived_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 300.0));
    assert_eq!(
        run_bands(&rig, derived_north, 0),
        vec![None, None, Some(brick), None, None],
        "{}",
        rig.ws.status
    );
    assert_eq!(run_bands(&rig, derived_north, 1), vec![None; 5]);
}

#[test]
fn painting_a_band_of_an_authored_wall_paints_that_segment_only() {
    let mut rig = GridRig::persp("wallfix-authored-band", authored_room());
    cut(&mut rig, authored_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 1200.0));
    assert_eq!(
        run_bands(&rig, authored_north, 1),
        vec![None, None, Some(brick), None, None],
        "{}",
        rig.ws.status
    );
    assert_eq!(run_bands(&rig, authored_north, 0), vec![None; 5]);
}

#[test]
fn painting_a_band_of_the_room_default_set_paints_that_segment_only() {
    let mut rig = GridRig::persp("wallfix-default-set", derived_room());
    {
        let room = &mut rig
            .ws
            .project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .rooms[0];
        let mut set = psxed_project::grid::BandSet::single("Walls", None);
        set.insert_line(0, psxed_project::grid::Anchor::Abs(768));
        room.band_sets.push(set);
        room.defaults.bands = Some(0);
    }
    rig.ws.grid_changed();
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 3, 300.0));
    assert_eq!(
        run_bands(&rig, derived_north, 0),
        vec![None, None, None, Some(brick), None],
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_selected_band_takes_a_palette_material_on_its_segment_only() {
    let mut rig = GridRig::persp("wallfix-select-band", derived_room());
    cut(&mut rig, derived_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Select);
    rig.click(on_wall(&rig, 2, 300.0));
    rig.ws.choose_palette_material(brick);
    assert_eq!(
        run_bands(&rig, derived_north, 0),
        vec![None, None, Some(brick), None, None],
        "{}",
        rig.ws.status
    );
}

#[test]
fn a_stroke_paints_each_segment_it_crosses_and_no_other() {
    let mut rig = GridRig::persp("wallfix-stroke", derived_room());
    cut(&mut rig, derived_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.drag(on_wall(&rig, 1, 300.0), on_wall(&rig, 2, 300.0));
    assert_eq!(
        run_bands(&rig, derived_north, 0),
        vec![None, Some(brick), Some(brick), None, None],
        "{}",
        rig.ws.status
    );
}

#[test]
fn painting_one_piece_is_one_undo_step() {
    let mut rig = GridRig::persp("wallfix-undo", derived_room());
    cut(&mut rig, derived_north(2));
    paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 300.0));
    rig.ws.do_undo();
    assert_eq!(run_bands(&rig, derived_north, 0), vec![None; 5]);
}

#[test]
fn a_cut_made_after_the_paint_keeps_every_segments_own_paint() {
    let mut rig = GridRig::persp("wallfix-cut-after", derived_room());
    cut(&mut rig, derived_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 300.0));
    // A second cut, over the whole run, at 1152.
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    assert!(edit_bands(world, derived_north(0), BandScope::Run, |set| {
        set.insert_line(1, psxed_project::grid::Anchor::Abs(1152))
    }));
    // Band 0 stays brick on the painted segment only; band 1 (new) is a
    // copy of the old band 1 everywhere.
    assert_eq!(
        run_bands(&rig, derived_north, 0),
        vec![None, None, Some(brick), None, None]
    );
    assert_eq!(run_bands(&rig, derived_north, 1), vec![None; 5]);
}

// Controls: walls with no bands keep the material per segment.

#[test]
fn painting_a_plain_derived_wall_paints_that_segment_only() {
    let mut rig = GridRig::persp("wallfix-plain-derived", derived_room());
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 300.0));
    let got: Vec<Option<ResourceId>> = (0..5)
        .map(|x| material_of(rig.world(), derived_north(x), None))
        .collect();
    assert_eq!(got, vec![None, None, Some(brick), None, None]);
}

#[test]
fn painting_a_plain_authored_wall_paints_that_segment_only() {
    let mut rig = GridRig::persp("wallfix-plain-authored", authored_room());
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.click(on_wall(&rig, 2, 300.0));
    let got: Vec<Option<ResourceId>> = (0..5)
        .map(|x| material_of(rig.world(), authored_north(x), None))
        .collect();
    assert_eq!(got, vec![None, None, Some(brick), None, None]);
}

#[test]
fn the_preview_shows_the_piece_the_click_would_paint_and_nothing_else() {
    let mut rig = GridRig::persp("wallfix-preview", derived_room());
    cut(&mut rig, derived_north(2));
    let brick = paint_with(&mut rig, "brick");
    rig.tool(GridTool::Paint);
    rig.move_to(on_wall(&rig, 2, 1200.0), egui::Modifiers::NONE);
    let ghost = rig.ws.paint_ghost().expect("a paint under the pointer");
    assert_eq!(
        ghost.target,
        crate::grid_tools::PaintTarget::Band {
            face: derived_north(2),
            band: 1
        }
    );
    // What the preview draws is the ghost applied to a copy of the world.
    let mut copy = rig.world().clone();
    assert!(ghost.apply(&mut copy));
    let painted = |world: &psxed_project::grid::GridWorld| -> Vec<(i32, usize)> {
        let mut out = Vec::new();
        for x in 0..5 {
            let set = effective_set(world, derived_north(x)).unwrap().1;
            for (i, b) in set.bands.iter().enumerate() {
                if b.material == Some(brick) {
                    out.push((x, i));
                }
            }
        }
        out
    };
    assert_eq!(painted(&copy), vec![(2, 1)]);
    // And the document itself is untouched until the click.
    assert!(painted(rig.world()).is_empty());
}
