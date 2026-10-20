//! The storey cursor, the go-to-level pick and the cutaway
//! (`docs/grid/multi-storey.md`, 5.1 and 5.3).
//!
//! The cursor is the height the author is working at, in grid units with room
//! offsets included, and is a view setting: it is never saved with the project
//! and no tool edits through it implicitly. Alt+Up and Alt+Down step it
//! through the storeys the document derives (`storeys`), `G` then a click sets
//! it to the surface under the pointer, and `Z` switches the cutaway.
//!
//! The cutaway hides what lies above the cursor's storey: every face whose
//! lowest point is at or above the floor of the next storey up. The 3D preview
//! (which only sees the project) reads the plane the workspace publishes, and
//! picking reads the same function, so what is drawn is what can be picked.
//! A face that straddles the plane is kept whole. Without a cursor, or with
//! no storey above it, nothing is hidden, so a level with one storey looks as
//! it always did.

use std::cell::RefCell;

use super::*;

use psxed_project::grid::storeys::{storey_above, storey_at, storey_below, storeys, Storey};
use psxed_project::grid::units::snap_to;
use psxed_project::grid::CLICK_UNITS;

/// How the 3D view treats geometry above the storey cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Cutaway {
    /// Hide the storeys above the cursor.
    #[default]
    Cut,
    /// Show everything.
    Off,
}

impl Cutaway {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Cut => "cut",
            Self::Off => "off",
        }
    }

    const fn toggled(self) -> Self {
        match self {
            Self::Cut => Self::Off,
            Self::Off => Self::Cut,
        }
    }
}

/// The storey cursor and what hangs off it.
#[derive(Default)]
pub(crate) struct LevelState {
    /// The working height, grid units, room offsets included. `None` until
    /// the author sets it.
    pub y: Option<i32>,
    /// How the 3D view treats what lies above `y`.
    pub cutaway: Cutaway,
    /// A level being renamed in the Levels panel: its floor height and the
    /// text typed so far.
    pub renaming: Option<(i32, String)>,
    /// `G` was pressed: the next click in a view sets the cursor.
    pub picking: bool,
    /// The derived storeys, per document revision.
    storeys: RefCell<Option<(u64, Vec<Storey>)>>,
}

impl EditorWorkspace {
    /// The storeys of the document, rebuilt only when the document changed.
    pub(crate) fn level_storeys(&self) -> Vec<Storey> {
        let Some(world) = self.project.active_scene().world.grid() else {
            return Vec::new();
        };
        let mut slot = self.grid.level.storeys.borrow_mut();
        match slot.as_ref() {
            Some((rev, list)) if *rev == self.grid.rev => list.clone(),
            _ => {
                let list = storeys(world);
                *slot = Some((self.grid.rev, list.clone()));
                list
            }
        }
    }

    /// The editor-unit height from which the cutaway hides geometry, when it
    /// hides any: the floor of the next storey above the cursor.
    pub(crate) fn cutaway_plane(&self) -> Option<i32> {
        if self.grid.level.cutaway == Cutaway::Off {
            return None;
        }
        let y = self.grid.level.y?;
        let world = self.project.active_scene().world.grid()?;
        let above = storey_above(&self.level_storeys(), y)?;
        Some(world.scale.to_editor(above))
    }

    /// Put the cursor at `y` (grid units, room offsets included), and the
    /// Floor tool's height with it: floors are made at the level.
    pub(crate) fn set_level_quiet(&mut self, y: i32) {
        self.grid.level.y = Some(y);
        let room = self.grid_target_room();
        let offset = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
            .map(|r| r.y_offset);
        if let Some(offset) = offset {
            self.grid.floor_height = y - offset;
        }
    }

    /// [`set_level_quiet`](Self::set_level_quiet), and the chip's text in
    /// the status line.
    pub(crate) fn set_level(&mut self, y: i32) {
        self.set_level_quiet(y);
        self.status = self.level_text().unwrap_or_default();
    }

    /// Alt+Up and Alt+Down: the cursor to the next storey up or down. The
    /// first press with no cursor puts it on the lowest storey.
    pub(crate) fn step_level(&mut self, up: bool) {
        let list = self.level_storeys();
        let target = match self.grid.level.y {
            None => list.first().map(|s| s.height),
            Some(y) if up => storey_above(&list, y),
            Some(y) => storey_below(&list, y),
        };
        match target {
            Some(y) => self.set_level(y),
            None if list.is_empty() => {
                self.status = format!(
                    "No storeys yet: a floor height needs {} cells",
                    psxed_project::grid::storeys::STOREY_MIN_CELLS
                );
            }
            None => {
                self.status = format!("No storey {}", if up { "above" } else { "below" });
            }
        }
    }

    /// `Z`: the cutaway on or off.
    pub(crate) fn cycle_cutaway(&mut self) {
        let next = self.grid.level.cutaway.toggled();
        self.grid.level.cutaway = next;
        self.status = format!("Cutaway {}", next.label());
    }

    /// `G`: the next click sets the cursor to the surface under it.
    pub(crate) fn begin_level_pick(&mut self) {
        self.grid.level.picking = true;
        self.status = "Go to level: click a surface (Esc cancels)".to_string();
    }

    /// Esc during the pick. Returns whether there was one.
    pub(crate) fn cancel_level_pick(&mut self) -> bool {
        let was = std::mem::take(&mut self.grid.level.picking);
        if was {
            self.status = "Cancelled".to_string();
        }
        was
    }

    /// The text of the Level chip: which storey, and the height in clicks and
    /// runtime units. `None` without a cursor.
    pub(crate) fn level_text(&self) -> Option<String> {
        let y = self.grid.level.y?;
        let world = self.project.active_scene().world.grid()?;
        let list = self.level_storeys();
        let name = if list.iter().any(|s| s.height == y) {
            super::levels_panel::level_name(world, &list, y)
        } else {
            "Storey -".to_string()
        };
        let ru = psxed_project::grid::cook::RuntimeUnits::new(world.scale)
            .map(|u| format!(" ({:+} ru)", u.rt(y)))
            .unwrap_or_default();
        Some(format!(
            "{name}   y {:+.2} clicks{ru}   cutaway {}",
            y as f32 / CLICK_UNITS as f32,
            self.grid.level.cutaway.label()
        ))
    }

    /// The cursor height a click at `ptr` selects, grid units. In the 3D
    /// view the surface under the pointer; in the Top view the topmost floor
    /// of the column under it. Snapped to the derived storey that holds it
    /// when there is one, else to a click.
    fn level_under(&self, ptr: &GridPointer) -> Option<i32> {
        let world = self.project.active_scene().world.grid()?;
        let raw = match ptr.view {
            GridView::Persp(rect) => {
                let hit = self.pick_face_3d(rect, ptr.pos)?;
                world.scale.to_grid(hit.point[1].round() as i32)
            }
            GridView::Top(transform) => {
                let [x, z] = transform.screen_to_world(ptr.pos);
                let cell = super::view::cell_at_editor(world.scale, x, z);
                let room = self.top_room_at(cell)?;
                let r = world.rooms.get(room)?;
                let sector = r.local(cell).and_then(|[x, z]| r.sector(x, z))?;
                let top = sector
                    .spans()
                    .iter()
                    .rev()
                    .find_map(|s| s.floor.as_ref().map(|f| f.height_range().0))?;
                top + r.y_offset
            }
        };
        // A surface within a quarter click above a storey floor belongs to it.
        Some(
            storey_at(&self.level_storeys(), raw, CLICK_UNITS / 4)
                .unwrap_or_else(|| snap_to(raw, CLICK_UNITS)),
        )
    }

    /// The click that follows `G`.
    pub(super) fn level_pick_click(&mut self, ptr: &GridPointer) {
        self.grid.level.picking = false;
        match self.level_under(ptr) {
            Some(y) => self.set_level(y),
            None => self.status = "Nothing to go to there".to_string(),
        }
    }

    /// The chip over the viewport, top centre of `rect` (the Top view keeps
    /// its own readout in the corner).
    pub(super) fn draw_level_chip(&self, painter: &egui::Painter, rect: Rect) {
        if let Some(text) = self.level_text() {
            super::overlay::label_chip(
                painter,
                rect.center_top() + Vec2::new(0.0, 10.0),
                Align2::CENTER_TOP,
                &text,
            );
        }
    }
}
