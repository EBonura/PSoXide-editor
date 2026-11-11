//! The dead-control sweep. A control is something the pointer can act on: a
//! text that lights up when the pointer is on it (a label does not). It is
//! alive when clicking it changes something it owns: the document, the
//! status line, the tool, the screen (a chip's selection, an open list, an
//! edit field), or, for a chip that is already selected, when its neighbour's
//! change is undone by clicking it. A control that lights up and does none of
//! these is dead. Menus are compared on what is left once they are closed.

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, press_key, texts};
use super::tool_audit::*;
use super::tool_flows::*;
use super::*;
use crate::grid_tools::GridTool;
use std::hash::{Hash, Hasher};

#[derive(Default, Debug)]
pub(super) struct Tally {
    pub alive: usize,
    pub labels: usize,
    pub icon_only: usize,
    pub dead: Vec<String>,
}

impl Tally {
    pub(super) fn add(&mut self, other: Tally) {
        self.alive += other.alive;
        self.labels += other.labels;
        self.icon_only += other.icon_only;
        self.dead.extend(other.dead);
    }
}

fn shapes_hash(rig: &WindowRig) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", rig.out.shapes).hash(&mut h);
    h.finish()
}

fn text_set(rig: &WindowRig) -> Vec<String> {
    let mut t: Vec<String> = texts(rig).into_iter().map(|(t, _)| t).collect();
    t.sort();
    t
}

#[derive(Clone, PartialEq)]
struct Snap {
    project: ProjectDocument,
    status: String,
    tool: GridTool,
    shapes: u64,
    texts: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Anything on screen counts (an options bar, a form).
    Screen,
    /// The text on screen, the document, the tool and the message count (a
    /// menu closes itself, which would otherwise count as a change).
    Content,
}

fn neutral(rig: &WindowRig) -> Pos2 {
    Pos2::new(rig.size.x - 2.0, rig.size.y * 0.5)
}

fn settle(rig: &mut WindowRig) {
    let n = neutral(rig);
    rig.move_to(n);
    frames(rig, 3);
}

fn snap(rig: &WindowRig, mode: Mode) -> Snap {
    Snap {
        project: rig.ws.project.clone(),
        status: rig.ws.status.clone(),
        tool: rig.ws.grid.tool,
        shapes: if mode == Mode::Screen {
            shapes_hash(rig)
        } else {
            0
        },
        texts: if mode == Mode::Content {
            text_set(rig)
        } else {
            Vec::new()
        },
    }
}

fn icon_only(label: &str) -> bool {
    !label.chars().any(|c| c.is_alphanumeric())
}

/// Hover `rect`, click it, and say what became of it. `reopen` puts the
/// surroundings back (a menu opened again, a form selected again).
fn probe(
    rig: &mut WindowRig,
    label: &str,
    rect: Rect,
    siblings: &[(String, Rect)],
    mode: Mode,
    click_it: bool,
    reopen: &dyn Fn(&mut WindowRig),
) -> Probe {
    settle(rig);
    let rest = shapes_hash(rig);
    rig.move_to(rect.center());
    frames(rig, 2);
    let lights_up = shapes_hash(rig) != rest;
    if !lights_up {
        return Probe::Label;
    }
    if !click_it || icon_only(label) {
        return Probe::NotClicked;
    }
    settle(rig);
    let before = snap(rig, mode);
    rig.click(rect.center());
    settle(rig);
    let changed = snap(rig, mode) != before;
    close_popups(rig);
    settle(rig);
    if changed {
        return Probe::Alive;
    }
    // Already selected? A neighbour's click changes the state, and this
    // control's click brings it back.
    let mut near: Vec<&(String, Rect)> = siblings
        .iter()
        .filter(|(l, r)| l != label && !icon_only(l) && *r != rect)
        .collect();
    near.sort_by(|a, b| {
        a.1.center()
            .distance(rect.center())
            .total_cmp(&b.1.center().distance(rect.center()))
    });
    for (_, other) in near.into_iter().take(5) {
        reopen(rig);
        rig.click(other.center());
        close_popups(rig);
        settle(rig);
        let after_other = snap(rig, mode);
        if after_other == before {
            continue;
        }
        reopen(rig);
        rig.click(rect.center());
        close_popups(rig);
        settle(rig);
        if snap(rig, mode) != after_other {
            return Probe::Alive;
        }
    }
    Probe::Dead
}

fn close_popups(rig: &mut WindowRig) {
    // An open list closes on Escape; it is also what a person does.
    press_key(rig, egui::Key::Escape, false);
}

#[derive(Debug, PartialEq, Eq)]
enum Probe {
    Label,
    NotClicked,
    Alive,
    Dead,
}

/// Sweep the texts of `region` of whatever is on screen.
fn sweep(
    rig: &mut WindowRig,
    what: &str,
    region: Rect,
    mode: Mode,
    click_it: bool,
    reopen: &dyn Fn(&mut WindowRig),
    reset: &dyn Fn(&mut WindowRig),
) -> Tally {
    let mut tally = Tally::default();
    let items: Vec<(String, Rect)> = texts(rig)
        .into_iter()
        .map(|(t, r)| (t.trim().to_string(), r))
        .filter(|(t, r)| !t.is_empty() && region.contains(r.center()) && r.width() > 0.0)
        .collect();
    for (label, rect) in &items {
        let base = rig.ws.project.clone();
        match probe(rig, label, *rect, &items, mode, click_it, reopen) {
            Probe::Label => tally.labels += 1,
            Probe::NotClicked => tally.icon_only += 1,
            Probe::Alive => tally.alive += 1,
            Probe::Dead => tally.dead.push(format!("{what}: {label}")),
        }
        rig.ws.project = base;
        reset(rig);
    }
    tally
}

fn bar_region(rig: &WindowRig) -> Rect {
    let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
    let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
    Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(rig.size.x, bottom))
}

/// The options bar of every tool.
pub(super) fn sweep_option_bars() -> Tally {
    let mut total = Tally::default();
    for tool in GridTool::ALL {
        let mut rig = View::Plan.rig(Fixture::Starter);
        View::Plan.aim_near(&mut rig, [-1, -1], [7, 6], 1.0);
        if !start(&mut rig, tool, Start::Strip) {
            total
                .dead
                .push(format!("{}: not on the strip", tool.label()));
            continue;
        }
        let region = bar_region(&rig);
        // The region tools wait for a region: make one, as a person does.
        let needs_region = matches!(
            tool,
            GridTool::Extrude
                | GridTool::Ramp
                | GridTool::Stairs
                | GridTool::Spiral
                | GridTool::Slab
                | GridTool::Break
        );
        let prepare = move |rig: &mut WindowRig| {
            if needs_region && rig.ws.grid.verb.region.is_none() {
                View::Plan.drag_cells(rig, [1, 1], [4, 3]);
            }
        };
        prepare(&mut rig);
        let reopen = move |rig: &mut WindowRig| {
            if rig.ws.grid.tool != tool {
                let _ = start(rig, tool, Start::Strip);
            }
            prepare(rig);
        };
        // Each control starts from the tool's defaults, not from what the
        // control before it left behind.
        let pristine_verb = rig.ws.grid.verb.clone();
        let pristine_build = rig.ws.grid.build.opts.clone();
        let reset = move |rig: &mut WindowRig| {
            rig.ws.grid.verb = pristine_verb.clone();
            rig.ws.grid.build.opts = pristine_build.clone();
            reopen(rig);
        };
        total.add(sweep(
            &mut rig,
            &format!("options bar of {}", tool.label()),
            region,
            Mode::Screen,
            true,
            &reopen,
            &reset,
        ));
    }
    total
}

/// The top-bar menus: View, Edit, Tools, Help, and File without the entries
/// that open a file dialog, close the editor or start a build.
pub(super) fn sweep_menus() -> (Tally, Vec<String>) {
    let mut total = Tally::default();
    let skipped: Vec<String> = [
        "New Project...",
        "Project",
        "Quit",
        "Emulator Menu",
        "Build Project",
        "Reload",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let bar_of = |rig: &WindowRig| {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(rig.size.x, ACTION_BAR_COMPACT_HEIGHT))
    };
    for menu in ["File", "Edit", "View", "Tools", "Help"] {
        // Each menu starts from a fresh window, nothing open.
        let mut rig = View::Plan.rig(Fixture::Starter);
        let baseline = text_set(&rig);
        let menus_open = |rig: &WindowRig| text_set(rig).len() > baseline.len() + 1;
        // Open the menu (a click on its name), closing what was open first.
        let open = |rig: &mut WindowRig| {
            for _ in 0..3 {
                close_popups(rig);
                settle(rig);
                if let Some(r) = super::grid_toolbar::text_in(rig, bar_of(rig), menu) {
                    rig.click(r.center());
                    frames(rig, 3);
                }
                if menus_open(rig) {
                    return;
                }
            }
        };
        open(&mut rig);
        let entries: Vec<(String, Rect)> = texts(&rig)
            .into_iter()
            .map(|(t, r)| (t.trim().to_string(), r))
            .filter(|(t, r)| {
                !t.is_empty()
                    && r.center().y > ACTION_BAR_COMPACT_HEIGHT
                    && r.center().y < ACTION_BAR_COMPACT_HEIGHT + 600.0
            })
            .filter(|(t, _)| !baseline.iter().any(|c| c.trim() == t))
            .collect();
        eprintln!(
            "{menu} menu entries: {:?}",
            entries.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>()
        );
        for (label, rect) in &entries {
            let plain = label
                .trim_start_matches(|c: char| !c.is_alphanumeric())
                .trim()
                .to_string();
            if skipped.iter().any(|s| plain.starts_with(s.as_str())) || plain.is_empty() {
                continue;
            }
            let base = rig.ws.project.clone();
            open(&mut rig);
            let rest = shapes_hash(&rig);
            let texts_open = text_set(&rig).len();
            rig.move_to(rect.center());
            frames(&mut rig, 2);
            if shapes_hash(&rig) == rest {
                total.labels += 1;
                continue;
            }
            if text_set(&rig).len() > texts_open {
                // Hovering it opens a submenu.
                total.alive += 1;
                continue;
            }
            // These are switched off by the code while there is nothing to
            // undo, redo or delete (workspace/input.rs add_enabled).
            if matches!(plain.as_str(), "Undo" | "Redo" | "Delete Selection")
                || plain.starts_with("Undo ")
                || plain.starts_with("Redo ")
            {
                total.labels += 1;
                continue;
            }
            close_popups(&mut rig);
            settle(&mut rig);
            let before = snap(&rig, Mode::Content);
            open(&mut rig);
            rig.click(rect.center());
            frames(&mut rig, 3);
            close_popups(&mut rig);
            close_popups(&mut rig);
            settle(&mut rig);
            if snap(&rig, Mode::Content) != before {
                total.alive += 1;
            } else {
                total.dead.push(format!("{menu} menu: {plain}"));
            }
            rig.ws.project = base;
        }
        close_popups(&mut rig);
    }
    (total, skipped)
}

/// The Edit tool's right-click menu, on a floor and on a wall.
pub(super) fn sweep_context_menus() -> Tally {
    let mut total = Tally::default();
    for on_wall in [false, true] {
        let make = move || {
            let mut rig = View::ThreeD.rig(Fixture::Starter);
            View::ThreeD.aim_near(&mut rig, [-1, -1], [7, 6], 0.55);
            assert!(start(&mut rig, GridTool::Edit, Start::Strip));
            let at = if on_wall {
                let (a, _) = View::ThreeD.wall(
                    &mut rig,
                    [2, 0],
                    psxed_project::grid::WallEdge::North,
                    0.5,
                    0.5,
                    0.0,
                    600.0,
                );
                a
            } else {
                View::ThreeD.cell(&rig, [2, 2])
            };
            (rig, at)
        };
        let (mut rig, at) = make();
        let closed = text_set(&rig);
        rig.secondary_click(at);
        let entries: Vec<(String, Rect)> = texts(&rig)
            .into_iter()
            .map(|(t, r)| (t.trim().to_string(), r))
            .filter(|(t, _)| !t.is_empty() && !closed.iter().any(|c| c.trim() == t))
            .collect();
        eprintln!(
            "context menu on a {}: {:?}",
            if on_wall { "wall" } else { "floor" },
            entries.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>()
        );
        for (label, _) in &entries {
            let (mut rig, at) = make();
            rig.secondary_click(at);
            let Some((_, rect)) = texts(&rig)
                .into_iter()
                .map(|(t, r)| (t.trim().to_string(), r))
                .find(|(t, _)| t == label)
            else {
                continue;
            };
            let rest = shapes_hash(&rig);
            rig.move_to(rect.center());
            frames(&mut rig, 2);
            if shapes_hash(&rig) == rest {
                total.labels += 1;
                continue;
            }
            let before = snap(&rig, Mode::Content);
            let sel_before = format!("{:?}", rig.ws.grid.selection);
            rig.click(rect.center());
            frames(&mut rig, 3);
            if snap(&rig, Mode::Content) != before
                || format!("{:?}", rig.ws.grid.selection) != sel_before
                || rig.ws.take_playtest_request().is_some()
            {
                total.alive += 1;
            } else {
                total.dead.push(format!(
                    "context menu ({}): {label}",
                    if on_wall { "wall" } else { "floor" }
                ));
            }
        }
    }
    total
}

/// Every tab of the Library drawer.
pub(super) fn sweep_library() -> Tally {
    use super::tool_matrix::lib_open;
    let mut total = Tally::default();
    for tab in ["Materials", "Meshes", "Actors", "Styles", "Buildings"] {
        let mut rig = View::Plan.rig(Fixture::Starter);
        View::Plan.aim(&mut rig, [-1, -3], [22, 8]);
        if tab == "Meshes" {
            assert!(start(&mut rig, GridTool::Arcade, Start::Strip));
            View::Plan.drag_cells(&mut rig, [1, 2], [4, 2]);
        }
        if !lib_open(&mut rig, tab) {
            total
                .dead
                .push(format!("Library: the {tab} tab does not open"));
            continue;
        }
        let tab_row = texts(&rig)
            .into_iter()
            .filter(|(t, r)| t.trim() == "Materials" && r.center().y > rig.size.y * 0.5)
            .map(|(_, r)| r)
            .min_by(|a, b| a.center().y.total_cmp(&b.center().y))
            .unwrap();
        let region = Rect::from_min_max(
            Pos2::new(tab_row.min.x - 40.0, tab_row.max.y + 2.0),
            Pos2::new(rig.size.x, rig.size.y),
        );
        let reopen = move |rig: &mut WindowRig| {
            let _ = lib_open(rig, tab);
        };
        total.add(sweep(
            &mut rig,
            &format!("Library {tab}"),
            region,
            Mode::Screen,
            true,
            &reopen,
            &reopen,
        ));
    }
    total
}

/// The Inspector form of each kind of thing that can be selected.
pub(super) fn sweep_inspector_forms() -> Tally {
    let mut total = Tally::default();
    // (what, how to select it)
    type Select = fn(&mut WindowRig) -> bool;
    let kinds: [(&str, Select); 8] = [
        ("World (nothing selected)", |rig| {
            rig.ws.clear_all_selections();
            frames(rig, 3);
            true
        }),
        ("floor face", |rig| {
            let p = View::Plan.cell(rig, [2, 2]);
            rig.click(p);
            true
        }),
        ("room", |rig| click_text(rig, "Starter room")),
        ("player actor", |rig| click_text(rig, "Aletha (Player)")),
        ("light", |rig| click_text(rig, "Starter Light")),
        ("lever", |rig| {
            if !start(rig, GridTool::Lever, Start::Strip) {
                return false;
            }
            View::Plan.click_cell(rig, [4, 2]);
            let _ = start(rig, GridTool::Edit, Start::Strip);
            click_text(rig, "Lever 1")
        }),
        ("checkpoint", |rig| {
            if !start(rig, GridTool::Checkpoint, Start::Strip) {
                return false;
            }
            View::Plan.click_cell(rig, [4, 2]);
            let _ = start(rig, GridTool::Edit, Start::Strip);
            click_text(rig, "Checkpoint 1")
        }),
        ("door", |rig| {
            if !start(rig, GridTool::Door, Start::Strip) {
                return false;
            }
            let (a, b) = View::Plan.wall(
                rig,
                [2, 0],
                psxed_project::grid::WallEdge::North,
                0.375,
                0.625,
                0.0,
                300.0,
            );
            rig.drag(a, b);
            let _ = start(rig, GridTool::Edit, Start::Strip);
            click_text(rig, "Door 1")
        }),
    ];
    for (what, select) in kinds {
        let mut rig = View::Plan.rig(Fixture::Starter);
        View::Plan.aim_near(&mut rig, [-1, -1], [7, 6], 1.0);
        assert!(start(&mut rig, GridTool::Edit, Start::Strip));
        if !select(&mut rig) {
            total
                .dead
                .push(format!("Inspector: could not select the {what}"));
            continue;
        }
        let after_select = rig.ws.project.clone();
        let sel_node = rig.ws.selection.selected_node;
        let sel_grid = rig.ws.grid.selection.clone();
        let region = Rect::from_min_max(
            Pos2::new(rig.size.x * 0.72, ACTION_BAR_COMPACT_HEIGHT + 60.0),
            Pos2::new(rig.size.x, rig.size.y - 40.0),
        );
        let reselect = move |rig: &mut WindowRig| {
            rig.ws.project = after_select.clone();
            rig.ws.selection.selected_node = sel_node;
            rig.ws.grid.selection = sel_grid.clone();
            frames(rig, 3);
        };
        let mut t = sweep(
            &mut rig,
            &format!("Inspector, {what}"),
            region,
            Mode::Screen,
            true,
            &reselect,
            &reselect,
        );
        // Not controls: the selectable headings and readouts of the form (the
        // selected thing's name and id, the section titles, the row names), and
        // Snap to Floor on an object that already stands on the floor.
        let names = [
            "Properties",
            "World",
            "Name",
            "Rotation",
            "Snap to Floor",
            "Lever 1",
            "Checkpoint 1",
            "Door 1",
        ];
        t.dead.retain(|d| {
            let label = d.split_once(": ").map_or(d.as_str(), |(_, l)| l);
            let label = d.rsplit(": ").next().unwrap_or(label);
            let plain = label
                .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '#')
                .trim();
            let heading = names.contains(&plain)
                || plain.starts_with('#')
                || plain.starts_with("Interaction")
                || plain.starts_with("Logic");
            if heading {
                t.labels += 1;
            }
            !heading
        });
        total.add(t);
    }
    total
}

#[test]
fn no_options_bar_control_is_dead_and_the_menus_have_no_dead_entry() {
    let bars = sweep_option_bars();
    eprintln!(
        "options bars: {} alive, {} labels, {} icon-only (hovered, not clicked), dead {:?}",
        bars.alive, bars.labels, bars.icon_only, bars.dead
    );
    let ctx = sweep_context_menus();
    eprintln!(
        "context menus: {} alive, {} labels, dead {:?}",
        ctx.alive, ctx.labels, ctx.dead
    );
    let lib = sweep_library();
    eprintln!(
        "library: {} alive, {} labels, {} icon-only, dead {:?}",
        lib.alive, lib.labels, lib.icon_only, lib.dead
    );
    let forms = sweep_inspector_forms();
    eprintln!(
        "inspector forms: {} alive, {} labels, {} icon-only, dead {:?}",
        forms.alive, forms.labels, forms.icon_only, forms.dead
    );
    let (menus, skipped) = sweep_menus();
    eprintln!(
        "menus: {} alive, {} labels, dead {:?} (skipped: {skipped:?})",
        menus.alive, menus.labels, menus.dead
    );
    if let Ok(path) = std::env::var("PSXED_CONTROLS_OUT") {
        let body = format!(
            "bars alive {} labels {} icon_only {} dead {}\nmenus alive {} labels {} dead {}\n{}\n{}\n",
            bars.alive, bars.labels, bars.icon_only, bars.dead.len(),
            menus.alive, menus.labels, menus.dead.len(),
            bars.dead.join("\n"), menus.dead.join("\n")
        );
        std::fs::write(path, body).unwrap();
    }
    let mut dead = bars.dead;
    dead.extend(menus.dead);
    dead.extend(ctx.dead);
    dead.extend(lib.dead);
    dead.extend(forms.dead);
    assert!(
        dead.is_empty(),
        "{} dead controls:\n{}",
        dead.len(),
        dead.join("\n")
    );
}
