//! Aiming the tools at the storey cursor (`docs/grid/multi-storey.md`, 5.1
//! and 5.5).
//!
//! A column can hold several floors, and the tools that make and shape
//! surfaces edit one. The storey cursor says which: for the length of an edit
//! the span open at the cursor stands in as the ground span of every column
//! the edit touches (`psxed_project::grid::edit::focus`), and the columns are
//! put back in order afterwards. Columns of one span are edited as they
//! always were. With no cursor a column of several spans refuses, naming the
//! way out, because a tool that guessed would edit the wrong floor.
//!
//! The cursor also follows the edit: a floor made or moved takes the cursor
//! with it, so the next verb acts on the storey the author is looking at.
//! And in the 3D view a press on a surface of a column with several floors
//! moves the cursor to that surface's floor first, so a click never edits a
//! storey other than the one under the pointer.

use super::*;

use psxed_project::grid::edit::focus::{Focus, FocusRefusal};
use psxed_project::grid::storeys::storey_at;
use psxed_project::grid::units::snap_to;
use psxed_project::grid::CLICK_UNITS;

impl EditorWorkspace {
    /// The height the tools aim at in `room`, relative to the room: the
    /// storey cursor, if one is set.
    pub(crate) fn aim_height(&self, room: usize) -> Option<i32> {
        let y = self.grid.level.y?;
        let world = self.project.active_scene().world.grid()?;
        Some(y - world.rooms.get(room)?.y_offset)
    }

    /// The span of the column at `cell` a Top view pick means: the one open
    /// at the cursor, the ground span without a cursor or in a column of one.
    pub(crate) fn span_at_cursor(&self, room: usize, cell: [i32; 2]) -> u8 {
        let Some(y) = self.aim_height(room) else {
            return 0;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return 0;
        };
        let Some(r) = world.rooms.get(room) else {
            return 0;
        };
        r.local(cell)
            .and_then(|[x, z]| r.sector(x, z))
            .filter(|s| !s.is_single_span())
            .and_then(|s| s.span_at(y, 0))
            .map_or(0, |i| u8::try_from(i).unwrap_or(u8::MAX))
    }

    /// The aim for an edit over `cells` (world sectors) of `room`, or the
    /// sentence that says why the edit cannot go ahead. A room with cells of
    /// several sizes has no spans to aim at and is edited as it always was.
    pub(crate) fn capture_focus(
        &self,
        room: usize,
        cells: impl IntoIterator<Item = [i32; 2]>,
    ) -> Result<Focus, String> {
        let Some(world) = self.project.active_scene().world.grid() else {
            return Ok(Focus::none(room));
        };
        let Some(target) = world.rooms.get(room) else {
            return Ok(Focus::none(room));
        };
        if !target.leaves.is_sectors_only() {
            return Ok(Focus::none(room));
        }
        Focus::capture(world, room, cells, self.aim_height(room))
            .map_err(|refusal| self.describe_refusal(refusal))
    }

    /// A refusal as the status line says it.
    pub(crate) fn describe_refusal(&self, refusal: FocusRefusal) -> String {
        match refusal {
            FocusRefusal::SeveralSpans { cell, count } => format!(
                "Sector [{}, {}] has {count} floors and no storey is set: Alt+Up or Alt+Down picks one, or G and click a surface",
                cell[0], cell[1]
            ),
            FocusRefusal::NoSpan { cell } => format!(
                "Sector [{}, {}] has nothing open at the storey; step the storey (Alt+Up or Alt+Down) or pick a region without it",
                cell[0], cell[1]
            ),
            other => capitalised(other.describe()),
        }
    }

    /// Run `f` with the world as the focus sees it: the aimed spans in the
    /// ground slots. For code that reads the world to begin a gesture
    /// (reference heights, snapshots); anything it writes to the world is
    /// discarded.
    pub(crate) fn with_focused_world<R>(
        &mut self,
        focus: &Focus,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if !focus.is_aimed() {
            return f(self);
        }
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return f(self);
        };
        let view = focus.view(world);
        let real = std::mem::replace(world, view);
        let out = f(self);
        if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
            *world = real;
        }
        out
    }

    /// The cursor follows an edit: with a cursor set, it moves to the floor
    /// the edit made or moved (`floor`, room-relative grid units), snapped to
    /// the storey that holds it, else to a click.
    pub(crate) fn follow_edit(&mut self, room: usize, floor: Option<i32>) {
        let (Some(_), Some(floor)) = (self.grid.level.y, floor) else {
            return;
        };
        let Some(offset) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
            .map(|r| r.y_offset)
        else {
            return;
        };
        let y = floor + offset;
        let target = storey_at(&self.level_storeys(), y, CLICK_UNITS / 4)
            .filter(|s| (s - y).abs() <= CLICK_UNITS / 4)
            .unwrap_or_else(|| snap_to(y, CLICK_UNITS));
        if self.grid.level.y != Some(target) {
            self.set_level_quiet(target);
        }
    }

    /// A press in the 3D view on a surface of a column of several floors puts
    /// the cursor on that surface's floor, so the tool acts on the storey
    /// under the pointer. A surface of a column of one floor changes nothing.
    pub(crate) fn cursor_follows_pick(&mut self, ptr: &GridPointer) {
        let GridView::Persp(rect) = ptr.view else {
            return;
        };
        let Some(hit) = self.pick_face_3d(rect, ptr.pos) else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(span) = hit.face.span_of(world) else {
            return;
        };
        let Some(sector) = hit.face.sector(world) else {
            return;
        };
        if sector.is_single_span() {
            return;
        }
        let offset = world.rooms.get(hit.face.room).map_or(0, |r| r.y_offset);
        // The floor of the span the surface belongs to; a span with no floor
        // (a pit) is level with its ceiling's low point.
        let Some(floor) = span
            .floor
            .as_ref()
            .or(span.ceiling.as_ref())
            .map(|s| s.height_range().0)
        else {
            return;
        };
        let y = floor + offset;
        let target = storey_at(&self.level_storeys(), y, CLICK_UNITS / 4)
            .filter(|s| (s - y).abs() <= CLICK_UNITS / 4)
            .unwrap_or_else(|| snap_to(y, CLICK_UNITS));
        if self.grid.level.y != Some(target) {
            self.set_level(target);
        }
    }
}

fn capitalised(mut text: String) -> String {
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}
