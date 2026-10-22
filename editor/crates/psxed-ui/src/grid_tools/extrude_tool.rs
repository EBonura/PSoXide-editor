//! The Extrude tool: floors, ceilings and wall pieces with one command.
//!
//! What moves follows what is pressed. In the 3D view a press on a floor, a
//! ceiling or a wall piece that is not picked yet picks it (replacing the
//! old pick) and starts the drag in the same press; a press on something
//! already picked drags the whole pick; a click only picks. Shift adds more
//! of the same kind and a pick of mixed kinds is refused in words.
//!
//! A box picks many at once. Shift-drag, or a plain drag that starts on
//! nothing, draws a screen rectangle; the faces inside it light up as it
//! grows and the release picks them. The box picks ONE kind, the kind of the
//! face under the drag start (floors when it starts on nothing), and never a
//! face the camera does not draw (the Select tool's box code). Shift+Alt (or
//! Ctrl/Cmd) adds the box to the pick; a box of another kind is then refused.
//! The release only picks: press a picked piece and drag, or Out and In, to
//! extrude the set.
//!
//! * **Floor** is a region of sectors, dragged up or down.
//! * **Ceiling** is the same flow moving the ceilings of the region.
//! * **Wall** works on pieces, not sectors. The piece under the pointer (a
//!   wall face, or one band of a banded wall) lights up; a drag along its
//!   normal pushes it out (away from the room, an alcove or, for a band, a
//!   niche) or in (the room fills), a sector per step, live. A box over the
//!   wall picks several, and the Out and In buttons, or `=` and `-`, push
//!   the picked pieces a step at a time.
//!
//! The plan shows no walls: its press on a sector is a floor pick, a
//! rectangle of sectors swept by a drag (Shift adds), then dragged by a
//! second press.

use super::view::editor_y;
use super::*;

use psxed_project::grid::derive;
use psxed_project::grid::edit::verbs::extrude::{
    pieces_of, ExtrudeDrag, ExtrudeRefusal, Extruded, WallPiece,
};
use psxed_project::grid::edit::{FaceRef, FaceTarget, GridElement, SurfaceKind};
use psxed_project::grid::{GridWorld, SECTOR_UNITS};

use super::pick::FaceHit;
use super::verb_state::{VerbDrag, VerbGesture};
use psxed_project::grid::edit::Region;

/// Most sectors one gesture pushes a wall.
pub(super) const MAX_STEPS: i32 = 16;

/// What the Extrude tool moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtrudeSurface {
    /// Floors: Raise and Lower.
    Floor,
    /// Ceilings.
    Ceiling,
    /// Wall pieces, out and in.
    Wall,
}

/// Said when a pick of one kind meets another kind.
pub(super) const MIXED: &str = "Pick only floors, only ceilings or only walls: Shift adds more of the same kind, a plain press starts a new pick";

/// A wall extrusion being dragged: the amount is steps along the normal.
#[derive(Clone, Debug)]
pub(crate) struct WallLive {
    pub drag: ExtrudeDrag,
    /// Where the pointer went down.
    pub press: Pos2,
    /// Screen travel of one step out.
    pub axis: Vec2,
}

impl WallLive {
    /// The steps the pointer is at: its travel along the normal.
    pub fn steps_at(&self, ptr: &GridPointer) -> i32 {
        let travel = ptr.pos - self.press;
        let steps = (travel.dot(self.axis) / self.axis.length_sq().max(1.0)).round() as i32;
        steps.clamp(-MAX_STEPS, MAX_STEPS)
    }
}

/// The wall piece a hit names, if it is a wall.
fn piece_of_hit(hit: &FaceHit) -> Option<GridElement> {
    match hit.face.target {
        FaceTarget::Derived { .. } | FaceTarget::Wall { .. } => Some(match hit.band {
            Some(band) => GridElement::Band {
                face: hit.face,
                band,
            },
            None => GridElement::Face(hit.face),
        }),
        FaceTarget::Surface { .. } => None,
    }
}

/// What a hit extrudes as: a wall piece, a ceiling or a floor.
fn kind_of(hit: &FaceHit) -> Option<ExtrudeSurface> {
    match hit.face.target {
        FaceTarget::Derived { .. } | FaceTarget::Wall { .. } => Some(ExtrudeSurface::Wall),
        FaceTarget::Surface {
            kind: SurfaceKind::Ceiling,
            ..
        } => Some(ExtrudeSurface::Ceiling),
        FaceTarget::Surface {
            kind: SurfaceKind::Floor,
            ..
        } => Some(ExtrudeSurface::Floor),
    }
}

fn hit_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<FaceHit> {
    match ptr.view {
        GridView::Persp(rect) => ws.pick_face_3d(rect, ptr.pos),
        GridView::Top(_) => None,
    }
}

/// Hover: the piece under the pointer lights up.
pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if !matches!(ws.grid.gesture, Gesture::Idle) {
        return;
    }
    if let Some(element) = hit_under(ws, ptr).as_ref().and_then(piece_of_hit) {
        ws.grid.hover = GridHover::Elements(vec![element]);
    }
}

/// Whether anything is picked, whatever its kind.
fn picked(ws: &EditorWorkspace) -> bool {
    match ws.grid.verb.surface {
        ExtrudeSurface::Wall => !ws.grid.selection.is_empty(),
        _ => ws.grid.verb.region.is_some(),
    }
}

/// Take `kind` as what the pick is about. A pick of another kind goes, unless
/// `add` (Shift) wants to join them: that is refused in words and `false` is
/// returned, with nothing changed.
fn adopt(ws: &mut EditorWorkspace, kind: ExtrudeSurface, add: bool) -> bool {
    let was = ws.grid.verb.surface;
    if kind == was {
        return true;
    }
    if add && picked(ws) {
        ws.status = MIXED.to_string();
        return false;
    }
    ws.grid.verb.surface = kind;
    ws.grid.verb.region = None;
    ws.grid.verb.path.clear();
    if !ws.grid.selection.is_empty() {
        ws.grid.selection.clear();
        ws.after_grid_selection_changed();
    }
    true
}

/// The plan: a press or click on a sector is a floor pick, except a press
/// inside a region that is already picked (which carries on being dragged,
/// ceilings included) and a Shift that joins a ceiling region. Returns
/// whether the pick was refused.
fn plan_adopt(ws: &mut EditorWorkspace, ptr: &GridPointer, press: bool) -> bool {
    let add = ptr.shift() || ptr.command();
    if press && !add {
        let inside =
            super::verb_tool::cell_under(ws, &ptr.at_press()).is_some_and(|(room, cell)| {
                ws.grid
                    .verb
                    .region
                    .as_ref()
                    .is_some_and(|r| r.room == room && r.contains(cell))
            });
        if inside {
            return false;
        }
    }
    let kind = if add && ws.grid.verb.surface == ExtrudeSurface::Ceiling {
        ExtrudeSurface::Ceiling
    } else {
        ExtrudeSurface::Floor
    };
    !adopt(ws, kind, add)
}

/// Press in the view (a drag begins). Returns whether the Extrude flow took
/// it; `false` leaves the press to the region flow (the plan's rectangle).
pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    if ptr.is_top() {
        return plan_adopt(ws, ptr, true);
    }
    let at = ptr.at_press();
    let hit = hit_under(ws, &at);
    let kind = hit.as_ref().and_then(kind_of);
    // A box: Shift (or Ctrl/Cmd) held, or a press on nothing.
    if ptr.shift() || ptr.command() || kind.is_none() {
        begin_box(ws, ptr, kind.unwrap_or(ExtrudeSurface::Floor));
        return true;
    }
    let (Some(kind), Some(hit)) = (kind, hit) else {
        return true;
    };
    if !adopt(ws, kind, false) {
        return true;
    }
    match piece_of_hit(&hit) {
        Some(piece) => {
            if !ws.grid.selection.contains(&piece) {
                pick(ws, piece, false);
            }
            begin_drag(ws, &at, hit.point);
        }
        None => press_surface(ws, ptr),
    }
    true
}

/// A press on a floor or a ceiling: what is under it is picked unless it is
/// already, and dragged.
fn press_surface(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some((room, cell)) = super::verb_tool::cell_under(ws, &ptr.at_press()) else {
        return;
    };
    let inside = ws
        .grid
        .verb
        .region
        .as_ref()
        .is_some_and(|r| r.room == room && r.contains(cell));
    if !inside {
        let mut region = Region::new_at(room, ws.grid.cell_size.level());
        region.extend(&Region::brush(room, cell, 0));
        ws.grid.verb.region = Some(region);
        ws.grid.verb.stairs_refusal = None;
    }
    super::verb_tool::begin_amount(ws, ptr, room, cell);
}

/// Whether the box being drawn adds to the pick (Shift+Alt, or Ctrl/Cmd).
fn box_adds(ptr: &GridPointer) -> bool {
    ptr.free() || ptr.command()
}

/// A box begins at the press. Adding a box of another kind than the pick is
/// refused before it is drawn.
fn begin_box(ws: &mut EditorWorkspace, ptr: &GridPointer, kind: ExtrudeSurface) {
    let add = box_adds(ptr);
    if add && picked(ws) && kind != ws.grid.verb.surface {
        ws.status = MIXED.to_string();
        return;
    }
    ws.grid.verb.box_kind = kind;
    ws.grid.gesture = Gesture::Marquee {
        start: ptr.at_press().pos,
        current: ptr.pos,
        additive: add,
    };
    ws.status = format!(
        "Drag a box over the {}: release picks them{}",
        kind_name(kind),
        if add { ", added to the pick" } else { "" }
    );
}

/// The plural a box of `kind` picks.
pub(super) fn kind_name(kind: ExtrudeSurface) -> &'static str {
    match kind {
        ExtrudeSurface::Floor => "floors",
        ExtrudeSurface::Ceiling => "ceilings",
        ExtrudeSurface::Wall => "wall pieces",
    }
}

/// The elements of the box's kind inside `rect`, the faces the camera draws
/// only (the Select tool's box code does the culling).
fn box_found(
    ws: &EditorWorkspace,
    ptr: &GridPointer,
    rect: Rect,
    kind: ExtrudeSurface,
) -> Vec<GridElement> {
    super::select_tool::marquee_elements(ws, ptr, rect)
        .into_iter()
        .filter(|e| match (e, kind) {
            (GridElement::Band { .. }, ExtrudeSurface::Wall) => true,
            (GridElement::Face(face), _) => kind_of_face(face.target) == Some(kind),
            _ => false,
        })
        .collect()
}

fn kind_of_face(target: FaceTarget) -> Option<ExtrudeSurface> {
    Some(match target {
        FaceTarget::Derived { .. } | FaceTarget::Wall { .. } => ExtrudeSurface::Wall,
        FaceTarget::Surface {
            kind: SurfaceKind::Ceiling,
            ..
        } => ExtrudeSurface::Ceiling,
        FaceTarget::Surface {
            kind: SurfaceKind::Floor,
            ..
        } => ExtrudeSurface::Floor,
    })
}

/// The box grew: what it would pick lights up.
pub(super) fn box_drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Marquee { start, current, .. } = &mut ws.grid.gesture else {
        return;
    };
    *current = ptr.pos;
    let rect = Rect::from_two_pos(*start, *current);
    let found = box_found(ws, ptr, rect, ws.grid.verb.box_kind);
    ws.grid.hover = if found.is_empty() {
        GridHover::None
    } else {
        GridHover::Elements(found)
    };
}

/// Esc while a box is being drawn: the box goes, the pick stays.
pub(super) fn box_cancel(ws: &mut EditorWorkspace) {
    ws.grid.hover = GridHover::None;
    ws.status = "Cancelled".to_string();
}

fn pick(ws: &mut EditorWorkspace, piece: GridElement, add: bool) {
    if add {
        ws.grid.selection.toggle(piece);
    } else {
        ws.grid.selection.set([piece]);
    }
    ws.after_grid_selection_changed();
    ws.status = pick_status(ws);
}

/// What the status line says about the wall pieces picked: how many, and
/// either the next step or why they cannot be extruded.
fn pick_status(ws: &EditorWorkspace) -> String {
    let n = ws.grid.selection.len();
    let picked = format!("{n} wall piece{} picked", if n == 1 { "" } else { "s" });
    match wall_refusal(ws) {
        Some(why) => format!("{picked}. {why}"),
        None => format!("{picked}: drag along the normal, or press Out and In (= and -)"),
    }
}

/// A click (no drag) in the view. Returns whether the Extrude flow took it;
/// a click on a floor or ceiling is the region flow's, after the pick has
/// followed what was clicked.
pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) -> bool {
    if ptr.is_top() {
        return plan_adopt(ws, ptr, false);
    }
    let add = ptr.shift() || ptr.command();
    let hit = hit_under(ws, ptr);
    let Some(kind) = hit.as_ref().and_then(kind_of) else {
        if ws.grid.verb.surface == ExtrudeSurface::Wall {
            if !add {
                ws.grid.selection.clear();
                ws.after_grid_selection_changed();
                ws.status = "No wall piece picked".to_string();
            }
            return true;
        }
        return false;
    };
    if !adopt(ws, kind, add) {
        return true;
    }
    match hit.as_ref().and_then(piece_of_hit) {
        Some(piece) => {
            pick(ws, piece, add);
            true
        }
        None => false,
    }
}

/// Why the picked wall pieces cannot be extruded, if they cannot.
pub(crate) fn wall_refusal(ws: &EditorWorkspace) -> Option<String> {
    if ws.grid.selection.is_empty() {
        return None;
    }
    let world = ws.project.active_scene().world.grid()?;
    let pieces = match pieces_of(world, ws.grid.selection.items()) {
        Ok(pieces) => pieces,
        Err(refusal) => return Some(refusal.describe()),
    };
    if let Err(refusal) = ExtrudeDrag::begin(world, pieces) {
        return Some(refusal.describe());
    }
    (ws.grid.cell_size.level() != 2)
        .then(|| "Walls extrude whole sectors: set the cell size to Sector".to_string())
}

/// The picked wall pieces, or the status line that says why not.
fn picked_pieces(ws: &mut EditorWorkspace) -> Option<(Vec<WallPiece>, usize)> {
    let world = ws.project.active_scene().world.grid()?;
    match pieces_of(world, ws.grid.selection.items()) {
        Ok(pieces) => {
            let room = pieces[0].room;
            Some((pieces, room))
        }
        Err(refusal) => {
            ws.status = refusal.describe();
            None
        }
    }
}

/// Screen travel of one step out, at the point of the wall that was
/// pressed (a step is a sector along the piece's normal).
fn axis_of(ws: &EditorWorkspace, rect: Rect, piece: &WallPiece, point: [f32; 3]) -> Vec2 {
    let fallback = Vec2::new(0.0, -48.0);
    let Some(world) = ws.project.active_scene().world.grid() else {
        return fallback;
    };
    let sector = world.scale.to_editor(SECTOR_UNITS) as f32;
    let out = piece.outward();
    let beyond = [
        point[0] + out[0] as f32 * sector,
        point[1],
        point[2] + out[1] as f32 * sector,
    ];
    let camera = ws.viewport_3d_camera();
    let (Some((s0, _)), Some((s1, _))) = (
        super::view::project_point(camera, rect, point),
        super::view::project_point(camera, rect, beyond),
    ) else {
        return fallback;
    };
    let axis = s1 - s0;
    if axis.length() < 8.0 {
        fallback
    } else {
        axis
    }
}

fn begin_drag(ws: &mut EditorWorkspace, at: &GridPointer, point: [f32; 3]) {
    let GridView::Persp(rect) = at.view else {
        return;
    };
    if ws.grid.cell_size.level() != 2 {
        ws.status = "Walls extrude whole sectors: set the cell size to Sector".to_string();
        return;
    }
    let Some((pieces, room)) = picked_pieces(ws) else {
        return;
    };
    let axis = axis_of(ws, rect, &pieces[0], point);
    let drag = {
        let Some(world) = ws.project.active_scene().world.grid() else {
            return;
        };
        match ExtrudeDrag::begin(world, pieces) {
            Ok(drag) => drag,
            Err(refusal) => {
                ws.status = refusal.describe();
                return;
            }
        }
    };
    ws.grid.gesture = Gesture::Verb(Box::new(VerbGesture {
        drag: VerbDrag::Wall(Box::new(WallLive {
            drag,
            press: at.pos,
            axis,
        })),
        press_y: at.pos.y,
        pixels_per_unit: 1.0,
        amount: 0,
        before: Some(Box::new(ws.project.clone())),
        max_rise: None,
        focus: psxed_project::grid::edit::focus::Focus::none(room),
    }));
    ws.status = "Drag along the wall's normal: out makes an alcove, in fills".to_string();
}

/// The box ended: pick what is inside it. A plain box replaces the pick, an
/// adding box (Shift+Alt) joins it.
pub(super) fn finish_marquee(
    ws: &mut EditorWorkspace,
    ptr: &GridPointer,
    start: Pos2,
    end: Pos2,
    additive: bool,
) {
    ws.grid.hover = GridHover::None;
    let kind = ws.grid.verb.box_kind;
    let found = box_found(ws, ptr, Rect::from_two_pos(start, end), kind);
    if kind != ExtrudeSurface::Wall && ws.grid.cell_size.level() != 2 {
        ws.status = format!(
            "A box picks whole sectors: set the cell size to Sector to box {}",
            kind_name(kind)
        );
        return;
    }
    if additive {
        if !adopt(ws, kind, true) {
            return;
        }
    } else {
        clear_pick(ws, kind);
    }
    match kind {
        ExtrudeSurface::Wall => {
            ws.grid.selection.extend(found);
            ws.after_grid_selection_changed();
            ws.status = if ws.grid.selection.is_empty() {
                "No wall pieces in the box".to_string()
            } else {
                pick_status(ws)
            };
        }
        _ => pick_surfaces(ws, &found, kind, additive),
    }
}

/// A new pick of `kind` starts: whatever was picked goes.
fn clear_pick(ws: &mut EditorWorkspace, kind: ExtrudeSurface) {
    ws.grid.verb.surface = kind;
    ws.grid.verb.region = None;
    ws.grid.verb.path.clear();
    ws.grid.selection.clear();
}

/// Floors or ceilings found by a box become the region. A region lives in
/// one room: the pick's own when adding, else the room with most faces.
fn pick_surfaces(
    ws: &mut EditorWorkspace,
    found: &[GridElement],
    kind: ExtrudeSurface,
    additive: bool,
) {
    let mut per_room: std::collections::BTreeMap<usize, Vec<[i32; 2]>> = Default::default();
    for e in found {
        if let GridElement::Face(face) = e {
            per_room.entry(face.room).or_default().push(face.cell());
        }
    }
    let kept = ws.grid.verb.region.clone().filter(|_| additive);
    let room = kept.as_ref().map(|r| r.room).or_else(|| {
        per_room
            .iter()
            .max_by_key(|(room, cells)| (cells.len(), std::cmp::Reverse(**room)))
            .map(|(room, _)| *room)
    });
    let Some(room) = room else {
        ws.status = format!("No {} in the box", kind_name(kind));
        return;
    };
    let mut region = kept.unwrap_or_else(|| Region::new(room));
    let others: usize = per_room
        .iter()
        .filter(|(r, _)| **r != room)
        .map(|(_, cells)| cells.len())
        .sum();
    region.extend(&Region::from_cells(
        room,
        per_room.remove(&room).unwrap_or_default(),
    ));
    ws.grid.verb.stairs_refusal = None;
    let n = region.len();
    ws.status = format!(
        "{n} {} picked: press one and drag up or down{}",
        if n == 1 {
            kind_singular(kind)
        } else {
            kind_name(kind)
        },
        if others > 0 {
            format!(" ({others} in other rooms left out)")
        } else {
            String::new()
        }
    );
    ws.grid.verb.region = Some(region);
}

fn kind_singular(kind: ExtrudeSurface) -> &'static str {
    match kind {
        ExtrudeSurface::Floor => "floor",
        ExtrudeSurface::Ceiling => "ceiling",
        ExtrudeSurface::Wall => "wall piece",
    }
}

/// The faces to pick again once the pieces have moved `steps` sectors, so
/// the next push carries on from the new wall.
fn followers(world: &GridWorld, pieces: &[WallPiece], steps: i32) -> Vec<GridElement> {
    let mut out = Vec::new();
    for piece in pieces {
        let step = piece.outward();
        let host = [
            piece.host[0] + step[0] * steps,
            piece.host[1] + step[1] * steps,
        ];
        let Some(room) = world.rooms.get(piece.room) else {
            continue;
        };
        let derived = derive::edge_walls(room, host, piece.edge);
        if derived.is_empty() {
            // A hand-placed wall closes it where nothing can be derived.
            let stack = room
                .local(host)
                .and_then(|[x, z]| room.sector(x, z))
                .map_or(0, |s| s.walls(piece.edge).len());
            out.extend((0..stack).map(|index| {
                GridElement::Face(FaceRef::wall(piece.room, host, piece.edge, index))
            }));
        }
        for wall in derived {
            out.push(GridElement::Face(FaceRef::of_derived(piece.room, &wall)));
        }
    }
    out
}

/// The drag ended after `steps` steps: the pieces now on the new wall are
/// the selection.
pub(super) fn finished(ws: &mut EditorWorkspace, live: &WallLive, steps: i32) {
    if steps == 0 {
        return;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let next = followers(world, live.drag.pieces(), steps);
    if !next.is_empty() {
        ws.grid.selection.set(next);
        ws.after_grid_selection_changed();
    }
}

/// The words for a result.
pub(super) fn readout(steps: i32, result: &Result<Extruded, ExtrudeRefusal>) -> String {
    match result {
        Err(refusal) => refusal.describe(),
        Ok(done) => done.describe(steps),
    }
}

/// The Out and In buttons and the `=` and `-` keys: push the picked pieces
/// `steps` sectors in one undo step. Pieces picked in the 3D view follow the
/// new wall, so the push can be repeated.
pub(crate) fn step(ws: &mut EditorWorkspace, steps: i32) {
    if ws.grid.cell_size.level() != 2 {
        ws.status = "Walls extrude whole sectors: set the cell size to Sector".to_string();
        return;
    }
    let Some((pieces, _)) = picked_pieces(ws) else {
        return;
    };
    let before = ws.project.clone();
    let result;
    {
        let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        let drag = match ExtrudeDrag::begin(world, pieces.clone()) {
            Ok(drag) => drag,
            Err(refusal) => {
                ws.status = refusal.describe();
                return;
            }
        };
        result = drag.apply(world, steps);
    }
    ws.status = readout(steps, &result);
    if result.is_err() {
        return;
    }
    ws.history.record(before);
    ws.grid_changed();
    if let Some(world) = ws.project.active_scene().world.grid() {
        let next = followers(world, &pieces, steps);
        if !next.is_empty() {
            ws.grid.selection.set(next);
            ws.after_grid_selection_changed();
        }
    }
}

/// `=` and `-` while the Extrude tool works on walls.
pub(super) fn step_by_key(ws: &mut EditorWorkspace, sign: i32) {
    if ws.grid.tool == GridTool::Extrude && ws.grid.verb.surface == ExtrudeSurface::Wall {
        let n = ws.grid.verb.wall_steps.max(1);
        step(ws, sign * n);
    }
}

impl EditorWorkspace {
    /// The chip over a wall extrusion in the 3D view.
    pub(super) fn draw_extrude_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Gesture::Verb(g) = &self.grid.gesture else {
            return;
        };
        let VerbDrag::Wall(live) = &g.drag else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(piece) = live.drag.pieces().first() else {
            return;
        };
        let y_offset = world.rooms.get(piece.room).map_or(0, |r| r.y_offset);
        let (a, b) = piece.edge.endpoints();
        let mid = [
            (a[0] + b[0]) as f32 * 0.5 + piece.host[0] as f32,
            (a[1] + b[1]) as f32 * 0.5 + piece.host[1] as f32,
        ];
        let y = piece.heights.iter().sum::<i32>() / 4;
        let at = [
            world.scale.to_editor((mid[0] * SECTOR_UNITS as f32) as i32) as f32,
            editor_y(world, y_offset, y),
            world.scale.to_editor((mid[1] * SECTOR_UNITS as f32) as i32) as f32,
        ];
        if let Some((s, _)) = super::view::project_point(self.viewport_3d_camera(), rect, at) {
            let text = if self.grid.verb.readout.is_empty() {
                self.status.clone()
            } else {
                self.grid.verb.readout.clone()
            };
            super::overlay::label_chip(
                painter,
                s - Vec2::new(0.0, 28.0),
                Align2::CENTER_CENTER,
                &text,
            );
        }
    }
}
