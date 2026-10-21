//! Extrude moves every wall: hand-placed ones too. 2026-10-21, Manny, on
//! dp-world's Machine Hall back wall: "That wall is hand-placed: Extrude
//! moves derived walls only. I'd love to be able to extrude EVERY wall: pull
//! the wall towards me to create a column, or back to create a corridor."
//!
//! The rig is the dp-world stand-in with its hall's back row (world z -1)
//! hand placed over the closure the user hid, the way dp-world's is; the
//! open variant has no ceiling and no closure height, so only the hand-placed
//! walls close that side.

use super::grid_extrudefix::{bar, floor, idle, look, north_wall, over, region_len, rig, HALL};
use super::grid_live::WindowRig;
use crate::grid_tools::extrude_tool::ExtrudeSurface;
use egui::Pos2;
use psxed_project::grid::derive::edge_key;
use psxed_project::grid::{GridRoom, GridWall, WallEdge};

/// World row of the hall's back wall.
const BACK: i32 = -1;

fn hall_mut(rig: &mut WindowRig) -> &mut GridRoom {
    &mut rig
        .ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms[HALL]
}

/// The stand-in hall with its back row hand placed; `ceilings` off makes
/// it open topped.
fn walled(ceilings: bool) -> WindowRig {
    let mut rig = rig();
    let wall = rig
        .ws
        .project
        .resources
        .iter()
        .find(|r| r.id.raw() == 160)
        .map(|r| r.id);
    let room = hall_mut(&mut rig);
    for x in 7..15 {
        let [lx, lz] = room.local([x, BACK]).unwrap();
        room.edge_overrides
            .edit(edge_key([x, BACK], WallEdge::North), |o| o.hidden = true);
        let sector = room.sector_or_insert(lx, lz).unwrap();
        sector
            .walls_mut(WallEdge::North)
            .push(GridWall::span(0, 3072, wall));
    }
    if !ceilings {
        for lz in 0..8 {
            for lx in 0..8 {
                room.sector_or_insert(lx, lz)
                    .unwrap()
                    .set_ground_ceiling(None);
            }
        }
    }
    rig.ws.grid_changed();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig
}

fn records(rig: &WindowRig, cell: [i32; 2], edge: WallEdge) -> usize {
    let room = &rig.world().rooms[HALL];
    room.local(cell)
        .and_then(|[x, z]| room.sector(x, z))
        .map_or(0, |s| s.walls(edge).len())
}

fn ceiling_at(rig: &WindowRig, cell: [i32; 2]) -> bool {
    let room = &rig.world().rooms[HALL];
    room.local(cell)
        .and_then(|[x, z]| room.sector(x, z))
        .is_some_and(|s| s.ground_ceiling().is_some())
}

/// Pull the back wall at world column `x` a sector away from the room.
fn pull_back(rig: &mut WindowRig, x: i32) {
    rig.drag(
        north_wall(rig, [x, BACK], 600),
        north_wall(rig, [x, BACK - 1], 600),
    );
}

/// Pull the back wall at world column `x` a sector towards the viewer.
fn pull_forward(rig: &mut WindowRig, x: i32) {
    rig.drag(
        north_wall(rig, [x, BACK], 600),
        north_wall(rig, [x, BACK + 1], 600),
    );
}

#[test]
fn pulling_a_hand_placed_back_wall_away_makes_a_corridor() {
    for ceilings in [true, false] {
        let mut rig = walled(ceilings);
        assert_eq!(records(&rig, [10, BACK], WallEdge::North), 1);
        pull_back(&mut rig, 10);
        assert!(!rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
        assert_eq!(floor(&rig, [10, BACK - 1]), Some(0), "{}", rig.ws.status);
        assert_eq!(ceiling_at(&rig, [10, BACK - 1]), ceilings);
        assert_eq!(records(&rig, [10, BACK], WallEdge::North), 0);
        // The neighbours' walls were not picked: they stay.
        assert_eq!(records(&rig, [9, BACK], WallEdge::North), 1);
        assert_eq!(records(&rig, [11, BACK], WallEdge::North), 1);
        assert!(idle(&rig));
        // The pick follows the new end wall, so it can be pushed again.
        assert_eq!(rig.ws.grid.selection.len(), 1, "ceilings {ceilings}");
        crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
        assert_eq!(floor(&rig, [10, BACK - 2]), Some(0), "{}", rig.ws.status);
    }
}

#[test]
fn pulling_a_hand_placed_back_wall_towards_the_viewer_makes_a_column() {
    for ceilings in [true, false] {
        let mut rig = walled(ceilings);
        pull_forward(&mut rig, 10);
        assert!(!rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
        // Under a ceiling the sector in front is solid and gone; in the
        // open hall it rises to the wall's top.
        let expected = if ceilings { None } else { Some(3072) };
        assert_eq!(floor(&rig, [10, BACK]), expected, "{}", rig.ws.status);
        assert_eq!(records(&rig, [10, BACK], WallEdge::North), 0);
        assert_eq!(records(&rig, [9, BACK], WallEdge::North), 1);
        assert_eq!(records(&rig, [11, BACK], WallEdge::North), 1);
        assert!(idle(&rig));
    }
}

#[test]
fn a_hand_placed_wall_click_picks_it_with_no_refusal_and_out_and_in_buttons_work() {
    let mut rig = walled(false);
    rig.click(north_wall(&rig, [10, BACK], 600));
    assert_eq!(rig.ws.grid.verb.surface, ExtrudeSurface::Wall);
    assert_eq!(rig.ws.grid.selection.len(), 1);
    assert!(!rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
    rig.frame(vec![]);
    assert!(!bar(&rig).contains("hand-placed"), "{}", bar(&rig));
    crate::grid_tools::extrude_tool::step(&mut rig.ws, -1);
    assert_eq!(floor(&rig, [10, BACK]), Some(3072), "{}", rig.ws.status);
}

#[test]
fn one_undo_gives_the_hall_back_exactly() {
    for (ceilings, steps) in [(true, 1), (true, -1), (false, 1), (false, -1)] {
        let mut rig = walled(ceilings);
        let before = rig.ws.project.clone();
        if steps > 0 {
            pull_back(&mut rig, 10);
        } else {
            pull_forward(&mut rig, 10);
        }
        assert_ne!(rig.ws.project, before, "ceilings {ceilings} steps {steps}");
        rig.ws.do_undo();
        assert_eq!(rig.ws.project, before, "ceilings {ceilings} steps {steps}");
    }
}

#[test]
fn a_whole_run_of_hand_placed_walls_goes_back_together() {
    let mut rig = walled(false);
    rig.click(north_wall(&rig, [9, BACK], 600));
    for x in [10, 11] {
        rig.modifiers = egui::Modifiers::SHIFT;
        rig.click(north_wall(&rig, [x, BACK], 600));
        rig.modifiers = egui::Modifiers::NONE;
    }
    assert_eq!(rig.ws.grid.selection.len(), 3, "{}", rig.ws.status);
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    for x in [9, 10, 11] {
        assert_eq!(floor(&rig, [x, BACK - 1]), Some(0), "{x}");
        assert_eq!(records(&rig, [x, BACK], WallEdge::North), 0, "{x}");
    }
    assert_eq!(floor(&rig, [12, BACK - 1]), None);
    assert_eq!(records(&rig, [12, BACK], WallEdge::North), 1);
}

#[test]
fn a_room_made_only_of_hand_placed_walls_extrudes_too() {
    // The starter room of the stand-in is the classic one: every wall of
    // its outline is a record, and the room derives nothing.
    let mut rig = rig();
    rig.ws.grid_active_room = Some(0);
    let mid = over(&rig, [3, 3], 0);
    let eu = |g: i32| rig.world().scale.to_editor(g) as f32;
    let (from, at) = (
        [mid[0], eu(6000), eu(12 * 1024)],
        [mid[0], eu(800), eu(1024)],
    );
    look(&mut rig, from, at);
    let before = rig.ws.project.clone();
    rig.drag(
        north_wall(&rig, [2, 0], 600),
        north_wall(&rig, [2, -1], 600),
    );
    assert!(!rig.ws.status.contains("hand-placed"), "{}", rig.ws.status);
    let room = &rig.world().rooms[0];
    assert_eq!(room.wall_mode, psxed_project::grid::WallMode::Derived);
    let [x, z] = room.local([2, -1]).expect("the room grew");
    assert!(room.sector(x, z).is_some());
    rig.ws.do_undo();
    assert_eq!(rig.ws.project, before);
    let _ = region_len(&rig);
}

#[test]
fn only_a_real_impossibility_is_refused_and_says_so() {
    let mut rig = walled(true);
    // Another room right behind the wall.
    let mut other = GridRoom::new("Annex", 1, 1);
    other.origin = [10, BACK - 1];
    other
        .sector_or_insert(0, 0)
        .unwrap()
        .set_ground_floor(Some(psxed_project::grid::GridSurface::flat(0, None)));
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms
        .push(other);
    rig.ws.grid_changed();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    let before = rig.ws.project.clone();
    pull_back(&mut rig, 10);
    assert!(rig.ws.status.contains("another room"), "{}", rig.ws.status);
    assert_eq!(rig.ws.project, before);
}

/// A flat-shaded stand-in for the 3D view (the rig's own is an empty
/// texture): every face the camera draws, far to near, filled with a colour
/// from its material, the picked pieces in orange.
fn paint(rig: &mut WindowRig) {
    use egui::{Color32, Stroke};
    use psxed_project::grid::edit::{FaceTarget, GridElement, SurfaceKind};
    let rect = rig.ws.last_persp_viewport_rect;
    let camera = rig.ws.viewport_3d_camera();
    let cam = crate::grid_tools::pick::Cam::new(camera, rect);
    let eye = camera.position_f32();
    let world = rig.world();
    let mut shapes: Vec<(f32, egui::Shape)> = Vec::new();
    rig.ws.with_pick_cache(|cache| {
        for pf in cache.faces.iter().filter(|pf| !pf.void) {
            let n = pf.points.len() as f32;
            let c = pf.points.iter().fold([0.0f32; 3], |a, p| {
                [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]
            });
            let to_eye = [eye[0] - c[0], eye[1] - c[1], eye[2] - c[2]];
            let facing =
                pf.normal[0] * to_eye[0] + pf.normal[1] * to_eye[1] + pf.normal[2] * to_eye[2];
            if facing <= 0.0 && !pf.double_sided {
                continue;
            }
            let points: Vec<Pos2> = pf
                .points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if points.len() != pf.points.len() {
                continue;
            }
            let material = match pf.face.target {
                FaceTarget::Derived { .. } => pf.face.derived_wall(world).and_then(|w| w.material),
                FaceTarget::Wall { .. } => pf.face.wall_of(world).and_then(|w| w.material),
                FaceTarget::Surface { .. } => pf.face.surface(world).and_then(|s| s.tri_a.material),
            };
            let floor = matches!(
                pf.face.target,
                FaceTarget::Surface {
                    kind: SurfaceKind::Floor,
                    ..
                }
            );
            let hue = |m: u64| {
                let h = (m.wrapping_mul(2654435761) % 360) as f32;
                let (s, v) = (0.45f32, 0.80f32);
                let k = |n: f32| {
                    let t = (n + h / 60.0) % 6.0;
                    v - v * s * t.min(4.0 - t).clamp(0.0, 1.0)
                };
                Color32::from_rgb(
                    (k(5.0) * 255.0) as u8,
                    (k(3.0) * 255.0) as u8,
                    (k(1.0) * 255.0) as u8,
                )
            };
            let picked = rig
                .ws
                .grid
                .selection
                .items()
                .iter()
                .any(|e| matches!(e, GridElement::Face(f) if *f == pf.face));
            let fill = if picked {
                Color32::from_rgb(255, 150, 30)
            } else if floor {
                Color32::from_rgb(78, 84, 98)
            } else {
                material.map_or(Color32::from_rgb(150, 150, 150), |m| hue(m.raw()))
            };
            let depth = to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2];
            shapes.push((
                depth,
                egui::Shape::convex_polygon(
                    points,
                    fill,
                    Stroke::new(1.0, Color32::from_rgb(30, 32, 40)),
                ),
            ));
        }
    });
    shapes.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, shape) in shapes {
        rig.out.shapes.push(egui::epaint::ClippedShape {
            clip_rect: rect,
            shape,
        });
    }
}

/// Captures for the review page: `PSXED_EXTRUDEALL_CAPTURE=<dir>
/// PSXED_EXTRUDEALL_PROJECT=<dp-world copy>`; without them this does
/// nothing. The hall's back wall (hand placed) pulled towards the viewer
/// and pushed away, before and after.
#[test]
fn extrudeall_captures() {
    let (Some(dir), Some(project)) = (
        std::env::var_os("PSXED_EXTRUDEALL_CAPTURE"),
        std::env::var_os("PSXED_EXTRUDEALL_PROJECT"),
    ) else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let fresh = || {
        let mut rig = WindowRig::from_dir(std::path::Path::new(&project), super::grid_live::WINDOW);
        rig.ws.apply_grid_pose("view=3d").unwrap();
        for _ in 0..3 {
            rig.frame(vec![]);
        }
        rig.ws.set_grid_tool(crate::grid_tools::GridTool::Extrude);
        rig.ws.grid_active_room = Some(HALL);
        let s = rig.world().scale.to_editor(1024) as f32;
        look(
            &mut rig,
            [6.4 * s, 5600.0, 8.0 * s],
            [3.5 * s, 500.0, 1.0 * s],
        );
        rig
    };
    let shot = |rig: &mut WindowRig, name: &str| {
        for _ in 0..4 {
            rig.frame(vec![]);
        }
        paint(rig);
        rig.capture(&dir.join(format!("{name}.png")));
    };
    // The hall's back wall is the row z = 1; its pieces are hand placed.
    let wall = |rig: &WindowRig, z: i32| north_wall(rig, [3, z], 1200);
    let (before, after) = {
        let mut rig = fresh();
        rig.click(wall(&rig, 1));
        shot(&mut rig, "extrudeall-1-column-before");
        let record = records(&rig, [3, 1], WallEdge::North);
        assert_eq!(record, 1, "the back wall is hand placed");
        let (a, b) = (wall(&rig, 1), wall(&rig, 2));
        rig.drag(a, b);
        shot(&mut rig, "extrudeall-1-column-after");
        (record, records(&rig, [3, 1], WallEdge::North))
    };
    assert_eq!((before, after), (1, 0));
    let mut rig = fresh();
    rig.click(wall(&rig, 1));
    shot(&mut rig, "extrudeall-2-corridor-before");
    let (a, b) = (wall(&rig, 1), wall(&rig, 0));
    rig.drag(a, b);
    shot(&mut rig, "extrudeall-2-corridor-after");
    assert!(floor(&rig, [3, 0]).is_some(), "{}", rig.ws.status);
    // Pushed on a second sector: the corridor grows from its end wall.
    crate::grid_tools::extrude_tool::step(&mut rig.ws, 1);
    shot(&mut rig, "extrudeall-3-corridor-longer");
}
