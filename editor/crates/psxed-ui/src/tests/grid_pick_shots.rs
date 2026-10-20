//! Captures for the review pages of the pick findings:
//! `PSXED_PICK_CAPTURE=<dir> PSXED_PICK_LABEL=before|after` writes one PNG
//! per finding from the whole window (dp-world stand-in, 1456 x 819); without
//! the variable this does nothing. Uses only what the editor offered before
//! the fixes, so the same test runs on both sides.

use super::grid_live::WindowRig;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::bands::{edit_bands, BandScope};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::{Anchor, WallEdge};

fn key(rig: &mut WindowRig, key: egui::Key) {
    for pressed in [true, false] {
        rig.frame(vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}

fn settle(rig: &mut WindowRig) {
    for _ in 0..4 {
        rig.frame(vec![]);
    }
}

/// The Machine Hall's sector size in editor units.
fn sector(rig: &WindowRig) -> f32 {
    rig.world().scale.to_editor(1024) as f32
}

fn look_from(rig: &mut WindowRig, from: [f32; 3], at: [f32; 3]) {
    let (f, a) = (from.map(|v| v as i32), at.map(|v| v as i32));
    let (yaw, pitch) = crate::editor_helpers::camera_angles_to_look_at(f, a).unwrap();
    let camera = &mut rig.ws.camera_rig;
    camera.mode = crate::ViewportCameraMode::Free;
    camera.free_initialized = true;
    camera.free_position = f;
    camera.free_yaw = yaw;
    camera.free_pitch = pitch;
}

/// Draw the world's faces as a wireframe over the 3D view of the last frame
/// (the rig's 3D view is an empty texture): what the camera draws in grey,
/// what it culls (a ceiling seen from above) in dim red.
fn wire(rig: &mut WindowRig) {
    let rect = rig.ws.last_persp_viewport_rect;
    let camera = rig.ws.viewport_3d_camera();
    let cam = crate::grid_tools::pick::Cam::new(camera, rect);
    let eye = camera.position_f32();
    let mut lines: Vec<egui::Shape> = Vec::new();
    rig.ws.with_pick_cache(|cache| {
        for pf in cache.faces.iter().filter(|pf| !pf.void) {
            let n = pf.points.len() as f32;
            let centre = pf.points.iter().fold([0.0; 3], |a, p| {
                [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]
            });
            let to_eye = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
            let facing =
                pf.normal[0] * to_eye[0] + pf.normal[1] * to_eye[1] + pf.normal[2] * to_eye[2];
            let drawn = facing > 0.0 || pf.double_sided;
            let color = if drawn {
                Color32::from_rgb(120, 126, 140)
            } else {
                Color32::from_rgb(110, 50, 50)
            };
            let points: Vec<Pos2> = pf
                .points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if points.len() == pf.points.len() {
                lines.push(egui::Shape::closed_line(points, Stroke::new(1.0, color)));
            }
        }
    });
    for shape in lines {
        rig.out.shapes.push(egui::epaint::ClippedShape {
            clip_rect: rect,
            shape,
        });
    }
}

fn three_d(rig: &mut WindowRig) {
    let button = rig.at("3D");
    rig.click(button);
    settle(rig);
}

#[test]
fn pick_captures() {
    let Some(dir) = std::env::var_os("PSXED_PICK_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_PICK_LABEL").unwrap_or_else(|_| "after".into());

    // Finding 1: a wall cut into two bands. Hover the lower one, then click
    // it. The hall is room 1; its north wall is the row z = -1.
    let mut rig = WindowRig::new();
    three_d(&mut rig);
    let s = sector(&rig);
    let wall = FaceRef::derived(1, [10, -1], WallEdge::North, DerivedKind::Closure);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        // Closure walls are 3072 high in the stand-in; cut at 1536.
        assert!(edit_bands(world, wall, BandScope::Run, |set| set
            .insert_line(0, Anchor::Abs(1536))));
    }
    rig.ws.grid_changed();
    look_from(
        &mut rig,
        [10.5 * s, 2400.0, 8.0 * s],
        [10.5 * s, 1200.0, -1.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Select);
    settle(&mut rig);
    let lower = rig.persp_screen([10.5 * s, 600.0, -1.0 * s]).unwrap();
    rig.move_to(lower);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join(format!("bands-hover-lower-{label}.png")));
    rig.click(lower);
    rig.move_to(lower + Vec2::new(0.0, 160.0));
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join(format!("bands-selected-lower-{label}.png")));

    // Finding 2: the camera above the hall, the pointer on a ceiling corner.
    let mut rig = WindowRig::new();
    three_d(&mut rig);
    let s = sector(&rig);
    look_from(
        &mut rig,
        [11.0 * s, 6200.0, 7.0 * s],
        [11.0 * s, 0.0, 3.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Select);
    settle(&mut rig);
    // A ceiling corner well away from every floor corner on screen, so
    // anything picked there is the ceiling's.
    let ceiling = rig.world().scale.to_editor(3072) as f32;
    let floors: Vec<Pos2> = (7..=15)
        .flat_map(|x| (-1..=7).map(move |z| (x, z)))
        .filter_map(|(x, z)| rig.persp_screen([x as f32 * s, 0.0, z as f32 * s]))
        .collect();
    let corner = (8..=14)
        .flat_map(|x| (0..=6).map(move |z| (x, z)))
        .filter_map(|(x, z)| rig.persp_screen([x as f32 * s, ceiling, z as f32 * s]))
        .filter(|p| rig.ws.last_persp_viewport_rect.shrink(20.0).contains(*p))
        .max_by(|a, b| {
            let far = |p: &Pos2| {
                floors
                    .iter()
                    .map(|f| f.distance(*p))
                    .fold(f32::MAX, f32::min)
            };
            far(a).total_cmp(&far(b))
        })
        .unwrap();
    rig.move_to(corner);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join(format!("ceiling-hover-{label}.png")));

    // Finding 3: two floors selected, then Paint, then a palette material.
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    settle(&mut rig);
    rig.ws.set_grid_tool(GridTool::Select);
    settle(&mut rig);
    rig.click(rig.cell(9, 1));
    rig.ws
        .grid
        .selection
        .add(GridElement::Face(FaceRef::floor(1, [10, 1])));
    settle(&mut rig);
    rig.capture(&dir.join(format!("paint-1-selected-{label}.png")));
    key(&mut rig, egui::Key::P);
    settle(&mut rig);
    rig.capture(&dir.join(format!("paint-2-in-paint-{label}.png")));
    let material = rig
        .ws
        .project
        .resources
        .iter()
        .find(|r| r.name == "DP 3.2")
        .map(|r| r.id)
        .unwrap();
    rig.ws.choose_palette_material(material);
    settle(&mut rig);
    rig.capture(&dir.join(format!("paint-3-material-chosen-{label}.png")));
}
