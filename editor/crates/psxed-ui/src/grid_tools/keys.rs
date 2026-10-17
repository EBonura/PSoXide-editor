//! Keyboard shortcuts of the grid tools.
//!
//! Handled before the editor's own shortcuts and consumed, so the keys the
//! grid gives new meanings (R turns a wall or the paste ghost, Delete removes
//! the selected faces, Ctrl+C and Ctrl+V copy and paste faces, Ctrl+A selects
//! the room) never also act on nodes.

use super::*;

use psxed_project::grid::edit::Transform;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

use super::{bands_tool, clipboard_tool, height_tool, plane_tool, select_tool};

/// Keys the level verbs (design: `docs/grid/level-tools-design.md`, 7.12)
/// will take, kept free of other meanings now: U Raise, S Sculpt, A Ramp,
/// T Stairs, B Room, K Corridor, I Opening, Y Stamps (E is the Bands tool).
/// Alt+W and Alt+P are the overlays; N is the Plane tool.
#[cfg(test)]
pub(crate) const RESERVED_VERB_KEYS: [egui::Key; 8] = [
    egui::Key::U,
    egui::Key::S,
    egui::Key::A,
    egui::Key::T,
    egui::Key::B,
    egui::Key::K,
    egui::Key::I,
    egui::Key::Y,
];

fn pressed(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

fn pressed_shift(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, key))
}

impl EditorWorkspace {
    /// Handle this frame's grid shortcuts. Call before the generic ones.
    pub(crate) fn handle_grid_shortcuts(&mut self, ctx: &egui::Context, focus_taken: bool) {
        self.handle_grid_help_key(ctx, focus_taken);
        if !self.is_grid_world() || self.active_workspace != WorkspaceView::Room || focus_taken {
            return;
        }
        if consume_command_shortcut(ctx, egui::Key::A) {
            select_tool::select_all(self);
        }
        if consume_copy_shortcut(ctx) {
            clipboard_tool::copy(self);
        }
        if consume_paste_shortcut(ctx) {
            clipboard_tool::begin_paste(self);
        }
        let modifiers = ctx.input(|i| i.modifiers);
        if modifiers.command || modifiers.ctrl || modifiers.alt {
            return;
        }

        if self.grid.plane_paste.is_some() {
            for key in [egui::Key::H, egui::Key::R] {
                if pressed(ctx, key) {
                    plane_tool::transform_ghost(self, key);
                }
            }
            if pressed(ctx, egui::Key::Escape) {
                self.grid.plane_paste = None;
                self.status = "Paste cancelled".to_string();
            }
            return;
        }
        if self.grid.paste.is_some() {
            if pressed(ctx, egui::Key::H) {
                clipboard_tool::transform_ghost(self, Transform::FlipH);
            }
            if pressed(ctx, egui::Key::V) {
                clipboard_tool::transform_ghost(self, Transform::FlipV);
            }
            if pressed(ctx, egui::Key::R) {
                clipboard_tool::transform_ghost(self, Transform::RotateCw);
            }
            if pressed(ctx, egui::Key::Escape) {
                self.grid.paste = None;
                self.status = "Paste cancelled".to_string();
            }
            return;
        }

        if pressed(ctx, egui::Key::Escape) {
            if plane_tool::cancel(self) || bands_tool::cancel(self) {
                self.status = "Cancelled".to_string();
            } else if !matches!(self.grid.gesture, Gesture::Idle) {
                // Undo whatever a height drag already did.
                if let Gesture::Height(g) = std::mem::take(&mut self.grid.gesture) {
                    if g.delta != 0 {
                        self.do_undo();
                    }
                }
                self.grid.gesture = Gesture::Idle;
                self.status = "Cancelled".to_string();
            } else if !self.grid.selection.is_empty() || !self.grid.plane_sel.is_empty() {
                self.grid.selection.clear();
                self.grid.plane_sel.clear();
                self.status = "Selection cleared".to_string();
            }
        }
        if pressed(ctx, egui::Key::Delete) || pressed(ctx, egui::Key::Backspace) {
            // With a band line selected, Delete removes the line.
            if !(self.grid.tool == GridTool::Bands && bands_tool::delete_line(self)) {
                clipboard_tool::delete_selection(self);
            }
        }
        // With the free camera's look button held, W and D fly the camera.
        let flying = ctx.input(|i| i.pointer.secondary_down());
        for tool in GridTool::ALL {
            if !flying && pressed(ctx, tool.key()) {
                self.set_grid_tool(tool);
            }
        }
        if pressed(ctx, egui::Key::R) {
            if !self.grid.plane_sel.is_empty() {
                plane_tool::turn_selected(self);
            } else if self.grid.tool == GridTool::Wall {
                self.grid.wall_dir = self.grid.wall_dir.next();
                self.status = format!("Wall direction: {}", self.grid.wall_dir.label());
            }
        }
        if pressed(ctx, egui::Key::L) {
            self.grid.link = !self.grid.link;
            self.status = if self.grid.link {
                "Vertices linked: coincident corners move together".to_string()
            } else {
                "Vertices unlinked".to_string()
            };
        }
        // Keyboard height nudges: a click, or a fine step with Shift.
        for (key, sign) in [
            (egui::Key::PageUp, 1),
            (egui::Key::PageDown, -1),
            (egui::Key::CloseBracket, 1),
            (egui::Key::OpenBracket, -1),
        ] {
            if pressed(ctx, key) {
                height_tool::nudge_selection(self, sign * CLICK_UNITS);
            }
            if pressed_shift(ctx, key) {
                height_tool::nudge_selection(self, sign * FINE_UNITS);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tool_takes_a_key_the_level_verbs_reserve() {
        for tool in GridTool::ALL {
            assert!(
                !RESERVED_VERB_KEYS.contains(&tool.key()),
                "{} uses a reserved verb key",
                tool.label()
            );
        }
        // N stays the Plane tool.
        assert_eq!(GridTool::Plane.key(), egui::Key::N);
    }
}
