//! Duplicate (Cmd+D): copy the selection and carry the copy on the pointer.
//!
//! One command for everything the grid editor selects. What is selected
//! decides what is copied:
//!
//! * **grid geometry** (cells or faces): the stamping the clipboard already
//!   does ([`GridClipboard::paste`]), with a ghost over the plan or the 3D
//!   view; the nodes standing on the copied floors are copied with it and
//!   carried the same distance;
//! * **free planes**: the plane clipboard's ghost, without touching Ctrl+C;
//! * **a static mesh**: a copy placed in its room, following the floor under
//!   the pointer on the Mesh tool's lattice;
//! * **scene nodes** (lights, actors, props, interactions): copies named the
//!   way a new one would be ("Light 4", "Crate 2"), moved by the pointer's
//!   travel on the editor's node snap.
//!
//! A click drops the copy, Escape puts the document back exactly, and the
//! whole gesture is one undo step. Up and Down lift the copy a click at a
//! time (Alt for a fine step). A drop over existing geometry replaces it,
//! as a paste does.

use super::view::cell_at_editor;
use super::*;

use psxed_project::grid::edit::{
    place_mesh, update_mesh, FaceRef, FaceTarget, GridClipboard, GridElement, PlaneClipboard,
    SurfaceKind,
};
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS, SECTOR_UNITS};

/// A node copy being carried, and where it started.
#[derive(Clone, Debug)]
pub(crate) struct NodeCopy {
    id: NodeId,
    origin: [f32; 3],
}

/// What the grab carries.
#[derive(Clone, Debug)]
pub(crate) enum DupKind {
    /// Scene nodes, moved by the pointer's travel from where it first was.
    Nodes {
        copies: Vec<NodeCopy>,
        anchor: Option<[f32; 2]>,
        delta: [f32; 2],
    },
    /// A static mesh placed by the pointer.
    Mesh { id: u16, saved: Option<u16> },
    /// Grid geometry with the nodes that stand on it.
    Cells {
        clip: GridClipboard,
        room: usize,
        /// World sector of the clip's smallest corner at capture.
        min: [i32; 2],
        nodes: Vec<NodeCopy>,
    },
}

/// A duplicate in flight.
#[derive(Clone, Debug)]
pub(crate) struct DupGrab {
    /// The document before the copy existed: Escape restores it, a drop
    /// records it as the undo step.
    before: Box<ProjectDocument>,
    /// The node selection to give back on Escape.
    saved_nodes: (NodeId, Vec<NodeId>),
    /// Grid units the copy is lifted.
    lift: i32,
    kind: DupKind,
}

/// Cmd+D.
pub(super) fn begin(ws: &mut EditorWorkspace) {
    if ws.grid.grab.is_some() {
        return;
    }
    ws.grid.gesture = Gesture::Idle;
    ws.grid.paste = None;
    ws.grid.plane_paste = None;
    ws.retain_valid_planes();
    let started = if !ws.grid.selection.is_empty() {
        begin_cells(ws)
    } else if !ws.grid.plane_sel.is_empty() {
        begin_planes(ws)
    } else if ws.grid.mesh_sel.is_some() {
        begin_mesh(ws)
    } else {
        begin_nodes(ws)
    };
    if !started && ws.status.is_empty() {
        ws.status = "Select something to duplicate".to_string();
    }
}

fn saved_nodes(ws: &EditorWorkspace) -> (NodeId, Vec<NodeId>) {
    (
        ws.selection.selected_node,
        ws.selection.selected_nodes.iter().copied().collect(),
    )
}

fn copies_of(ws: &EditorWorkspace, ids: &[NodeId]) -> Vec<NodeCopy> {
    let scene = ws.project.active_scene();
    ids.iter()
        .filter_map(|id| {
            scene.node(*id).map(|n| NodeCopy {
                id: *id,
                origin: n.transform.translation,
            })
        })
        .collect()
}

fn begin_nodes(ws: &mut EditorWorkspace) -> bool {
    let roots = ws.duplicate_roots();
    if roots.is_empty() {
        ws.status = "Select something to duplicate".to_string();
        return false;
    }
    let before = Box::new(ws.project.clone());
    let saved = saved_nodes(ws);
    let ids = ws.copy_nodes(roots, true);
    if ids.is_empty() {
        return false;
    }
    ws.select_nodes(&ids);
    let copies = copies_of(ws, &ids);
    ws.mark_dirty();
    ws.status = format!(
        "Duplicated {}: move, click to drop, Esc cancels",
        count_label(ids.len(), "node")
    );
    ws.grid.grab = Some(DupGrab {
        before,
        saved_nodes: saved,
        lift: 0,
        kind: DupKind::Nodes {
            copies,
            anchor: None,
            delta: [0.0, 0.0],
        },
    });
    true
}

fn count_label(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

fn begin_planes(ws: &mut EditorWorkspace) -> bool {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    let Some(clip) = PlaneClipboard::capture(world, &ws.grid.plane_sel) else {
        return false;
    };
    ws.grid.plane_paste = Some(PlanePaste { clip, at: None });
    ws.status = "Duplicate planes: click to place, R turn, H flip, Esc cancel".to_string();
    true
}

fn begin_mesh(ws: &mut EditorWorkspace) -> bool {
    let Some(id) = ws.grid.mesh_sel else {
        return false;
    };
    let before = Box::new(ws.project.clone());
    let saved = saved_nodes(ws);
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let Some((room, index)) = world.find_mesh(id) else {
        ws.status = "That mesh is gone".to_string();
        return false;
    };
    let copy = world.rooms[room].meshes[index].clone();
    let placed = match place_mesh(world, room, copy) {
        Ok(new_id) => new_id,
        Err(refusal) => {
            ws.status = refusal.describe();
            return false;
        }
    };
    ws.grid.mesh_sel = Some(placed);
    ws.mark_dirty();
    ws.status = "Duplicated mesh: move, click to drop, Esc cancels".to_string();
    ws.grid.grab = Some(DupGrab {
        before,
        saved_nodes: saved,
        lift: 0,
        kind: DupKind::Mesh {
            id: placed,
            saved: Some(id),
        },
    });
    true
}

/// Whether a scene node stands on one of the selected floors: in the sector,
/// and above that floor by no more than a hall's height.
fn nodes_on_floors(ws: &EditorWorkspace, faces: &[FaceRef]) -> Vec<NodeId> {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return Vec::new();
    };
    let cells: std::collections::BTreeSet<[i32; 2]> = faces
        .iter()
        .filter(|f| {
            matches!(
                f.target,
                FaceTarget::Surface {
                    kind: SurfaceKind::Floor,
                    ..
                }
            )
        })
        .map(|f| f.cell())
        .collect();
    let reach = 3 * world.scale.to_editor(SECTOR_UNITS);
    let scene = ws.project.active_scene();
    scene
        .nodes()
        .iter()
        .filter(|n| n.id != NodeId::ROOT)
        .filter(|n| !matches!(n.kind, NodeKind::Group | NodeKind::Node))
        // Only the top of a subtree: its children follow it.
        .filter(|n| {
            n.parent
                .is_none_or(|p| p == NodeId::ROOT || scene.node(p).is_none())
        })
        .filter(|n| {
            let [x, y, z] = n.transform.translation;
            let cell = cell_at_editor(world.scale, x, z);
            if !cells.contains(&cell) {
                return false;
            }
            world
                .floor_height_at(x.round() as i32, z.round() as i32, y.round() as i32 + 96)
                .is_some_and(|floor| y.round() as i32 - floor <= reach)
        })
        .map(|n| n.id)
        .collect()
}

fn begin_cells(ws: &mut EditorWorkspace) -> bool {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    let faces = ws.grid.selection.faces(world);
    let Some(clip) = GridClipboard::capture(world, &faces) else {
        ws.status = "Select faces or cells to duplicate".to_string();
        return false;
    };
    let room = faces[0].room;
    let min = [
        faces.iter().map(|f| f.cell()[0]).min().unwrap_or(0),
        faces.iter().map(|f| f.cell()[1]).min().unwrap_or(0),
    ];
    let standing = nodes_on_floors(ws, &faces);
    let before = Box::new(ws.project.clone());
    let saved = saved_nodes(ws);
    // Copy the standing nodes through the same naming as a node duplicate.
    let roots: Vec<NodeId> = standing;
    let ids = ws.copy_nodes(roots, true);
    let nodes = copies_of(ws, &ids);
    ws.grid.last_room = room;
    ws.grid.paste = Some(PasteGhost {
        clip: clip.clone(),
        at: None,
    });
    ws.mark_dirty();
    ws.status = format!(
        "Duplicated {}{}: click to drop, Up/Down lift, Esc cancels",
        count_label(faces.len(), "face"),
        if nodes.is_empty() {
            String::new()
        } else {
            format!(" and {}", count_label(nodes.len(), "node"))
        }
    );
    ws.grid.grab = Some(DupGrab {
        before,
        saved_nodes: saved,
        lift: 0,
        kind: DupKind::Cells {
            clip,
            room,
            min,
            nodes,
        },
    });
    true
}

fn editor_lift(ws: &EditorWorkspace, lift: i32) -> f32 {
    ws.project
        .active_scene()
        .world
        .grid()
        .map_or(0.0, |w| w.scale.to_editor(lift) as f32)
}

fn place_nodes(ws: &mut EditorWorkspace, nodes: &[NodeCopy], delta: [f32; 3]) {
    let scene = ws.project.active_scene_mut();
    for copy in nodes {
        if let Some(node) = scene.node_mut(copy.id) {
            node.transform.translation = [
                copy.origin[0] + delta[0],
                copy.origin[1] + delta[1],
                copy.origin[2] + delta[2],
            ];
        }
    }
}

/// Move the copy for the pointer.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(mut grab) = ws.grid.grab.take() else {
        return;
    };
    let lift = editor_lift(ws, grab.lift);
    let mut changed = false;
    match &mut grab.kind {
        DupKind::Nodes {
            copies,
            anchor,
            delta,
        } => {
            let y = copies.first().map_or(0.0, |c| c.origin[1]);
            if let Some(p) = ws.grid_plane_point(ptr, y) {
                let anchor = *anchor.get_or_insert([p[0], p[2]]);
                let step = if ptr.free() {
                    1.0
                } else {
                    f32::from(ws.snap_units.max(1))
                };
                let snap = |v: f32| (v / step).round() * step;
                *delta = [snap(p[0] - anchor[0]), snap(p[2] - anchor[1])];
            }
            place_nodes(ws, copies, [delta[0], lift, delta[1]]);
            changed = true;
        }
        DupKind::Mesh { id, .. } => {
            if let Some((_, position)) = super::mesh_tool::spot(ws, ptr) {
                let id = *id;
                if let Some(world) = ws.project.active_scene_mut().world.grid_mut() {
                    let lifted = [position[0], position[1] + grab.lift, position[2]];
                    changed = update_mesh(world, id, |m| m.position = lifted).unwrap_or(false);
                }
            }
        }
        DupKind::Cells {
            clip, min, nodes, ..
        } => {
            if let Some(ghost) = ws.grid.paste.as_mut() {
                ghost.clip = clip.lifted(grab.lift);
            }
            super::clipboard_tool::hover(ws, ptr);
            if let Some(at) = ws.grid.paste.as_ref().and_then(|g| g.at) {
                let sector =
                    ws.project
                        .active_scene()
                        .world
                        .grid()
                        .map_or(0, |w| w.scale.to_editor(SECTOR_UNITS)) as f32;
                let d = [
                    (at[0] - min[0]) as f32 * sector,
                    (at[1] - min[1]) as f32 * sector,
                ];
                place_nodes(ws, nodes, [d[0], lift, d[1]]);
                changed = !nodes.is_empty();
            }
        }
    }
    if changed {
        ws.mark_dirty();
    }
    ws.grid.grab = Some(grab);
}

/// The drop.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    hover(ws, ptr);
    let Some(grab) = ws.grid.grab.take() else {
        return;
    };
    match &grab.kind {
        DupKind::Cells { clip, room, .. } => {
            let Some(at) = ws.grid.paste.as_ref().and_then(|g| g.at) else {
                ws.grid.grab = Some(grab);
                return;
            };
            let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
                ws.grid.grab = Some(grab);
                return;
            };
            let outcome = clip.lifted(grab.lift).paste(world, *room, at);
            if let Some(refusal) = outcome.refused {
                ws.status = format!("Cannot place it there: {refusal:?}");
                ws.grid.grab = Some(grab);
                return;
            }
            ws.status = format!("Duplicated {} faces", outcome.faces.len());
            ws.grid
                .selection
                .set(outcome.faces.into_iter().map(GridElement::Face));
            ws.grid.paste = None;
            ws.after_grid_selection_changed();
        }
        DupKind::Nodes { copies, .. } => {
            ws.status = format!("Duplicated {}", count_label(copies.len(), "node"));
        }
        DupKind::Mesh { .. } => ws.status = "Duplicated mesh".to_string(),
    }
    ws.history.record(*grab.before);
    ws.grid_changed();
}

/// Escape (or anything that swaps the document): put everything back.
/// Returns whether a grab was in flight.
pub(crate) fn cancel(ws: &mut EditorWorkspace) -> bool {
    let Some(grab) = ws.grid.grab.take() else {
        return false;
    };
    ws.project = *grab.before;
    ws.grid.paste = None;
    if let DupKind::Mesh { saved, .. } = grab.kind {
        ws.grid.mesh_sel = saved;
    }
    let (primary, rest) = grab.saved_nodes;
    ws.selection.selected_node = primary;
    ws.selection.selected_nodes = rest.into_iter().collect();
    ws.mark_dirty();
    ws.status = "Duplicate cancelled".to_string();
    true
}

/// Up and Down: lift the copy `steps` clicks (a fine step with `fine`).
pub(super) fn lift(ws: &mut EditorWorkspace, steps: i32, fine: bool) {
    let Some(grab) = ws.grid.grab.as_mut() else {
        return;
    };
    grab.lift += steps * if fine { FINE_UNITS } else { CLICK_UNITS };
    let lift = grab.lift;
    ws.status = format!("Lift {lift:+} units");
    // Reapply at the last pointer position the ghost knew.
    let nodes = match &ws.grid.grab.as_ref().map(|g| &g.kind) {
        Some(DupKind::Nodes { copies, delta, .. }) => Some((copies.clone(), *delta)),
        _ => None,
    };
    if let Some((copies, delta)) = nodes {
        let y = editor_lift(ws, lift);
        place_nodes(ws, &copies, [delta[0], y, delta[1]]);
        ws.mark_dirty();
    }
    if let Some(DupKind::Mesh { id, .. }) = ws.grid.grab.as_ref().map(|g| g.kind.clone()) {
        let step = steps * if fine { FINE_UNITS } else { CLICK_UNITS };
        if let Some(world) = ws.project.active_scene_mut().world.grid_mut() {
            let _ = update_mesh(world, id, |m| m.position[1] += step);
            ws.mark_dirty();
        }
    }
    if let Some(DupKind::Cells {
        clip, min, nodes, ..
    }) = ws.grid.grab.as_ref().map(|g| g.kind.clone())
    {
        if let Some(ghost) = ws.grid.paste.as_mut() {
            ghost.clip = clip.lifted(lift);
            if let Some(at) = ghost.at {
                let sector =
                    ws.project
                        .active_scene()
                        .world
                        .grid()
                        .map_or(0, |w| w.scale.to_editor(SECTOR_UNITS)) as f32;
                let y = editor_lift(ws, lift);
                let d = [
                    (at[0] - min[0]) as f32 * sector,
                    (at[1] - min[1]) as f32 * sector,
                ];
                place_nodes(ws, &nodes, [d[0], y, d[1]]);
                ws.mark_dirty();
            }
        }
    }
}

impl EditorWorkspace {
    /// Escape or a document swap while a duplicate is in flight.
    pub(crate) fn cancel_duplicate(&mut self) -> bool {
        cancel(self)
    }
}
