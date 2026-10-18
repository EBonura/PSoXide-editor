//! Clicking again on the same spot in the Top view takes the next thing
//! under the pointer: a free plane first (the topmost one, then the ones
//! beneath it), then the floor of the sector, then its ceiling.
//!
//! The Top view looks straight down, so a plane lies over the sector under
//! it and a ceiling over its floor. Alt used to click through; it now means
//! the fine snap and nothing else, so the way down is another click. A click
//! counts when it lands within a few pixels of the previous click with the
//! same tool, and a drag or another spot starts again at the top. The
//! pointer carries the count (`GridPointer::cycle`) so every picker agrees.

use super::events::Ev;
use super::*;

/// Pixels the pointer may wander between two clicks of one cycle.
const SAME_SPOT: f32 = 4.0;

/// Where the cycle stands. Not saved.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CycleState {
    /// Where the last click landed, when the next click may continue it.
    at: Option<Pos2>,
    /// The tool that click was made with.
    tool: Option<GridTool>,
    /// Which thing that click took, counting from the top.
    step: usize,
}

impl CycleState {
    fn continues(&self, pos: Pos2, tool: GridTool) -> bool {
        self.tool == Some(tool) && self.at.is_some_and(|at| at.distance(pos) <= SAME_SPOT)
    }

    /// The cycle starts again from the top.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

impl EditorWorkspace {
    /// Which thing under the pointer `ev` takes, 0 being the topmost, and
    /// advance the cycle for a click. Hover previews the thing the next
    /// click would take; a press (a drag starting) takes the topmost and
    /// ends the cycle.
    pub(super) fn cycle_for(&mut self, ev: Ev, ptr: &GridPointer) -> usize {
        if !ptr.is_top() {
            return 0;
        }
        let tool = self.grid.tool;
        let state = &mut self.grid.cycle;
        match ev {
            Ev::Click => {
                state.step = if state.continues(ptr.pos, tool) {
                    state.step + 1
                } else {
                    0
                };
                state.at = Some(ptr.pos);
                state.tool = Some(tool);
                state.step
            }
            Ev::Hover => {
                if state.continues(ptr.pos, tool) {
                    state.step + 1
                } else {
                    0
                }
            }
            Ev::Press => {
                state.reset();
                0
            }
            Ev::Drag | Ev::Release => 0,
        }
    }

    /// The planes under the Top view pointer, topmost first, and how many
    /// sector layers lie beneath them (floor, then ceiling).
    pub(crate) fn top_stack(&self, ptr: &GridPointer) -> (Vec<PlaneId>, usize) {
        let GridView::Top(transform) = ptr.view else {
            return (Vec::new(), 0);
        };
        let planes = self.top_planes_at(transform, ptr.pos, 6.0);
        let layers = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|world| {
                let [wx, wz] = transform.screen_to_world(ptr.pos);
                let cell = view::cell_at_editor(world.scale, wx, wz);
                let room = world.rooms.get(self.top_room_at(cell)?)?;
                let [x, z] = room.local(cell)?;
                let sector = room.sector(x, z)?;
                Some(
                    usize::from(sector.floor().is_some()) + usize::from(sector.ceiling().is_some()),
                )
            })
            .unwrap_or(0);
        (planes, layers)
    }

    /// What the pointer's cycle step takes in the Top view: a plane, or a
    /// sector layer (0 the floor, 1 the ceiling).
    pub(crate) fn top_choice(&self, ptr: &GridPointer) -> TopChoice {
        let (planes, layers) = self.top_stack(ptr);
        let total = planes.len() + layers;
        if total == 0 {
            return TopChoice::Layer(0);
        }
        let choice = ptr.cycle % total;
        match planes.get(choice) {
            Some(id) => TopChoice::Plane(*id),
            None => TopChoice::Layer(choice - planes.len()),
        }
    }
}

/// One thing under the Top view pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopChoice {
    /// A free plane.
    Plane(PlaneId),
    /// A sector layer: 0 the floor, 1 the ceiling.
    Layer(usize),
}
