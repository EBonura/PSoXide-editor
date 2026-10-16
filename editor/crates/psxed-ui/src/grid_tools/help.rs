//! The `?` help overlay: every grid tool key and gesture on one page.
//!
//! Toggled with `?` (or the `?` button in the toolbar) and closed with `?`
//! or Escape. The text is written here once; the tool tooltips and the
//! status bar hints say the same things in fewer words.

use super::*;

/// One tool's keys and gestures, `(gesture, what it does)` rows.
type Rows = &'static [(&'static str, &'static str)];

const SELECT: Rows = &[
    ("Click", "Pick the vertex, edge or face under the pointer"),
    ("Shift+click", "Extend to a range"),
    ("Ctrl/Cmd+click", "Toggle one element in the selection"),
    ("Drag", "Box select"),
    (
        "Shift+drag",
        "Raise or lower the selected corners (256 units, Alt for 64)",
    ),
    ("Delete", "Delete the selected faces"),
    ("Ctrl/Cmd+A", "Select everything in the room"),
];
const FLOOR: Rows = &[
    ("Drag", "Create a rectangle of floor; the room grows to fit"),
    ("Shift while dragging", "Set the height of the new surface"),
    ("Ctrl/Cmd+drag", "Delete floor under the rectangle"),
    ("Esc", "Cancel the drag"),
];
const WALL: Rows = &[
    (
        "Drag",
        "Place a run of walls along an edge, filling the gap",
    ),
    ("R", "Turn the direction: auto, N, E, S, W, two diagonals"),
    ("Ctrl/Cmd+drag", "Remove walls"),
];
const HEIGHT: Rows = &[
    ("Drag", "Raise or lower a vertex, edge or face"),
    ("Alt", "Fine step (64 units instead of 256)"),
    ("L", "Link or unlink corners that share a position"),
    (
        "PgUp / PgDn, ] / [",
        "Nudge the selection a click; Shift for a fine step",
    ),
];
const PAINT: Rows = &[
    ("Click, drag", "Apply the palette material to faces"),
    (
        "Ctrl/Cmd+click",
        "Sample a face's material into the palette",
    ),
];
const OTHER: Rows = &[
    ("Split (D)", "Click a floor or ceiling to flip its diagonal"),
    (
        "Place (O)",
        "Click to put the chosen node at a sector centre; Alt for free placement",
    ),
    (
        "Ctrl/Cmd+C, V",
        "Copy faces, then paste with a ghost: click to stamp, Shift to keep stamping",
    ),
    (
        "H, V, R while pasting",
        "Flip horizontally, flip vertically, turn",
    ),
];
const VIEW: Rows = &[
    ("Middle or right drag", "Orbit the 3D camera"),
    ("Shift+middle or right drag", "Pan"),
    ("Scroll", "Zoom"),
    ("W A S D (right button held)", "Fly the free camera"),
    (".", "Frame the selection"),
    ("Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z", "Undo, redo"),
    ("Ctrl/Cmd+S", "Save the project"),
    ("?", "This page"),
];

impl EditorWorkspace {
    /// The `?` key toggles the overlay, Escape closes it. Call with the
    /// other grid shortcuts.
    pub(crate) fn handle_grid_help_key(&mut self, ctx: &egui::Context, focus_taken: bool) {
        if !self.is_grid_world() || focus_taken {
            return;
        }
        let question = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Questionmark)
        });
        if question {
            self.grid.help_open = !self.grid.help_open;
        }
        if self.grid.help_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.grid.help_open = false;
        }
    }

    /// Draw the overlay when it is open.
    pub(crate) fn draw_grid_help(&mut self, ctx: &egui::Context) {
        if !self.grid.help_open || !self.is_grid_world() {
            return;
        }
        let mut open = true;
        egui::Window::new("Grid editor keys")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(640.0);
                ui.label(
                    RichText::new("Tools: V Select, F Floor, C Ceiling, W Wall, H Height, P Paint, D Split, O Place")
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    for (title, rows) in [
                        ("Select (V)", SELECT),
                        ("Floor (F) and Ceiling (C)", FLOOR),
                        ("Wall (W)", WALL),
                        ("Height (H)", HEIGHT),
                        ("Paint (P)", PAINT),
                        ("More", OTHER),
                        ("Camera and project", VIEW),
                    ] {
                        ui.add_space(4.0);
                        ui.label(RichText::new(title).strong().color(STUDIO_TEXT));
                        egui::Grid::new(format!("grid_help_{title}"))
                            .num_columns(2)
                            .spacing([16.0, 2.0])
                            .show(ui, |ui| {
                                for (gesture, what) in rows {
                                    ui.label(RichText::new(*gesture).monospace().color(STUDIO_ACCENT));
                                    ui.label(*what);
                                    ui.end_row();
                                }
                            });
                    }
                });
            });
        if !open {
            self.grid.help_open = false;
        }
    }
}
