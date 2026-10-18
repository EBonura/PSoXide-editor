//! Working on one triangle of a floor or ceiling: Shift+click in the Split
//! tool voids or restores it, and Alt+drag in the Height tool moves just that
//! half.
//!
//! A void triangle draws nothing and the sector closes itself along the
//! diagonal. A triangle drag first makes the surface split-level (the second
//! triangle gets its own copy of the heights), then moves the three corners of
//! the picked triangle, so a derived diagonal wall (riser or drop) shows
//! between the halves while the pointer moves. The split is part of the same
//! undo step as the drag, and a drag brought back to zero puts the surface
//! back exactly as it was.

use super::face_tools::{face_under, tri_at};
use super::height_tool::nudge_gesture;
use super::pick::PickLevel;
use super::*;

use psxed_project::grid::edit::natural::{make_split_level, set_void, Triangles};
use psxed_project::grid::edit::{CornerRef, FaceRef, FaceTarget, GridElement, HeightDrag};
use psxed_project::grid::{GridSurface, GridWorld, CLICK_UNITS};

/// A triangle drag in flight, beside the plain height gesture.
#[derive(Clone, Debug)]
pub(crate) struct TriangleGrab {
    /// The floor or ceiling.
    pub face: FaceRef,
    /// Which half: `0` is triangle A, `1` triangle B.
    pub tri: u8,
    /// The surface as it was when the drag began, kept when the drag had to
    /// make it split-level, so a zero drag restores it exactly.
    pub original: Option<GridSurface>,
}

impl TriangleGrab {
    /// Bring the surface to the state `delta` needs: split-level when the
    /// drag has moved, the original again when it is back at zero.
    pub(crate) fn prepare(&self, world: &mut GridWorld, delta: i32) {
        let Some(original) = &self.original else {
            return;
        };
        let Some(surface) = self.face.surface_mut(world) else {
            return;
        };
        if delta == 0 {
            *surface = original.clone();
        } else {
            make_split_level(surface);
        }
    }

    /// "Triangle B" and whether it is a floor or a ceiling.
    pub(crate) fn label(&self) -> String {
        let name = if self.tri == 0 { "A" } else { "B" };
        let kind = match self.face.target {
            FaceTarget::Surface {
                kind: psxed_project::grid::edit::SurfaceKind::Ceiling,
                ..
            } => "ceiling",
            _ => "floor",
        };
        format!("Triangle {name} of the {kind}")
    }
}

/// The triangle of a floor or ceiling under the pointer, when it is over a
/// face and not over a vertex or an edge handle.
pub(super) fn triangle_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(FaceRef, u8)> {
    let world = ws.project.active_scene().world.grid()?;
    match ptr.view {
        GridView::Top(transform) => {
            let GridElement::Face(face) = super::height_tool::top_element(ws, transform, ptr.pos)?
            else {
                return None;
            };
            face.surface(world)?;
            let [x, z] = transform.screen_to_world(ptr.pos);
            Some((face, tri_at(world, face, x, z)?))
        }
        GridView::Persp(rect) => {
            let (GridElement::Face(face), Some(hit)) =
                ws.pick_element_3d(rect, ptr.pos, PickLevel::Vertex)?
            else {
                return None;
            };
            face.surface(world)?;
            let tri = hit
                .tri
                .or_else(|| tri_at(world, face, hit.point[0], hit.point[2]))?;
            Some((face, tri))
        }
    }
}

/// Make the grab of triangle `tri` of `face`: the drag over its three
/// corners (the surface made split-level for the building of it, then put
/// back, the drag itself does the split when it first moves).
fn grab(ws: &mut EditorWorkspace, face: FaceRef, tri: u8) -> Option<(HeightDrag, TriangleGrab)> {
    let world = ws.project.active_scene_mut().world.grid_mut()?;
    let surface = face.surface_mut(world)?;
    let original = surface.clone();
    let needs_split = surface.heights_b.is_none();
    make_split_level(surface);
    let ids = surface.split.triangle(usize::from(tri.min(1)));
    let corners = ids.map(|corner| CornerRef {
        face,
        layer: tri.min(1),
        corner: corner as u8,
    });
    let drag = HeightDrag::begin(world, &corners, false);
    if needs_split {
        *face.surface_mut(world)? = original.clone();
    }
    (!drag.is_empty()).then_some((
        drag,
        TriangleGrab {
            face,
            tri: tri.min(1),
            original: needs_split.then_some(original),
        },
    ))
}

/// Alt+press in the Height tool: start a triangle drag when the pointer is
/// on a floor or ceiling face. Returns whether it did.
pub(super) fn begin_drag(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    let press = ptr.at_press();
    let Some((face, tri)) = triangle_under(ws, &press) else {
        return false;
    };
    let Some((drag, triangle)) = grab(ws, face, tri) else {
        return false;
    };
    let world = ws.project.active_scene().world.grid();
    let grab_point = world
        .and_then(|w| {
            let c = CornerRef {
                face,
                layer: 0,
                corner: 0,
            };
            c.grid_pos(w).map(|g| {
                [
                    w.scale.to_editor(g[0]) as f32,
                    w.scale.to_editor(g[1]) as f32,
                    w.scale.to_editor(g[2]) as f32,
                ]
            })
        })
        .unwrap_or([0.0; 3]);
    let ppu = ws.grid_pixels_per_unit(&press, grab_point);
    ws.status = format!(
        "{}: drag up or down, Shift for the fine step",
        triangle.label()
    );
    ws.grid.gesture = Gesture::Height(Box::new(HeightGesture {
        drag,
        press_y: press.pos.y,
        pixels_per_unit: ppu,
        delta: 0,
        before: Some(Box::new(ws.project.clone())),
        triangle: Some(triangle),
    }));
    true
}

/// A triangle drag in flight at `clicks` (poses, tests).
pub(crate) fn pose_drag(
    ws: &mut EditorWorkspace,
    face: FaceRef,
    tri: u8,
    clicks: f32,
) -> Result<(), String> {
    let (drag, triangle) = grab(ws, face, tri).ok_or("no floor or ceiling there")?;
    let delta = (clicks * CLICK_UNITS as f32).round() as i32;
    let before = ws.project.clone();
    let mut gesture = HeightGesture {
        drag,
        press_y: 0.0,
        pixels_per_unit: 1.0,
        delta: 0,
        before: Some(Box::new(before)),
        triangle: Some(triangle),
    };
    nudge_gesture(ws, &mut gesture, delta);
    ws.grid.gesture = Gesture::Height(Box::new(gesture));
    Ok(())
}

/// Shift+click in the Split tool: void or restore the triangle under the
/// pointer, one undo step.
pub(super) fn void_click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((faces, tri)) = face_under(ws, ptr) else {
        return;
    };
    let Some(face) = faces
        .into_iter()
        .find(|f| matches!(f.target, FaceTarget::Surface { .. }))
    else {
        ws.status = "Void works on floors and ceilings".to_string();
        return;
    };
    let (Some(tri), FaceTarget::Surface { kind, .. }) = (tri, face.target) else {
        ws.status = "Point at one triangle of the face".to_string();
        return;
    };
    let which = if tri == 0 { Triangles::A } else { Triangles::B };
    let name = if tri == 0 { "A" } else { "B" };
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let now_void = face.surface(world).is_some_and(|s| s.is_void(tri.into()));
    if !set_void(world, face.room, face.cell(), kind, which, !now_void) {
        return;
    }
    ws.history.record(before);
    ws.status = if now_void {
        format!("Triangle {name} restored")
    } else {
        format!("Triangle {name} voided: the other half closes itself with a diagonal wall")
    };
    ws.grid_changed();
}

/// Hover of the Height tool with Alt held: the triangle a press would grab.
pub(super) fn hover_triangle(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    if !ptr.alt() {
        return false;
    }
    let Some((face, tri)) = triangle_under(ws, ptr) else {
        return false;
    };
    ws.grid.hover = GridHover::Face {
        face,
        tri: Some(tri),
    };
    true
}
