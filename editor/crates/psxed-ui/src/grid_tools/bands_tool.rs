//! The Bands tool (E): cut a wall run into horizontal bands in the 3D view.
//!
//! Hover a wall and a horizontal rule previews at the pointer height,
//! snapped to a click (Alt for the fine step); click inserts a line there.
//! Near the bottom of the wall the line follows the bottom edge, near the top
//! the top edge, in between it is an absolute datum (the Inspector can
//! change the kind). Press on an existing line and drag to move it between
//! its neighbours; Delete removes the selected line; Shift+click selects the
//! band under the pointer so a palette click can paint it. The scope (this
//! run, or the whole room) is a toggle in the option row. Undo: one step per
//! insert, per drag, per removal.
//!
//! In the Top view a click selects the wall run only; the lines are edited
//! in the Inspector or in 3D.

use super::view::project_point;
use super::*;

use psxed_project::grid::edit::bands::{
    anchor_ends_of, anchor_for, band_at, edit_bands, effective_set, insert_index, line_ends,
    move_line, position_on_wall, run_faces, run_len, wall_spans, BandScope,
};
use psxed_project::grid::edit::{FaceRef, FaceTarget};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

/// Pixels within which the pointer grabs an existing line.
const GRAB_PIXELS: f32 = 7.0;

/// A line being dragged.
#[derive(Clone, Debug)]
pub(crate) struct BandDrag {
    pub face: FaceRef,
    pub line: usize,
    /// The document before the first change, recorded for undo on it.
    pub before: Option<Box<ProjectDocument>>,
}

/// A wall under the pointer, with where on it.
struct WallPoint {
    face: FaceRef,
    /// Position along the wall, grid units.
    u: i32,
    /// Room-relative height, grid units.
    y: i32,
    /// The hit point, editor units.
    point: [f32; 3],
}

/// The wall (derived or authored) under a 3D pointer and where on it.
fn wall_point(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<WallPoint> {
    let GridView::Persp(rect) = ptr.view else {
        return None;
    };
    let hit = ws.pick_face_3d(rect, ptr.pos)?;
    if !matches!(
        hit.face.target,
        FaceTarget::Derived { .. } | FaceTarget::Wall { .. }
    ) {
        return None;
    }
    let world = ws.project.active_scene().world.grid()?;
    let room = world.rooms.get(hit.face.room)?;
    let scale = world.scale;
    let gx = scale.to_grid(hit.point[0].round() as i32);
    let gz = scale.to_grid(hit.point[2].round() as i32);
    let y = scale.to_grid(hit.point[1].round() as i32) - room.y_offset;
    let u = position_on_wall(hit.face, [gx, gz])?;
    Some(WallPoint {
        face: hit.face,
        u,
        y,
        point: hit.point,
    })
}

/// The line of `face`'s set the pointer is on, if any.
fn grabbed_line(ws: &EditorWorkspace, ptr: &GridPointer, at: &WallPoint) -> Option<usize> {
    let GridView::Persp(rect) = ptr.view else {
        return None;
    };
    let world = ws.project.active_scene().world.grid()?;
    let room = world.rooms.get(at.face.room)?;
    let (span, _) = wall_spans(world, at.face)?;
    let ends = line_ends(world, at.face);
    let camera = ws.viewport_3d_camera();
    let here = project_point(camera, rect, at.point)?.0;
    let t = f64::from(at.u - span.u[0]) / f64::from((span.u[1] - span.u[0]).max(1));
    let mut best: Option<(f32, usize)> = None;
    for (index, e) in ends.iter().enumerate() {
        let h = (f64::from(e[0]) + f64::from(e[1] - e[0]) * t).round() as i32;
        let y = world.scale.to_editor(room.y_offset + h) as f32;
        let Some((p, _)) = project_point(camera, rect, [at.point[0], y, at.point[2]]) else {
            continue;
        };
        let d = (p.y - here.y).abs();
        if d <= GRAB_PIXELS && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, index));
        }
    }
    best.map(|(_, i)| i)
}

fn snapped(y: i32, ptr: &GridPointer) -> i32 {
    snap_to(y, if ptr.alt() { FINE_UNITS } else { CLICK_UNITS })
}

/// Pointer moved without a button.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = GridHover::None;
    match ptr.view {
        GridView::Persp(_) => {
            let Some(at) = wall_point(ws, ptr) else {
                return;
            };
            if let Some(line) = grabbed_line(ws, ptr, &at) {
                ws.grid.hover = GridHover::Band {
                    face: at.face,
                    anchor: None,
                    y: at.y,
                    line: Some(line),
                };
                return;
            }
            let Some(world) = ws.project.active_scene().world.grid() else {
                return;
            };
            let y = snapped(at.y, ptr);
            let anchor = anchor_for(world, at.face, at.u, y);
            if let Some(a) = anchor {
                ws.status = format!(
                    "Click to cut at {} {}; drag a line to move it",
                    a.label(),
                    describe_clicks(a.value())
                );
            }
            ws.grid.hover = GridHover::Band {
                face: at.face,
                anchor,
                y,
                line: None,
            };
        }
        GridView::Top(transform) => {
            if let Some(face) = wall_at_top(ws, transform, ptr.pos) {
                ws.grid.hover =
                    GridHover::Elements(vec![psxed_project::grid::edit::GridElement::Face(face)]);
            }
        }
    }
}

/// The wall edge under a Top view pointer: a derived wall first, else the
/// first authored one.
fn wall_at_top(ws: &EditorWorkspace, transform: ViewportTransform, pos: Pos2) -> Option<FaceRef> {
    let (room, cell, edge) = ws.top_wall_at(transform, pos, 6.0)?;
    let world = ws.project.active_scene().world.grid()?;
    let r = world.rooms.get(room)?;
    if let Some(wall) = psxed_project::grid::derive::edge_walls(r, cell, edge)
        .into_iter()
        .find(|w| !w.hidden)
    {
        return Some(FaceRef::derived(room, cell, edge, wall.kind));
    }
    let [x, z] = r.local(cell)?;
    (!r.sector(x, z)?.walls(edge).is_empty()).then(|| FaceRef::wall(room, cell, edge, 0))
}

/// Primary button down: start dragging a line when the pointer is on one.
pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if !matches!(ws.grid.gesture, Gesture::Idle) {
        return;
    }
    let at = ptr.at_press();
    let Some(wall) = wall_point(ws, &at) else {
        return;
    };
    if let Some(line) = grabbed_line(ws, &at, &wall) {
        select_face(ws, wall.face);
        ws.grid.band_line = Some((wall.face, line));
        ws.grid.gesture = Gesture::Band(BandDrag {
            face: wall.face,
            line,
            before: None,
        });
        ws.status = "Drag the line up or down; Esc cancels".to_string();
    }
}

/// Pointer moved with the button down.
pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Band(drag) = ws.grid.gesture.clone() else {
        return;
    };
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    // Where on the dragged wall the pointer is, even when the ray leaves it
    // for a neighbour: intersect the wall's own vertical plane.
    let Some((u, y)) = pointer_on_wall(ws, world, drag.face, ptr) else {
        return;
    };
    let y = snapped(y, ptr);
    let before = ws.project.clone();
    let scope = ws.grid.band_scope;
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    if move_line(world, drag.face, drag.line, u, y, scope) {
        if let Gesture::Band(g) = &mut ws.grid.gesture {
            if g.before.is_none() {
                g.before = Some(Box::new(before));
            }
        }
        ws.grid_changed();
    }
}

/// Position and height on `face` under a 3D pointer, from the ray against
/// the wall's vertical plane.
fn pointer_on_wall(
    ws: &EditorWorkspace,
    world: &psxed_project::grid::GridWorld,
    face: FaceRef,
    ptr: &GridPointer,
) -> Option<(i32, i32)> {
    let GridView::Persp(rect) = ptr.view else {
        return None;
    };
    let room = world.rooms.get(face.room)?;
    let (origin, dir) = ws.grid_ray(rect, ptr.pos)?;
    let (FaceTarget::Derived { edge, .. } | FaceTarget::Wall { edge, .. }) = face.target else {
        return None;
    };
    let (ea, eb) = edge.endpoints();
    let at = |e: [i32; 2]| {
        [
            world
                .scale
                .to_editor((face.cell()[0] + e[0]) * psxed_project::grid::SECTOR_UNITS)
                as f32,
            world
                .scale
                .to_editor((face.cell()[1] + e[1]) * psxed_project::grid::SECTOR_UNITS)
                as f32,
        ]
    };
    let (a, b) = (at(ea), at(eb));
    // The wall's plane contains the edge and the vertical: its normal is
    // the edge direction turned a quarter in plan.
    let along = [b[0] - a[0], b[1] - a[1]];
    let normal = [-along[1], along[0]];
    let denom = normal[0] * dir[0] + normal[1] * dir[2];
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (normal[0] * (a[0] - origin[0]) + normal[1] * (a[1] - origin[2])) / denom;
    if t <= 0.0 {
        return None;
    }
    let p = [
        origin[0] + dir[0] * t,
        origin[1] + dir[1] * t,
        origin[2] + dir[2] * t,
    ];
    let scale = world.scale;
    let gx = scale.to_grid(p[0].round() as i32);
    let gz = scale.to_grid(p[2].round() as i32);
    let u = position_on_wall(face, [gx, gz])?;
    let y = scale.to_grid(p[1].round() as i32) - room.y_offset;
    Some((u, y))
}

/// Primary button up.
pub(super) fn release(ws: &mut EditorWorkspace) {
    if let Gesture::Band(drag) = std::mem::take(&mut ws.grid.gesture) {
        if let Some(before) = drag.before {
            ws.history.record(*before);
            ws.status = "Moved the line".to_string();
        }
    }
}

/// Escape: put a dragged line back.
pub(super) fn cancel(ws: &mut EditorWorkspace) -> bool {
    match std::mem::take(&mut ws.grid.gesture) {
        Gesture::Band(drag) => {
            if let Some(before) = drag.before {
                ws.project = *before;
                ws.grid_changed();
            }
            true
        }
        other => {
            ws.grid.gesture = other;
            false
        }
    }
}

fn select_face(ws: &mut EditorWorkspace, face: FaceRef) {
    ws.grid
        .selection
        .set(vec![psxed_project::grid::edit::GridElement::Face(face)]);
    ws.after_grid_selection_changed();
}

/// A click: pick a line, pick a band (Shift), or cut the wall.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    match ptr.view {
        GridView::Top(transform) => match wall_at_top(ws, transform, ptr.pos) {
            Some(face) => {
                select_face(ws, face);
                ws.grid.band_line = None;
                let n = ws
                    .project
                    .active_scene()
                    .world
                    .grid()
                    .map_or(0, |w| run_len(w, face));
                ws.status = format!("Wall run of {n} segments selected; cut it in the 3D view");
            }
            None => {
                ws.grid.selection.clear();
                ws.grid.band_line = None;
            }
        },
        GridView::Persp(_) => {
            let Some(at) = wall_point(ws, ptr) else {
                ws.grid.band_line = None;
                return;
            };
            if let Some(line) = grabbed_line(ws, ptr, &at) {
                select_face(ws, at.face);
                ws.grid.band_line = Some((at.face, line));
                ws.status = format!("Line {} selected; drag it or press Delete", line + 1);
                return;
            }
            if ptr.shift() {
                let band = ws
                    .project
                    .active_scene()
                    .world
                    .grid()
                    .and_then(|w| band_at(w, at.face, at.u, at.y));
                select_face(ws, at.face);
                ws.grid.band_sel = band.map(|b| (at.face, b));
                ws.status = match band {
                    Some(b) => format!("Band {} selected; click a material in the palette", b + 1),
                    None => "No band there".to_string(),
                };
                return;
            }
            insert(ws, &at, ptr);
        }
    }
}

/// Cut the wall at the pointer.
fn insert(ws: &mut EditorWorkspace, at: &WallPoint, ptr: &GridPointer) {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let y = snapped(at.y, ptr);
    let Some(anchor) = anchor_for(world, at.face, at.u, y) else {
        return;
    };
    let index = insert_index(world, at.face, at.u, y);
    let scope = ws.grid.band_scope;
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let added = edit_bands(world, at.face, scope, |set| set.insert_line(index, anchor));
    if !added {
        ws.status = "A set holds at most six lines".to_string();
        return;
    }
    let count = run_len(world, at.face);
    ws.history.record(before);
    select_face(ws, at.face);
    ws.grid.band_line = Some((at.face, index));
    ws.status = format!(
        "Cut the wall at {} {} ({}); applies to {} wall segments",
        anchor.label(),
        describe_clicks(anchor.value()),
        match scope {
            BandScope::Run => "this run",
            BandScope::Room => "the whole room",
        },
        count
    );
    ws.grid_changed();
}

fn describe_clicks(units: i32) -> String {
    let clicks = units as f32 / CLICK_UNITS as f32;
    format!("{clicks:+.2} clicks")
}

/// Delete: remove the selected line. Returns whether one was removed.
pub(super) fn delete_line(ws: &mut EditorWorkspace) -> bool {
    let Some((face, line)) = ws.grid.band_line else {
        return false;
    };
    let scope = ws.grid.band_scope;
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    if !edit_bands(world, face, scope, |set| set.remove_line(line)) {
        return false;
    }
    ws.history.record(before);
    ws.grid.band_line = None;
    ws.status = "Removed the line; the bands above and below joined".to_string();
    ws.grid_changed();
    true
}

/// The band under a 3D pointer on a banded wall, for the Paint tool.
pub(super) fn band_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(FaceRef, usize)> {
    let at = wall_point(ws, ptr)?;
    let world = ws.project.active_scene().world.grid()?;
    effective_set(world, at.face)?;
    let band = band_at(world, at.face, at.u, at.y)?;
    Some((at.face, band))
}

// ---- drawing ----

impl EditorWorkspace {
    /// The wall whose run the Bands tool shows: the hovered one, else the
    /// selected one.
    fn band_target(&self) -> Option<FaceRef> {
        if self.grid.tool != GridTool::Bands {
            return None;
        }
        if let GridHover::Band { face, .. } = &self.grid.hover {
            return Some(*face);
        }
        let world = self.project.active_scene().world.grid()?;
        self.grid.selection.faces(world).into_iter().find(|f| {
            matches!(
                f.target,
                FaceTarget::Derived { .. } | FaceTarget::Wall { .. }
            )
        })
    }

    /// The run's outline, its cut lines and the rule a click would add, over
    /// the 3D view.
    pub(crate) fn draw_band_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(face) = self.band_target() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let segment = |a: [f32; 3], b: [f32; 3], color: Color32, width: f32| {
            if let Some([pa, pb]) = view::project_segment(camera, rect, a, b) {
                painter.line_segment([pa, pb], Stroke::new(width, color));
            }
        };
        let hover_anchor = match &self.grid.hover {
            GridHover::Band { anchor, .. } => *anchor,
            _ => None,
        };
        let hover_line = match &self.grid.hover {
            GridHover::Band { line, .. } => *line,
            _ => None,
        };
        let selected_line = self
            .grid
            .band_line
            .filter(|(f, _)| *f == face)
            .map(|(_, l)| l);
        for member in run_faces(world, face) {
            let Some(room) = world.rooms.get(member.room) else {
                continue;
            };
            let (FaceTarget::Derived { edge, .. } | FaceTarget::Wall { edge, .. }) = member.target
            else {
                continue;
            };
            let (ea, eb) = edge.endpoints();
            let ground =
                |e: [i32; 2], y: i32| {
                    [
                        world.scale.to_editor(
                            (member.cell()[0] + e[0]) * psxed_project::grid::SECTOR_UNITS,
                        ) as f32,
                        view::editor_y(world, room.y_offset, y),
                        world.scale.to_editor(
                            (member.cell()[1] + e[1]) * psxed_project::grid::SECTOR_UNITS,
                        ) as f32,
                    ]
                };
            // The run's outline: the wall's own bottom and top.
            if let Some((span, _)) = wall_spans(world, member) {
                let (bottom, top) = (span.bottom, span.top);
                segment(
                    ground(ea, bottom[0]),
                    ground(eb, bottom[1]),
                    super::overlay::HOVER,
                    2.0,
                );
                segment(
                    ground(ea, top[0]),
                    ground(eb, top[1]),
                    super::overlay::HOVER,
                    2.0,
                );
            }
            for (index, ends) in line_ends(world, member).iter().enumerate() {
                let (color, width) = if selected_line == Some(index) && member.cell() == face.cell()
                {
                    (Color32::WHITE, 4.0)
                } else if hover_line == Some(index) {
                    (super::overlay::HOVER, 3.5)
                } else {
                    (super::overlay::SELECT, 2.5)
                };
                segment(ground(ea, ends[0]), ground(eb, ends[1]), color, width);
            }
            if let Some(anchor) = hover_anchor {
                if let Some(h) = anchor_ends_of(world, member, anchor) {
                    segment(
                        ground(ea, h[0]),
                        ground(eb, h[1]),
                        super::overlay::CREATE,
                        3.0,
                    );
                }
            }
        }
    }
}

/// Anchor kinds, for the option row.
pub(super) fn scope_label(scope: BandScope) -> &'static str {
    match scope {
        BandScope::Run => "This run",
        BandScope::Room => "Whole room",
    }
}
