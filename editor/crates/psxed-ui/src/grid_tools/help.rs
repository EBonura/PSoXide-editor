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
    (
        "Click again, same spot (Top view)",
        "Take the next thing under it: plane, then the sector (Paint and Split go on to the ceiling)",
    ),
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
const PLANE: Rows = &[
    (
        "Click a surface",
        "Put a plane on it, facing out, snapped (a click is one sector)",
    ),
    ("Drag", "Size the new plane from the press point"),
    (
        "Click a plane",
        "Select it (Select works too); Shift extends, Ctrl toggles",
    ),
    (
        "Corner, edge handles",
        "Stretch; the opposite edges stay put",
    ),
    (
        "Green ring knob",
        "Turn about the vertical axis (15 degree steps)",
    ),
    ("Blue arrow tip", "Tilt the front up and down"),
    (
        "Drag the body",
        "Slide in the plane; Shift moves it up and down",
    ),
    ("Alt, Shift+Alt", "Fine snap (64 units), free movement"),
    ("R", "Quarter turn"),
    ("PgUp / PgDn, ] / [", "Nudge up or down a click"),
    (
        "Del, Ctrl/Cmd+C, V",
        "Delete, copy, paste (R turns, H flips the ghost)",
    ),
    (
        "Inspector",
        "Position, size, angles, material, texture, sides, collision",
    ),
];
const HEIGHT: Rows = &[
    ("Drag", "Raise or lower a vertex, edge or face"),
    ("Alt", "Fine step (64 units instead of 256)"),
    (
        "Alt+drag a floor or ceiling face",
        "Move one triangle of it: the surface becomes split-level and a diagonal wall shows between the halves live; Shift is the fine step in this gesture, Esc undoes it",
    ),
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
const BANDS: Rows = &[
    (
        "Hover a wall",
        "A rule previews where a click would cut; its kind shows in the status bar",
    ),
    (
        "Click",
        "Cut the wall run at the rule (this run, or the whole room: Scope in the option row)",
    ),
    (
        "Drag a line",
        "Move it between its neighbours (Alt for the fine step); Esc puts it back",
    ),
    (
        "Click a line, Delete",
        "Remove it; the bands around it join",
    ),
    (
        "Shift+click",
        "Pick the band under the pointer, then click a material in the palette",
    ),
    (
        "Paint (P)",
        "Painting a banded wall paints the band under the pointer",
    ),
    ("Inspector", "Lines, bands, materials and tiling per band"),
];
const RAISE: Rows = &[
    (
        "Drag, click (Marquee)",
        "Pick a rectangle of sectors, or one with a click; Shift adds to the region",
    ),
    (
        "Brush, Path, Lasso (option row)",
        "Paint sectors with a disc, click waypoints and press Enter, or click the corners of a shape: [ and ] change the radius or width",
    ),
    (
        "Lasso: click corners, Enter",
        "Enter closes the shape into the sectors whose centres lie inside it; Shift snaps each new segment to 45 degrees and whole sectors, Esc drops the shape",
    ),
    (
        "Cut corners (Raise option row)",
        "Crack the outer triangle of the region's convex corners down by the last raise (a click if none): a plateau with 45 degree corners, one undo step",
    ),
    (
        "Press on the region, drag",
        "Up raises, down lowers (256 units a step, Alt for 64); the sides appear live, release commits",
    ),
    ("Tab", "Switch Offset (keep the shape) and Level (flatten to the height under the pointer)"),
    ("Follow ceiling", "Ceilings move with the floors; off holds a floor a click under its ceiling"),
    ("Esc", "Cancel the drag exactly; again to forget the region"),
    ("Amount, Apply", "A typed amount in clicks, applied as one undo step"),
];
const RAMP: Rows = &[
    (
        "Pick a region",
        "Marquee, brush or path, as for Raise",
    ),
    (
        "Hover the region",
        "The side nearest the pointer lights up: that side (A) keeps its height",
    ),
    (
        "Press near it, drag up or down",
        "The opposite side (B) rises or falls; every sector between slopes evenly, rounded to 64 units",
    ),
    ("R", "Turn the kept side a quarter turn and hold it"),
    (
        "Tab, Walkable",
        "Hold the rise to the steepest slope the character motor stands on",
    ),
    (
        "Green, orange",
        "Sectors a body can stand on, sectors too steep for it (the readout says which)",
    ),
];
const STAIRS: Rows = &[
    (
        "Click, Enter",
        "Click the waypoints of the flight, Enter finishes the path",
    ),
    (
        "Press on it, drag up or down",
        "The total rise; each sector is a tread, a turn is a landing, the ghost numbers the steps",
    ),
    ("[ and ]", "Width of the flight in sectors"),
    (
        "Riser (option row)",
        "1 click, Max (what the character steps up) or a typed riser; steps follow from the rise",
    ),
    (
        "Tab, Ramp",
        "Slope every tread instead of stepping; also the answer to a refused, too steep flight",
    ),
];
const BREAK: Rows = &[
    (
        "Pick a region, press on it, drag up",
        "The intensity, 0 to 100: lips crumble, plateau edges bite, floor tiles tilt and crack, the derived faces appear live. Release commits as one undo step",
    ),
    (
        "Tab",
        "Cycle what is broken: walls and floor, walls, floor, ceiling, all",
    ),
    (
        "Shift+X",
        "Re-roll: the next seed, live while dragging; the seed shows in the option row and can be typed. The same seed always gives the same result",
    ),
    ("Enter", "Commit where the drag is"),
    ("Esc", "Put the document back exactly"),
    (
        "Option row",
        "Targets, Keep walkable (refuses changes that cut a route, make a step too tall, a floor too steep or headroom too low), Fine or Click steps, amplitude, Mix materials, rough level, seed, a typed intensity",
    ),
    (
        "Alt+W",
        "Keeps working while you drag: the status line counts what the walkability mask refused",
    ),
];
const ROUGH: Rows = &[
    (
        "Pick a region, press on it, drag up",
        "The rock level, 0 to 3, about 24 pixels a level, from the level the region holds now; Enter or the release commits as one undo step, Esc puts it back",
    ),
    (
        "Option row, Rooms panel",
        "The room's seed, the three amplitudes in grid units (with the runtime units beside them, orange over 4) and the tint percent",
    ),
    (
        "What it does",
        "Jitters the corners of the faces of rough sectors in the render only; collision stays on the grid",
    ),
];
const OVERLAYS: Rows = &[
    (
        "Alt+W, Walk button",
        "Walkability: orange too steep, blue headroom under the body, red step taller than it climbs, yellow dashed drop, hatched floors no spawn reaches",
    ),
    (
        "Alt+P, Perf button",
        "Performance heatmap: a model of the world render cost at every spot, green under 60% of the budget, amber to 90%, red above (the Reserve field edits what the player and sky take)",
    ),
];
const OTHER: Rows = &[
    ("Split (D)", "Click a floor or ceiling to flip its diagonal"),
    (
        "Split: Shift+click",
        "Void or restore the triangle under the pointer: the other half closes itself with a diagonal wall",
    ),
    (
        "Place (O)",
        "Click to put the chosen node at a sector centre; Alt for the fine lattice, Shift+Alt for free placement",
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
const SNAP: Rows = &[
    ("Alt", "Fine snap: 64 units (every tool, planes included)"),
    (
        "Shift+Alt",
        "Free movement where a tool can leave the lattice (planes, Place)",
    ),
    (
        "S B K I Y G",
        "Reserved for the level verbs still to come: Sculpt, Room, Corridor, Opening, Stamps, Vault",
    ),
    (
        "Alt+W, Alt+P",
        "Reserved for the walkability and performance overlays",
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
        let sections: [(&str, Rows); 16] = [
            ("Select (V)", SELECT),
            ("Floor (F) and Ceiling (C)", FLOOR),
            ("Wall (W)", WALL),
            ("Plane (N)", PLANE),
            ("Height (H)", HEIGHT),
            ("Paint (P)", PAINT),
            ("Bands (E)", BANDS),
            ("Raise (U)", RAISE),
            ("Ramp (A)", RAMP),
            ("Stairs (T)", STAIRS),
            ("Break (X)", BREAK),
            ("Rough (M)", ROUGH),
            ("Overlays", OVERLAYS),
            ("More", OTHER),
            ("Snapping and reserved keys", SNAP),
            ("Camera and project", VIEW),
        ];
        egui::Window::new("Grid editor keys")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fade_in(false)
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 96.0))
            .show(ctx, |ui| {
                ui.set_width(1500.0_f32.min(ctx.screen_rect().width() - 60.0));
                ui.label(
                    RichText::new("Tools: V Select, H Height | F Floor, C Ceiling, W Wall, N Plane, P Paint, D Split, O Place, E Bands | U Raise, A Ramp, T Stairs, X Break, M Rough")
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.add_space(4.0);
                // Three columns of sections, split where the rows balance
                // (long descriptions count double, they wrap).
                let weight = |rows: Rows| -> usize {
                    2 + rows
                        .iter()
                        .map(|(_, what)| 1 + usize::from(what.len() > 70))
                        .sum::<usize>()
                };
                let total: usize = sections.iter().map(|(_, r)| weight(r)).sum();
                let mut column_of = Vec::with_capacity(sections.len());
                let mut running = 0;
                for (_, rows) in sections.iter() {
                    column_of.push((running * 3 / total.max(1)).min(2));
                    running += weight(rows);
                }
                let column_width = (ui.available_width() / 3.0 - 20.0).max(200.0);
                ui.columns(3, |columns| {
                    for (index, (title, rows)) in sections.iter().enumerate() {
                        let ui = &mut columns[column_of[index]];
                        ui.add_space(4.0);
                        ui.label(RichText::new(*title).strong().color(STUDIO_TEXT));
                        egui::Grid::new(format!("grid_help_{title}"))
                            .num_columns(2)
                            .max_col_width(column_width * 0.64)
                            .spacing([12.0, 2.0])
                            .show(ui, |ui| {
                                for (gesture, what) in rows.iter() {
                                    ui.label(
                                        RichText::new(*gesture).monospace().color(STUDIO_ACCENT),
                                    );
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
