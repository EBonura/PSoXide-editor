//! Authored wall runs and the overrides of derived walls.

use psxed_project::grid::edit::{
    place_wall_run, remove_wall, reset_derived, set_derived_hidden, set_material, set_solid,
    set_wall_facing, wall_run_cells, FaceRef, WallSpec,
};
use psxed_project::grid::{derive, GridWorld, WallFacing};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, face_label, parse_edge, pick_room, Cells, Materials};

fn parse_facing(name: Option<&str>) -> Result<WallFacing, String> {
    match name.map(str::to_ascii_lowercase).as_deref() {
        None | Some("front") => Ok(WallFacing::Front),
        Some("back") => Ok(WallFacing::Back),
        Some("both") => Ok(WallFacing::Both),
        Some(other) => Err(format!("unknown facing {other:?}; use front, back or both")),
    }
}

/// Place a run of authored walls along one edge.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceWallReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The run: for a north or south wall the cells along X (the row of
    /// `from`), for east or west the cells along Z (the column of `from`),
    /// for a diagonal the 45 degree line. Inclusive.
    #[serde(flatten)]
    pub cells: Cells,
    /// `north` (-Z side of each cell), `east` (+X), `south` (+Z), `west`
    /// (-X), `nwse` or `nesw`.
    pub edge: String,
    /// Which gap to fill when the edge has several: a room-relative height
    /// near it. Omit to fill from the bottom.
    pub pick_y: Option<i32>,
    /// Visible side: `front` (the interior of the owning cell, default),
    /// `back` or `both`.
    pub facing: Option<String>,
    /// Material name or id.
    pub material: Option<String>,
}

/// Place the run with the editor's gap-filling wall tool. A derived face on
/// an edge counts as a wall already standing, so the answer there is
/// "covered": derived rooms only need authored walls for what heights cannot
/// say (a freestanding wall, a parapet).
pub fn place_wall(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PlaceWallReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let edge = parse_edge(&req.edge)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let spec = WallSpec {
        facing: parse_facing(req.facing.as_deref())?,
        material: mats.resolve_opt(req.material.as_deref())?,
    };
    let start = req.cells.from;
    let end = req.cells.to.unwrap_or(start);
    let cells = wall_run_cells(start, end, edge);
    let outcome = place_wall_run(world, room, &cells, edge, req.pick_y, spec);
    let mut out = format!(
        "room {room}: placed {} {} wall(s) over {} cell(s); {} edge(s) had no gap left (covered)",
        outcome.placed.len(),
        edge.label(),
        cells.len(),
        outcome.covered
    );
    if let Some(first) = outcome.placed.first() {
        if let Some(wall) = first.wall_of(world) {
            out.push_str(&format!(
                "\nfirst wall at {:?}: heights [bottom-a, bottom-b, top-b, top-a] {:?}",
                first.cell, wall.heights
            ));
        }
    }
    if outcome.too_large {
        out.push_str("\nsome walls were refused: the room would pass its size cap");
    }
    if outcome.placed.is_empty() && outcome.covered > 0 {
        out.push_str(
            "\nNothing was placed. In a derived room the edge of a floor already has a closure wall and a step already has a riser; pick_y chooses another gap.",
        );
    }
    Ok(out)
}

/// Remove authored walls along an edge.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveWallReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The cells whose wall on `edge` goes (a rectangle, inclusive).
    #[serde(flatten)]
    pub cells: Cells,
    /// `north`, `east`, `south`, `west`, `nwse` or `nesw`.
    pub edge: String,
    /// Remove the wall nearest this room-relative height. Omit for the
    /// topmost wall of each stack.
    pub pick_y: Option<i32>,
}

/// Remove one wall per cell (the topmost, or the one nearest `pick_y`).
pub fn remove_walls(world: &mut GridWorld, req: &RemoveWallReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let edge = parse_edge(&req.edge)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let mut removed = 0usize;
    for cell in rect.cells() {
        if remove_wall(world, room, cell, edge, req.pick_y) {
            removed += 1;
        }
    }
    Ok(format!(
        "room {room}: removed {removed} authored {} wall(s) from {} cell(s). Derived walls are not records; hide one with grid_wall_override.",
        edge.label(),
        rect.count()
    ))
}

/// One wall face: a derived wall (by kind) or an authored one (by index).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct WallRef {
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The HOST sector `[x, z]`: the cell whose edge the wall is listed on.
    /// grid_walls and grid_sector print it.
    pub cell: [i32; 2],
    /// `north`, `east`, `south` or `west`.
    pub edge: String,
    /// A derived wall: `riser`, `drop` or `closure`. Omit when the edge has
    /// only one derived wall.
    pub kind: Option<String>,
    /// An authored wall: its index in the edge's stack (grid_sector lists it).
    pub index: Option<usize>,
}

/// Find the face a [`WallRef`] names, or say which candidates exist.
pub fn resolve_wall(world: &GridWorld, wall: &WallRef) -> Result<FaceRef, String> {
    let room = pick_room(world, wall.room)?;
    let edge = parse_edge(&wall.edge)?;
    let r = &world.rooms[room];
    // A diagonal derived face (a split-level riser, a void closure) is
    // addressed by its host cell, the diagonal and its kind.
    let derived: Vec<_> = derive::edge_walls(r, wall.cell, edge);
    let authored = r
        .local(wall.cell)
        .and_then(|[x, z]| r.sector(x, z))
        .map_or(0, |s| s.walls(edge).len());
    let candidates = || {
        let mut c: Vec<String> = derived
            .iter()
            .map(|d| format!("kind {:?}", d.kind.label()))
            .collect();
        c.extend((0..authored).map(|i| format!("index {i}")));
        if c.is_empty() {
            "none: that edge has no wall".to_string()
        } else {
            c.join(", ")
        }
    };
    if let Some(kind) = &wall.kind {
        let found = derived
            .iter()
            .find(|d| d.kind.label() == kind.to_ascii_lowercase());
        return found
            .map(|d| FaceRef::derived(room, wall.cell, edge, d.kind))
            .ok_or_else(|| {
                format!(
                    "no derived {kind} on the {} edge of {:?}; this edge has {}",
                    edge.label(),
                    wall.cell,
                    candidates()
                )
            });
    }
    if let Some(index) = wall.index {
        return if index < authored {
            Ok(FaceRef::wall(room, wall.cell, edge, index))
        } else {
            Err(format!(
                "no authored wall {index} on the {} edge of {:?}; this edge has {}",
                edge.label(),
                wall.cell,
                candidates()
            ))
        };
    }
    match (derived.as_slice(), authored) {
        ([only], _) => Ok(FaceRef::derived(room, wall.cell, edge, only.kind)),
        ([], 1) => Ok(FaceRef::wall(room, wall.cell, edge, 0)),
        _ => Err(format!(
            "the {} edge of {:?} needs `kind` or `index`: {}",
            edge.label(),
            wall.cell,
            candidates()
        )),
    }
}

/// Override one wall: hide it, make it solid or passable, paint it.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WallOverrideReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// Derived walls: hide (true) or show (false) the face. A hidden closure
    /// wall keeps its collision unless `solid` says otherwise.
    pub hidden: Option<bool>,
    /// Does it block movement? Derived closure walls block by default; a
    /// riser is left to the step rule unless forced solid; false makes a
    /// ghost wall you can walk through.
    pub solid: Option<bool>,
    /// Material name or id; "none" removes the override (derived) or the
    /// texture (authored).
    pub material: Option<String>,
    /// Authored walls only: `front`, `back` or `both`.
    pub facing: Option<String>,
    /// Derived walls: forget everything set on this segment ("Back to
    /// automatic").
    pub reset: Option<bool>,
    /// A derived closure wall of a floor with no ceiling: offset its top, grid
    /// units, at the two ends of its segment (in +X or +Z order for a
    /// cardinal edge): the crumbled top of an open rock wall. `[0, 0]` clears.
    pub top_delta: Option<[i32; 2]>,
}

/// Apply [`WallOverrideReq`].
pub fn wall_override(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &WallOverrideReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let derived = matches!(
        face.target,
        psxed_project::grid::edit::FaceTarget::Derived { .. }
    );
    let mut notes: Vec<String> = Vec::new();
    if req.reset.unwrap_or(false) {
        if !derived {
            return Err("reset applies to derived walls only".to_string());
        }
        reset_derived(world, face);
        notes.push("reset to automatic".to_string());
    }
    if let Some(hidden) = req.hidden {
        if !derived {
            return Err(
                "hidden applies to derived walls; delete an authored wall with grid_remove_wall"
                    .to_string(),
            );
        }
        set_derived_hidden(world, face, hidden);
        notes.push(if hidden { "hidden" } else { "shown" }.to_string());
    }
    if let Some(solid) = req.solid {
        set_solid(world, face, solid);
        notes.push(if solid { "solid" } else { "passable (ghost)" }.to_string());
    }
    if let Some(name) = &req.material {
        let material = mats.resolve_opt(Some(name))?;
        set_material(world, face, None, material);
        notes.push(format!("material {}", mats.label(material)));
    }
    if let Some(delta) = req.top_delta {
        if !derived {
            return Err("top_delta applies to derived closure walls; use grid_set_wall to change an authored wall's heights".to_string());
        }
        let to_i16 = |v: i32| {
            i16::try_from(v).map_err(|_| format!("top_delta {v} is out of range (at most 32767)"))
        };
        psxed_project::grid::edit::derived::set_top_delta(
            world,
            face,
            [to_i16(delta[0])?, to_i16(delta[1])?],
        );
        notes.push(format!("top offset {delta:?}"));
    }
    if let Some(facing) = &req.facing {
        if derived {
            return Err("facing applies to authored walls".to_string());
        }
        let facing = parse_facing(Some(facing))?;
        set_wall_facing(world, face, facing);
        notes.push(format!("facing {facing:?}"));
    }
    if notes.is_empty() {
        return Err(
            "nothing to change: give hidden, solid, material, facing, top_delta or reset"
                .to_string(),
        );
    }
    Ok(format!(
        "room {} {:?} {}: {}",
        face.room,
        face.cell,
        face_label(&face),
        notes.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::build::{set_heights, SetHeightsReq, Surface};

    use crate::grid::test_support::fixture;

    #[test]
    fn a_derived_closure_wall_can_be_hidden_and_reset() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let wall = WallRef {
            room: None,
            cell: [0, 0],
            edge: "north".into(),
            kind: None,
            index: None,
        };
        let out = wall_override(
            &mut w,
            &mats,
            &WallOverrideReq {
                scene: None,
                wall: wall.clone(),
                hidden: Some(true),
                solid: None,
                material: None,
                facing: None,
                reset: None,
                top_delta: None,
            },
        )
        .unwrap();
        assert!(out.contains("hidden"), "{out}");
        assert!(w.rooms[0].edge_overrides.len() == 1);
        wall_override(
            &mut w,
            &mats,
            &WallOverrideReq {
                scene: None,
                wall,
                hidden: None,
                solid: None,
                material: None,
                facing: None,
                reset: Some(true),
                top_delta: None,
            },
        )
        .unwrap();
        assert!(w.rooms[0].edge_overrides.is_empty());
    }

    #[test]
    fn an_edge_with_two_candidates_asks_which() {
        let (_, mut w) = fixture();
        // A raised platform next to a pit-free floor: one riser per edge.
        set_heights(
            &mut w,
            &SetHeightsReq {
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: Cells {
                    from: [2, 2],
                    to: None,
                },
                corners: None,
                edge: None,
                height: Some(512),
                delta: None,
                snap: None,
                link: None,
                layer: None,
            },
        )
        .unwrap();
        // Cell [2,1] sees the riser on its south edge.
        let found = resolve_wall(
            &w,
            &WallRef {
                room: None,
                cell: [2, 1],
                edge: "south".into(),
                kind: None,
                index: None,
            },
        )
        .unwrap();
        assert_eq!(face_label(&found), "south riser");
        let missing = resolve_wall(
            &w,
            &WallRef {
                room: None,
                cell: [1, 1],
                edge: "south".into(),
                kind: Some("riser".into()),
                index: None,
            },
        );
        assert!(missing.is_err());
    }

    #[test]
    fn authored_walls_place_and_remove() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        // Inside the room nothing is free: floor to ceiling derived walls
        // exist only at the outline, so a freestanding wall across the middle
        // fills the floor-to-ceiling gap.
        let out = place_wall(
            &mut w,
            &mats,
            &PlaceWallReq {
                scene: None,
                room: None,
                cells: Cells {
                    from: [2, 3],
                    to: Some([3, 3]),
                },
                edge: "north".into(),
                pick_y: None,
                facing: None,
                material: None,
            },
        )
        .unwrap();
        assert!(out.contains("placed 2"), "{out}");
        let gone = remove_walls(
            &mut w,
            &RemoveWallReq {
                scene: None,
                room: None,
                cells: Cells {
                    from: [2, 3],
                    to: Some([3, 3]),
                },
                edge: "north".into(),
                pick_y: None,
            },
        )
        .unwrap();
        assert!(gone.contains("removed 2"), "{gone}");
    }
}
