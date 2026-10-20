//! State of the level verbs (Raise, Ramp, Stairs): the region they act on,
//! how it is made, and the options each verb keeps between gestures.
//!
//! Nothing here is saved with the project.

use psxed_project::grid::edit::focus::Focus;
use psxed_project::grid::edit::verbs::fine::FineDrag;
use psxed_project::grid::edit::verbs::raise::{RaiseDrag, RaiseMode};
use psxed_project::grid::edit::verbs::ramp::{RampDir, RampDrag};
use psxed_project::grid::edit::verbs::spiral::SpiralDrag;
use psxed_project::grid::edit::verbs::stairs::{StairsDrag, StairsParams};
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
    /// Click the corners of a shape; Enter closes it.
    Lasso,
}

impl RegionSource {
    pub(crate) const ALL: [Self; 4] = [Self::Marquee, Self::Brush, Self::Path, Self::Lasso];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Marquee => "Marquee",
            Self::Brush => "Brush",
            Self::Path => "Path",
            Self::Lasso => "Lasso",
        }
    }

    pub(crate) const fn hint(self) -> &'static str {
        match self {
            Self::Marquee => "Drag a rectangle of sectors",
            Self::Brush => "Paint sectors; [ and ] change the radius",
            Self::Path => "Click waypoints, Enter finishes; [ and ] change the width",
            Self::Lasso => {
                "Click the corners of a shape, Enter closes it; Shift snaps a segment to 45 degrees"
            }
        }
    }
}

/// A Stairs flight the options bar refused to lay, kept until the next try so
/// the bar can say why and offer a ramp in its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StairsRefusal {
    /// The riser the flight would need, grid units.
    pub riser: i32,
    /// The tallest riser the character steps up, grid units.
    pub limit: i32,
    /// The rise that was asked for, grid units.
    pub amount: i32,
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
    /// Extrude on a ceiling: the region's ceilings move.
    Ceiling(psxed_project::grid::edit::verbs::extrude::CeilingDrag),
    /// Extrude on wall pieces: the amount is sectors along the normal.
    Wall(Box<super::extrude_tool::WallLive>),
    Ramp(RampDrag),
    Stairs(StairsDrag),
    /// Spiral stairs round a ring of sectors.
    Spiral(SpiralDrag),
    /// Raise, Ramp or Stairs on cells smaller than a sector, or in a room
    /// with leaves of several sizes: the library's fine verbs through the
    /// lens (what the MCP's `size` runs).
    Fine(Box<FineDrag>),
    /// Break: the amount is the intensity, 0 to 100.
    Break(Box<super::break_tool::BreakLive>),
    /// Rough: the amount is the level, 0 to 3.
    Rough(Box<super::rough_tool::RoughLive>),
    /// Slab: the amount is the height of an inserted slab over the level.
    Slab(Box<super::slab_tool::SlabLive>),
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
    /// The largest rise the motor can walk, for a Ramp.
    pub max_rise: Option<i32>,
    /// The span of each column the verb was aimed at when the gesture began.
    /// Fixed for the whole gesture, so a floor that moves out from under the
    /// cursor is still the one being dragged.
    pub focus: Focus,
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
    /// Corners of a lasso being drawn, in sector coordinates.
    pub lasso: Vec<[f64; 2]>,
    /// The room the lasso's first corner was in.
    pub lasso_room: usize,
    /// The lasso adds to the region instead of replacing it (Shift on the
    /// first corner).
    pub lasso_add: bool,
    /// Where the next lasso corner would land, for the rubber band.
    pub lasso_cursor: Option<[f64; 2]>,
    /// What the Extrude tool moves.
    pub surface: super::extrude_tool::ExtrudeSurface,
    /// Sectors the Out and In buttons (and `=` and `-`) push a wall.
    pub wall_steps: i32,
    /// Ceilings move with their floors.
    pub follow_ceiling: bool,
    /// Offset or Level; `None` picks from the region (Level when it is flat).
    pub raise_mode: Option<RaiseMode>,
    /// The Level reference of the gesture in flight, room-relative.
    pub reference: i32,
    /// Typed amount in clicks, for the option row.
    pub typed_clicks: f32,
    /// The size of the last Raise or Lower, grid units (what Cut corners
    /// drops a corner by; a click when there has been none).
    pub last_raise: i32,
    /// Ramp and Stairs: the side that keeps its height (a flight's bottom),
    /// once R has set it; `None` follows the pointer.
    pub ramp_from: Option<RampDir>,
    /// Ramp: hold the rise to the steepest slope the motor stands on.
    pub ramp_clamp: bool,
    /// Ramp: the side a press at the pointer would keep.
    pub ramp_hover: Option<RampDir>,
    /// Stairs: the riser option and ramp switch (width, ceilings and the
    /// wall material come from the shared options).
    pub stairs: StairsParams,
    /// Stairs: the waypoints of the finished path (the Path source).
    pub stairs_path: Vec<[i32; 2]>,
    /// Stairs: how the flight is placed, a rectangle (the default) or a path.
    pub stairs_source: RegionSource,
    /// Stairs: the sector the last press or typed apply was on, which picks
    /// the bottom side of a rectangle when R has not.
    pub stairs_press: [i32; 2],
    /// Stairs: the last flight that was too steep for treads.
    pub stairs_refusal: Option<StairsRefusal>,
    /// Spiral: the storeys to climb.
    pub spiral: super::spiral_tool::SpiralOptions,
    /// Break: its options and the seed of the next break.
    pub brk: super::break_tool::BreakOptions,
    /// Rough: its options.
    pub rough: super::rough_tool::RoughOptions,
    /// Slab: its mode and typed height.
    pub slab: super::slab_tool::SlabOptions,
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
            lasso: Vec::new(),
            lasso_room: 0,
            lasso_add: false,
            lasso_cursor: None,
            surface: super::extrude_tool::ExtrudeSurface::Floor,
            wall_steps: 1,
            follow_ceiling: false,
            raise_mode: None,
            reference: 0,
            typed_clicks: 1.0,
            last_raise: 0,
            ramp_from: None,
            ramp_clamp: false,
            ramp_hover: None,
            stairs: StairsParams::default(),
            stairs_path: Vec::new(),
            stairs_source: RegionSource::Marquee,
            stairs_press: [0, 0],
            stairs_refusal: None,
            spiral: super::spiral_tool::SpiralOptions::default(),
            brk: super::break_tool::BreakOptions::default(),
            rough: super::rough_tool::RoughOptions::default(),
            slab: super::slab_tool::SlabOptions::default(),
            readout: String::new(),
        }
    }
}

impl VerbState {
    /// Forget the region and any path in progress.
    pub(crate) fn clear(&mut self) {
        self.region = None;
        self.path.clear();
        self.lasso.clear();
        self.lasso_cursor = None;
        self.preview.clear();
        self.ramp_hover = None;
        self.ramp_from = None;
        self.stairs_path.clear();
        self.stairs_refusal = None;
        self.readout.clear();
    }
}
