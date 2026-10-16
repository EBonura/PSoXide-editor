//! The Paint and Split tools: both act on the face under the pointer.

use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::{
    material_of, set_material, toggle_split, FaceRef, FaceTarget, GridElement,
};
use psxed_project::grid::{GridSplit, SECTOR_UNITS};

/// The face (and triangle of a split surface) the pointer is over.
pub(super) fn face_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
) -> Option<(Vec<FaceRef>, Option<u8>)> {
    match ptr.view {
        GridView::Persp(rect) => {
            let hit = ws.pick_face_3d(rect, ptr.pos)?;
            Some((vec![hit.face], hit.tri))
        }
        GridView::Top(transform) => {
            let world = ws.project.active_scene().world.grid()?;
            if let Some((room, cell, edge)) = ws.top_wall_at(transform, ptr.pos, 6.0) {
                let count = FaceRef::floor(room, cell)
                    .sector(world)
                    .map_or(0, |s| s.walls(edge).len());
                return Some((
                    (0..count)
                        .map(|i| FaceRef::wall(room, cell, edge, i))
                        .collect(),
                    None,
                ));
            }
            let [wx, wz] = transform.screen_to_world(ptr.pos);
            let cell = cell_at_editor(world.scale, wx, wz);
            let room = ws.top_room_at(cell)?;
            let sector = FaceRef::floor(room, cell).sector(world)?;
            let face = if ptr.alt() || sector.floor.is_none() {
                sector.ceiling.as_ref()?;
                FaceRef::ceiling(room, cell)
            } else {
                FaceRef::floor(room, cell)
            };
            // Which triangle: fractions inside the cell against the diagonal.
            let size = world.scale.sector_editor_units() as f32;
            let (u, v) = (
                (wx - world.scale.to_editor(cell[0] * SECTOR_UNITS) as f32) / size,
                (wz - world.scale.to_editor(cell[1] * SECTOR_UNITS) as f32) / size,
            );
            let tri = face.surface(world).and_then(|s| {
                let split = !s.is_planar() || s.heights_b.is_some() || s.tri_b.is_some();
                split.then(|| {
                    let first = match s.split {
                        GridSplit::NwSe => u >= v,
                        GridSplit::NeSw => u + v <= 1.0,
                    };
                    u8::from(!first)
                })
            });
            Some((vec![face], tri))
        }
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = match face_under(ws, ptr) {
        Some((faces, tri)) if faces.len() == 1 => GridHover::Face {
            face: faces[0],
            tri,
        },
        Some((faces, _)) => GridHover::Elements(faces.into_iter().map(GridElement::Face).collect()),
        None => GridHover::None,
    };
}

/// Apply the palette material to the faces under the pointer. Returns
/// whether anything changed. `before` is taken on the first change of a
/// stroke and recorded for undo.
fn paint_at(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    before: &mut Option<ProjectDocument>,
) -> bool {
    let Some(material) = ws.grid.material else {
        ws.status = "Pick a material in the palette first".to_string();
        return false;
    };
    let Some((faces, tri)) = face_under(ws, ptr) else {
        return false;
    };
    let snapshot = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let mut changed = false;
    for face in faces {
        changed |= set_material(world, face, tri, Some(material));
    }
    if changed {
        if before.is_none() {
            *before = Some(snapshot);
        }
        ws.grid_changed();
    }
    changed
}

pub(super) fn paint_press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if ptr.command() {
        sample(ws, &ptr.at_press());
        return;
    }
    let mut before = None;
    paint_at(ws, &ptr.at_press(), &mut before);
    ws.grid.gesture = Gesture::Paint { undo_pushed: false };
    if let Some(snapshot) = before {
        ws.history.record(snapshot);
        ws.grid.gesture = Gesture::Paint { undo_pushed: true };
    }
}

pub(super) fn paint_drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Paint { undo_pushed } = ws.grid.gesture else {
        return;
    };
    let mut before = None;
    if paint_at(ws, ptr, &mut before) {
        if !undo_pushed {
            if let Some(snapshot) = before {
                ws.history.record(snapshot);
            }
        }
        ws.grid.gesture = Gesture::Paint { undo_pushed: true };
    }
}

pub(super) fn paint_release(ws: &mut EditorWorkspace) {
    if matches!(ws.grid.gesture, Gesture::Paint { .. }) {
        ws.grid.gesture = Gesture::Idle;
    }
}

pub(super) fn paint_click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if ptr.command() {
        sample(ws, ptr);
        return;
    }
    let mut before = None;
    if paint_at(ws, ptr, &mut before) {
        if let Some(snapshot) = before {
            ws.history.record(snapshot);
        }
        let name = ws
            .grid
            .material
            .and_then(|id| ws.project.resource_name(id))
            .unwrap_or("material")
            .to_string();
        ws.status = format!("Painted {name}");
    }
}

/// Eyedropper: take the material of the face under the pointer.
fn sample(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((faces, tri)) = face_under(ws, ptr) else {
        return;
    };
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    if let Some(material) = faces.first().and_then(|f| material_of(world, *f, tri)) {
        ws.grid.material = Some(material);
        ws.grid.palette_scroll_to = Some(material);
        ws.status = "Sampled material".to_string();
    } else {
        ws.status = "That face has no material".to_string();
    }
}

pub(super) fn split_click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((faces, _)) = face_under(ws, ptr) else {
        return;
    };
    let Some(face) = faces
        .into_iter()
        .find(|f| matches!(f.target, FaceTarget::Surface(_)))
    else {
        ws.status = "Split works on floors and ceilings".to_string();
        return;
    };
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    if toggle_split(world, face) {
        let now = face.surface(world).map(|s| s.split);
        ws.history.record(before);
        ws.status = format!(
            "Diagonal now {}",
            match now {
                Some(GridSplit::NwSe) => "NW to SE",
                _ => "NE to SW",
            }
        );
        ws.grid_changed();
    }
}
