//! What the options bar says about the level verbs: the next step to take.
//!
//! A verb is region, then amount, and the first time that is not obvious from
//! the options alone. The bar therefore leads, for Extrude, Ramp, Stairs and the
//! Upper floor tool, with one line that always names the next thing to do and
//! switches to the live readout while a drag is in flight.

use super::extrude_tool::ExtrudeSurface;
use super::slab_tool::SlabMode;
use super::verb_state::{RegionGesture, RegionSource, VerbDrag};
use super::*;
use psxed_project::grid::edit::ConvertMode;
use psxed_project::grid::WallMode;

impl EditorWorkspace {
    /// The line the options bar leads with for the active tool, if it has one.
    pub(super) fn verb_step_text(&self) -> Option<String> {
        let tool = self.grid.tool;
        if !matches!(
            tool,
            GridTool::Extrude | GridTool::Ramp | GridTool::Stairs | GridTool::Slab
        ) {
            return None;
        }
        let verb = &self.grid.verb;
        if tool == GridTool::Extrude
            && verb.surface == ExtrudeSurface::Wall
            && (!self.grid.selection.is_empty()
                || matches!(self.grid.gesture, Gesture::Marquee { .. }))
        {
            return Some(self.wall_step_text());
        }
        match &self.grid.gesture {
            // A flight being dragged: the steps, the riser and whether it can
            // be walked, as short as the bar needs.
            Gesture::Verb(g) if matches!(g.drag, VerbDrag::Stairs(_)) => {
                if let (VerbDrag::Stairs(d), Some(world)) =
                    (&g.drag, self.project.active_scene().world.grid())
                {
                    return Some(super::stairs_tool::chip(world, d, g.amount));
                }
            }
            // Any other drag in flight: its live readout.
            Gesture::Verb(_) => {
                return Some(if verb.readout.is_empty() {
                    self.status.clone()
                } else {
                    verb.readout.clone()
                });
            }
            Gesture::Region(RegionGesture::Marquee { .. }) => {
                return Some("Release to set the region".to_string())
            }
            Gesture::Region(RegionGesture::Brush { .. }) => {
                return Some("Keep painting, release to set the region".to_string())
            }
            _ => {}
        }
        let has_region = verb.region.is_some();
        // Extrude with nothing picked yet: press what to move.
        if tool == GridTool::Extrude && !has_region && verb.path.is_empty() && verb.lasso.is_empty()
        {
            return Some(self.first_step(tool).to_string());
        }
        // A path or lasso still being clicked out.
        if !verb.path.is_empty() {
            return Some("Click more waypoints, Enter to finish".to_string());
        }
        if !verb.lasso.is_empty() {
            return Some("Click more corners, Enter closes the shape".to_string());
        }
        if !has_region {
            if tool == GridTool::Stairs && verb.source == RegionSource::Path {
                return Some("Click the waypoints of the flight, Enter to finish".to_string());
            }
            return Some(match verb.source {
                RegionSource::Marquee => self.first_step(tool).to_string(),
                other => other.hint().to_string(),
            });
        }
        Some(self.amount_step(tool))
    }

    /// The Extrude tool's line while it works on walls.
    fn wall_step_text(&self) -> String {
        match &self.grid.gesture {
            Gesture::Verb(_) => {
                if self.grid.verb.readout.is_empty() {
                    self.status.clone()
                } else {
                    self.grid.verb.readout.clone()
                }
            }
            Gesture::Marquee { .. } => "Release to pick the wall pieces in the box".to_string(),
            _ => match super::extrude_tool::wall_refusal(self) {
                Some(why) => why,
                None => {
                    "Drag the picked wall pieces along the normal, or press Out and In (= and -)"
                        .to_string()
                }
            },
        }
    }

    /// The first step with a rectangle: where the verb goes.
    fn first_step(&self, tool: GridTool) -> &'static str {
        match tool {
            GridTool::Extrude => {
                "Press a floor, a ceiling or a wall piece and drag: it is picked and moves in one go (Shift adds, Shift-drag sweeps floors)"
            }
            GridTool::Ramp => "Drag a rectangle where the ramp goes",
            GridTool::Stairs => "Drag a rectangle where the stairs go",
            GridTool::Slab => match self.grid.verb.slab.mode {
                SlabMode::Insert => {
                    "Adds a floor above this one, with open space underneath (needs a ceiling or open sky above)"
                }
                SlabMode::Cut => "Drag a rectangle over the upper floor to remove",
                SlabMode::Fill => "Drag a rectangle over the opening to fill",
            },
            _ => "",
        }
    }

    /// The step once a region is picked.
    fn amount_step(&self, tool: GridTool) -> String {
        let verb = &self.grid.verb;
        match tool {
            GridTool::Extrude if verb.surface == ExtrudeSurface::Ceiling => {
                "Drag the picked ceilings up or down, or press another to pick it".to_string()
            }
            GridTool::Extrude => {
                "Drag the picked floors up or down, or press another to pick it".to_string()
            }
            GridTool::Ramp => {
                "Press near the side that stays put and drag up (R turns the side)".to_string()
            }
            GridTool::Stairs if verb.source == RegionSource::Path => {
                "Press on the path and drag up to set the height".to_string()
            }
            GridTool::Stairs => "Press on the region and drag up to set the height".to_string(),
            GridTool::Slab => match self.grid.verb.slab.mode {
                SlabMode::Insert => {
                    "Press on the region and drag up to set the new floor's height".to_string()
                }
                SlabMode::Cut => "Click the region to remove its upper floor".to_string(),
                SlabMode::Fill => "Click the region to fill its opening".to_string(),
            },
            _ => String::new(),
        }
    }
}

/// What a button in the options bar's notice does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BarAction {
    /// Convert the room to derived walls (one undo step).
    ConvertWalls(usize),
    /// Leave the room's walls alone for the rest of the session.
    KeepWalls(usize),
    /// Lay the flight that was too steep for treads as a ramp.
    StairsAsRamp,
}

/// A message that takes the whole options bar, with the buttons that answer
/// it. The bar goes back to the tool's options once it is answered.
pub(super) struct BarNotice {
    pub text: String,
    pub buttons: Vec<(&'static str, BarAction)>,
}

impl EditorWorkspace {
    /// Whether the active tool makes heights, and so wants derived walls to
    /// give them risers.
    fn tool_makes_heights(&self) -> bool {
        matches!(
            self.grid.tool,
            GridTool::Extrude
                | GridTool::Ramp
                | GridTool::Stairs
                | GridTool::Break
                | GridTool::Floor
        )
    }

    /// The room the active tool works in: the region's, else the target room.
    fn verb_room(&self) -> usize {
        match (&self.grid.verb.region, self.grid.tool.is_verb()) {
            (Some(region), true) => region.room,
            _ => self.grid_target_room(),
        }
    }

    /// A notice that should take the bar now, if there is one.
    pub(super) fn bar_notice(&self) -> Option<BarNotice> {
        let world = self.project.active_scene().world.grid()?;
        let verb = &self.grid.verb;
        let in_flight = matches!(self.grid.gesture, Gesture::Verb(_) | Gesture::Region(_));
        // Walls: a room with hand-placed walls gets no risers from new heights.
        if self.tool_makes_heights() && !in_flight {
            let room = self.verb_room();
            let legacy = world
                .rooms
                .get(room)
                .is_some_and(|r| r.wall_mode == WallMode::Authored);
            if legacy && !self.grid.walls_prompt_dismissed.contains(&room) {
                return Some(BarNotice {
                    text: "This room uses hand-placed walls, so new heights won't get walls. Convert to derived walls?"
                        .to_string(),
                    buttons: vec![
                        ("Convert", BarAction::ConvertWalls(room)),
                        ("Not now", BarAction::KeepWalls(room)),
                    ],
                });
            }
        }
        // Stairs: a flight too steep for treads says why and offers a ramp.
        if self.grid.tool == GridTool::Stairs && verb.region.is_some() {
            if let Some(refusal) = verb.stairs_refusal {
                let ru = |v: i32| {
                    psxed_project::grid::cook::RuntimeUnits::new(world.scale).map_or(0, |u| u.rt(v))
                };
                return Some(BarNotice {
                    text: format!(
                        "Too steep for stairs: each step would rise {} ru, the character climbs {} ru.",
                        ru(refusal.riser),
                        ru(refusal.limit)
                    ),
                    buttons: vec![("Make it a ramp (Tab)", BarAction::StairsAsRamp)],
                });
            }
        }
        None
    }

    /// A notice button was pressed.
    pub(super) fn run_bar_action(&mut self, action: BarAction) {
        match action {
            BarAction::ConvertWalls(room) => {
                self.grid_convert_room(room, ConvertMode::PreserveLook);
            }
            BarAction::KeepWalls(room) => {
                self.grid.walls_prompt_dismissed.insert(room);
                self.status =
                    "Walls left as they are: new heights get no walls in that room".into();
            }
            BarAction::StairsAsRamp => super::stairs_tool::make_ramp(self),
        }
    }
}
