//! The Cells tool: make the plan finer where detail is needed and coarser
//! where it is not (phase T9, `docs/grid/level-tools-design.md` 3.2).
//!
//! * **Brush**: press and drag over the plan; every cell under the pointer
//!   is subdivided (one level, or down to a half or a quarter sector with
//!   the options row). Hold Alt to merge instead: the four cells that share
//!   a parent become it, when it reproduces them exactly. One undo step per
//!   stroke. `[` and `]` change the brush radius, in quarter sectors.
//! * **Selection**: the Subdivide and Merge buttons (Enter and Shift+Enter)
//!   act on the cells the Select tool picked.
//!
//! Subdividing changes how the room is stored, never what it looks like or
//! stands on; the tree is balanced afterwards, so a neighbour two levels
//! coarser is subdivided too and the hanging corners are constrained.

use std::collections::BTreeSet;

use super::view::{cell_at_editor, click_at_editor};
use super::*;

use psxed_project::grid::edit::GridElement;
use psxed_project::grid::tree::{self, MergeBlock};
use psxed_project::grid::LeafKey;

/// How far Subdivide goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellsTarget {
    /// One level finer than each cell is now.
    One,
    /// Down to half a sector.
    Half,
    /// Down to a quarter of a sector.
    Quarter,
}

impl CellsTarget {
    pub(crate) const ALL: [Self; 3] = [Self::One, Self::Half, Self::Quarter];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::One => "One level",
            Self::Half => "1/2 sector",
            Self::Quarter => "1/4 sector",
        }
    }

    /// The level cells are taken down to, `None` for one level at a time.
    const fn level(self) -> Option<u8> {
        match self {
            Self::One => None,
            Self::Half => Some(3),
            Self::Quarter => Some(4),
        }
    }
}

/// How far Merge goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MergeTop {
    /// Up to a whole sector.
    Sector,
    /// Up to two sectors across.
    Two,
    /// Up to four sectors across.
    Four,
}

impl MergeTop {
    pub(crate) const ALL: [Self; 3] = [Self::Sector, Self::Two, Self::Four];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Sector => "1 sector",
            Self::Two => "2 sectors",
            Self::Four => "4 sectors",
        }
    }

    /// The coarsest level a merge may produce.
    const fn level(self) -> u8 {
        match self {
            Self::Sector => 2,
            Self::Two => 1,
            Self::Four => 0,
        }
    }
}

/// What the options row keeps for the Cells tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CellsOptions {
    pub target: CellsTarget,
    pub merge_to: MergeTop,
    /// Brush radius in clicks (quarter sectors); 0 is the cell under the
    /// pointer.
    pub radius: i32,
}

impl Default for CellsOptions {
    fn default() -> Self {
        Self {
            target: CellsTarget::One,
            merge_to: MergeTop::Sector,
            radius: 0,
        }
    }
}

/// Largest brush radius, clicks.
const MAX_RADIUS: i32 = 8;

/// A stroke in flight.
#[derive(Clone, Debug)]
pub(crate) struct CellsStroke {
    /// The document before the stroke, recorded for undo if it changed.
    before: Box<ProjectDocument>,
    merge: bool,
    /// Leaves this stroke made: not subdivided or merged again by it.
    fresh: BTreeSet<LeafKey>,
    changed: usize,
    refused: Option<MergeBlock>,
    /// Cells the stroke would have divided that hold several storeys.
    storeys: usize,
}

/// The room and click under the pointer.
fn click_under(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(usize, [i32; 2])> {
    let world = ws.project.active_scene().world.grid()?;
    let scale = world.scale;
    let (room, x, z) = match ptr.view {
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            let cell = cell_at_editor(scale, x, z);
            let room = ws
                .top_room_at(cell)
                .unwrap_or_else(|| ws.grid_target_room());
            (room, x, z)
        }
        GridView::Persp(rect) => {
            let hit = ws.pick_face_3d(rect, ptr.pos)?;
            (hit.face.room, hit.point[0], hit.point[2])
        }
    };
    Some((room, click_at_editor(scale, x, z)))
}

/// The leaves a brush of `radius` clicks at `click` touches, each once.
fn leaves_under(
    room: &psxed_project::grid::GridRoom,
    click: [i32; 2],
    radius: i32,
) -> Vec<LeafKey> {
    let mut keys = BTreeSet::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz <= radius * radius {
                if let Some((key, _)) = room.leaf_at([click[0] + dx, click[1] + dz]) {
                    keys.insert(key);
                }
            }
        }
    }
    keys.into_iter().collect()
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = GridHover::None;
    let Some((room, click)) = click_under(ws, ptr) else {
        return;
    };
    let radius = ws.grid.cells.radius;
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let Some(r) = world.rooms.get(room) else {
        return;
    };
    if let Some(&key) = leaves_under(r, click, radius).first() {
        ws.grid.hover = GridHover::Leaf { room, key };
    }
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let stroke = CellsStroke {
        before: Box::new(ws.project.clone()),
        merge: ptr.alt(),
        fresh: BTreeSet::new(),
        changed: 0,
        refused: None,
        storeys: 0,
    };
    ws.grid.gesture = Gesture::Cells(Box::new(stroke));
    drag(ws, &ptr.at_press());
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Cells(mut stroke) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    if let Some((room, click)) = click_under(ws, ptr) {
        act(ws, &mut stroke, room, click);
    }
    ws.grid.gesture = Gesture::Cells(stroke);
}

fn act(ws: &mut EditorWorkspace, stroke: &mut CellsStroke, room: usize, click: [i32; 2]) {
    let opts = ws.grid.cells;
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let Some(r) = world.rooms.get_mut(room) else {
        return;
    };
    let mut did = 0;
    match (stroke.merge, opts.target.level()) {
        (false, Some(level)) => {
            // Dig down to the target at every click the brush covers.
            let radius = opts.radius;
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dz * dz > radius * radius {
                        continue;
                    }
                    let at = [click[0] + dx, click[1] + dz];
                    let before = r.leaf_count();
                    match tree::subdivide_to(r, at, level) {
                        Ok(_) => did += usize::from(r.leaf_count() != before),
                        Err(tree::SubdivideBlock::Storeys) => stroke.storeys += 1,
                        Err(_) => {}
                    }
                }
            }
        }
        _ => {
            for key in leaves_under(r, click, opts.radius) {
                if stroke.fresh.contains(&key) {
                    continue;
                }
                if stroke.merge {
                    let Some(parent) = key.parent().filter(|p| p.level >= opts.merge_to.level())
                    else {
                        continue;
                    };
                    match tree::merge(r, parent) {
                        Ok(()) => {
                            stroke.fresh.insert(parent);
                            did += 1;
                        }
                        Err(block) => stroke.refused = Some(block),
                    }
                } else {
                    match tree::subdivide(r, key) {
                        Ok(children) => {
                            stroke.fresh.extend(children);
                            did += 1;
                        }
                        Err(tree::SubdivideBlock::Storeys) => stroke.storeys += 1,
                        Err(_) => {}
                    }
                }
            }
        }
    }
    if did > 0 {
        stroke.changed += did;
        ws.grid_changed();
    }
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    let Gesture::Cells(stroke) = std::mem::take(&mut ws.grid.gesture) else {
        return;
    };
    let leaves = ws
        .project
        .active_scene()
        .world
        .grid()
        .map_or(0, |w| w.rooms.iter().map(|r| r.leaf_count()).sum::<usize>());
    if stroke.changed > 0 {
        ws.history.record(*stroke.before);
        ws.status = format!(
            "{} {} cells, {leaves} leaves in the world",
            if stroke.merge { "Merged" } else { "Subdivided" },
            stroke.changed
        );
    } else if let Some(block) = stroke.refused {
        ws.status = format!("Cannot merge: {}", block.describe());
    } else if stroke.storeys > 0 {
        ws.status = format!(
            "Cannot subdivide: {}",
            tree::SubdivideBlock::Storeys.describe()
        );
    } else if stroke.merge {
        ws.status = "Nothing to merge there".to_string();
    } else {
        ws.status = "Those cells are already as fine as the target".to_string();
    }
}

/// `[` and `]`: the brush radius.
pub(super) fn adjust_radius(ws: &mut EditorWorkspace, delta: i32) {
    let radius = (ws.grid.cells.radius + delta).clamp(0, MAX_RADIUS);
    ws.grid.cells.radius = radius;
    ws.status = format!("Cells brush radius: {radius} (quarter sectors)");
}

/// The leaves the Select tool's selection names, per room.
fn selected_leaves(ws: &EditorWorkspace) -> Vec<(usize, LeafKey)> {
    let Some(world) = ws.project.active_scene().world.grid() else {
        return Vec::new();
    };
    let mut out: BTreeSet<(usize, LeafKey)> = BTreeSet::new();
    for item in ws.grid.selection.items() {
        match item {
            GridElement::Cell { room, cell } => {
                if let Some(r) = world.rooms.get(*room) {
                    out.extend(
                        r.leaves
                            .in_sector(*cell)
                            .into_iter()
                            .map(|(k, _)| (*room, k)),
                    );
                }
            }
            GridElement::Face(f) => {
                out.insert((f.room, f.leaf));
            }
            GridElement::Edge(e) => {
                out.insert((e.face.room, e.face.leaf));
            }
            GridElement::Vertex(v) => {
                out.insert((v.face.room, v.face.leaf));
            }
        }
    }
    out.into_iter().collect()
}

/// Subdivide (or, with `merge`, merge) the cells of the selection as one
/// undo step. Returns whether anything changed.
pub(super) fn apply_selection(ws: &mut EditorWorkspace, merge: bool) -> bool {
    let picked = selected_leaves(ws);
    if picked.is_empty() {
        ws.status = "Select cells first (V), or paint with the Cells brush".to_string();
        return false;
    }
    let opts = ws.grid.cells;
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    let mut did = 0;
    let mut refused = None;
    let mut rooms: Vec<usize> = picked.iter().map(|(r, _)| *r).collect();
    rooms.sort_unstable();
    rooms.dedup();
    for room in rooms {
        let Some(r) = world.rooms.get_mut(room) else {
            continue;
        };
        let keys: Vec<LeafKey> = picked
            .iter()
            .filter(|(rm, k)| *rm == room && r.leaf(*k).is_some())
            .map(|(_, k)| *k)
            .collect();
        if merge {
            let merged = tree::merge_all(r, Some(&keys), opts.merge_to.level());
            did += merged;
            if merged == 0 {
                refused = keys
                    .iter()
                    .filter_map(|k| k.parent())
                    .find_map(|p| tree::can_merge(r, p).err());
            }
        } else {
            did += match opts.target.level() {
                Some(level) => tree::subdivide_all_to(r, &keys, level),
                None => keys
                    .iter()
                    .filter(|k| tree::subdivide(r, **k).is_ok())
                    .count(),
            };
        }
    }
    if did > 0 {
        // The picked leaves are gone or joined: the selection becomes the
        // sectors they lay in, so the next Subdivide or Merge finds what is
        // there now.
        let cells: BTreeSet<(usize, [i32; 2])> = picked
            .iter()
            .flat_map(|(room, key)| {
                let (lo, hi) = key.sector_range();
                (lo[1]..=hi[1]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| (*room, [x, z])))
            })
            .collect();
        ws.history.record(before);
        ws.grid.selection.set(
            cells
                .into_iter()
                .map(|(room, cell)| GridElement::Cell { room, cell }),
        );
        ws.grid_changed();
        ws.after_grid_selection_changed();
        ws.status = format!(
            "{} {did} cells",
            if merge { "Merged" } else { "Subdivided" }
        );
        true
    } else {
        ws.status = match (merge, refused) {
            (true, Some(block)) => format!("Cannot merge: {}", block.describe()),
            (true, None) => "Nothing to merge in the selection".to_string(),
            (false, _) => "The selection is already as fine as the target".to_string(),
        };
        false
    }
}
