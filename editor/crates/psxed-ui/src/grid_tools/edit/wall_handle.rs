//! One handle on a wall, whatever made it.
//!
//! The Edit tool never asks how a wall is stored. It asks this module to
//! read the wall's four corners, to name the corners a gesture moves, and to
//! say what else moves with them. Today that is answered over the model as it
//! is: a wall that stands between two floors or under a ceiling moves the
//! floor or ceiling that owns its edge, a wall that was placed by hand moves
//! its own corners. When the SDK offers one Wall API, this file is the only
//! place that changes: the three functions below ([`WallHandle::corners`],
//! [`WallHandle::edge_move`], [`WallHandle::slide`]) keep their shapes and
//! the rest of the tool does not notice.
//!
//! The SDK's Wall API (`grid::edit::wall_api`: `WallAddr`, `walls_at`,
//! `set_wall_heights`, `clear_wall_property`) will answer the same three
//! questions with "set these corners on this wall, the floor stays". Then
//! [`WallHandle::edge_move`] and [`WallHandle::slide`] return the wall's own
//! corners with no side effect, and [`WallHandle::reset_to_automatic`]
//! calls `clear_wall_property(Heights)`.
//!
//! What the user reads about a wall never names how it is stored. Where an
//! edit has a side effect (a floor moves), [`CornerMove::note`] says so for
//! the hint bar.

use std::collections::BTreeSet;

use psxed_project::grid::derive::DerivedKind;
use psxed_project::grid::edit::verbs::extrude::pieces_of;
use psxed_project::grid::edit::walls::edge_corner_indices;
use psxed_project::grid::edit::{CornerRef, FaceRef, FaceTarget, GridElement, SurfaceKind};
use psxed_project::grid::{GridWorld, WallEdge};

/// The top or the bottom edge of a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WallSide {
    /// The upper edge.
    Top,
    /// The lower edge.
    Bottom,
}

/// A wall's corner heights, room relative, along its edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct WallCorners {
    /// Bottom corners, `[a, b]`.
    pub bottom: [i32; 2],
    /// Top corners, `[a, b]`.
    pub top: [i32; 2],
}

/// The corners a wall gesture moves, and what that moves besides the wall.
#[derive(Clone, Debug, Default)]
pub(crate) struct CornerMove {
    /// The corners to move.
    pub corners: Vec<CornerRef>,
    /// What else moves, for the hint ("the floor of 2 cells"); empty when the
    /// wall's own corners are all that move.
    pub moves: BTreeSet<(Moved, usize, [i32; 2])>,
}

/// What a wall edit moves besides the wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Moved {
    /// A floor.
    Floor,
    /// A ceiling.
    Ceiling,
}

impl CornerMove {
    fn extend(&mut self, other: Self) {
        for corner in other.corners {
            if !self.corners.contains(&corner) {
                self.corners.push(corner);
            }
        }
        self.moves.extend(other.moves);
    }

    /// The side effect in words, or `None` when only the wall moves.
    pub fn note(&self) -> Option<String> {
        if self.moves.is_empty() {
            return None;
        }
        let count = |what: Moved| self.moves.iter().filter(|(m, ..)| *m == what).count();
        let (floors, ceilings) = (count(Moved::Floor), count(Moved::Ceiling));
        let cells = |n: usize| format!("{n} cell{}", if n == 1 { "" } else { "s" });
        Some(match (floors, ceilings) {
            (f, c) if f > 0 && f == c => format!("moves the floor and the ceiling of {}", cells(f)),
            (f, 0) => format!("moves the floor of {}", cells(f)),
            (0, c) => format!("moves the ceiling of {}", cells(c)),
            (f, c) => format!(
                "moves the floor of {} and the ceiling of {}",
                cells(f),
                cells(c)
            ),
        })
    }
}

/// A wall, by the face the picker found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WallHandle {
    face: FaceRef,
}

#[allow(dead_code)]
impl WallHandle {
    /// The handle for `face`, when it is a wall.
    pub fn of(face: FaceRef) -> Option<Self> {
        matches!(
            face.target,
            FaceTarget::Wall { .. } | FaceTarget::Derived { .. }
        )
        .then_some(Self { face })
    }

    /// The wall's face.
    pub fn face(&self) -> FaceRef {
        self.face
    }

    /// Bottom and top corner heights, room relative.
    pub fn corners(&self, world: &GridWorld) -> Option<WallCorners> {
        let piece = pieces_of(world, &[GridElement::Face(self.face)])
            .ok()?
            .into_iter()
            .next()?;
        let [ba, bb, tb, ta] = piece.heights;
        Some(WallCorners {
            bottom: [ba, bb],
            top: [ta, tb],
        })
    }

    /// The corners that move the wall's top or bottom edge, with the side
    /// effects. `Err` says why the wall cannot be stretched that way.
    pub fn edge_move(&self, world: &GridWorld, side: WallSide) -> Result<CornerMove, String> {
        match self.face.target {
            FaceTarget::Wall { .. } => {
                let (a, b) = match side {
                    WallSide::Bottom => (0, 1),
                    WallSide::Top => (3, 2),
                };
                Ok(CornerMove {
                    corners: [a, b]
                        .into_iter()
                        .map(|corner| CornerRef {
                            face: self.face,
                            layer: 0,
                            corner,
                        })
                        .collect(),
                    moves: BTreeSet::new(),
                })
            }
            FaceTarget::Derived { edge, kind, .. } => {
                let wall = self
                    .face
                    .derived_wall(world)
                    .ok_or_else(|| "That wall is not there any more".to_string())?;
                if edge.is_diagonal() {
                    return Err("A diagonal wall follows its two halves of floor".to_string());
                }
                let room = self.face.room;
                let (host, owner, span) = (wall.host, wall.owner, wall.span);
                // The facing edge of the neighbour is the opposite edge.
                let facing = edge.opposite().map(|(opposite, _)| opposite);
                let surface = |cell: [i32; 2], kind: SurfaceKind, edge: WallEdge| {
                    surface_edge(world, room, cell, span, kind, edge)
                };
                match (kind, side) {
                    (DerivedKind::Closure, WallSide::Bottom) => {
                        surface(host, SurfaceKind::Floor, edge)
                    }
                    (DerivedKind::Closure, WallSide::Top) => {
                        surface(host, SurfaceKind::Ceiling, edge)
                    }
                    (DerivedKind::Riser, WallSide::Bottom) => {
                        surface(host, SurfaceKind::Floor, edge)
                    }
                    (DerivedKind::Riser, WallSide::Top) => {
                        let facing = facing.ok_or("No neighbour".to_string())?;
                        surface(owner, SurfaceKind::Floor, facing)
                    }
                    (DerivedKind::Drop, WallSide::Top) => surface(host, SurfaceKind::Ceiling, edge),
                    (DerivedKind::Drop, WallSide::Bottom) => {
                        let facing = facing.ok_or("No neighbour".to_string())?;
                        surface(owner, SurfaceKind::Ceiling, facing)
                    }
                    (DerivedKind::Slab, _) => {
                        Err("This wall is the side of a slab: move the slab".to_string())
                    }
                }
            }
            _ => Err("That is not a wall".to_string()),
        }
    }

    /// The corners that slide the whole wall up or down at its height. A wall
    /// that closes a room carries the floor and ceiling of its cell with it;
    /// a step between two floors has nothing to slide.
    pub fn slide(&self, world: &GridWorld) -> Result<CornerMove, String> {
        match self.face.target {
            FaceTarget::Wall { .. } => Ok(CornerMove {
                corners: (0..4)
                    .map(|corner| CornerRef {
                        face: self.face,
                        layer: 0,
                        corner,
                    })
                    .collect(),
                moves: BTreeSet::new(),
            }),
            FaceTarget::Derived {
                kind: DerivedKind::Closure,
                span,
                ..
            } => {
                let wall = self
                    .face
                    .derived_wall(world)
                    .ok_or_else(|| "That wall is not there any more".to_string())?;
                let room = self.face.room;
                let mut out = CornerMove::default();
                for kind in [SurfaceKind::Floor, SurfaceKind::Ceiling] {
                    let face = surface_face(room, wall.host, span, kind);
                    if face.exists(world) {
                        out.corners.extend(face.corners(world));
                        out.moves.insert((moved(kind), room, wall.host));
                    }
                }
                if out.corners.is_empty() {
                    return Err("Nothing to slide".to_string());
                }
                Ok(out)
            }
            FaceTarget::Derived { .. } => {
                Err("A step between two floors cannot slide: stretch it instead".to_string())
            }
            _ => Err("That is not a wall".to_string()),
        }
    }

    /// The corners of every wall in `walls` for `side`, merged.
    pub fn edge_move_all(
        walls: &[Self],
        world: &GridWorld,
        side: WallSide,
    ) -> Result<CornerMove, String> {
        let mut out = CornerMove::default();
        for wall in walls {
            out.extend(wall.edge_move(world, side)?);
        }
        Ok(out)
    }

    /// The corners of every wall in `walls` for a slide, merged.
    pub fn slide_all(walls: &[Self], world: &GridWorld) -> Result<CornerMove, String> {
        let mut out = CornerMove::default();
        for wall in walls {
            out.extend(wall.slide(world)?);
        }
        Ok(out)
    }

    /// Back to the height the cells give it. Needs the SDK Wall API; until
    /// then a wall has nothing of its own to reset.
    pub fn reset_to_automatic(&self, _world: &mut GridWorld) -> Result<(), String> {
        Err("Reset to automatic arrives with the new wall model".to_string())
    }

    /// Whether the wall offers the slide arrow.
    pub fn can_slide(&self, world: &GridWorld) -> bool {
        self.slide(world).is_ok()
    }
}

fn moved(kind: SurfaceKind) -> Moved {
    match kind {
        SurfaceKind::Floor => Moved::Floor,
        SurfaceKind::Ceiling => Moved::Ceiling,
    }
}

fn surface_face(room: usize, cell: [i32; 2], span: u8, kind: SurfaceKind) -> FaceRef {
    match kind {
        SurfaceKind::Floor => FaceRef::floor_in(room, cell, span),
        SurfaceKind::Ceiling => FaceRef::ceiling_in(room, cell, span),
    }
}

/// The two corners of `cell`'s floor or ceiling along `edge`.
fn surface_edge(
    world: &GridWorld,
    room: usize,
    cell: [i32; 2],
    span: u8,
    kind: SurfaceKind,
    edge: WallEdge,
) -> Result<CornerMove, String> {
    let face = surface_face(room, cell, span, kind);
    if !face.exists(world) {
        return Err(format!(
            "No {} on that side to move",
            match kind {
                SurfaceKind::Floor => "floor",
                SurfaceKind::Ceiling => "ceiling",
            }
        ));
    }
    let [a, b] = edge_corner_indices(edge);
    let mut out = CornerMove::default();
    for corner in [a, b] {
        out.corners.push(CornerRef {
            face,
            layer: 0,
            corner: corner as u8,
        });
    }
    out.moves.insert((moved(kind), room, cell));
    Ok(out)
}
