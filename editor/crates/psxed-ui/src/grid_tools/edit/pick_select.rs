//! Selecting with the Edit tool.
//!
//! Click selects what is under the pointer. Shift+click adds a range (the
//! rectangle of faces, the run of walls, the line of corners), Cmd or Ctrl
//! toggles, a click on nothing clears. A double click replaces what the
//! first click did: a floor or ceiling becomes all of that room's surface of
//! the span, a wall becomes its whole run. Boxes are the Shift+drag and the
//! press on nothing of [`super::press`].

use std::time::{Duration, Instant};

use super::target::{Hit, Target};
use super::*;

use psxed_project::grid::edit::{all_faces_of_room, FaceTarget, GridElement};
use psxed_project::grid::WallEdge;

/// Time and distance within which a second click is a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(450);
const DOUBLE_CLICK_PIXELS: f32 = 8.0;

/// A coarser reading of a pick, one step per backtick: a corner or an edge
/// of a face becomes the face, a wall's edge becomes the wall.
pub(super) fn coarsen(hit: Hit, level: usize) -> Hit {
    let mut hit = hit;
    for _ in 0..level {
        hit.target = match hit.target {
            Target::Corner(c) => Target::Surface(c.face),
            Target::SurfaceEdge(e) => Target::Surface(e.face),
            Target::WallEdge(f, _) | Target::WallArrow(f) => Target::Wall(f, Vec::new()),
            other => other,
        };
    }
    hit
}

/// A placed mesh the pointer is over, when the pick under it is only a
/// floor, a wall or nothing (a corner, an edge or an arrow is a handle and
/// keeps priority) and no modifier asks to extend a selection.
pub(super) fn mesh_over(ws: &EditorWorkspace, ptr: &GridPointer, hit: &Hit) -> Option<u16> {
    if ptr.shift() || ptr.command() {
        return None;
    }
    if !matches!(
        hit.target,
        Target::Nothing | Target::Surface(_) | Target::Wall(..)
    ) {
        return None;
    }
    ws.mesh_in_front(ptr)
}

/// A click.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let now = Instant::now();
    let double = ws.grid.edit.last_click.is_some_and(|(at, pos)| {
        now.duration_since(at) < DOUBLE_CLICK && pos.distance(ptr.pos) < DOUBLE_CLICK_PIXELS
    });
    ws.grid.edit.last_click = Some((now, ptr.pos));
    let hit = super::candidate(ws, ptr);
    if let Some(id) = mesh_over(ws, ptr, &hit) {
        ws.select_mesh_instance(id);
        return;
    }
    if hit.target == Target::Nothing {
        if !ptr.shift() && !ptr.command() {
            ws.grid.selection.clear();
            ws.grid.plane_sel.clear();
            ws.grid.mesh_sel = None;
            ws.status = "Selection cleared".to_string();
        }
        return;
    }
    if double {
        ws.grid.edit.last_click = None;
        select_group(ws, &hit);
        return;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let picked = hit.target.elements();
    if picked.is_empty() {
        return;
    }
    if ptr.command() {
        for element in picked {
            ws.grid.selection.toggle(element);
        }
    } else if ptr.shift() {
        let selected: Vec<GridElement> = ws.grid.selection.items().to_vec();
        let first = picked[0];
        let mut added = select_tool::extend_range(world, &selected, first);
        added.extend(picked.iter().skip(1).copied());
        ws.grid.selection.extend(added);
        ws.grid.selection.add(first);
    } else {
        ws.grid.selection.set(picked);
    }
    ws.after_grid_selection_changed();
    ws.status = format!("{} selected", ws.grid.selection.len());
}

/// Double click: the room's surface, or the wall's whole run.
pub(super) fn select_group(ws: &mut EditorWorkspace, hit: &Hit) {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let elements: Vec<GridElement> = match &hit.target {
        Target::Surface(face)
        | Target::Corner(psxed_project::grid::edit::CornerRef { face, .. }) => {
            same_surface(world, *face)
        }
        Target::SurfaceEdge(edge) => same_surface(world, edge.face),
        Target::Wall(face, _) | Target::WallEdge(face, _) | Target::WallArrow(face) => {
            wall_run(world, *face)
        }
        Target::Nothing => return,
    };
    if elements.is_empty() {
        return;
    }
    let count = elements.len();
    ws.grid.selection.set(elements);
    ws.after_grid_selection_changed();
    ws.status = format!("{count} faces selected");
}

/// Every floor (or ceiling) of the room that is the span of `face`.
fn same_surface(world: &psxed_project::grid::GridWorld, face: FaceRef) -> Vec<GridElement> {
    let FaceTarget::Surface { kind, span } = face.target else {
        return Vec::new();
    };
    all_faces_of_room(world, face.room)
        .into_iter()
        .filter(|f| matches!(f.target, FaceTarget::Surface { kind: k, span: s } if k == kind && s == span))
        .map(GridElement::Face)
        .collect()
}

/// The faces of the wall that run on from `face` along its edge: the same
/// kind of wall, on the same edge of the next cell, as far as it goes.
fn wall_run(world: &psxed_project::grid::GridWorld, face: FaceRef) -> Vec<GridElement> {
    type Maker = Box<dyn Fn([i32; 2]) -> FaceRef>;
    let (edge, make): (WallEdge, Maker) = match face.target {
        FaceTarget::Derived {
            edge,
            kind,
            seg,
            span,
        } => (
            edge,
            Box::new(move |cell| {
                FaceRef::new(
                    face.room,
                    psxed_project::grid::LeafKey::sector(cell),
                    FaceTarget::Derived {
                        edge,
                        kind,
                        seg,
                        span,
                    },
                )
            }),
        ),
        FaceTarget::Wall { edge, index } => (
            edge,
            Box::new(move |cell| FaceRef::wall(face.room, cell, edge, index)),
        ),
        FaceTarget::Surface { .. } => return Vec::new(),
    };
    let step: [i32; 2] = match edge {
        WallEdge::North | WallEdge::South => [1, 0],
        WallEdge::East | WallEdge::West => [0, 1],
        _ => return vec![GridElement::Face(face)],
    };
    let mut out = vec![face];
    for sign in [1, -1] {
        let mut cell = face.cell();
        // A run is as long as a room is wide.
        for _ in 0..512 {
            cell = [cell[0] + step[0] * sign, cell[1] + step[1] * sign];
            let next = make(cell);
            if !next.exists(world) {
                break;
            }
            out.push(next);
        }
    }
    out.into_iter().map(GridElement::Face).collect()
}
