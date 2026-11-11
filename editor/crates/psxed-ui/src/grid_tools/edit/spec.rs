//! What a press does: the decision table of the Edit tool.
//!
//! | Pressed                         | In the selection | Does                                        |
//! |---------------------------------|------------------|---------------------------------------------|
//! | floor or ceiling                | no               | that face moves up and down as a flat block |
//! | floor or ceiling                | yes              | the selection's faces move together         |
//! | edge of a floor or ceiling      | no               | that face tilts about its opposite edge     |
//! | edge                            | yes              | the selection tilts about its opposite side |
//! | corner                          | no, or one face  | that corner lifts                           |
//! | corner                          | yes, many faces  | the selection tilts about the far corner    |
//! | wall face                       | any              | the room's wall pushes out or in a sector   |
//! | top or bottom edge of a wall    | any              | the wall stretches                          |
//! | arrow over a wall               | any              | the wall slides up and down at its height   |
//!
//! Walls and surfaces never mix: the pressed element's kind decides, and the
//! other kinds in the selection stay where they are.

use super::plan::{HeightsPlan, Plan, Profile, PushPlan, TiltPlan, WallPlan};
use super::target::{Hit, Target};
use super::wall_handle::{WallHandle, WallMove, WallSide};
use super::{Axis, EditorWorkspace, FaceRef};

use std::collections::BTreeSet;

use psxed_project::grid::edit::tilt::{BoxCorner, Hinge, TiltTarget};
use psxed_project::grid::edit::verbs::ramp::RampDir;
use psxed_project::grid::edit::{CellRect, CornerRef, FaceTarget, GridElement, SurfaceKind};
use psxed_project::grid::GridWorld;
use psxed_project::grid::SECTOR_UNITS;

/// A side of a selection's outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    North,
    East,
    South,
    West,
}

impl Side {
    /// The side of a face an edge number is (a surface runs N, E, S, W).
    pub fn of_edge(edge: u8) -> Self {
        match edge & 3 {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            _ => Self::West,
        }
    }
}

/// What kind of gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Floors or ceilings move up and down.
    Translate,
    /// Floors or ceilings tilt about the side opposite `Side`.
    Tilt(Side),
    /// Floors or ceilings tilt about the corner opposite the one at `[x, z]`.
    TiltCorner([i32; 2]),
    /// One corner lifts.
    Lift(CornerRef),
    /// Walls stretch at their top or bottom edge.
    Stretch(WallSide),
    /// Walls slide up and down.
    Slide,
    /// Walls push out and in.
    Push,
}

/// A press resolved against the selection: the kind and the faces it acts on.
#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub kind: Kind,
    /// The surface faces or the wall faces the gesture acts on.
    pub faces: Vec<FaceRef>,
}

impl Spec {
    /// How many faces take part.
    pub fn count(&self) -> usize {
        self.faces.len().max(1)
    }
}

/// A plan with its words.
pub(crate) struct Built {
    pub plan: Plan,
    pub axis: Axis,
    pub label: String,
    pub note: Option<String>,
}

fn surface_kind(face: &FaceRef) -> Option<SurfaceKind> {
    match face.target {
        FaceTarget::Surface { kind, .. } => Some(kind),
        _ => None,
    }
}

/// Whether the pressed target belongs to the selection.
pub(super) fn in_selection(ws: &EditorWorkspace, target: &Target) -> bool {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return false;
    };
    let faces = ws.grid.selection.faces(world);
    match target {
        Target::Nothing => false,
        Target::Surface(face) => faces.contains(face),
        Target::SurfaceEdge(edge) => {
            faces.contains(&edge.face) || ws.grid.selection.contains(&GridElement::Edge(*edge))
        }
        Target::Corner(corner) => {
            faces.contains(&corner.face)
                || ws.grid.selection.contains(&GridElement::Vertex(*corner))
        }
        Target::Wall(face, run) => {
            faces.contains(face) || (!run.is_empty() && run.iter().all(|f| faces.contains(f)))
        }
        Target::WallEdge(face, _) | Target::WallArrow(face) => faces.contains(face),
    }
}

/// The surface faces a gesture on `face` acts on: the selection's of the same
/// kind when `face` is selected, else `face` alone.
fn scope_surfaces(world: &GridWorld, ws: &EditorWorkspace, face: FaceRef) -> Vec<FaceRef> {
    let selected = ws.grid.selection.faces(world);
    if !selected.contains(&face) {
        return vec![face];
    }
    let kind = surface_kind(&face);
    selected
        .into_iter()
        .filter(|f| surface_kind(f) == kind)
        .collect()
}

/// The walls a gesture on `face` acts on.
fn scope_walls(
    world: &GridWorld,
    ws: &EditorWorkspace,
    face: FaceRef,
    run: &[FaceRef],
) -> Vec<FaceRef> {
    let selected = ws.grid.selection.faces(world);
    if selected.contains(&face) {
        return selected
            .into_iter()
            .filter(|f| WallHandle::of(*f).is_some())
            .collect();
    }
    if run.is_empty() {
        vec![face]
    } else {
        run.to_vec()
    }
}

/// What a press on `hit` does, given the selection. `None` for nothing.
pub(super) fn for_press(ws: &EditorWorkspace, hit: &Hit) -> Option<Spec> {
    let world = ws.project.active_scene().world.grid()?;
    Some(match &hit.target {
        Target::Nothing => return None,
        Target::Surface(face) => Spec {
            kind: Kind::Translate,
            faces: scope_surfaces(world, ws, *face),
        },
        Target::SurfaceEdge(edge) => Spec {
            kind: Kind::Tilt(Side::of_edge(edge.edge)),
            faces: scope_surfaces(world, ws, edge.face),
        },
        Target::Corner(corner) => {
            let faces = scope_surfaces(world, ws, corner.face);
            if faces.len() > 1 {
                let at = corner.grid_pos(world)?;
                Spec {
                    kind: Kind::TiltCorner([at[0], at[2]]),
                    faces,
                }
            } else {
                Spec {
                    kind: Kind::Lift(*corner),
                    faces: vec![corner.face],
                }
            }
        }
        Target::Wall(face, run) => Spec {
            kind: Kind::Push,
            faces: scope_walls(world, ws, *face, run),
        },
        Target::WallEdge(face, side) => Spec {
            kind: Kind::Stretch(*side),
            faces: scope_walls(world, ws, *face, &[]),
        },
        Target::WallArrow(face) => Spec {
            kind: Kind::Slide,
            faces: scope_walls(world, ws, *face, &[]),
        },
    })
}

/// The label and the side effect of a spec, for the hint.
pub(super) fn describe(spec: &Spec) -> (String, Option<String>) {
    let many = spec.faces.len() > 1;
    let ceiling = spec.faces.first().and_then(surface_kind) == Some(SurfaceKind::Ceiling);
    let what = if ceiling { "ceiling" } else { "floor" };
    match spec.kind {
        Kind::Translate => (
            if many {
                format!("raise the selected {what}s together")
            } else {
                format!("raise {what}")
            },
            None,
        ),
        Kind::Tilt(_) => (
            if many {
                "slope the selection from the far side".to_string()
            } else {
                "slope from this edge".to_string()
            },
            None,
        ),
        Kind::TiltCorner(_) => ("slope the selection from the far corner".to_string(), None),
        Kind::Lift(_) => ("lift this corner".to_string(), None),
        Kind::Stretch(side) => (
            match side {
                WallSide::Top => "stretch the wall from its top".to_string(),
                WallSide::Bottom => "stretch the wall from its foot".to_string(),
            },
            Some("the floor and the ceiling stay".to_string()),
        ),
        Kind::Slide => (
            "slide the wall, keeping its height".to_string(),
            Some("the floor and the ceiling stay".to_string()),
        ),
        Kind::Push => ("push the room out or in, a sector a step".to_string(), None),
    }
}

/// The plan for a spec. The world may gain leaves (see `refine_corners`).
pub(super) fn build(world: &mut GridWorld, spec: &Spec, weld: bool) -> Result<Built, String> {
    let (label, note) = describe(spec);
    let (plan, axis) = match spec.kind {
        Kind::Push => {
            let elements: Vec<GridElement> =
                spec.faces.iter().copied().map(GridElement::Face).collect();
            let plan = PushPlan::new(world, &elements)?;
            let out = plan.outward;
            let len = ((out[0] * out[0] + out[1] * out[1]) as f32).sqrt().max(1.0);
            let axis = Axis::Horizontal([out[0] as f32 / len, out[1] as f32 / len]);
            (Plan::Push(Box::new(plan)), axis)
        }
        Kind::Stretch(_) | Kind::Slide => {
            let how = match spec.kind {
                Kind::Stretch(side) => WallMove::Edge(side),
                _ => WallMove::Slide,
            };
            let walls: Vec<WallHandle> = spec
                .faces
                .iter()
                .filter_map(|f| WallHandle::of(*f))
                .collect();
            (
                Plan::Walls(WallPlan::new(world, &walls, how)?),
                Axis::Vertical,
            )
        }
        Kind::Lift(corner) => (
            Plan::Heights(HeightsPlan::new(world, &[corner], Profile::Uniform, weld)?),
            Axis::Vertical,
        ),
        Kind::Tilt(_) | Kind::TiltCorner(_) if sdk_tilt(world, spec).is_some() => {
            let plan = sdk_tilt(world, spec).ok_or("Nothing to slope")?;
            (Plan::Tilt(Box::new(plan?)), Axis::Vertical)
        }
        Kind::Translate | Kind::Tilt(_) | Kind::TiltCorner(_) => {
            let corners: Vec<CornerRef> =
                spec.faces.iter().flat_map(|f| f.corners(world)).collect();
            let profile = match spec.kind {
                Kind::Translate => Profile::Uniform,
                kind => tilt_profile(world, &corners, kind)?,
            };
            (
                Plan::Heights(HeightsPlan::new(world, &corners, profile, weld)?),
                Axis::Vertical,
            )
        }
    };
    Ok(Built {
        plan,
        axis,
        label,
        note,
    })
}

/// The SDK tilt of a spec, when it applies: ground floors or ceilings of one
/// room (the SDK tilts the ground span; other storeys use the corner plan).
fn sdk_tilt(world: &GridWorld, spec: &Spec) -> Option<Result<TiltPlan, String>> {
    let first = spec.faces.first()?;
    let kind = surface_kind(first)?;
    let mut cells: BTreeSet<[i32; 2]> = BTreeSet::new();
    for face in &spec.faces {
        match face.target {
            FaceTarget::Surface { kind: k, span: 0 } if k == kind && face.room == first.room => {}
            _ => return None,
        }
        let (lo, hi) = face.leaf.sector_range();
        cells.extend(CellRect { min: lo, max: hi }.cells());
    }
    let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
    for c in &cells {
        lo = [lo[0].min(c[0]), lo[1].min(c[1])];
        hi = [hi[0].max(c[0]), hi[1].max(c[1])];
    }
    let hinge = match spec.kind {
        Kind::Tilt(Side::East) => Hinge::Edge(RampDir::West),
        Kind::Tilt(Side::West) => Hinge::Edge(RampDir::East),
        Kind::Tilt(Side::South) => Hinge::Edge(RampDir::North),
        Kind::Tilt(Side::North) => Hinge::Edge(RampDir::South),
        Kind::TiltCorner(at) => {
            // The corner that stays is opposite the one dragged.
            let west = at[0] <= lo[0] * SECTOR_UNITS;
            let north = at[1] <= lo[1] * SECTOR_UNITS;
            Hinge::Corner(match (west, north) {
                (true, true) => BoxCorner::SouthEast,
                (false, true) => BoxCorner::SouthWest,
                (false, false) => BoxCorner::NorthWest,
                (true, false) => BoxCorner::NorthEast,
            })
        }
        _ => return None,
    };
    let target = match kind {
        SurfaceKind::Floor => TiltTarget::Floor,
        SurfaceKind::Ceiling => TiltTarget::Ceiling,
    };
    Some(TiltPlan::new(world, first.room, cells, target, hinge))
}

/// The hinge and the dragged side of a tilt, from the outline of `corners`.
fn tilt_profile(world: &GridWorld, corners: &[CornerRef], kind: Kind) -> Result<Profile, String> {
    let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
    for corner in corners {
        let Some(p) = corner.grid_pos(world) else {
            continue;
        };
        for (axis, value) in [(0, p[0]), (1, p[2])] {
            lo[axis] = lo[axis].min(value);
            hi[axis] = hi[axis].max(value);
        }
    }
    if lo[0] > hi[0] {
        return Err("Nothing to slope".to_string());
    }
    Ok(match kind {
        Kind::Tilt(Side::East) => Profile::Edge {
            axis: 0,
            hinge: lo[0],
            drag: hi[0],
        },
        Kind::Tilt(Side::West) => Profile::Edge {
            axis: 0,
            hinge: hi[0],
            drag: lo[0],
        },
        Kind::Tilt(Side::South) => Profile::Edge {
            axis: 1,
            hinge: lo[1],
            drag: hi[1],
        },
        Kind::Tilt(Side::North) => Profile::Edge {
            axis: 1,
            hinge: hi[1],
            drag: lo[1],
        },
        Kind::TiltCorner(at) => Profile::Diagonal {
            hinge: [lo[0] + hi[0] - at[0], lo[1] + hi[1] - at[1]],
            corner: at,
        },
        _ => Profile::Uniform,
    })
}

/// The spec of `kind` over what is selected now (the repeat command).
pub(super) fn from_selection(ws: &EditorWorkspace, kind: Kind) -> Result<Spec, String> {
    let world = ws
        .project
        .active_scene()
        .world
        .grid()
        .ok_or_else(|| "No grid world".to_string())?;
    let selected = ws.grid.selection.faces(world);
    let faces: Vec<FaceRef> = match kind {
        Kind::Push | Kind::Slide | Kind::Stretch(_) => selected
            .into_iter()
            .filter(|f| WallHandle::of(*f).is_some())
            .collect(),
        _ => {
            // Floors if there are any, else ceilings.
            let floors: Vec<FaceRef> = selected
                .iter()
                .copied()
                .filter(|f| surface_kind(f) == Some(SurfaceKind::Floor))
                .collect();
            if floors.is_empty() {
                selected
                    .into_iter()
                    .filter(|f| surface_kind(f) == Some(SurfaceKind::Ceiling))
                    .collect()
            } else {
                floors
            }
        }
    };
    if faces.is_empty() {
        return Err("Select what the last action worked on to repeat it".to_string());
    }
    Ok(Spec { kind, faces })
}
