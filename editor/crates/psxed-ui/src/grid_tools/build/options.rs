//! The options of the Build tools: what the options bar edits and the
//! gestures read. Nothing here is saved with the project.

use std::collections::BTreeSet;

use psxed_project::grid::edit::verbs::capture::BayTemplate;
use psxed_project::grid::edit::verbs::style::Style;
use psxed_project::{DoorDirection, DoorSide, SwitchBehavior};

use super::super::GridTool;

/// The Build tools that run through the shared gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BuildKind {
    Room,
    Corridor,
    Opening,
    Arcade,
    Colonnade,
    Bridge,
    Building,
    Cathedral,
    Terrain,
    Ruin,
    Door,
    Lever,
    Checkpoint,
}

impl BuildKind {
    /// The kind a tool is, if it is one of these.
    pub(crate) const fn of(tool: GridTool) -> Option<Self> {
        Some(match tool {
            GridTool::Room => Self::Room,
            GridTool::Corridor => Self::Corridor,
            GridTool::Opening => Self::Opening,
            GridTool::Arcade => Self::Arcade,
            GridTool::Colonnade => Self::Colonnade,
            GridTool::Bridge => Self::Bridge,
            GridTool::Building => Self::Building,
            GridTool::Cathedral => Self::Cathedral,
            GridTool::Terrain => Self::Terrain,
            GridTool::Ruin => Self::Ruin,
            GridTool::Door => Self::Door,
            GridTool::Lever => Self::Lever,
            GridTool::Checkpoint => Self::Checkpoint,
            _ => return None,
        })
    }

    /// The vertical amount this kind reads from a Shift-drag or typed
    /// number: its name, and whether it counts clicks (else whole numbers).
    pub(crate) const fn amount(self) -> Option<(&'static str, bool)> {
        match self {
            Self::Room => Some(("height", true)),
            Self::Opening | Self::Door => Some(("lintel", true)),
            Self::Corridor => Some(("width", false)),
            Self::Arcade | Self::Colonnade => Some(("bays", false)),
            Self::Building => Some(("storeys", false)),
            Self::Cathedral => Some(("bay size", false)),
            Self::Bridge => Some(("width", true)),
            Self::Terrain => Some(("amplitude", true)),
            Self::Ruin => Some(("intensity", true)),
            Self::Lever | Self::Checkpoint => None,
        }
    }
}

/// A style as the picker names it and the verbs take it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StyleResolved {
    Plain,
    Style(Style),
    Bay(BayTemplate),
}

/// One row of the style picker.
#[derive(Clone, Debug)]
pub(crate) struct StyleEntry {
    pub name: String,
    pub what: StyleResolved,
    /// Stored with the project (else built in).
    pub project: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RoomShape {
    Rectangle,
    Octagon,
    Lasso,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpeningHead {
    Rectangular,
    Pointed,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArcProfile {
    Round,
    Pointed,
    Segmental,
    Flat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lock {
    Open,
    Locked,
    /// Refuses from this side.
    OneWay(DoorSide),
}

#[derive(Clone, Debug)]
pub(crate) struct RoomOpts {
    pub shape: RoomShape,
    pub into_this: bool,
    /// Floor to ceiling, grid units.
    pub height: i32,
    pub cut: i32,
    pub style: StyleResolved,
}

#[derive(Clone, Debug)]
pub(crate) struct CorridorOpts {
    pub width: i32,
    pub headroom: i32,
    pub stairs: bool,
    pub into_this: bool,
    pub style: StyleResolved,
}

#[derive(Clone, Debug)]
pub(crate) struct OpeningOpts {
    pub window: bool,
    pub head: OpeningHead,
    pub top: i32,
    pub sill: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct ArcadeOpts {
    pub bays: i32,
    /// 0 fills the pitch.
    pub span: i32,
    pub pier: i32,
    pub spring: i32,
    /// 0 takes the profile's.
    pub rise: i32,
    pub profile: ArcProfile,
    pub perch: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BridgeOpts {
    pub width: i32,
    pub thickness: i32,
    /// 0 for none.
    pub parapet: i32,
    /// 0 for none.
    pub pier_spacing: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct BuildingOpts {
    pub storeys: i32,
    pub storey_height: i32,
    pub door: bool,
    pub window_every: i32,
    pub window_head: OpeningHead,
    pub stair: bool,
    pub parapet: bool,
    pub seed: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct CathedralOpts {
    pub bay: i32,
    pub aisle_width: i32,
    pub nave_height: i32,
    pub transept: bool,
    pub apse: bool,
    pub west_door: bool,
    pub style: StyleResolved,
}

#[derive(Clone, Debug)]
pub(crate) struct TerrainOpts {
    pub seed: u32,
    pub amplitude: i32,
    pub octaves: u32,
    pub wavelength: i32,
    pub slope_x: i32,
    pub slope_z: i32,
    pub keep_walkable: bool,
    pub blend: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct RuinOpts {
    pub seed: u32,
    pub intensity: u32,
    pub whole_room: bool,
    pub keep: BTreeSet<[i32; 2]>,
    /// The brush is painting the kept path.
    pub painting: bool,
    pub crumble: bool,
    pub sink: bool,
    pub floor_bays: bool,
    pub roof_bays: bool,
    pub breaches: bool,
    pub fallen_arches: bool,
    pub rubble: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DoorOpts {
    pub direction: DoorDirection,
    /// Editor units; 0 slides the whole doorway.
    pub distance: i32,
    pub ticks: u16,
    pub lock: Lock,
    pub refusal: String,
    pub persistent: bool,
    pub height: i32,
    pub head: OpeningHead,
}

#[derive(Clone, Debug)]
pub(crate) struct LeverOpts {
    pub behavior: SwitchBehavior,
    pub unlocks: bool,
    pub persistent: bool,
}

/// Every Build tool's options.
#[derive(Clone, Debug)]
pub(crate) struct Options {
    pub room: RoomOpts,
    pub corridor: CorridorOpts,
    pub opening: OpeningOpts,
    pub arcade: ArcadeOpts,
    pub bridge: BridgeOpts,
    pub building: BuildingOpts,
    pub cathedral: CathedralOpts,
    pub terrain: TerrainOpts,
    pub ruin: RuinOpts,
    pub door: DoorOpts,
    pub lever: LeverOpts,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            room: RoomOpts {
                shape: RoomShape::Rectangle,
                into_this: false,
                height: 3072,
                cut: 2,
                style: StyleResolved::Plain,
            },
            corridor: CorridorOpts {
                width: 1,
                headroom: 2048,
                stairs: false,
                into_this: false,
                style: StyleResolved::Plain,
            },
            opening: OpeningOpts {
                window: false,
                head: OpeningHead::Rectangular,
                top: 2048,
                sill: 768,
            },
            arcade: ArcadeOpts {
                bays: 4,
                span: 0,
                pier: 256,
                spring: 1536,
                rise: 0,
                profile: ArcProfile::Round,
                perch: false,
            },
            bridge: BridgeOpts {
                width: 1024,
                thickness: 256,
                parapet: 768,
                pier_spacing: 0,
            },
            building: BuildingOpts {
                storeys: 2,
                storey_height: 3072,
                door: true,
                window_every: 2,
                window_head: OpeningHead::Rectangular,
                stair: true,
                parapet: true,
                seed: 0,
            },
            cathedral: CathedralOpts {
                bay: 2,
                aisle_width: 2,
                nave_height: 9216,
                transept: false,
                apse: true,
                west_door: true,
                style: StyleResolved::Plain,
            },
            terrain: TerrainOpts {
                seed: 1,
                amplitude: 1,
                octaves: 2,
                wavelength: 4,
                slope_x: 0,
                slope_z: 0,
                keep_walkable: true,
                blend: 2,
            },
            ruin: RuinOpts {
                seed: 1,
                intensity: 40,
                whole_room: false,
                keep: BTreeSet::new(),
                painting: false,
                crumble: true,
                sink: true,
                floor_bays: true,
                roof_bays: true,
                breaches: true,
                fallen_arches: true,
                rubble: true,
            },
            door: DoorOpts {
                direction: DoorDirection::Up,
                distance: 0,
                ticks: 60,
                lock: Lock::Open,
                refusal: String::new(),
                persistent: true,
                height: 2048,
                head: OpeningHead::Rectangular,
            },
            lever: LeverOpts {
                behavior: SwitchBehavior::Toggle,
                unlocks: false,
                persistent: true,
            },
        }
    }
}

impl Options {
    /// Set the number a typed amount or a Shift-drag stands for.
    pub(crate) fn set_amount(&mut self, kind: BuildKind, value: i32) {
        match kind {
            BuildKind::Room => self.room.height = value.max(1024),
            BuildKind::Opening => self.opening.top = value.max(256),
            BuildKind::Door => self.door.height = value.max(256),
            BuildKind::Corridor => self.corridor.width = value.clamp(1, 8),
            BuildKind::Arcade | BuildKind::Colonnade => self.arcade.bays = value.clamp(1, 32),
            BuildKind::Building => self.building.storeys = value.clamp(1, 4),
            BuildKind::Cathedral => self.cathedral.bay = value.clamp(1, 4),
            BuildKind::Bridge => self.bridge.width = value.max(256),
            BuildKind::Terrain => self.terrain.amplitude = (value / 256).max(0),
            BuildKind::Ruin => self.ruin.intensity = (value / 256).clamp(0, 100) as u32,
            BuildKind::Lever | BuildKind::Checkpoint => {}
        }
    }
}
