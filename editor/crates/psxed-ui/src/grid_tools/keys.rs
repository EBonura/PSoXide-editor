//! Keyboard shortcuts of the grid tools.
//!
//! Handled before the editor's own shortcuts and consumed, so the keys the
//! grid gives new meanings (R turns a wall or the paste ghost, Delete removes
//! the selected faces, Ctrl+C and Ctrl+V copy and paste faces, Ctrl+A selects
//! the room) never also act on nodes.

use super::*;

use psxed_project::grid::edit::Transform;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

use super::{bands_tool, clipboard_tool, height_tool, mesh_tool, plane_tool, select_tool, verb_tool};

/// Keys the level verbs still to come (design: `docs/grid/level-tools-design.md`,
/// 7.12) will take, kept free of other meanings now: S Sculpt, B Room,
/// K Corridor, I Opening, Y Stamps, G Vault. U Raise, A Ramp and T Stairs are
/// tools now, and so are X Break and M Rough; E is the Bands tool and N the
/// Plane tool. Alt+W and Alt+P are the overlays.
#[cfg(test)]
pub(crate) const RESERVED_VERB_KEYS: [egui::Key; 6] = [
    egui::Key::S,
    egui::Key::B,
    egui::Key::K,
    egui::Key::I,
    egui::Key::Y,
    egui::Key::G,
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
        // The overlays: Alt+W walkability, Alt+P performance.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::W)) {
            self.toggle_walk_overlay();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::P)) {
            self.toggle_perf_overlay();
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
            } else if verb_tool::cancel(self) {
                // A verb drag, a pick in flight or the region itself.
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
            // With a mesh selected in the Mesh tool, Delete removes it; with
            // a band line selected, the line.
            if self.grid.tool == GridTool::Mesh && mesh_tool::delete_selected(self) {
            } else if !(self.grid.tool == GridTool::Bands && bands_tool::delete_line(self)) {
                clipboard_tool::delete_selection(self);
            }
        }
        // Shift+N is the Mesh tool (before the tool keys, which would take
        // the N for the Plane tool whatever Shift says).
        if pressed_shift(ctx, egui::Key::N) {
            self.set_grid_tool(GridTool::Mesh);
        }
        // Shift+X draws the next Break seed (before the tool keys, which
        // would take the X whatever Shift says).
        if pressed_shift(ctx, egui::Key::X) {
            if self.grid.tool == GridTool::Break {
                super::break_tool::reroll(self);
            } else {
                self.set_grid_tool(GridTool::Break);
            }
        }
        // With the free camera's look button held, W and D fly the camera.
        let flying = ctx.input(|i| i.pointer.secondary_down());
        for tool in GridTool::ALL {
            if !flying && !tool.shift() && pressed(ctx, tool.key()) {
                self.set_grid_tool(tool);
            }
        }
        if pressed(ctx, egui::Key::R) {
            if !self.grid.plane_sel.is_empty() {
                plane_tool::turn_selected(self);
            } else if self.grid.tool == GridTool::Wall {
                self.grid.wall_dir = self.grid.wall_dir.next();
                self.status = format!("Wall direction: {}", self.grid.wall_dir.label());
            } else if self.grid.tool == GridTool::Ramp {
                super::ramp_tool::turn(self);
            } else if self.grid.tool == GridTool::Mesh {
                mesh_tool::turn(self, 90);
            }
        }
        if self.grid.tool == GridTool::Mesh {
            if pressed(ctx, egui::Key::OpenBracket) {
                mesh_tool::turn(self, -15);
            }
            if pressed(ctx, egui::Key::CloseBracket) {
                mesh_tool::turn(self, 15);
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
        if self.grid.tool.is_verb() {
            if pressed(ctx, egui::Key::Tab) {
                verb_tool::toggle_mode(self);
            }
            if pressed(ctx, egui::Key::Enter) {
                if self.grid.tool == GridTool::Stairs {
                    super::stairs_tool::keep_path(self);
                }
                verb_tool::finish_path(self);
            }
            for (key, delta) in [(egui::Key::CloseBracket, 1), (egui::Key::OpenBracket, -1)] {
                if pressed(ctx, key) {
                    verb_tool::adjust_size(self, delta);
                }
            }
            return;
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
        // The letters the planes and natural terrain tools took.
        for (tool, key) in [
            (GridTool::Raise, egui::Key::U),
            (GridTool::Ramp, egui::Key::A),
            (GridTool::Stairs, egui::Key::T),
            (GridTool::Bands, egui::Key::E),
            (GridTool::Plane, egui::Key::N),
            (GridTool::Break, egui::Key::X),
            (GridTool::Rough, egui::Key::M),
        ] {
            assert_eq!(tool.key(), key, "{}", tool.label());
            assert!(!RESERVED_VERB_KEYS.contains(&key));
        }
        // Every tool has its own key.
        for (i, a) in GridTool::ALL.iter().enumerate() {
            for b in &GridTool::ALL[i + 1..] {
                assert_ne!(
                    (a.key(), a.shift()),
                    (b.key(), b.shift()),
                    "{} and {}",
                    a.label(),
                    b.label()
                );
            }
        }
        // Mesh is the chord Shift+N; N alone stays the Plane tool.
        assert_eq!((GridTool::Mesh.key(), GridTool::Mesh.shift()), (egui::Key::N, true));
    }
}
