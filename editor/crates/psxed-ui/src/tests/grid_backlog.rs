//! The editor backlog of 2026-10-22: the Cells tool's dead buttons, a whole
//! wall painted in the plan over banded pieces, a box that picked faces
//! hidden behind others, and the Keys page.
//!
//! Everything runs through real pointer and key input.

use super::grid_derived::derived_project_for_tiling;
use super::grid_live::WindowRig;
use super::grid_rig::*;
use super::grid_toolbar::{frames, texts};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::bands::{edit_bands, effective_set, BandScope};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::{Anchor, GridSurface, GridWall, WallEdge};
use psxed_project::{MaterialResource, ResourceData, ResourceId};

/// Every text on screen.
fn all_texts(rig: &WindowRig) -> Vec<String> {
    texts(rig).into_iter().map(|(t, _)| t).collect()
}

// ---------------------------------------------------------------------
// 1. The Cells tool: every visible button acts.

#[test]
fn the_cells_tool_offers_no_button_that_can_never_act() {
    // A tool switch drops the selection, and the brush is the only way to
    // work in the Cells tool, so "Subdivide selection" and "Merge cells"
    // were disabled whenever they were drawn.
    let mut rig = WindowRig::new();
    rig.ws.grid.walls_prompt_dismissed.insert(0);
    rig.ws.apply_grid_pose("view=3d").unwrap();
    rig.ws.set_grid_tool(GridTool::Cells);
    frames(&mut rig, 3);
    let shown = all_texts(&rig);
    for gone in ["Subdivide selection", "Merge cells"] {
        assert!(!shown.iter().any(|t| t == gone), "{gone} in {shown:?}");
    }
    assert!(
        shown.iter().any(|t| t == "Subdivide to"),
        "the options that still work stay: {shown:?}"
    );
}

#[test]
fn the_select_tool_subdivides_the_cells_it_picked() {
    let mut rig = WindowRig::new();
    rig.ws.grid.walls_prompt_dismissed.insert(0);
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    let before = rig.world().rooms[0].leaf_count();
    let (key, _) = rig.world().rooms[0].leaves().next().unwrap();
    let (lo, _) = key.sector_range();
    // Nothing picked: no button.
    frames(&mut rig, 3);
    assert!(
        !all_texts(&rig).iter().any(|t| t == "Subdivide cells"),
        "a button with nothing to act on"
    );
    rig.ws
        .grid
        .selection
        .replace(GridElement::Cell { room: 0, cell: lo });
    rig.ws.after_grid_selection_changed();
    frames(&mut rig, 3);
    let at = rig
        .find("Subdivide cells")
        .expect("a picked cell offers Subdivide cells");
    rig.click(at);
    frames(&mut rig, 2);
    assert_eq!(
        rig.world().rooms[0].leaf_count(),
        before + 3,
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.status.contains("Subdivided"), "{}", rig.ws.status);
    rig.ws.do_undo();
    assert_eq!(rig.world().rooms[0].leaf_count(), before);
}

// ---------------------------------------------------------------------
// 2. A whole wall painted in the plan reaches its bands.

/// A 3 x 2 room of hand-placed walls; the north wall of cell (1, 0) is cut
/// at 512 into a bottom band and a top band, each with a material of its
/// own. Returns the rig and the `[bottom, top, paint]` materials.
fn banded_plan() -> (GridRig, [ResourceId; 3]) {
    let mut project = flat_project(3, 2, false);
    let mut add = |name: &str| {
        project.add_resource(name, ResourceData::Material(MaterialResource::opaque(None)))
    };
    let ids = [add("bottom"), add("top"), add("red")];
    {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        world.rooms[0]
            .sector_or_insert(1, 0)
            .unwrap()
            .walls_mut(WallEdge::North)
            .push(GridWall::span(0, 1024, None));
        let wall = FaceRef::wall(0, [1, 0], WallEdge::North, 0);
        assert!(edit_bands(world, wall, BandScope::Piece, |set| {
            assert!(set.insert_line(0, Anchor::Abs(512)));
            set.set_band_material(0, Some(ids[0]));
            set.set_band_material(1, Some(ids[1]));
            true
        }));
    }
    (GridRig::top("plan-wall-bands", project), ids)
}

fn band_materials(rig: &GridRig) -> Vec<Option<ResourceId>> {
    let wall = FaceRef::wall(0, [1, 0], WallEdge::North, 0);
    effective_set(rig.world(), wall)
        .map(|(_, set)| set.bands.iter().map(|b| b.material).collect())
        .unwrap_or_default()
}

#[test]
fn painting_a_wall_in_the_plan_paints_its_bands_and_says_so() {
    let (mut rig, [bottom, top, paint]) = banded_plan();
    rig.tool(GridTool::Paint);
    rig.ws.grid.material = Some(paint);
    let on_wall = rig.top_at(1.5 * S, 0.0);
    rig.move_to(on_wall, egui::Modifiers::NONE);
    rig.click(on_wall);
    assert_eq!(
        band_materials(&rig),
        vec![Some(paint), Some(paint)],
        "the bands kept their old paint: {}",
        rig.ws.status
    );
    assert!(
        rig.ws.status.contains("2 bands"),
        "the status names what else changed: {}",
        rig.ws.status
    );
    // One undo step puts the wall and its bands back.
    rig.ws.do_undo();
    assert_eq!(band_materials(&rig), vec![Some(bottom), Some(top)]);
}

#[test]
fn the_palette_over_a_picked_wall_reaches_its_bands_too() {
    let (mut rig, [_, _, paint]) = banded_plan();
    rig.tool(GridTool::Select);
    rig.ws
        .grid
        .selection
        .replace(GridElement::Face(FaceRef::wall(
            0,
            [1, 0],
            WallEdge::North,
            0,
        )));
    assert_eq!(rig.ws.apply_grid_material_to_selection(paint), 1);
    assert_eq!(band_materials(&rig), vec![Some(paint), Some(paint)]);
}

// ---------------------------------------------------------------------
// 4. A box picks what the camera can see.

/// A 6 x 8 derived hall (floor 0, ceiling 3072) seen from the south and
/// above, with a block of sectors (2..=3, 5..=6) standing 2560 high in
/// front. The floors behind it are hidden from the camera.
fn hall_with_a_block(block: bool) -> GridRig {
    let mut project = derived_project_for_tiling(6, 8);
    if block {
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        for z in 5..=6 {
            for x in 2..=3 {
                world.rooms[0]
                    .sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_floor(Some(GridSurface::flat(2560, None)));
            }
        }
    }
    GridRig::persp("occlusion", project)
}

/// A screen rectangle holding the middle of the hidden floor (2, 3) and the
/// top of the block (2, 5).
fn box_over_hidden_floor_and_block(rig: &GridRig) -> (Pos2, Pos2) {
    let hidden = rig.screen_of([2.5 * S, 0.0, 3.5 * S]);
    let top = rig.screen_of([2.5 * S, 2560.0, 5.5 * S]);
    (
        Pos2::new(hidden.x.min(top.x) - 30.0, hidden.y.min(top.y) - 30.0),
        Pos2::new(hidden.x.max(top.x) + 30.0, hidden.y.max(top.y) + 30.0),
    )
}

fn floor_of(cell: [i32; 2]) -> GridElement {
    GridElement::Face(FaceRef::floor(0, cell))
}

#[test]
fn the_select_box_ignores_floors_hidden_behind_a_block() {
    // The control: the same box on a hall with no block takes the floor.
    let mut open = hall_with_a_block(false);
    open.tool(GridTool::Select);
    let (a, b) = box_over_hidden_floor_and_block(&open);
    open.drag(a, b);
    assert!(
        open.ws.grid.selection.items().contains(&floor_of([2, 3])),
        "the box must cover the floor: {}",
        open.ws.status
    );
    // With the block in front the floor is behind it.
    let mut rig = hall_with_a_block(true);
    rig.tool(GridTool::Select);
    let (a, b) = box_over_hidden_floor_and_block(&rig);
    rig.drag(a, b);
    let picked = rig.ws.grid.selection.items();
    assert!(picked.contains(&floor_of([2, 5])), "{}", rig.ws.status);
    assert!(
        !picked.contains(&floor_of([2, 3])),
        "a floor behind the block was picked: {}",
        rig.ws.status
    );
}

#[test]
fn the_extrude_box_ignores_floors_hidden_behind_a_block() {
    let mut rig = hall_with_a_block(true);
    rig.tool(GridTool::Extrude);
    let (a, b) = box_over_hidden_floor_and_block(&rig);
    // A Shift-drag that starts on a visible floor picks floors; it ends past
    // the block, so the box holds the hidden floor and the block's top.
    let start = rig.screen_of([4.5 * S, 0.0, 3.0 * S]);
    assert!(
        rig.ws.pick_face_3d(rig.persp_rect(), start).is_some(),
        "the box starts on a floor"
    );
    rig.drag_with(start, Pos2::new(a.x, b.y), egui::Modifiers::SHIFT);
    let region: Vec<[i32; 2]> = rig
        .ws
        .grid
        .verb
        .region
        .as_ref()
        .map(|r| r.cells().collect())
        .unwrap_or_default();
    assert!(region.contains(&[2, 5]), "{region:?}: {}", rig.ws.status);
    assert!(
        !region.contains(&[2, 3]),
        "a floor behind the block was picked: {region:?}"
    );
}

// ---------------------------------------------------------------------
// 5. The Keys page scrolls inside its panel; hints are said once.

fn keys_rig() -> WindowRig {
    let mut rig = WindowRig::with_size(Vec2::new(1456.0, 819.0));
    rig.ws.grid.walls_prompt_dismissed.insert(0);
    rig.ws.apply_grid_pose("view=3d").unwrap();
    rig.ws.grid.help_open = true;
    frames(&mut rig, 4);
    rig
}

fn keys_area(rig: &WindowRig) -> Rect {
    rig.ctx
        .memory(|m| m.area_rect(crate::grid_tools::help::keys_window_id(rig.size.y)))
        .expect("the Keys window is open")
}

#[test]
fn the_keys_page_stays_inside_the_window_and_scrolls() {
    let mut rig = keys_rig();
    let area = keys_area(&rig);
    assert!(
        area.max.y <= rig.size.y + 0.5,
        "the page runs off the bottom of the screen: {area:?}"
    );
    assert!(area.min.y >= 90.0, "the page covers the top bar: {area:?}");
    // The last section is below the fold until the page is scrolled.
    // The last row of the last section.
    let last = "This page";
    assert!(
        rig.find(last).is_none_or(|p| p.y > area.max.y),
        "the last section is already on screen"
    );
    let over = area.center();
    for _ in 0..12 {
        rig.move_to(over);
        rig.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -400.0),
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    frames(&mut rig, 10);
    let at = rig.find(last).expect("scrolling reaches the last section");
    assert!(area.contains(at), "{at:?} outside {area:?}");
}

#[test]
fn a_tools_key_hints_are_said_once() {
    let mut rig = WindowRig::new();
    rig.ws.grid.walls_prompt_dismissed.insert(0);
    rig.ws.apply_grid_pose("view=3d").unwrap();
    let said =
        |rig: &WindowRig, hint: &str| all_texts(rig).iter().filter(|t| t.contains(hint)).count();
    // Diagonal has no options, so its hints fill the options bar.
    rig.ws.set_grid_tool(GridTool::Split);
    frames(&mut rig, 3);
    assert_eq!(said(&rig, "flips its diagonal"), 1, "{:?}", all_texts(&rig));
    // Floor has options: the hints go to the status bar instead.
    rig.ws.set_grid_tool(GridTool::Floor);
    frames(&mut rig, 3);
    assert_eq!(said(&rig, "Drag rectangle"), 1);
}
