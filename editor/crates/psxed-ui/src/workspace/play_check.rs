//! The check before Play: what a level designer would trip over, told in
//! plain sentences before the guest is built rather than as a halt inside the
//! emulator. The cook's own refusals (limits, memory, a missing player) join
//! the list; the walkability check adds the sectors the player cannot walk to
//! from the start. Every problem that points at something carries a link that
//! selects it.

use psxed_project::grid::walk::{analyse, player_spawns};
use psxed_project::playtest::PlaytestValidationTarget;

use super::*;

/// What a problem points at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ProblemTarget {
    /// A node or resource the cook blamed.
    Cook(PlaytestValidationTarget),
    /// A sector of a room (world sector).
    Sector { room: usize, cell: [i32; 2] },
}

/// One thing to fix, or to know, before Play.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlayProblem {
    /// A sentence a designer reads.
    pub text: String,
    /// Whether Play cannot go on until it is fixed.
    pub blocking: bool,
    /// What the "Select it" link goes to, when there is something.
    pub target: Option<ProblemTarget>,
}

impl EditorWorkspace {
    /// The walkability findings: a missing player start, and per room the
    /// walkable sectors no walk from the start reaches. These do not stop
    /// Play (a lever may open the way); they are listed so a level that
    /// cannot be finished is noticed before it is played.
    pub(crate) fn walkability_problems(&self) -> Vec<PlayProblem> {
        let Some(world) = self.project.active_scene().world.grid() else {
            return Vec::new();
        };
        let spawns = player_spawns(&self.project, world);
        if spawns.is_empty() {
            return vec![PlayProblem {
                text: "There is no player start standing on a floor, so the player has nowhere \
                       to begin. Place the player actor on a floor."
                    .to_string(),
                blocking: false,
                target: None,
            }];
        }
        let Some(analysis) = analyse(world, &spawns, &psx_engine::WalkLimits::DEFAULT) else {
            return Vec::new();
        };
        if !analysis.reach_ran {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (index, room) in analysis.rooms.iter().enumerate() {
            let mut count = 0usize;
            let mut first = None;
            for z in 0..room.depth as i32 {
                for x in 0..room.width as i32 {
                    let cell = [room.origin[0] + x, room.origin[1] + z];
                    if room.cell(cell).is_some_and(|c| c.unreachable) {
                        count += 1;
                        first.get_or_insert(cell);
                    }
                }
            }
            let Some(cell) = first else { continue };
            let name = world
                .rooms
                .get(index)
                .map_or_else(|| format!("Room {index}"), |r| r.name.clone());
            out.push(PlayProblem {
                text: format!(
                    "In {name}, {count} {} cannot be walked to from the player start: a step is \
                     too tall, a drop is in the way, or a door is shut. The first is at {}, {}.",
                    if count == 1 { "sector" } else { "sectors" },
                    cell[0],
                    cell[1],
                ),
                blocking: false,
                target: Some(ProblemTarget::Sector { room: index, cell }),
            });
        }
        out
    }

    /// Put the problems of a Play attempt on show: the list the Console
    /// carries, the Console lines, and the status bar.
    pub(crate) fn report_play_problems(&mut self, problems: Vec<PlayProblem>) {
        for p in &problems {
            self.append_console_lines([format!(
                "[check] {}: {}",
                if p.blocking { "Play stopped" } else { "Note" },
                p.text
            )]);
        }
        if let Some(first) = problems.iter().find(|p| p.blocking).or(problems.first()) {
            let more = problems.len() - 1;
            let tail = if more == 0 {
                String::new()
            } else {
                format!(" (and {more} more in the Console)")
            };
            self.status = if first.blocking {
                format!("Play stopped: {}{tail}", first.text)
            } else {
                format!("Heads up: {}{tail}", first.text)
            };
        }
        self.play_problems = problems;
    }

    /// Select what a problem points at and show it.
    pub(crate) fn select_play_problem(&mut self, target: ProblemTarget) -> bool {
        match target {
            ProblemTarget::Cook(t) => self.focus_playtest_validation_target(t),
            ProblemTarget::Sector { room, cell } => {
                let Some(world) = self.project.active_scene().world.grid() else {
                    return false;
                };
                let Some(r) = world.rooms.get(room) else {
                    return false;
                };
                let scale = world.scale;
                let floor = r.y_offset;
                let edit = |g: i32| scale.to_editor(g) as f32;
                let half = scale.to_editor(psxed_project::grid::units::SECTOR_UNITS) as f32 / 2.0;
                let centre = [
                    edit(cell[0] * psxed_project::grid::units::SECTOR_UNITS) + half,
                    edit(floor),
                    edit(cell[1] * psxed_project::grid::units::SECTOR_UNITS) + half,
                ];
                self.selection.selected_room = Some(room);
                self.frame_3d_bounds(centre, [half, half, half]);
                true
            }
        }
    }

    /// The Problems block at the top of the Console.
    pub(crate) fn draw_play_problems(&mut self, ui: &mut egui::Ui) {
        if self.play_problems.is_empty() {
            return;
        }
        let mut select = None;
        let mut clear = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Before Play: {} to look at",
                    self.play_problems.len()
                ))
                .strong()
                .color(STUDIO_WARNING),
            );
            if ui.small_button("Dismiss").clicked() {
                clear = true;
            }
        });
        for p in &self.play_problems {
            ui.horizontal_wrapped(|ui| {
                let colour = if p.blocking {
                    STUDIO_ERROR
                } else {
                    STUDIO_TEXT
                };
                ui.add(egui::Label::new(RichText::new(&p.text).color(colour)).selectable(true));
                if let Some(target) = p.target {
                    if ui.link("Select it").clicked() {
                        select = Some(target);
                    }
                }
            });
        }
        ui.separator();
        if clear {
            self.play_problems.clear();
        }
        if let Some(target) = select {
            self.select_play_problem(target);
        }
    }
}
