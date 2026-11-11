//! One handle on a wall, whatever made it.
//!
//! The Edit tool never asks how a wall is stored. Every wall gesture goes
//! through the SDK's wall API (`grid::edit::wall_api`): sliding a wall or
//! stretching its top or its foot calls `set_wall_heights`, so the wall keeps
//! the heights the user gave it and the floor and the ceiling do not move. A
//! wall made by the cells and a wall placed by hand give the same faces. A gap
//! that opens above or below the wall comes back as a warning for the hint,
//! never a refusal.
//!
//! The first time a wall's heights are set its address can change (a wall
//! that followed the cells becomes a wall of its own); the face returned by
//! [`WallHandle::move_by`] is the one the selection follows.

use psxed_project::grid::edit::wall_api::{
    clear_wall_property, set_wall_heights, walls_at, WallAddr, WallProp, WallView, WallWarning,
};
use psxed_project::grid::edit::{FaceRef, FaceTarget};
use psxed_project::grid::{GridWorld, CLICK_UNITS};

/// The top or the bottom edge of a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WallSide {
    /// The upper edge.
    Top,
    /// The lower edge.
    Bottom,
}

/// What a wall gesture moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WallMove {
    /// The top or the foot: the wall gets taller or shorter.
    Edge(WallSide),
    /// All four corners: the wall keeps its height and moves.
    Slide,
}

impl WallMove {
    /// The corners of `[bottom a, bottom b, top b, top a]` this moves.
    fn corners(self) -> &'static [usize] {
        match self {
            Self::Edge(WallSide::Bottom) => &[0, 1],
            Self::Edge(WallSide::Top) => &[2, 3],
            Self::Slide => &[0, 1, 2, 3],
        }
    }
}

/// A wall, by the face the picker found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WallHandle {
    face: FaceRef,
}

/// What moving a wall did.
#[derive(Clone, Debug)]
pub(crate) struct Moved {
    /// The wall's face now (its address can change when it is first set).
    pub face: FaceRef,
    /// What the new heights leave open.
    pub warnings: Vec<WallWarning>,
}

/// The face a wall address is.
pub(crate) fn face_of(addr: WallAddr) -> FaceRef {
    FaceRef {
        room: addr.room,
        leaf: addr.leaf,
        target: FaceTarget::Wall {
            edge: addr.side,
            index: addr.index,
        },
    }
}

/// A warning in words, for the hint.
pub(crate) fn warning_text(warning: WallWarning) -> String {
    let (gap, what) = match warning {
        WallWarning::GapAbove(gap) => (gap, "above the wall"),
        WallWarning::GapBelow(gap) => (gap, "under the wall"),
    };
    let clicks = f64::from(gap) / f64::from(CLICK_UNITS);
    format!("a gap of {clicks:.1} clicks opens {what}")
}

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

    /// The wall that took the place of a generated `face` once another wall
    /// of its edge was written (the storeys of an edge are pinned together),
    /// skipping the faces in `taken`.
    pub fn stand_in(
        world: &GridWorld,
        face: FaceRef,
        taken: &std::collections::BTreeSet<FaceRef>,
    ) -> Option<Self> {
        let FaceTarget::Derived { edge, span, .. } = face.target else {
            return None;
        };
        walls_at(world, face.room, face.leaf, edge)
            .into_iter()
            .filter(|v| v.addr.authored && v.addr.span == span)
            .map(|v| face_of(v.addr))
            .find(|f| !taken.contains(f))
            .map(|face| Self { face })
    }

    /// The wall as the wall API sees it now.
    pub fn view(&self, world: &GridWorld) -> Option<WallView> {
        super::super::wall_section::view_of(world, self.face)
    }

    /// Whether the wall can be moved that way, and why not when it cannot.
    pub fn can_move(&self, world: &GridWorld) -> Result<(), String> {
        match self.view(world) {
            Some(_) => Ok(()),
            None => Err("That wall is not there any more or has no four corners".to_string()),
        }
    }

    /// Move the corners `how` names by `amount` grid units, through the wall
    /// API: the wall keeps those heights and the floor and ceiling stay.
    pub fn move_by(
        &self,
        world: &mut GridWorld,
        how: WallMove,
        amount: i32,
    ) -> Result<Moved, String> {
        let view = self
            .view(world)
            .ok_or_else(|| "That wall is not there any more".to_string())?;
        let base = view.heights.value();
        let mut corners = [None; 4];
        for &i in how.corners() {
            corners[i] = Some(base[i] + amount);
        }
        let (addr, warnings) =
            set_wall_heights(world, view.addr, corners).map_err(|r| refusal_text(&r))?;
        Ok(Moved {
            face: face_of(addr),
            warnings,
        })
    }

    /// Back to what the cells give it, through the SDK's wall API.
    pub fn reset_to_automatic(&self, world: &mut GridWorld) -> Result<(), String> {
        let view = self.view(world).ok_or_else(|| "No wall".to_string())?;
        for prop in [
            WallProp::Heights,
            WallProp::Material,
            WallProp::Solid,
            WallProp::Bands,
        ] {
            clear_wall_property(world, view.addr, prop).map_err(|e| refusal_text(&e))?;
        }
        Ok(())
    }
}

fn refusal_text(refusal: &psxed_project::grid::edit::wall_api::WallRefusal) -> String {
    use psxed_project::grid::edit::wall_api::WallRefusal as R;
    match refusal {
        R::NoWall => "That wall is not there any more".to_string(),
        R::NotRectangular => "A wall on a diagonal has no four corners to move".to_string(),
        R::TooManyWalls => "That side already holds as many walls as it may".to_string(),
        R::BigLeaf => "A large cell carries no walls of its own: split it first".to_string(),
        R::Opening(text) => text.clone(),
    }
}
