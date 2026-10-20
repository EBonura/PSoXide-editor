//! Captures for the Duplicate and Extrude review pages:
//! `PSXED_DUPEXT_CAPTURE=<dir> PSXED_DUPEXT_LABEL=before|after` writes PNGs
//! from the whole window (dp-world stand-in, 1456 x 819); without the
//! variable this does nothing. The 3D view of the rig is an empty texture,
//! so 3D shots draw the world's faces as a wireframe over it.

use super::grid_live::WindowRig;
use super::grid_pick_shots::{key, look_from, sector, settle, three_d, wire};
use super::*;
use crate::grid_tools::extrude_tool::ExtrudeSurface;
use crate::grid_tools::GridTool;
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::{edit_bands, BandScope};
use psxed_project::grid::edit::{CellRect, FaceRef, GridElement, Region};
use psxed_project::grid::{Anchor, WallEdge};

fn cmd_d(rig: &mut WindowRig) {
    rig.modifiers = egui::Modifiers::COMMAND;
    for pressed in [true, false] {
        rig.frame(vec![egui::Event::Key {
            key: egui::Key::D,
            physical_key: Some(egui::Key::D),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
    }
    rig.modifiers = egui::Modifiers::NONE;
}

/// A drag held mid-flight: press, move in steps, no release.
fn hold(rig: &mut WindowRig, from: Pos2, to: Pos2) {
    rig.move_to(from);
    let e = rig.press(from, true);
    rig.frame(vec![e]);
    for step in 1..=6 {
        rig.move_to(from + (to - from) * (step as f32 / 6.0));
    }
    settle(rig);
}

fn release(rig: &mut WindowRig, at: Pos2) {
    let e = rig.press(at, false);
    rig.frame(vec![e]);
    settle(rig);
}

fn lift_floor(rig: &mut WindowRig, cells: &[[i32; 2]], height: i32) {
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    let room = &mut world.rooms[1];
    for cell in cells {
        let [x, z] = room.local(*cell).unwrap();
        room.sector_or_insert(x, z)
            .unwrap()
            .ground_floor_mut()
            .unwrap()
            .heights = [height; 4];
    }
    rig.ws.grid_changed();
}

/// The plan over the Machine Hall.
fn frame_hall(rig: &mut WindowRig) {
    let s = sector(rig);
    rig.ws.orthographic_focus = [11.0 * s, 0.0, 2.5 * s];
    rig.ws.viewport_zoom = 0.07;
    rig.ws.grid.fit_pending = false;
}

fn hall_in_3d() -> WindowRig {
    let mut rig = WindowRig::new();
    three_d(&mut rig);
    rig
}

#[test]
fn dupext_captures() {
    let Some(dir) = std::env::var_os("PSXED_DUPEXT_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_DUPEXT_LABEL").unwrap_or_else(|_| "after".into());
    let shot = |rig: &WindowRig, name: &str| rig.capture(&dir.join(format!("{name}-{label}.png")));

    // ---- Duplicate: cells with a raised platform, a light standing on it.
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    lift_floor(&mut rig, &[[9, 1], [10, 1], [9, 2], [10, 2]], 512);
    let s = sector(&rig);
    let light = rig.ws.project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Light 1",
        NodeKind::PointLight {
            color: [255, 220, 160],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    rig.ws
        .project
        .active_scene_mut()
        .node_mut(light)
        .unwrap()
        .transform
        .translation = [9.5 * s, 900.0, 1.5 * s];
    rig.ws.set_grid_tool(GridTool::Select);
    rig.ws
        .grid
        .selection
        .set([[9, 1], [10, 1], [9, 2], [10, 2]].map(|cell| GridElement::Cell { room: 1, cell }));
    frame_hall(&mut rig);
    settle(&mut rig);
    shot(&rig, "dup-1-cells-selected");
    cmd_d(&mut rig);
    rig.move_to(rig.cell(12, 3));
    rig.move_to(rig.cell(12, 4));
    settle(&mut rig);
    shot(&rig, "dup-2-ghost-follows-the-pointer");
    rig.click(rig.cell(12, 4));
    settle(&mut rig);
    shot(&rig, "dup-3-dropped");
    three_d(&mut rig);
    look_from(
        &mut rig,
        [11.5 * s, 3600.0, 9.0 * s],
        [11.5 * s, 300.0, 2.5 * s],
    );
    settle(&mut rig);
    wire(&mut rig);
    shot(&rig, "dup-4-dropped-in-3d");
    // Escape leaves nothing.
    rig.ws.apply_grid_pose("view=plan").unwrap();
    rig.ws.set_grid_tool(GridTool::Select);
    rig.ws.grid.selection.set([GridElement::Cell {
        room: 1,
        cell: [9, 1],
    }]);
    settle(&mut rig);
    cmd_d(&mut rig);
    rig.move_to(rig.cell(11, 4));
    settle(&mut rig);
    key(&mut rig, egui::Key::Escape);
    settle(&mut rig);
    shot(&rig, "dup-5-escape-leaves-nothing");

    // A light copy mid-grab.
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    let s = sector(&rig);
    let light = rig.ws.project.active_scene_mut().add_node(
        NodeId::ROOT,
        "Light 1",
        NodeKind::PointLight {
            color: [255, 220, 160],
            intensity: 1.0,
            radius: 4.0,
        },
    );
    rig.ws
        .project
        .active_scene_mut()
        .node_mut(light)
        .unwrap()
        .transform
        .translation = [10.5 * s, 900.0, 2.5 * s];
    rig.ws.replace_node_selection(light);
    rig.ws.set_grid_tool(GridTool::Select);
    frame_hall(&mut rig);
    settle(&mut rig);
    shot(&rig, "dup-6-light-selected");
    cmd_d(&mut rig);
    rig.move_to(rig.cell(10, 2));
    rig.move_to(rig.cell(12, 4));
    settle(&mut rig);
    shot(&rig, "dup-7-light-copy-follows");
    rig.click(rig.cell(12, 4));
    settle(&mut rig);
    shot(&rig, "dup-8-light-copy-dropped");

    // ---- Extrude, floor: the Raise gesture under its new name.
    let mut rig = hall_in_3d();
    let s = sector(&rig);
    look_from(
        &mut rig,
        [10.5 * s, 4200.0, 9.0 * s],
        [10.5 * s, 0.0, 2.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Extrude);
    rig.ws.grid.verb.region = Some(Region::from_rect(
        1,
        CellRect::from_corners([9, 1], [11, 2]),
    ));
    settle(&mut rig);
    wire(&mut rig);
    shot(&rig, "extrude-floor-1-region");
    let from = rig.persp_screen([10.5 * s, 0.0, 2.0 * s]).unwrap();
    hold(&mut rig, from, from - Vec2::new(0.0, 90.0));
    wire(&mut rig);
    shot(&rig, "extrude-floor-2-dragging");
    release(&mut rig, from - Vec2::new(0.0, 90.0));

    // ---- Extrude, ceiling.
    let mut rig = hall_in_3d();
    let s = sector(&rig);
    look_from(
        &mut rig,
        [10.5 * s, 800.0, 9.0 * s],
        [10.5 * s, 2000.0, 1.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Extrude);
    rig.ws.grid.verb.surface = ExtrudeSurface::Ceiling;
    rig.ws.grid.verb.region = Some(Region::from_rect(
        1,
        CellRect::from_corners([9, 1], [11, 2]),
    ));
    settle(&mut rig);
    wire(&mut rig);
    shot(&rig, "extrude-ceiling-1-region");
    let ceiling = rig.world().scale.to_editor(3072) as f32;
    let from = rig.persp_screen([10.5 * s, ceiling, 1.5 * s]).unwrap();
    hold(&mut rig, from, from - Vec2::new(0.0, 70.0));
    wire(&mut rig);
    shot(&rig, "extrude-ceiling-2-dragging");
    release(&mut rig, from - Vec2::new(0.0, 70.0));

    // ---- Extrude, wall out, band out (niche), wall in.
    for (name, band, steps) in [
        ("wall-out", false, 1.0f32),
        ("niche", true, 1.0),
        ("wall-in", false, -1.0),
    ] {
        let mut rig = hall_in_3d();
        let s = sector(&rig);
        if band {
            let wall = FaceRef::derived(1, [10, -1], WallEdge::North, DerivedKind::Closure);
            let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
            assert!(edit_bands(world, wall, BandScope::Run, |set| {
                set.insert_line(0, Anchor::Abs(1024)) && set.insert_line(1, Anchor::Abs(2048))
            }));
            rig.ws.grid_changed();
        }
        look_from(
            &mut rig,
            [10.5 * s, 2400.0, 8.0 * s],
            [10.5 * s, 1200.0, -s],
        );
        rig.ws.set_grid_tool(GridTool::Extrude);
        settle(&mut rig);
        let y = if band { 1500.0 } else { 600.0 };
        let from = rig.persp_screen([10.5 * s, y, -s]).unwrap();
        rig.move_to(from);
        settle(&mut rig);
        wire(&mut rig);
        shot(&rig, &format!("extrude-{name}-1-piece-lit"));
        let to = rig.persp_screen([10.5 * s, y, -s - steps * s]).unwrap();
        hold(&mut rig, from, to);
        wire(&mut rig);
        shot(&rig, &format!("extrude-{name}-2-dragging"));
        release(&mut rig, to);
        wire(&mut rig);
        shot(&rig, &format!("extrude-{name}-3-released"));
    }
}
