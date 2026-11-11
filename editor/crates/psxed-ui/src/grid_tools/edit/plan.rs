//! What a press-drag will do to the document, worked out at the press.
//!
//! A plan is built once, when the press lands, from what was pressed and the
//! selection that gives it scope. Every pointer movement then asks the plan
//! for the document at one amount: the plan puts back what it captured and
//! applies that amount, so the drag is a function of the amount alone (a
//! drag back to zero is the document as it was, and the typed amount, the
//! pointer and the repeat command all share one code path).

use std::collections::{BTreeMap, BTreeSet};

use super::wall_handle::{warning_text, WallHandle, WallMove};

use psxed_project::grid::cook::RuntimeUnits;
use psxed_project::grid::edit::heights::refine_corners;
use psxed_project::grid::edit::tilt::{tilt_cells, Hinge, TiltParams, TiltTarget};
use psxed_project::grid::edit::verbs::extrude::{pieces_of, ExtrudeDrag};
use psxed_project::grid::edit::verbs::ramp::RampDir;
use psxed_project::grid::edit::{CornerRef, FaceRef, GridElement, HeightDrag, SurfaceKind};
use psxed_project::grid::slope::{surface_class, SlopeClass};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::{GridRoom, GridWorld, FINE_UNITS};

/// How much of the amount each corner takes, by where it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Profile {
    /// Every corner takes all of it.
    Uniform,
    /// A hinge line: nothing at `hinge`, all of it at `drag`, along `axis`
    /// (0 is x, 1 is z), world grid units.
    Edge {
        /// The axis the amount grows along.
        axis: usize,
        /// Coordinate of the side that stays.
        hinge: i32,
        /// Coordinate of the side that is dragged.
        drag: i32,
    },
    /// Nothing at the hinge corner, all of it at the dragged corner, in
    /// proportion to the position along the line between them.
    Diagonal {
        /// The corner that stays, `[x, z]`.
        hinge: [i32; 2],
        /// The corner that is dragged, `[x, z]`.
        corner: [i32; 2],
    },
}

impl Profile {
    /// The share of the amount at world position `[x, z]`.
    fn weight(self, pos: [i32; 2]) -> f32 {
        match self {
            Self::Uniform => 1.0,
            Self::Edge { axis, hinge, drag } => {
                let span = (drag - hinge) as f32;
                if span.abs() < 1.0 {
                    return 1.0;
                }
                ((pos[axis] - hinge) as f32 / span).clamp(0.0, 1.0)
            }
            Self::Diagonal { hinge, corner } => {
                let (dx, dz) = ((corner[0] - hinge[0]) as f32, (corner[1] - hinge[1]) as f32);
                let len2 = dx * dx + dz * dz;
                if len2 < 1.0 {
                    return 1.0;
                }
                let t = ((pos[0] - hinge[0]) as f32 * dx + (pos[1] - hinge[1]) as f32 * dz) / len2;
                t.clamp(0.0, 1.0)
            }
        }
    }

    /// Whether the amount is shared out over a distance (and so rounds).
    fn is_tilt(self) -> bool {
        !matches!(self, Self::Uniform)
    }
}

/// What applying a plan at an amount did.
#[derive(Clone, Debug, Default)]
pub(crate) struct Applied {
    /// Whether the document differs from the captured one.
    pub changed: bool,
    /// The greatest distance, grid units, between a corner's ideal share and
    /// what the 64 lattice allowed.
    pub deviation: i32,
    /// Floors that now slope more than a body can stand on.
    pub steep: usize,
    /// Why the amount was refused (the document is as captured then).
    pub refused: Option<String>,
    /// The words for the result of a push.
    pub text: Option<String>,
    /// What a wall edit leaves open, in words (never a refusal).
    pub warnings: Vec<String>,
    /// The faces a wall edit made: the selection follows them.
    pub faces: Option<Vec<FaceRef>>,
}

/// Corner height changes, shared over the corners by a profile.
#[derive(Clone, Debug)]
pub(crate) struct HeightsPlan {
    /// The rooms as captured, restored before every apply.
    rooms: Vec<(usize, GridRoom)>,
    corners: Vec<CornerRef>,
    weights: Vec<f32>,
    /// Position of each corner along the tilt axis, for the lattice check.
    coords: Vec<i32>,
    tilt: bool,
}

impl HeightsPlan {
    /// Plan moving `corners` by `profile`. The world may gain leaves (see
    /// [`refine_corners`]); that changes nothing about how it looks.
    pub fn new(
        world: &mut GridWorld,
        corners: &[CornerRef],
        profile: Profile,
        weld: bool,
    ) -> Result<Self, String> {
        if corners.is_empty() {
            return Err("Nothing to move".to_string());
        }
        let (moving, _) = refine_corners(world, corners, weld);
        let axis = match profile {
            Profile::Edge { axis, .. } => axis,
            _ => 0,
        };
        let mut weights = Vec::new();
        let mut coords = Vec::new();
        for corner in &moving {
            let pos = corner
                .grid_pos(world)
                .ok_or_else(|| "A corner is gone".to_string())?;
            weights.push(profile.weight([pos[0], pos[2]]));
            if matches!(profile, Profile::Edge { .. }) {
                coords.push([pos[0], pos[2]][axis]);
            }
        }
        let mut rooms: Vec<usize> = moving.iter().map(|c| c.face.room).collect();
        rooms.sort_unstable();
        rooms.dedup();
        if profile.is_tilt() && rooms.len() > 1 {
            return Err("A slope runs over one room: pick faces of one room".to_string());
        }
        Ok(Self {
            rooms: rooms
                .into_iter()
                .filter_map(|r| world.rooms.get(r).map(|room| (r, room.clone())))
                .collect(),
            corners: moving,
            weights,
            coords,
            tilt: profile.is_tilt(),
        })
    }

    /// Whether the plan is a slope (an amount shared over a distance).
    pub fn is_tilt(&self) -> bool {
        self.tilt
    }

    fn restore(&self, world: &mut GridWorld) {
        for (index, room) in &self.rooms {
            if let Some(slot) = world.rooms.get_mut(*index) {
                *slot = room.clone();
            }
        }
    }

    /// Put the captured document back, then move the corners by `amount`
    /// grid units (their share of it, for a slope).
    pub fn apply(&self, world: &mut GridWorld, amount: i32) -> Applied {
        self.restore(world);
        let mut out = Applied::default();
        let mut deltas = Vec::with_capacity(self.corners.len());
        for weight in &self.weights {
            let ideal = amount as f32 * weight;
            let delta = if self.tilt {
                snap_to(ideal.round() as i32, FINE_UNITS)
            } else {
                amount
            };
            out.deviation = out
                .deviation
                .max((ideal - delta as f32).abs().round() as i32);
            deltas.push(delta);
        }
        if self.tilt && amount != 0 && !self.coords.is_empty() && too_gentle(&self.coords, &deltas)
        {
            out.refused = Some(
                "Too gentle for the 64 lattice: drag further or slope fewer cells".to_string(),
            );
            out.deviation = 0;
            return out;
        }
        let mut groups: BTreeMap<i32, Vec<CornerRef>> = BTreeMap::new();
        for (corner, delta) in self.corners.iter().zip(&deltas) {
            if *delta != 0 {
                groups.entry(*delta).or_default().push(*corner);
            }
        }
        // Every drag is begun before any is applied, so each remembers the
        // diagonals as they were captured.
        let drags: Vec<(HeightDrag, i32)> = groups
            .iter()
            .map(|(delta, group)| (HeightDrag::begin(world, group, false), *delta))
            .collect();
        for (drag, delta) in &drags {
            out.changed |= drag.apply(world, *delta);
        }
        if out.changed && self.tilt {
            out.steep = steep_floors(world, &self.corners);
        }
        out
    }
}

/// Whether the lattice flattened more than a third of the steps between
/// neighbouring columns of a slope.
fn too_gentle(coords: &[i32], deltas: &[i32]) -> bool {
    let mut by_column: BTreeMap<i32, i32> = BTreeMap::new();
    for (coord, delta) in coords.iter().zip(deltas) {
        by_column.insert(*coord, *delta);
    }
    let values: Vec<i32> = by_column.values().copied().collect();
    if values.len() < 3 {
        return false;
    }
    let flat = values.windows(2).filter(|w| w[0] == w[1]).count();
    flat * 3 > values.len() - 1
}

/// How many of the floors the corners belong to are steeper than a body can
/// stand on.
fn steep_floors(world: &GridWorld, corners: &[CornerRef]) -> usize {
    let Ok(units) = RuntimeUnits::new(world.scale) else {
        return 0;
    };
    let limits = psx_engine::WalkLimits::DEFAULT;
    let mut faces: Vec<_> = corners
        .iter()
        .map(|c| c.face)
        .filter(|f| {
            matches!(
                f.target,
                psxed_project::grid::edit::FaceTarget::Surface {
                    kind: SurfaceKind::Floor,
                    ..
                }
            )
        })
        .collect();
    faces.sort();
    faces.dedup();
    faces
        .iter()
        .filter(|face| {
            let Some(room) = face.room_of(world) else {
                return false;
            };
            face.surface(world).is_some_and(|surface| {
                surface_class(&units, room.y_offset, surface, &limits) == SlopeClass::Steep
            })
        })
        .count()
}

/// Pushing wall pieces out or in, a sector a step.
#[derive(Clone, Debug)]
pub(crate) struct PushPlan {
    drag: ExtrudeDrag,
    /// The horizontal step out through the wall, `[x, z]` sectors.
    pub outward: [i32; 2],
}

impl PushPlan {
    /// Plan pushing the walls named by `elements`.
    pub fn new(world: &GridWorld, elements: &[GridElement]) -> Result<Self, String> {
        let pieces = pieces_of(world, elements).map_err(|r| r.describe())?;
        let outward = pieces.first().map_or([0, 0], |p| p.outward());
        let drag = ExtrudeDrag::begin(world, pieces).map_err(|r| r.describe())?;
        Ok(Self { drag, outward })
    }

    /// Put the room back, then push `steps` sectors.
    pub fn apply(&self, world: &mut GridWorld, steps: i32) -> Applied {
        match self.drag.apply(world, steps) {
            Ok(done) => Applied {
                changed: steps != 0,
                text: Some(done.describe(steps)),
                ..Applied::default()
            },
            Err(refusal) => Applied {
                refused: Some(refusal.describe()),
                ..Applied::default()
            },
        }
    }
}

/// A selection tilted through the SDK's tilt: every corner gains its share of
/// the rise (offset semantics, so a platform stays a platform), rounded to
/// the 64 lattice the same way for neighbours.
#[derive(Clone, Debug)]
pub(crate) struct TiltPlan {
    /// The room as captured, restored before every apply.
    room: (usize, GridRoom),
    params: TiltParams,
    /// Whether the rise grows along x (else z), for the lattice check.
    axis: usize,
}

impl TiltPlan {
    /// Plan tilting `cells` of `room` about `hinge`.
    pub fn new(
        world: &GridWorld,
        room: usize,
        cells: BTreeSet<[i32; 2]>,
        target: TiltTarget,
        hinge: Hinge,
    ) -> Result<Self, String> {
        let captured = world
            .rooms
            .get(room)
            .ok_or_else(|| "No such room".to_string())?
            .clone();
        let axis = match hinge {
            Hinge::Edge(RampDir::East | RampDir::West) => 0,
            _ => 1,
        };
        Ok(Self {
            room: (room, captured),
            params: TiltParams {
                room,
                cells,
                target,
                hinge,
                rise: 0,
                force: true,
            },
            axis,
        })
    }

    /// Put the captured room back, then tilt by `amount` grid units at the
    /// far side.
    pub fn apply(&self, world: &mut GridWorld, amount: i32) -> Applied {
        if let Some(slot) = world.rooms.get_mut(self.room.0) {
            *slot = self.room.1.clone();
        }
        let mut out = Applied::default();
        if amount == 0 {
            return out;
        }
        let params = TiltParams {
            rise: amount,
            ..self.params.clone()
        };
        let done = match tilt_cells(world, &params) {
            Ok(done) => done,
            Err(refusal) => {
                out.refused = Some(refusal.describe());
                return out;
            }
        };
        out.deviation = done
            .cells
            .iter()
            .flat_map(|c| c.exact_16ths.iter().zip(c.applied.iter()))
            .map(|(exact, applied)| (exact / 16 - applied).abs())
            .max()
            .unwrap_or(0);
        if matches!(self.params.hinge, Hinge::Edge(_)) {
            // Corner positions along the axis, in sectors, for the lattice check
            // (a leaf larger than a sector counts as one).
            let mut coords = Vec::new();
            let mut deltas = Vec::new();
            for cell in &done.cells {
                let at = |i: usize| {
                    let [dx, dz] = [[0, 0], [1, 0], [1, 1], [0, 1]][i];
                    [cell.cell[0] + dx, cell.cell[1] + dz][self.axis]
                };
                for i in 0..4 {
                    coords.push(at(i));
                    deltas.push(cell.applied[i]);
                }
            }
            if too_gentle(&coords, &deltas) {
                if let Some(slot) = world.rooms.get_mut(self.room.0) {
                    *slot = self.room.1.clone();
                }
                out.deviation = 0;
                out.refused = Some(
                    "Too gentle for the 64 lattice: drag further or slope fewer cells".to_string(),
                );
                return out;
            }
        }
        out.changed = done.tilted > 0;
        out.steep = done.steep;
        out
    }
}

/// Walls moved through the wall API: they keep the heights they are given
/// and the floor and the ceiling stay where they are.
#[derive(Clone, Debug)]
pub(crate) struct WallPlan {
    /// The rooms as captured, restored before every apply.
    rooms: Vec<(usize, GridRoom)>,
    walls: Vec<WallHandle>,
    how: WallMove,
}

impl WallPlan {
    /// Plan moving `walls` the way `how` says.
    pub fn new(world: &GridWorld, walls: &[WallHandle], how: WallMove) -> Result<Self, String> {
        if walls.is_empty() {
            return Err("No wall to move".to_string());
        }
        for wall in walls {
            wall.can_move(world)?;
        }
        let mut walls = walls.to_vec();
        walls.sort_by_key(|w| w.face());
        walls.dedup();
        // The later faces of one edge first: writing a generated wall shifts
        // the ones after it.
        walls.reverse();
        let mut rooms: Vec<usize> = walls.iter().map(|w| w.face().room).collect();
        rooms.sort_unstable();
        rooms.dedup();
        Ok(Self {
            rooms: rooms
                .into_iter()
                .filter_map(|r| world.rooms.get(r).map(|room| (r, room.clone())))
                .collect(),
            walls,
            how,
        })
    }

    /// Put the captured rooms back, then move the walls by `amount`.
    pub fn apply(&self, world: &mut GridWorld, amount: i32) -> Applied {
        for (index, room) in &self.rooms {
            if let Some(slot) = world.rooms.get_mut(*index) {
                *slot = room.clone();
            }
        }
        let mut out = Applied::default();
        if amount == 0 {
            return out;
        }
        let mut faces = Vec::new();
        let mut seen: BTreeSet<FaceRef> = BTreeSet::new();
        for wall in &self.walls {
            // A wall written earlier can have taken this one's place (the
            // storeys of one edge are written together): find it again.
            let handle = if wall.can_move(world).is_ok() {
                *wall
            } else {
                match WallHandle::stand_in(world, wall.face(), &seen) {
                    Some(handle) => handle,
                    None => continue,
                }
            };
            match handle.move_by(world, self.how, amount) {
                Ok(moved) => {
                    out.changed = true;
                    for warning in moved.warnings {
                        let text = warning_text(warning);
                        if !out.warnings.contains(&text) {
                            out.warnings.push(text);
                        }
                    }
                    seen.insert(moved.face);
                    faces.push(moved.face);
                }
                Err(reason) => {
                    self.restore(world);
                    return Applied {
                        refused: Some(reason),
                        ..Applied::default()
                    };
                }
            }
        }
        out.faces = Some(faces);
        out
    }

    fn restore(&self, world: &mut GridWorld) {
        for (index, room) in &self.rooms {
            if let Some(slot) = world.rooms.get_mut(*index) {
                *slot = room.clone();
            }
        }
    }
}

/// What a press-drag does.
#[derive(Clone, Debug)]
pub(crate) enum Plan {
    /// Corners move.
    Heights(HeightsPlan),
    /// Walls keep their height and move, or stretch.
    Walls(WallPlan),
    /// A selection leans through the SDK's tilt.
    Tilt(Box<TiltPlan>),
    /// A room's wall is pushed.
    Push(Box<PushPlan>),
}

impl Plan {
    /// Whether the amount counts sectors (a push), not grid units.
    pub fn counts_steps(&self) -> bool {
        matches!(self, Self::Push(_))
    }

    /// Whether the plan is a slope.
    pub fn is_tilt(&self) -> bool {
        matches!(self, Self::Tilt(_)) || matches!(self, Self::Heights(p) if p.is_tilt())
    }

    /// Put the captured document back and apply `amount`.
    pub fn apply(&self, world: &mut GridWorld, amount: i32) -> Applied {
        match self {
            Self::Heights(plan) => plan.apply(world, amount),
            Self::Walls(plan) => plan.apply(world, amount),
            Self::Tilt(plan) => plan.apply(world, amount),
            Self::Push(plan) => plan.apply(world, amount),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_profile_grows_from_the_hinge() {
        let p = Profile::Edge {
            axis: 0,
            hinge: 0,
            drag: 4096,
        };
        assert_eq!(p.weight([0, 7]), 0.0);
        assert_eq!(p.weight([2048, 7]), 0.5);
        assert_eq!(p.weight([4096, 7]), 1.0);
    }

    #[test]
    fn one_click_over_eight_sectors_is_too_gentle() {
        // 256 over eight sectors: 32 units a sector, rounded to 64.
        let coords: Vec<i32> = (0..=8).map(|i| i * 1024).collect();
        let deltas: Vec<i32> = coords
            .iter()
            .map(|c| snap_to((256.0 * *c as f32 / 8192.0).round() as i32, FINE_UNITS))
            .collect();
        assert!(too_gentle(&coords, &deltas), "{deltas:?}");
        // Two clicks over four sectors is 128 a sector: fine.
        let coords: Vec<i32> = (0..=4).map(|i| i * 1024).collect();
        let deltas: Vec<i32> = coords
            .iter()
            .map(|c| snap_to((512.0 * *c as f32 / 4096.0).round() as i32, FINE_UNITS))
            .collect();
        assert!(!too_gentle(&coords, &deltas), "{deltas:?}");
    }
}
