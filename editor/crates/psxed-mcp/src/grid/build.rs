//! Floors, ceilings, corner heights, the split diagonal.

use std::collections::BTreeSet;

use psxed_project::grid::edit::{
    create_surfaces, delete_surfaces, toggle_split, CornerRef, FaceRef, RectRefusal, SurfaceKind,
    WeldIndex,
};
use psxed_project::grid::overrides::WallMode;
use psxed_project::grid::{GridSplit, GridWorld};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, pick_room, Cells, Materials, Snap};

/// Floor or ceiling.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// The surface you stand on.
    Floor,
    /// The surface overhead.
    Ceiling,
}

impl Surface {
    /// The editor's kind.
    pub const fn kind(self) -> SurfaceKind {
        match self {
            Self::Floor => SurfaceKind::Floor,
            Self::Ceiling => SurfaceKind::Ceiling,
        }
    }
}

fn describe_refusal(refusal: RectRefusal) -> String {
    match refusal {
        RectRefusal::NoSuchRoom => "no such room".to_string(),
        RectRefusal::RoomTooLarge { needed } => format!(
            "the room would need {needed} sectors along one side; the cap is {}",
            psxed_project::grid::limits::MAX_ROOM_SIDE
        ),
    }
}

/// Add a flat floor or ceiling over a rectangle of sectors.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddSurfaceReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `floor` or `ceiling`.
    pub kind: Surface,
    /// The rectangle of WORLD sectors; the room grows to hold it.
    #[serde(flatten)]
    pub cells: Cells,
    /// Height in grid units, relative to the room's y_offset (1024 per
    /// sector, 256 per click). A floor is usually 0, a ceiling 2048.
    pub height: i32,
    /// `click` (default, 256) or `fine` (64): the snap the value goes through.
    pub snap: Option<Snap>,
    /// Material name or id (see `materials`). Omit for untextured.
    pub material: Option<String>,
}

/// Create the surfaces. Sectors that already have one are left untouched.
pub fn add_surface(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &AddSurfaceReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let (height, note) = req.snap.unwrap_or_default().apply(req.height);
    let material = mats.resolve_opt(req.material.as_deref())?;
    let outcome = create_surfaces(world, room, req.kind.kind(), rect, height, material);
    if let Some(refusal) = outcome.refused {
        return Err(describe_refusal(refusal));
    }
    let r = &world.rooms[room];
    let mut out = format!(
        "room {room}: {} {} created over {}x{} sectors [{}..{}, {}..{}] at {height} grid units ({} clicks), {} already had one{}",
        outcome.changed,
        req.kind.kind().label(),
        rect.width(),
        rect.depth(),
        rect.min[0],
        rect.max[0],
        rect.min[1],
        rect.max[1],
        super::clicks(height),
        outcome.skipped,
        if outcome.resized {
            format!("; the room grew to {}x{} from {:?}", r.width, r.depth, r.origin)
        } else {
            String::new()
        }
    );
    if let Some(note) = note {
        out.push_str(&format!("\n{note}"));
    }
    if outcome.skipped > 0 {
        out.push_str(
            "\nSectors that already had a surface were not changed; use grid_set_heights to move them.",
        );
    }
    if r.wall_mode == WallMode::Derived {
        out.push_str("\nWalls follow from the height differences (grid_walls lists them).");
    }
    Ok(out)
}

/// Remove floors or ceilings over a rectangle.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveSurfaceReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `floor` or `ceiling`.
    pub kind: Surface,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
}

/// Delete the surfaces; emptied sectors go and the room shrinks around what
/// is left.
pub fn remove_surface(world: &mut GridWorld, req: &RemoveSurfaceReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let outcome = delete_surfaces(world, room, req.kind.kind(), rect);
    if let Some(refusal) = outcome.refused {
        return Err(describe_refusal(refusal));
    }
    Ok(format!(
        "room {room}: removed {} {}(s), {} sector(s) had none{}",
        outcome.changed,
        req.kind.kind().label(),
        outcome.skipped,
        if outcome.resized {
            "; the room shrank around what is left"
        } else {
            ""
        }
    ))
}

/// Set or move corner heights of floors or ceilings.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetHeightsReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `floor` or `ceiling`.
    pub kind: Surface,
    /// The rectangle of WORLD sectors whose surfaces change.
    #[serde(flatten)]
    pub cells: Cells,
    /// Which corners of each sector: any of `nw`, `ne`, `se`, `sw`. Default
    /// all four (a whole face). Ignored when `edge` is given.
    pub corners: Option<Vec<String>>,
    /// The two corners of one side: `north` (nw+ne), `east` (ne+se), `south`
    /// (se+sw) or `west` (sw+nw). Use it to build a slope.
    pub edge: Option<String>,
    /// Absolute height, grid units relative to the room's y_offset.
    pub height: Option<i32>,
    /// Relative change in grid units instead of an absolute height.
    pub delta: Option<i32>,
    /// `click` (default) or `fine`: the snap the value goes through.
    pub snap: Option<Snap>,
    /// Move every corner that shares a position with a selected one, in any
    /// room, as the editor's linked drag does. Default false: only the named
    /// corners move, so a raised platform keeps its edge and gets a riser.
    pub link: Option<bool>,
    /// Height layer: 0 (default) or 1, the second triangle of a split-level
    /// surface (only when it has one).
    pub layer: Option<u8>,
}

fn corner_indices(req: &SetHeightsReq) -> Result<Vec<u8>, String> {
    let index = |name: &str| -> Result<u8, String> {
        match name.to_ascii_lowercase().as_str() {
            "nw" => Ok(0),
            "ne" => Ok(1),
            "se" => Ok(2),
            "sw" => Ok(3),
            other => Err(format!("unknown corner {other:?}; use nw, ne, se or sw")),
        }
    };
    if let Some(edge) = &req.edge {
        return match edge.to_ascii_lowercase().as_str() {
            "north" => Ok(vec![0, 1]),
            "east" => Ok(vec![1, 2]),
            "south" => Ok(vec![2, 3]),
            "west" => Ok(vec![3, 0]),
            other => Err(format!(
                "unknown edge {other:?}; use north, east, south or west"
            )),
        };
    }
    match &req.corners {
        None => Ok(vec![0, 1, 2, 3]),
        Some(list) if list.is_empty() => Err("corners is empty".to_string()),
        Some(list) => list.iter().map(|n| index(n)).collect(),
    }
}

/// Apply [`SetHeightsReq`].
pub fn set_heights(world: &mut GridWorld, req: &SetHeightsReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let corners = corner_indices(req)?;
    let layer = req.layer.unwrap_or(0);
    let snap = req.snap.unwrap_or_default();
    let (value, note) = match (req.height, req.delta) {
        (Some(h), None) => snap.apply(h),
        (None, Some(d)) => snap.apply(d),
        _ => return Err("give exactly one of height (absolute) or delta (relative)".to_string()),
    };
    let relative = req.delta.is_some();
    let link = req.link.unwrap_or(false);
    let mut targets: Vec<CornerRef> = Vec::new();
    let mut missing = 0usize;
    let mut touched: Vec<FaceRef> = Vec::new();
    for cell in rect.cells() {
        let face = match req.kind {
            Surface::Floor => FaceRef::floor(room, cell),
            Surface::Ceiling => FaceRef::ceiling(room, cell),
        };
        if !face.exists(world) {
            missing += 1;
            continue;
        }
        if layer == 1 && face.heights(world, 1).is_none() {
            return Err(format!(
                "the {} at {cell:?} has no second triangle layer",
                req.kind.kind().label()
            ));
        }
        touched.push(face);
        for &corner in &corners {
            targets.push(CornerRef {
                face,
                layer,
                corner,
            });
        }
    }
    if targets.is_empty() {
        return Err(format!(
            "no {} in room {room} over that rectangle ({missing} sector(s) have none); create them with grid_add_surface",
            req.kind.kind().label()
        ));
    }
    let mut changed = 0usize;
    if relative {
        // The editor's own height drag: welded corners ride along when linked.
        let before: Vec<Option<i32>> = targets.iter().map(|c| c.height(world)).collect();
        psxed_project::grid::edit::nudge(world, &targets, value, link);
        changed = targets
            .iter()
            .zip(&before)
            .filter(|(c, b)| c.height(world) != **b)
            .count();
    } else {
        let y_offset = world.rooms[room].y_offset;
        let index = link.then(|| WeldIndex::build(world));
        let mut all: BTreeSet<CornerRef> = targets.iter().copied().collect();
        if let Some(index) = &index {
            for corner in &targets {
                all.extend(index.coincident(world, corner));
            }
        }
        // `value` is relative to the named room's offset; welded partners in
        // other rooms take the same WORLD height.
        let world_y = y_offset + value;
        for corner in &all {
            if corner.set_world_height(world, world_y) {
                changed += 1;
            }
        }
    }
    let mut bent = 0usize;
    for face in &touched {
        if let Some(surface) = face.surface(world) {
            if !surface.is_planar() {
                bent += 1;
            }
        }
    }
    let mut out = format!(
        "room {room}: {} {} {} at {} corner(s) of {} sector(s); {changed} corner height(s) changed",
        req.kind.kind().label(),
        if relative { "raised by" } else { "set to" },
        value,
        corners.len(),
        touched.len()
    );
    if let Some(note) = note {
        out.push_str(&format!("\n{note}"));
    }
    if missing > 0 {
        out.push_str(&format!(
            "\n{missing} sector(s) in the rectangle had no {} and were skipped",
            req.kind.kind().label()
        ));
    }
    if bent > 0 {
        out.push_str(&format!(
            "\n{bent} surface(s) are now bent (not planar): the split diagonal decides which way they fold; grid_set_split flips it"
        ));
    }
    Ok(out)
}

/// Set the diagonal of floors or ceilings.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSplitReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `floor` or `ceiling`.
    pub kind: Surface,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `nw_se`, `ne_sw`, or `flip` (default).
    pub split: Option<String>,
}

/// Apply [`SetSplitReq`].
pub fn set_split(world: &mut GridWorld, req: &SetSplitReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let want = match req.split.as_deref() {
        None | Some("flip") => None,
        Some("nw_se") => Some(GridSplit::NwSe),
        Some("ne_sw") => Some(GridSplit::NeSw),
        Some(other) => return Err(format!("unknown split {other:?}; use nw_se, ne_sw or flip")),
    };
    let mut changed = 0usize;
    let mut missing = 0usize;
    for cell in rect.cells() {
        let face = match req.kind {
            Surface::Floor => FaceRef::floor(room, cell),
            Surface::Ceiling => FaceRef::ceiling(room, cell),
        };
        let Some(current) = face.surface(world).map(|s| s.split) else {
            missing += 1;
            continue;
        };
        if want.is_none_or(|w| w != current) && toggle_split(world, face) {
            changed += 1;
        }
    }
    Ok(format!(
        "room {room}: flipped the diagonal of {changed} {}(s); {missing} sector(s) had none",
        req.kind.kind().label()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::grid::test_support::fixture;

    fn cells(from: [i32; 2], to: [i32; 2]) -> Cells {
        Cells { from, to: Some(to) }
    }

    #[test]
    fn a_platform_is_a_surface_then_a_height_and_gets_risers() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = set_heights(
            &mut w,
            &SetHeightsReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: cells([2, 2], [3, 3]),
                corners: None,
                edge: None,
                height: Some(500),
                delta: None,
                snap: None,
                link: None,
                layer: None,
            },
        )
        .unwrap();
        assert!(out.contains("snapped to 512"), "{out}");
        let r = &w.rooms[0];
        assert_eq!(
            r.sector(2, 2).unwrap().floor.as_ref().unwrap().heights,
            [512; 4]
        );
        assert_eq!(
            r.sector(1, 2).unwrap().floor.as_ref().unwrap().heights,
            [0; 4]
        );
        // The risers come from the height difference alone.
        let walls = psxed_project::grid::derive::cell_walls(r, [1, 2]);
        assert!(!walls.is_empty());
        let _ = mats;
    }

    #[test]
    fn linked_heights_move_welded_neighbours_and_unlinked_do_not() {
        let (_, mut w) = fixture();
        let req = |link| SetHeightsReq {
            scene: None,
            room: None,
            kind: Surface::Floor,
            cells: Cells {
                from: [2, 2],
                to: None,
            },
            corners: Some(vec!["se".to_string()]),
            edge: None,
            height: Some(256),
            delta: None,
            snap: None,
            link: Some(link),
            layer: None,
        };
        let h = |w: &GridWorld, x, z| {
            w.rooms[0]
                .sector(x, z)
                .unwrap()
                .floor
                .as_ref()
                .unwrap()
                .heights
        };
        // Unlinked: only the named corner moves, the neighbour's stays.
        set_heights(&mut w, &req(false)).unwrap();
        assert_eq!(h(&w, 2, 2)[2], 256);
        assert_eq!(h(&w, 3, 3)[0], 0, "the neighbour's shared corner stays");
        // Linked, from a flat floor: the coincident corners of the three
        // other sectors around that lattice point follow.
        let (_, mut flat) = fixture();
        set_heights(&mut flat, &req(true)).unwrap();
        assert_eq!(h(&flat, 2, 2)[2], 256);
        assert_eq!(h(&flat, 3, 3)[0], 256, "linked: the shared corner follows");
        assert_eq!(h(&flat, 3, 2)[3], 256);
        assert_eq!(h(&flat, 2, 3)[1], 256);
    }

    #[test]
    fn a_relative_move_counts_the_corners_it_moved() {
        let (_, mut w) = fixture();
        let out = set_heights(
            &mut w,
            &SetHeightsReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: Cells {
                    from: [1, 1],
                    to: Some([2, 1]),
                },
                corners: None,
                edge: Some("north".into()),
                height: None,
                delta: Some(256),
                snap: None,
                link: None,
                layer: None,
            },
        )
        .unwrap();
        assert!(out.contains("raised by 256"), "{out}");
        assert!(out.contains("4 corner height(s) changed"), "{out}");
        let heights = w.rooms[0]
            .sector(1, 1)
            .unwrap()
            .floor
            .as_ref()
            .unwrap()
            .heights;
        assert_eq!(heights, [256, 256, 0, 0], "only the north edge rose");
        assert!(set_heights(
            &mut w,
            &SetHeightsReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: Cells {
                    from: [1, 1],
                    to: None
                },
                corners: None,
                edge: None,
                height: Some(0),
                delta: Some(256),
                snap: None,
                link: None,
                layer: None,
            },
        )
        .is_err());
    }

    #[test]
    fn surfaces_are_added_and_removed_and_the_room_follows() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = add_surface(
            &mut w,
            &mats,
            &AddSurfaceReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: cells([6, 0], [8, 2]),
                height: 256,
                snap: None,
                material: None,
            },
        )
        .unwrap();
        assert!(out.contains("9 floor"), "{out}");
        assert!(out.contains("grew"), "{out}");
        let out = remove_surface(
            &mut w,
            &RemoveSurfaceReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: cells([6, 0], [8, 2]),
            },
        )
        .unwrap();
        assert!(out.contains("removed 9"), "{out}");
        assert_eq!(w.rooms[0].width, 6, "the room shrank back");
    }

    #[test]
    fn split_flips_are_counted() {
        let (_, mut w) = fixture();
        let out = set_split(
            &mut w,
            &SetSplitReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: cells([0, 0], [1, 1]),
                split: Some("ne_sw".into()),
            },
        )
        .unwrap();
        assert!(out.contains("4 floor"), "{out}");
        let again = set_split(
            &mut w,
            &SetSplitReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: cells([0, 0], [1, 1]),
                split: Some("ne_sw".into()),
            },
        )
        .unwrap();
        assert!(again.contains("0 floor"), "{again}");
    }
}
