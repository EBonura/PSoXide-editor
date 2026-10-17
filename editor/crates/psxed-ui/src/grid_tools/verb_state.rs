//! State of the level verbs (Raise, Ramp, Stairs): the region they act on,
//! how it is made, and the options each verb keeps between gestures.
//!
//! Nothing here is saved with the project.

use psxed_project::grid::edit::verbs::raise::{RaiseDrag, RaiseMode};
use psxed_project::grid::edit::Region;

use super::*;

/// How a region is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegionSource {
    /// Drag a rectangle of sectors.
    Marquee,
    /// Paint sectors with a disc.
    Brush,
    /// Click waypoints; Enter finishes.
    Path,
}

impl RegionSource {
    pub(crate) const ALL: [Self; 3] = [Self::Marquee, Self::Brush, Self::Path];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Marquee => "Marquee",
            Self::Brush => "Brush",
            Self::Path => "Path",
        }
    }

    pub(crate) const fn hint(self) -> &'static str {
        match self {
            Self::Marquee => "Drag a rectangle of sectors",
            Self::Brush => "Paint sectors; [ and ] change the radius",
            Self::Path => "Click waypoints, Enter finishes; [ and ] change the width",
        }
    }
}

/// A region being made by the pointer.
#[derive(Clone, Debug)]
pub(crate) enum RegionGesture {
    /// A rectangle from `anchor` to `current`.
    Marquee {
        room: usize,
        anchor: [i32; 2],
        current: [i32; 2],
        add: bool,
    },
    /// A brush stroke that has painted `cells` so far.
    Brush { room: usize, cells: Region },
}

/// The verb drag in flight.
#[derive(Clone, Debug)]
pub(crate) enum VerbDrag {
    Raise(RaiseDrag),
}

/// A verb being dragged: the amount follows the pointer's vertical travel.
#[derive(Clone, Debug)]
pub(crate) struct VerbGesture {
    pub drag: VerbDrag,
    pub press_y: f32,
    pub pixels_per_unit: f32,
    /// Grid units, snapped.
    pub amount: i32,
    /// The document before the drag, recorded for undo at the first change.
    pub before: Option<Box<ProjectDocument>>,
}

/// Options and region of the level verbs.
#[derive(Clone, Debug)]
pub(crate) struct VerbState {
    pub source: RegionSource,
    /// Radius of the brush, in sectors.
    pub brush_radius: i32,
    /// Width of a path, in sectors.
    pub path_width: i32,
    /// The region the next verb acts on. Stays after a verb until Esc or a
    /// new pick, so verbs chain.
    pub region: Option<Region>,
    /// Waypoints of a path being placed.
    pub path: Vec<[i32; 2]>,
    /// Cells under the pointer for the brush and path previews.
    pub preview: Vec<[i32; 2]>,
    /// Ceilings move with their floors.
    pub follow_ceiling: bool,
    /// Offset or Level; `None` picks from the region (Level when it is flat).
    pub raise_mode: Option<RaiseMode>,
    /// The Level reference of the gesture in flight, room-relative.
    pub reference: i32,
    /// Typed amount in clicks, for the option row.
    pub typed_clicks: f32,
    /// The last line a verb reported (amount, counts), for the Inspector.
    pub readout: String,
}

impl Default for VerbState {
    fn default() -> Self {
        Self {
            source: RegionSource::Marquee,
            brush_radius: 1,
            path_width: 1,
            region: None,
            path: Vec::new(),
            preview: Vec::new(),
            follow_ceiling: true,
            raise_mode: None,
            reference: 0,
            typed_clicks: 1.0,
            readout: String::new(),
        }
    }
}

impl VerbState {
    /// Forget the region and any path in progress.
    pub(crate) fn clear(&mut self) {
        self.region = None;
        self.path.clear();
        self.preview.clear();
        self.readout.clear();
    }
}
