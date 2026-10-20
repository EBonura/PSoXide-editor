//! Captures for the review page of the wall findings:
//! `PSXED_WALLFIX_CAPTURE=<dir> PSXED_WALLFIX_LABEL=before|after
//! PSXED_WALLFIX_PROJECT=<dp-world copy>` writes the paint captures from
//! the whole window (1456 x 819); without the variables this does nothing.
//! Uses only what the editor offered before the fixes, so the same test runs
//! on both sides.

use super::grid_live::WindowRig;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::bands::effective_set;
use psxed_project::grid::edit::FaceRef;
use psxed_project::{ResourceData, ResourceId};

fn settle(rig: &mut WindowRig) {
    for _ in 0..4 {
        rig.frame(vec![]);
    }
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

/// Draw the wall pieces over the 3D view of the last frame (the rig's 3D
/// view is an empty texture): the pieces painted with `paint` filled orange,
/// the other pieces of banded walls outlined in grey.
fn wire(rig: &mut WindowRig, paint: Option<ResourceId>) {
    let rect = rig.ws.last_persp_viewport_rect;
    let camera = rig.ws.viewport_3d_camera();
    let cam = crate::grid_tools::pick::Cam::new(camera, rect);
    let mut shapes: Vec<egui::Shape> = Vec::new();
    rig.ws.with_pick_cache(|cache| {
        for pf in cache.faces.iter().filter(|pf| !pf.void) {
            let Some(band) = pf.band else { continue };
            let points: Vec<Pos2> = pf
                .points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if points.len() != pf.points.len() {
                continue;
            }
            let painted = paint.is_some() && piece_material_of(rig.world(), pf.face, band) == paint;
            if painted {
                shapes.push(egui::Shape::convex_polygon(
                    points,
                    Color32::from_rgba_unmultiplied(255, 140, 20, 170),
                    Stroke::new(1.5, Color32::from_rgb(255, 200, 80)),
                ));
            } else {
                shapes.push(egui::Shape::closed_line(
                    points,
                    Stroke::new(1.0, Color32::from_rgb(130, 136, 150)),
                ));
            }
        }
    });
    for shape in shapes {
        rig.out.shapes.push(egui::epaint::ClippedShape {
            clip_rect: rect,
            shape,
        });
    }
}

fn piece_material_of(
    world: &psxed_project::grid::GridWorld,
    face: FaceRef,
    band: u8,
) -> Option<ResourceId> {
    effective_set(world, face).and_then(|(_, set)| set.bands.get(usize::from(band))?.material)
}

#[test]
fn wall_captures() {
    let (Some(dir), Some(project)) = (
        std::env::var_os("PSXED_WALLFIX_CAPTURE"),
        std::env::var_os("PSXED_WALLFIX_PROJECT"),
    ) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_WALLFIX_LABEL").unwrap_or_else(|_| "after".into());
    let mut rig = WindowRig::from_dir(Path::new(&project), super::grid_live::WINDOW);
    // The Starter room: its back wall (the row z = 6) is banded in two runs.
    let s = rig.world().scale.to_editor(1024) as f32;
    let button = rig.at("3D");
    rig.click(button);
    settle(&mut rig);
    look_from(
        &mut rig,
        [2.0 * s, 3600.0, 3.0 * s],
        [2.0 * s, 500.0, 6.0 * s],
    );
    // The material to paint with: one that no wall of the room uses.
    let brick = rig
        .ws
        .project
        .resources
        .iter()
        .filter(|r| matches!(r.data, ResourceData::Material(_)))
        .map(|r| r.id)
        .last()
        .unwrap();
    rig.ws.grid.material = Some(brick);
    rig.ws.set_grid_tool(GridTool::Paint);
    settle(&mut rig);
    // The middle segment of the run, its lowest band.
    let at = rig.persp_screen([1.5 * s, 300.0, 6.0 * s]).unwrap();
    rig.move_to(at);
    settle(&mut rig);
    wire(&mut rig, Some(brick));
    rig.capture(&dir.join(format!("paint-1-hover-{label}.png")));
    rig.click(at);
    settle(&mut rig);
    wire(&mut rig, Some(brick));
    rig.capture(&dir.join(format!("paint-2-after-click-{label}.png")));
}
