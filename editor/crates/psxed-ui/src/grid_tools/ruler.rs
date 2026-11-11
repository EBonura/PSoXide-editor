//! A ruler for level design: sizes told in sectors and in player heights,
//! and an optional player-sized ghost at the pointer, so headroom, doorways
//! and steps are judged by eye.
//!
//! The player is 1024 editor units tall. What the player steps up is the
//! shared walk limit (`psx_engine::WalkLimits`), the same number the
//! walkability overlay and the cook use.

use super::*;

use psxed_project::grid::cook::RuntimeUnits;
use psxed_project::grid::units::{CLICK_UNITS, SECTOR_UNITS};
use psxed_project::grid::GridWorld;

/// The player's height, editor units.
pub(crate) const PLAYER_HEIGHT_EDITOR: f32 = 1024.0;
/// The player's width, editor units.
pub(crate) const PLAYER_WIDTH_EDITOR: f32 = 188.0;

/// `grid` units as player heights.
fn player_heights(world: &GridWorld, grid: i32) -> f32 {
    world.scale.to_editor(grid.abs()) as f32 / PLAYER_HEIGHT_EDITOR
}

/// The tallest riser the player steps up, in grid units, by the shared walk
/// limit. `None` when the world's scale cannot be cooked.
pub(crate) fn step_up_grid(world: &GridWorld) -> Option<i32> {
    let units = RuntimeUnits::new(world.scale).ok()?;
    let per_click = units.rt(CLICK_UNITS).max(1);
    Some(psx_engine::WalkLimits::DEFAULT.step_up * CLICK_UNITS / per_click)
}

/// "3 sectors, 3.8 player heights" for a length of `grid` units.
pub(crate) fn length_text(world: &GridWorld, grid: i32) -> String {
    let sectors = grid.abs() as f32 / SECTOR_UNITS as f32;
    format!(
        "{} {}, {:.1} player heights",
        trim(sectors),
        if (sectors - 1.0).abs() < 0.05 {
            "sector"
        } else {
            "sectors"
        },
        player_heights(world, grid)
    )
}

/// What a vertical change of `grid` units means to the player: its size and
/// whether a body steps up it.
pub(crate) fn rise_text(world: &GridWorld, grid: i32) -> String {
    let steps = match step_up_grid(world) {
        Some(limit) if grid > 0 && grid <= limit => ", the player steps up it",
        Some(_) if grid > 0 => ", too tall to step up",
        _ => "",
    };
    format!(
        "{} sector, {:.1} player heights{steps}",
        trim(grid as f32 / SECTOR_UNITS as f32),
        player_heights(world, grid)
    )
}

/// The footprint of a rectangle of `w` by `d` sectors, with its longer side
/// in player heights.
pub(crate) fn footprint_text(world: &GridWorld, w: i32, d: i32) -> String {
    let longer = w.max(d) * SECTOR_UNITS;
    format!(
        "{w} x {d} sectors, the long side {:.1} player heights",
        player_heights(world, longer)
    )
}

/// A number with one decimal only when it has one.
fn trim(value: f32) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").map_or(text.clone(), str::to_string)
}

impl EditorWorkspace {
    /// The player-sized ghost at the sector under the pointer: a ring at the
    /// feet and one at the head, joined by four lines, 1024 editor units
    /// tall. Drawn only when the overlay is on.
    pub(super) fn draw_player_ghost_3d(&self, painter: &egui::Painter, rect: Rect) {
        if !self.grid.overlays.player_ghost {
            return;
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let GridHover::Cell { room, cell, height } = self.grid.hover else {
            return;
        };
        let Some(r) = world.rooms.get(room) else {
            return;
        };
        let scale = world.scale;
        let ed = |g: i32| scale.to_editor(g) as f32;
        let centre = [
            ed(cell[0] * SECTOR_UNITS + SECTOR_UNITS / 2),
            ed(r.y_offset + height),
            ed(cell[1] * SECTOR_UNITS + SECTOR_UNITS / 2),
        ];
        let camera = self.viewport_3d_camera();
        let stroke = Stroke::new(1.6, Color32::from_rgb(120, 220, 255));
        let radius = PLAYER_WIDTH_EDITOR / 2.0;
        let ring = |y: f32| -> Vec<[f32; 3]> {
            (0..12)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 12.0;
                    [
                        centre[0] + radius * a.cos(),
                        centre[1] + y,
                        centre[2] + radius * a.sin(),
                    ]
                })
                .collect()
        };
        let feet = ring(0.0);
        let head = ring(PLAYER_HEIGHT_EDITOR);
        let line = |a: [f32; 3], b: [f32; 3]| {
            if let Some([pa, pb]) = super::view::project_segment(camera, rect, a, b) {
                painter.line_segment([pa, pb], stroke);
            }
        };
        for i in 0..12 {
            let j = (i + 1) % 12;
            line(feet[i], feet[j]);
            line(head[i], head[j]);
        }
        for i in (0..12).step_by(3) {
            line(feet[i], head[i]);
        }
    }
}
