//! The Paint and Split tools: both act on the face under the pointer.

use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::{
    material_of, set_material, toggle_split, FaceRef, FaceTarget, GridElement,
};
use psxed_project::grid::{GridSplit, GridWorld};

/// Which triangle of `face` a point (editor units) is in, from where it
/// falls against the diagonal.
pub(super) fn tri_at(world: &GridWorld, face: FaceRef, x: f32, z: f32) -> Option<u8> {
    let surface = face.surface(world)?;
    let ([x0, z0], size) = super::view::leaf_square(world.scale, face.leaf);
    let u = (x - x0) / size;
    let v = (z - z0) / size;
    let first = match surface.split {
        GridSplit::NwSe => u >= v,
        GridSplit::NeSw => u + v <= 1.0,
    };
    Some(u8::from(!first))
}

/// The face (and triangle of a split surface) the pointer is over. Holding
/// Shift names the triangle even on a flat quad, so one half can be painted.
pub(super) fn face_under(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
) -> Option<(Vec<FaceRef>, Option<u8>)> {
    face_under_with(ws, ptr, ptr.shift())
}

/// [`face_under`] with `name_triangle` saying whether a triangle is named on
/// a plain quad (a split surface always names one).
pub(super) fn face_under_with(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    name_triangle: bool,
) -> Option<(Vec<FaceRef>, Option<u8>)> {
    match ptr.view {
        GridView::Persp(rect) => {
            let hit = ws.pick_face_3d(rect, ptr.pos)?;
            let world = ws.project.active_scene().world.grid()?;
            let tri = hit.tri.or_else(|| {
                name_triangle
                    .then(|| tri_at(world, hit.face, hit.point[0], hit.point[2]))
                    .flatten()
            });
            Some((vec![hit.face], tri))
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
            // A room with leaves of several sizes names the leaf itself.
            let (leaf, sector) = match world.rooms.get(room) {
                Some(r) if !r.leaves.is_sectors_only() => {
                    super::view::leaf_at_editor(world.scale, r, wx, wz)?
                }
                _ => (
                    psxed_project::grid::LeafKey::sector(cell),
                    FaceRef::floor(room, cell).sector(world)?,
                ),
            };
            // A repeated click takes the next layer down (see `cycle`): the
            // floor, then the ceiling. A sector with only one has only it.
            let layer = match ws.top_choice(ptr) {
                super::cycle::TopChoice::Layer(layer) => layer,
                super::cycle::TopChoice::Plane(_) => 0,
            };
            // The floor open at the storey cursor in a column of several.
            let span = ws.span_at_cursor(room, cell);
            let open = sector.spans().get(usize::from(span))?;
            let face = if layer == 1 && open.ceiling.is_some() || open.floor.is_none() {
                open.ceiling.as_ref()?;
                FaceRef::new(room, leaf, FaceTarget::ceiling(span))
            } else {
                FaceRef::new(room, leaf, FaceTarget::floor(span))
            };
            let tri = face.surface(world).and_then(|s| {
                let split =
                    !s.is_planar() || s.heights_b.is_some() || s.tri_b.is_some() || s.void != 0;
                (split || name_triangle)
                    .then(|| tri_at(world, face, wx, wz))
                    .flatten()
            });
            Some((vec![face], tri))
        }
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if ws.grid.tool == GridTool::Paint {
        if let Some(id) = super::plane_tool::paint_target(ws, ptr) {
            ws.grid.hover = GridHover::Plane(id);
            return;
        }
    }
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
    if let Some(id) = super::plane_tool::paint_target(ws, ptr) {
        let snapshot = ws.project.clone();
        let changed = super::plane_tool::paint(ws, id, Some(material));
        if changed {
            if before.is_none() {
                *before = Some(snapshot);
            }
            ws.grid_changed();
        }
        return changed;
    }
    // A banded wall paints the band under the pointer, not the wall.
    if let Some((face, band)) = super::bands_tool::band_under(ws, ptr) {
        let snapshot = ws.project.clone();
        let scope = ws.grid.band_scope;
        let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
            return false;
        };
        let changed =
            psxed_project::grid::edit::bands::paint_band(world, face, band, Some(material), scope);
        if changed {
            if before.is_none() {
                *before = Some(snapshot);
            }
            ws.grid_changed();
        }
        return changed;
    }
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
    if let Some(id) = super::plane_tool::paint_target(ws, ptr) {
        let material = ws
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| id.get(w))
            .and_then(|p| p.material);
        if let Some(material) = material {
            ws.grid.material = Some(material);
            ws.grid.palette_scroll_to = Some(material);
            ws.status = "Sampled material".to_string();
        } else {
            ws.status = "That plane has no material".to_string();
        }
        return;
    }
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
    // Shift+click voids or restores the triangle under the pointer.
    if ptr.shift() {
        super::triangle_tool::void_click(ws, ptr);
        return;
    }
    let Some((faces, _)) = face_under(ws, ptr) else {
        return;
    };
    let Some(face) = faces
        .into_iter()
        .find(|f| matches!(f.target, FaceTarget::Surface { .. }))
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
