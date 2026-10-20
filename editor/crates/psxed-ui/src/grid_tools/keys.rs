//! Keyboard shortcuts of the grid tools.
//!
//! Handled before the editor's own shortcuts and consumed, so the keys the
//! grid gives new meanings (R turns a wall or the paste ghost, Delete removes
//! the selected faces, Ctrl+C and Ctrl+V copy and paste faces, Ctrl+A selects
//! the room) never also act on nodes.

use super::*;

use psxed_project::grid::edit::Transform;
use psxed_project::grid::{CLICK_UNITS, FINE_UNITS};

use super::{
    bands_tool, clipboard_tool, dup_tool, height_tool, mesh_tool, plane_tool, select_tool,
    verb_tool,
};

/// Keys the level verbs still to come (design: `docs/grid/level-tools-design.md`,
/// 7.12) will take, kept free of other meanings now: B Room,
/// K Corridor, I Opening, Y Stamps, J Vault. U Raise, A Ramp, T Stairs and
/// S Sculpt are tools now, and so is X Break; E is the Bands tool and N the
/// Plane tool, Q the Cells tool and Shift+Q the Upper floor tool; G goes to a level
/// and Z switches the cutaway. Alt+W, Alt+P and Alt+L are the overlays.
#[cfg(test)]
pub(crate) const RESERVED_VERB_KEYS: [egui::Key; 5] = [
    egui::Key::B,
    egui::Key::K,
    egui::Key::I,
    egui::Key::Y,
    egui::Key::J,
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
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::L)) {
            self.toggle_leaf_overlay();
        }
        // The storey cursor: Alt+Up and Alt+Down step through the storeys.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp)) {
            self.step_level(true);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown)) {
            self.step_level(false);
        }
        // Cmd+D duplicates the selection and carries the copy on the pointer.
        if self.grid.grab.is_none() && consume_command_shortcut(ctx, egui::Key::D) {
            dup_tool::begin(self);
            return;
        }
        if self.grid.grab.is_some() {
            if pressed(ctx, egui::Key::Escape) {
                dup_tool::cancel(self);
            }
            for (key, sign) in [
                (egui::Key::ArrowUp, 1),
                (egui::Key::PageUp, 1),
                (egui::Key::ArrowDown, -1),
                (egui::Key::PageDown, -1),
            ] {
                if pressed(ctx, key) {
                    dup_tool::lift(self, sign, false);
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, key)) {
                    dup_tool::lift(self, sign, true);
                }
            }
            return;
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
            if self.cancel_level_pick() {
                // The go-to-level click was waiting.
            } else if plane_tool::cancel(self)
                || bands_tool::cancel(self)
                || super::sculpt_tool::cancel(self)
            {
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
        let delete_key =
            ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete_key {
            // With a mesh selected in the Mesh tool, Delete removes it; with
            // a band line selected, the line; otherwise the picked faces.
            // The key is taken only when one of them did something: with
            // nothing of the grid picked it is left for the generic handler,
            // which deletes the picked node (a light just placed).
            let acted = (self.grid.tool == GridTool::Mesh && mesh_tool::delete_selected(self))
                || (self.grid.tool == GridTool::Bands && bands_tool::delete_line(self))
                || clipboard_tool::delete_selection(self);
            if acted {
                pressed(ctx, egui::Key::Delete);
                pressed(ctx, egui::Key::Backspace);
            }
        }
        // A tool is chosen between gestures, never in the middle of one: with
        // a pointer button down the tool keys are taken and ignored, so a key
        // landing during a drag cannot swap the tool and abandon what the
        // drag is making.
        let may_choose = !ctx.input(|i| i.pointer.any_down());
        // Shift+M is Merge in the tools that split things (before the tool
        // keys, which would take the M for a tool key whatever Shift
        // says).
        if may_choose
            && matches!(
                self.grid.tool,
                GridTool::Bands | GridTool::Cells | GridTool::Select
            )
            && pressed_shift(ctx, egui::Key::M)
        {
            self.grid_merge();
        }
        // Shift+N is the Mesh tool (before the tool keys, which would take
        // the N for the Plane tool whatever Shift says).
        if pressed_shift(ctx, egui::Key::N) && may_choose {
            self.set_grid_tool(GridTool::Mesh);
        }
        // Shift+Q is the Upper floor tool, beside the Cells tool's Q.
        if pressed_shift(ctx, egui::Key::Q) && may_choose {
            self.set_grid_tool(GridTool::Slab);
        }
        // Shift+T is the Spiral tool, beside the Stairs tool's T.
        if pressed_shift(ctx, egui::Key::T) && may_choose {
            self.set_grid_tool(GridTool::Spiral);
        }
        // Shift+X draws the next Break seed (before the tool keys, which
        // would take the X whatever Shift says).
        if pressed_shift(ctx, egui::Key::X) {
            if self.grid.tool == GridTool::Break {
                super::break_tool::reroll(self);
            } else if may_choose {
                self.set_grid_tool(GridTool::Break);
            }
        }
        // Paint: R turns the brush a quarter clockwise (Shift+R the other
        // way), F flips it horizontally (Shift+F vertically). Taken before
        // the tool keys, because F is also the Floor tool's key; the Floor
        // tool is a click away on the tool row while Paint is active. Like
        // the tool keys they are taken and ignored with a pointer button
        // down, so a key during a stroke cannot change the paint under it.
        if self.grid.tool == GridTool::Paint {
            // Shift first: the plain key also matches with Shift held.
            let turn_back = pressed_shift(ctx, egui::Key::R);
            let turn = !turn_back && pressed(ctx, egui::Key::R);
            let flip_v = pressed_shift(ctx, egui::Key::F);
            let flip_h = !flip_v && pressed(ctx, egui::Key::F);
            // `]` draws the paint bigger, `[` smaller (1/4x, 1/2x, 1x, 2x,
            // 4x). Taken here, before the height nudge those keys have in
            // the other tools.
            let smaller = pressed(ctx, egui::Key::OpenBracket);
            let bigger = pressed(ctx, egui::Key::CloseBracket);
            if may_choose {
                if turn_back || turn {
                    self.paint_turn(turn);
                }
                if flip_h || flip_v {
                    self.paint_flip(usize::from(flip_v));
                }
                if smaller {
                    self.paint_zoom(-1);
                }
                if bigger {
                    self.paint_zoom(1);
                }
            }
        }
        // Select: `=` scales the selected faces up a step, `-` down (the
        // keys the Mesh tool scales with; the brackets nudge the height).
        if self.grid.tool == GridTool::Select && may_choose {
            if pressed(ctx, egui::Key::Equals) {
                self.zoom_selection(1);
            }
            if pressed(ctx, egui::Key::Minus) {
                self.zoom_selection(-1);
            }
        }
        // With the free camera's look button held, W and D fly the camera.
        let flying = ctx.input(|i| i.pointer.secondary_down());
        for tool in GridTool::ALL {
            if !flying && !tool.shift() && pressed(ctx, tool.key()) && may_choose {
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
            } else if self.grid.tool == GridTool::Stairs
                && self.grid.verb.source != super::verb_state::RegionSource::Path
            {
                super::stairs_tool::turn(self);
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
            if pressed(ctx, egui::Key::Minus) {
                mesh_tool::scale_step(self, -1);
            }
            if pressed(ctx, egui::Key::Equals) {
                mesh_tool::scale_step(self, 1);
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
        // G goes to a level, Z switches the cutaway.
        if pressed(ctx, egui::Key::G) {
            self.begin_level_pick();
        }
        if pressed(ctx, egui::Key::Z) {
            self.cycle_cutaway();
        }
        if self.grid.tool == GridTool::Cells {
            // Shift+Enter first: a plain Enter also matches when Shift is held.
            if pressed_shift(ctx, egui::Key::Enter) {
                super::cells_tool::apply_selection(self, true);
            } else if pressed(ctx, egui::Key::Enter) {
                super::cells_tool::apply_selection(self, false);
            }
            for (key, delta) in [(egui::Key::CloseBracket, 1), (egui::Key::OpenBracket, -1)] {
                if pressed(ctx, key) {
                    super::cells_tool::adjust_radius(self, delta);
                }
            }
            return;
        }
        if self.grid.tool == GridTool::Sculpt {
            for (key, delta) in [
                (egui::Key::CloseBracket, 1.0),
                (egui::Key::OpenBracket, -1.0),
            ] {
                if pressed(ctx, key) {
                    super::sculpt_tool::adjust_radius(self, delta);
                }
            }
            if pressed(ctx, egui::Key::Tab) {
                super::sculpt_tool::cycle_mode(self);
            }
            return;
        }
        if self.grid.tool.is_verb() {
            if self.grid.tool == GridTool::Extrude {
                if pressed(ctx, egui::Key::Equals) || pressed(ctx, egui::Key::Plus) {
                    super::extrude_tool::step_by_key(self, 1);
                }
                if pressed(ctx, egui::Key::Minus) {
                    super::extrude_tool::step_by_key(self, -1);
                }
            }
            if pressed(ctx, egui::Key::Tab) {
                verb_tool::toggle_mode(self);
            }
            if pressed(ctx, egui::Key::Enter) {
                if self.grid.tool == GridTool::Stairs {
                    super::stairs_tool::keep_path(self);
                }
                if self.grid.tool == GridTool::Spiral {
                    super::spiral_tool::keep_ring(self);
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
            (GridTool::Extrude, egui::Key::U),
            (GridTool::Ramp, egui::Key::A),
            (GridTool::Stairs, egui::Key::T),
            (GridTool::Bands, egui::Key::E),
            (GridTool::Plane, egui::Key::N),
            (GridTool::Break, egui::Key::X),
            (GridTool::Sculpt, egui::Key::S),
            (GridTool::Cells, egui::Key::Q),
            (GridTool::Slab, egui::Key::Q),
            (GridTool::Spiral, egui::Key::T),
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
        assert_eq!(
            (GridTool::Mesh.key(), GridTool::Mesh.shift()),
            (egui::Key::N, true)
        );
    }
}
