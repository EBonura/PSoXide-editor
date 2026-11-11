//! Repeating the last action, offsetting the selection by a typed amount,
//! and the Move mode.
//!
//! Repeat is `R` (free in the grid keys; `.` frames, Tab toggles verb modes,
//! Cmd+D duplicates). It runs the last Edit action again on whatever is
//! selected now, with the same amount.
//!
//! Move is a handle at the middle of the selection: drag it over the plan or
//! the floor in whole sectors and the selection goes with everything that
//! stands on it. It is done with the copy and delete operations of the
//! clipboard behind [`move_cells`], the one function an SDK Move operation
//! will replace.

use super::spec::{self, Kind};
use super::*;

use psxed_project::grid::edit::{delete_faces, FaceTarget, GridClipboard, SurfaceKind};
use psxed_project::grid::SECTOR_UNITS;

/// The last action: its kind and amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::grid_tools) struct Repeat {
    kind: Kind,
    amount: i32,
}

impl Repeat {
    /// What a finished gesture leaves to repeat; corner gestures name one
    /// corner of one face, which a new selection does not have.
    pub fn of(kind: Kind, amount: i32) -> Option<Self> {
        match kind {
            Kind::Lift(_) | Kind::TiltCorner(_) => None,
            kind => Some(Self { kind, amount }),
        }
    }
}

/// `R`: do the last action again on the selection.
pub(super) fn repeat(ws: &mut EditorWorkspace) {
    let Some(last) = ws.grid.edit.last else {
        ws.status = "Nothing to repeat yet: do an Edit action first".to_string();
        return;
    };
    apply_to_selection(ws, last.kind, last.amount, "Repeated");
}

/// A typed number then Enter: raise or lower the selection by `units`.
pub(super) fn offset_selection(ws: &mut EditorWorkspace, units: i32) {
    let kind = if ws.project.active_scene().world.grid().is_some_and(|w| {
        ws.grid
            .selection
            .faces(w)
            .iter()
            .any(|f| WallHandleOf::is_wall(f) && !has_surface(ws, w))
    }) {
        Kind::Slide
    } else {
        Kind::Translate
    };
    apply_to_selection(ws, kind, units, "Offset");
    if ws.grid.edit.last.is_none() || units != 0 {
        ws.grid.edit.last = Repeat::of(kind, units);
    }
}

struct WallHandleOf;

impl WallHandleOf {
    fn is_wall(face: &FaceRef) -> bool {
        super::wall_handle::WallHandle::of(*face).is_some()
    }
}

fn has_surface(ws: &EditorWorkspace, world: &psxed_project::grid::GridWorld) -> bool {
    ws.grid
        .selection
        .faces(world)
        .iter()
        .any(|f| matches!(f.target, FaceTarget::Surface { .. }))
}

fn apply_to_selection(ws: &mut EditorWorkspace, kind: Kind, amount: i32, verb: &str) {
    let spec = match spec::from_selection(ws, kind) {
        Ok(spec) => spec,
        Err(reason) => {
            ws.status = reason;
            return;
        }
    };
    let before = ws.project.clone();
    let weld = ws.grid.edit.weld;
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let built = match spec::build(world, &spec, weld) {
        Ok(built) => built,
        Err(reason) => {
            ws.project = before;
            ws.status = reason;
            return;
        }
    };
    let out = built.plan.apply(world, amount);
    if let Some(reason) = out.refused {
        ws.project = before;
        ws.status = reason;
        return;
    }
    if !out.changed {
        ws.project = before;
        ws.status = format!("{verb}: nothing changed");
        return;
    }
    ws.history.record(before);
    ws.grid_changed();
    ws.status = match out.text {
        Some(text) => format!("{verb}: {text}"),
        None => format!(
            "{verb}: {} {} clicks",
            built.label,
            amount::clicks_text(amount)
        ),
    };
}

// ---- Move ----

/// A Move drag in flight.
#[derive(Clone, Debug)]
pub(in crate::grid_tools) struct MoveDrag {
    /// The document at the press.
    pub before: Box<ProjectDocument>,
    faces: Vec<FaceRef>,
    nodes: Vec<NodeId>,
    /// Where the pointer first met the floor, editor units `[x, z]`.
    anchor: [f32; 2],
    /// The height of the plane the pointer travels on, editor units.
    plane_y: f32,
    /// The sectors moved now, `[x, z]`.
    delta: [i32; 2],
    recorded: bool,
}

/// The surface faces of the selection, with nothing but surfaces and the
/// walls placed on them.
fn movable(ws: &EditorWorkspace) -> Vec<FaceRef> {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return Vec::new();
    };
    ws.grid.selection.faces(world)
}

/// Where the Move handle stands: the middle of the selection, on screen.
pub(super) fn handle_position(ws: &EditorWorkspace, view: GridView) -> Option<(Pos2, [f32; 3])> {
    let faces = movable(ws);
    if faces.is_empty() || ws.grid.tool != GridTool::Edit {
        return None;
    }
    let world = ws.project.active_scene().world.grid()?;
    // At least one floor or ceiling: a lone wall has the arrow, not this.
    if !faces
        .iter()
        .any(|f| matches!(f.target, FaceTarget::Surface { .. }))
    {
        return None;
    }
    let (centre, half) = ws.grid_selection_bounds()?;
    let _ = world;
    let anchor = [centre[0], centre[1] + half[1], centre[2]];
    match view {
        GridView::Top(transform) => {
            Some((transform.world_to_screen([anchor[0], anchor[2]]), anchor))
        }
        GridView::Persp(rect) => {
            let (screen, _) = view::project_point(ws.viewport_3d_camera(), rect, anchor)?;
            Some((screen, anchor))
        }
    }
}

/// Start a Move when the press is on the handle.
pub(super) fn press_move(ws: &mut EditorWorkspace, press: &GridPointer) -> bool {
    let Some((screen, anchor)) = handle_position(ws, press.view) else {
        return false;
    };
    if screen.distance(press.pos) > 12.0 {
        return false;
    }
    let faces = movable(ws);
    let nodes = dup_tool::nodes_standing_on(ws, &faces);
    ws.grid.edit.moving = Some(MoveDrag {
        before: Box::new(ws.project.clone()),
        faces,
        nodes,
        anchor: [anchor[0], anchor[2]],
        plane_y: anchor[1],
        delta: [0, 0],
        recorded: false,
    });
    ws.grid.gesture = Gesture::Edit;
    ws.grid.edit.hint = "Move: drag in whole sectors | Esc cancels".to_string();
    true
}

/// A drag step of a Move.
pub(super) fn drag_move(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    let Some(mut drag) = ws.grid.edit.moving.take() else {
        return false;
    };
    let Some(world) = ws.project.active_scene().world.grid() else {
        ws.grid.edit.moving = Some(drag);
        return true;
    };
    let sector = world.scale.sector_editor_units() as f32;
    let at = match ptr.view {
        GridView::Top(transform) => Some(transform.screen_to_world(ptr.pos)),
        GridView::Persp(rect) => ws
            .grid_ray(rect, ptr.pos)
            .and_then(|(o, d)| view::ray_plane_y(o, d, drag.plane_y))
            .map(|p| [p[0], p[2]]),
    };
    if let Some(at) = at {
        let delta = [
            ((at[0] - drag.anchor[0]) / sector).round() as i32,
            ((at[1] - drag.anchor[1]) / sector).round() as i32,
        ];
        if delta != drag.delta {
            drag.delta = delta;
            let mut project = (*drag.before).clone();
            match move_cells(&mut project, &drag.faces, &drag.nodes, delta) {
                Ok(()) => {
                    ws.project = project;
                    if !drag.recorded && delta != [0, 0] {
                        ws.history.record((*drag.before).clone());
                        drag.recorded = true;
                    }
                    ws.grid_changed();
                    ws.status = format!("Move {:+}, {:+} sectors", delta[0], delta[1]);
                }
                Err(reason) => {
                    ws.project = (*drag.before).clone();
                    ws.status = reason;
                }
            }
        }
    }
    ws.grid.edit.moving = Some(drag);
    true
}

/// The end of a Move.
pub(super) fn release_move(ws: &mut EditorWorkspace) -> bool {
    let Some(drag) = ws.grid.edit.moving.take() else {
        return false;
    };
    ws.grid.gesture = Gesture::Idle;
    ws.grid.edit.hint.clear();
    if drag.delta == [0, 0] || !drag.recorded {
        ws.project = *drag.before;
        ws.grid_changed();
        return true;
    }
    // The faces moved: the selection follows them.
    let faces: Vec<FaceRef> = drag
        .faces
        .iter()
        .filter_map(|f| moved_face(*f, drag.delta))
        .collect();
    ws.grid
        .selection
        .set(faces.into_iter().map(GridElement::Face));
    ws.after_grid_selection_changed();
    true
}

fn moved_face(face: FaceRef, delta: [i32; 2]) -> Option<FaceRef> {
    let cell = face.cell();
    let to = [cell[0] + delta[0], cell[1] + delta[1]];
    match face.target {
        FaceTarget::Surface {
            kind: SurfaceKind::Floor,
            span,
        } => Some(FaceRef::floor_in(face.room, to, span)),
        FaceTarget::Surface {
            kind: SurfaceKind::Ceiling,
            span,
        } => Some(FaceRef::ceiling_in(face.room, to, span)),
        FaceTarget::Wall { edge, index } => Some(FaceRef::wall(face.room, to, edge, index)),
        FaceTarget::Derived { .. } => None,
    }
}

/// Move `faces` and the `nodes` standing on them by `delta` whole sectors,
/// in place. Copy, delete and paste for now; one function so an SDK Move
/// replaces it alone.
pub(super) fn move_cells(
    project: &mut ProjectDocument,
    faces: &[FaceRef],
    nodes: &[NodeId],
    delta: [i32; 2],
) -> Result<(), String> {
    let Some(world) = project.active_scene_mut().world.grid_mut() else {
        return Err("No grid world".to_string());
    };
    let room = faces.first().ok_or("Nothing to move")?.room;
    let clip = GridClipboard::capture(world, faces).ok_or("Nothing to move")?;
    let min = [
        faces.iter().map(|f| f.cell()[0]).min().unwrap_or(0),
        faces.iter().map(|f| f.cell()[1]).min().unwrap_or(0),
    ];
    let scale = world.scale;
    delete_faces(world, faces);
    let outcome = clip.paste(world, room, [min[0] + delta[0], min[1] + delta[1]]);
    if let Some(refusal) = outcome.refused {
        return Err(format!("Cannot move there: {refusal:?}"));
    }
    let step = scale.to_editor(SECTOR_UNITS) as f32;
    let scene = project.active_scene_mut();
    for id in nodes {
        if let Some(node) = scene.node_mut(*id) {
            node.transform.translation[0] += delta[0] as f32 * step;
            node.transform.translation[2] += delta[1] as f32 * step;
        }
    }
    Ok(())
}
