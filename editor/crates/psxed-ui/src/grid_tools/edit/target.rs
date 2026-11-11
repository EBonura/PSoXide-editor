//! What is under the pointer, for the Edit tool.
//!
//! One pick for hover, press and click, so what lights up is what a press
//! grabs. In the 3D view the face under the pointer decides; its corners and
//! edges are offered only within radii that shrink with the face's size on
//! screen (a tile seen from afar picks as a face, never as a corner), and
//! the corners and edges of the face under the pointer win over those of
//! its neighbours. In the plan a wall line near the pointer is a wall,
//! else the floor of the storey shows through.

use std::collections::{HashMap, HashSet};

use super::wall_handle::{WallHandle, WallSide};
use super::*;

use psxed_project::grid::edit::{CornerRef, EdgeRef, FaceRef, FaceTarget, GridElement};

/// Widest pick radius of a corner, in pixels, and of an edge.
const VERTEX_RADIUS: f32 = 9.0;
const EDGE_RADIUS: f32 = 6.0;
/// A radius under this many pixels switches that kind of pick off.
const MIN_RADIUS: f32 = 3.0;
/// The share of a face's shortest projected edge a radius may take.
const FACE_SHARE: f32 = 0.2;
/// Pixels the slide arrow stands above the top of its wall, and its reach.
pub(super) const ARROW_LIFT: f32 = 22.0;
const ARROW_RADIUS: f32 = 12.0;

/// What a press would grab.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    /// Nothing: sky, void, outside every room.
    Nothing,
    /// A floor or ceiling.
    Surface(FaceRef),
    /// One side of a floor or ceiling.
    SurfaceEdge(EdgeRef),
    /// One corner of a floor or ceiling.
    Corner(CornerRef),
    /// The body of a wall; the faces of the wall stack on that edge in the
    /// plan, where the line stands for all of them.
    Wall(FaceRef, Vec<FaceRef>),
    /// The top or the bottom edge of a wall.
    WallEdge(FaceRef, WallSide),
    /// The arrow that slides a wall up and down.
    WallArrow(FaceRef),
}

/// A pick: the target and the point on it, editor units.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    /// What was found.
    pub target: Target,
    /// The point the pointer's ray met the face, or the corner or edge
    /// point, editor units; the origin for the plan.
    pub point: [f32; 3],
}

impl Hit {
    /// Nothing under the pointer.
    pub fn nothing() -> Self {
        Self {
            target: Target::Nothing,
            point: [0.0; 3],
        }
    }
}

impl Target {
    /// The element a click selects.
    pub fn element(&self) -> Option<GridElement> {
        match self {
            Self::Nothing => None,
            Self::Surface(face) => Some(GridElement::Face(*face)),
            Self::SurfaceEdge(edge) => Some(GridElement::Edge(*edge)),
            Self::Corner(corner) => Some(GridElement::Vertex(*corner)),
            Self::Wall(face, _) | Self::WallEdge(face, _) | Self::WallArrow(face) => {
                Some(GridElement::Face(*face))
            }
        }
    }

    /// The elements a click selects (a wall line in the plan is a stack).
    pub fn elements(&self) -> Vec<GridElement> {
        match self {
            Self::Wall(_, run) if !run.is_empty() => {
                run.iter().copied().map(GridElement::Face).collect()
            }
            other => other.element().into_iter().collect(),
        }
    }

    /// The element that lights up under the pointer.
    pub fn highlight(&self, world: &psxed_project::grid::GridWorld) -> Option<GridElement> {
        match self {
            Self::WallEdge(face, side) => {
                let edge = match side {
                    WallSide::Bottom => 0,
                    WallSide::Top => 2,
                };
                if face.corners(world).is_empty() {
                    Some(GridElement::Face(*face))
                } else {
                    Some(GridElement::Edge(EdgeRef {
                        face: *face,
                        layer: 0,
                        edge,
                    }))
                }
            }
            other => other.element(),
        }
    }

    /// The wall this target is part of.
    pub fn wall(&self) -> Option<FaceRef> {
        match self {
            Self::Wall(face, _) | Self::WallEdge(face, _) | Self::WallArrow(face) => Some(*face),
            _ => None,
        }
    }
}

/// Distance from `p` to the segment `a b`.
fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// The shortest edge, in pixels, of a face as the camera projects it.
fn shortest_edge_px(cam: &pick::Cam, points: &[[f32; 3]]) -> f32 {
    let screen: Vec<Option<Pos2>> = points
        .iter()
        .map(|p| cam.project(*p).map(|s| s.0))
        .collect();
    let mut shortest = f32::MAX;
    for i in 0..screen.len() {
        let (Some(a), Some(b)) = (screen[i], screen[(i + 1) % screen.len()]) else {
            return 0.0;
        };
        let length = a.distance(b);
        if length > 0.5 {
            shortest = shortest.min(length);
        }
    }
    if shortest == f32::MAX {
        0.0
    } else {
        shortest
    }
}

/// The point of the world segment `wa wb` that lies nearest the pointer on
/// screen, where `sa sb` are its ends as projected.
fn lerp_along(pos: Pos2, sa: Pos2, sb: Pos2, wa: [f32; 3], wb: [f32; 3]) -> [f32; 3] {
    let ab = sb - sa;
    let len2 = ab.length_sq();
    let t = if len2 < 1e-6 {
        0.0
    } else {
        ((pos - sa).dot(ab) / len2).clamp(0.0, 1.0)
    };
    [
        wa[0] + (wb[0] - wa[0]) * t,
        wa[1] + (wb[1] - wa[1]) * t,
        wa[2] + (wb[2] - wa[2]) * t,
    ]
}

/// A pick radius: `base` pixels, at most [`FACE_SHARE`] of the face's
/// shortest edge, or `None` when that leaves under [`MIN_RADIUS`].
fn radius(base: f32, face_px: f32) -> Option<f32> {
    let r = base.min(FACE_SHARE * face_px);
    (r >= MIN_RADIUS).then_some(r)
}

/// The pick for a pointer in either view.
pub(crate) fn at(ws: &EditorWorkspace, ptr: &GridPointer) -> Hit {
    match ptr.view {
        GridView::Persp(rect) => at_3d(ws, rect, ptr.pos),
        GridView::Top(transform) => at_plan(ws, transform, ptr),
    }
}

/// Where the slide arrow of a wall stands on screen in the 3D view: above
/// the middle of the wall's top edge.
pub(super) fn arrow_position(
    ws: &EditorWorkspace,
    rect: Rect,
    face: FaceRef,
) -> Option<(Pos2, [f32; 3])> {
    let world = ws.project.active_scene().world.grid()?;
    WallHandle::of(face)?.can_move(world).ok()?;
    let camera = ws.viewport_3d_camera();
    ws.with_pick_cache(|cache| {
        let pf = cache.faces.iter().find(|pf| pf.face == face)?;
        let top = pf.points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let n = pf.points.len() as f32;
        let (x, z) = pf
            .points
            .iter()
            .fold((0.0, 0.0), |(x, z), p| (x + p[0] / n, z + p[2] / n));
        let anchor = [x, top, z];
        let (screen, _) = view::project_point(camera, rect, anchor)?;
        Some((screen - Vec2::new(0.0, ARROW_LIFT), anchor))
    })
    .flatten()
}

fn at_3d(ws: &EditorWorkspace, rect: Rect, pos: Pos2) -> Hit {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return Hit::nothing();
    };
    // The slide arrows come first: they stand outside the walls they move.
    let mut arrows: Vec<FaceRef> = ws.grid.edit.arrow_wall.into_iter().collect();
    for element in ws.grid.selection.items() {
        if let GridElement::Face(face) = element {
            if WallHandle::of(*face).is_some() && !arrows.contains(face) {
                arrows.push(*face);
            }
        }
    }
    for face in arrows.into_iter().take(16) {
        if let Some((screen, anchor)) = arrow_position(ws, rect, face) {
            if screen.distance(pos) <= ARROW_RADIUS {
                return Hit {
                    target: Target::WallArrow(face),
                    point: anchor,
                };
            }
        }
    }
    let face_hit = ws.pick_face_3d(rect, pos);
    let camera = ws.viewport_3d_camera();
    let cam = pick::Cam::new(camera, rect);
    let eye = camera.position_f32();
    let cut = ws.cutaway_plane().map(|y| y as f32);
    let limit = face_hit.map(|h| h.depth);
    let visible = |depth: f32| limit.is_none_or(|l| depth <= l * 1.02 + 24.0);
    let hit_face = face_hit.map(|h| h.face);
    let found = ws
        .with_pick_cache(|cache| {
            // Which faces the camera draws, and how big each looks.
            let mut drawn: HashSet<FaceRef> = HashSet::new();
            let mut known: HashSet<FaceRef> = HashSet::new();
            let mut size: HashMap<FaceRef, f32> = HashMap::new();
            let mut near_pointer: Vec<usize> = Vec::new();
            for (index, pf) in cache.faces.iter().enumerate() {
                if pf.void {
                    continue;
                }
                known.insert(pf.face);
                if pf.faces(eye) {
                    drawn.insert(pf.face);
                }
                let _ = index;
            }
            let culled = |face: &FaceRef| known.contains(face) && !drawn.contains(face);
            let mut face_px = |face: FaceRef| -> f32 {
                *size.entry(face).or_insert_with(|| {
                    cache
                        .faces
                        .iter()
                        .filter(|pf| pf.face == face && !pf.void)
                        .map(|pf| shortest_edge_px(&cam, &pf.points))
                        .fold(f32::MAX, f32::min)
                })
            };
            let _ = &mut near_pointer;
            // Corners, nearest the pointer; the hit face's own win ties.
            let mut best_corner: Option<(f32, f32, CornerRef, [f32; 3])> = None;
            for c in &cache.corners {
                let face = c.corner.face;
                if ws.grid_hidden_rooms.contains(&face.room)
                    || cut.is_some_and(|cut| c.pos[1] >= cut)
                    || culled(&face)
                    || !matches!(
                        face.target,
                        FaceTarget::Surface { .. } | FaceTarget::Wall { .. }
                    )
                {
                    continue;
                }
                let Some((screen, depth)) = cam.project(c.pos) else {
                    continue;
                };
                let distance = screen.distance(pos);
                if distance > VERTEX_RADIUS || !visible(depth) {
                    continue;
                }
                let Some(r) = radius(VERTEX_RADIUS, face_px(face)) else {
                    continue;
                };
                if distance > r {
                    continue;
                }
                let score = distance - if Some(face) == hit_face { 2.0 } else { 0.0 };
                if best_corner.is_none_or(|(s, ..)| score < s) {
                    best_corner = Some((score, depth, c.corner, c.pos));
                }
            }
            if let Some((_, _, corner, point)) = best_corner {
                return Some(match corner.face.target {
                    FaceTarget::Wall { .. } => Hit {
                        target: Target::WallEdge(
                            corner.face,
                            if corner.corner < 2 {
                                WallSide::Bottom
                            } else {
                                WallSide::Top
                            },
                        ),
                        point,
                    },
                    _ => Hit {
                        target: Target::Corner(corner),
                        point,
                    },
                });
            }
            // Edges of floors and ceilings.
            let mut best_edge: Option<(f32, EdgeRef, [f32; 3])> = None;
            for e in &cache.edges {
                let face = e.edge.face;
                if ws.grid_hidden_rooms.contains(&face.room)
                    || cut.is_some_and(|cut| e.a[1].min(e.b[1]) >= cut)
                    || culled(&face)
                    || !matches!(face.target, FaceTarget::Surface { .. })
                {
                    continue;
                }
                let (Some((a, da)), Some((b, db))) = (cam.project(e.a), cam.project(e.b)) else {
                    continue;
                };
                let distance = segment_distance(pos, a, b);
                if distance > EDGE_RADIUS || !visible(da.min(db)) {
                    continue;
                }
                let Some(r) = radius(EDGE_RADIUS, face_px(face)) else {
                    continue;
                };
                if distance > r {
                    continue;
                }
                let score = distance - if Some(face) == hit_face { 2.0 } else { 0.0 };
                if best_edge.is_none_or(|(s, ..)| score < s) {
                    best_edge = Some((score, e.edge, lerp_along(pos, a, b, e.a, e.b)));
                }
            }
            if let Some((_, edge, point)) = best_edge {
                return Some(Hit {
                    target: Target::SurfaceEdge(edge),
                    point,
                });
            }
            // The top and bottom edges of the wall under the pointer, from
            // its drawn outline (a derived wall has no corners of its own).
            let mut best: Option<(f32, WallSide, [f32; 3], FaceRef)> = None;
            for pf in cache.faces.iter().filter(|pf| {
                !pf.void
                    && WallHandle::of(pf.face).is_some()
                    && !ws.grid_hidden_rooms.contains(&pf.face.room)
                    && !culled(&pf.face)
                    && drawn.contains(&pf.face)
            }) {
                let Some(r) = radius(EDGE_RADIUS, face_px(pf.face)) else {
                    continue;
                };
                let bonus = if Some(pf.face) == hit_face { 2.0 } else { 0.0 };
                let mid_y = pf.points.iter().map(|p| p[1]).sum::<f32>() / pf.points.len() as f32;
                for i in 0..pf.points.len() {
                    let (a, b) = (pf.points[i], pf.points[(i + 1) % pf.points.len()]);
                    if (a[1] - b[1]).abs() > 1.0 {
                        continue;
                    }
                    let (Some((sa, _)), Some((sb, _))) = (cam.project(a), cam.project(b)) else {
                        continue;
                    };
                    let distance = segment_distance(pos, sa, sb);
                    let side = if a[1] >= mid_y {
                        WallSide::Top
                    } else {
                        WallSide::Bottom
                    };
                    let depth = cam.project(a).map_or(f32::MAX, |p| p.1);
                    if distance <= r
                        && visible(depth)
                        && best.is_none_or(|(d, ..)| distance - bonus < d)
                    {
                        best = Some((
                            distance - bonus,
                            side,
                            lerp_along(pos, sa, sb, a, b),
                            pf.face,
                        ));
                    }
                }
            }
            best.map(|(_, side, point, face)| Hit {
                target: Target::WallEdge(face, side),
                point,
            })
        })
        .flatten();
    if let Some(hit) = found {
        return hit;
    }
    let _ = world;
    match face_hit {
        Some(h) => Hit {
            target: if WallHandle::of(h.face).is_some() {
                Target::Wall(h.face, Vec::new())
            } else {
                Target::Surface(h.face)
            },
            point: h.point,
        },
        None => Hit::nothing(),
    }
}

fn at_plan(ws: &EditorWorkspace, transform: ViewportTransform, ptr: &GridPointer) -> Hit {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return Hit::nothing();
    };
    let size = world.scale.sector_editor_units() as f32;
    let cell_px = (size * transform.screen_radius(1.0)).abs();
    let wall_radius = radius(EDGE_RADIUS, cell_px);
    if let Some(r) = wall_radius {
        if ws.top_wall_at(transform, ptr.pos, r).is_some() {
            if let Some(elements) = ws.grid_pick_for_select(ptr, pick::PickLevel::Face) {
                let run: Vec<FaceRef> = elements
                    .iter()
                    .filter_map(|e| match e {
                        GridElement::Face(f) if WallHandle::of(*f).is_some() => Some(*f),
                        _ => None,
                    })
                    .collect();
                if let Some(first) = run.first().copied() {
                    return Hit {
                        target: Target::Wall(first, run),
                        point: [0.0; 3],
                    };
                }
            }
        }
    }
    let vertex = radius(10.0, cell_px).unwrap_or(0.0);
    let edge = radius(5.0, cell_px).unwrap_or(0.0);
    let Some(element) = height_tool::top_element_with(ws, transform, ptr.pos, vertex, edge) else {
        return Hit::nothing();
    };
    let target = match element {
        GridElement::Vertex(c) => Target::Corner(c),
        GridElement::Edge(e) => Target::SurfaceEdge(e),
        GridElement::Face(f) => Target::Surface(f),
        _ => return Hit::nothing(),
    };
    Hit {
        target,
        point: [0.0; 3],
    }
}
