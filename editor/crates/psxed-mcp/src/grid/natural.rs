//! Natural terrain, part one: cracks, void triangles and direct wall heights.
//!
//! Thin wrappers over `psxed_project::grid::edit::natural`, the functions the
//! editor's gestures and the Break verb write with.

use std::fmt::Write as _;

use psxed_project::grid::derive;
use psxed_project::grid::edit::natural::{self, Triangles};
use psxed_project::grid::edit::outline::{
    self, corridor_cells, cut_corners, fill_cells, polygon_cells, restore_voids, CornerCut,
    FillSpec,
};
use psxed_project::grid::edit::FaceRef;
use psxed_project::grid::faces::face_count;
use psxed_project::grid::GridWorld;
use schemars::JsonSchema;
use serde::Deserialize;

use super::build::Surface;
use super::cells::{click_rect, Unit};
use super::walls::{resolve_wall, WallRef};
use super::{check_rect, parse_edge, pick_room, Cells, Materials, Snap, MAX_CELLS_PER_CALL};

/// Which triangle of a cell.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Triangle {
    /// The first triangle: NW-NE-SE for the nw_se split, NW-NE-SW for ne_sw.
    A,
    /// The second triangle: NW-SE-SW for nw_se, NE-SE-SW for ne_sw.
    B,
    /// Both.
    Both,
    /// Neither: used by `grid_void` to restore.
    None,
}

fn tri_index(triangle: Triangle) -> Option<usize> {
    match triangle {
        Triangle::A => Some(0),
        Triangle::B => Some(1),
        _ => None,
    }
}

/// Move one triangle of a floor or ceiling to its own level.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CrackReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `floor` (default) or `ceiling`.
    pub kind: Option<Surface>,
    /// `a` or `b` (default `b`). Which triangle that is depends on the
    /// diagonal: grid_sector prints it.
    pub triangle: Option<Triangle>,
    /// Offset the triangle by this many grid units (positive raises). The
    /// surface becomes split-level first if it was not.
    pub delta: Option<i32>,
    /// Or set every corner of the triangle to this height (relative to the
    /// room's y_offset), flat.
    pub height: Option<i32>,
    /// Merge the two triangles back into one plane (the first triangle's
    /// heights win).
    pub heal: Option<bool>,
    /// `click` (default) or `fine`.
    pub snap: Option<Snap>,
    /// What the corners count: `sector` (default) or `click` (quarter
    /// sectors). In a room with cells of several sizes either way names the
    /// cells that touch the rectangle, whatever their size.
    pub unit: Option<Unit>,
}

/// Apply [`CrackReq`].
pub fn crack(world: &mut GridWorld, req: &CrackReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    if req.unit == Some(Unit::Click) || !world.rooms[room].leaves.is_sectors_only() {
        return crack_leaves(world, room, req);
    }
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let kind = req.kind.unwrap_or(Surface::Floor).kind();
    let heal = req.heal.unwrap_or(false);
    let tri = tri_index(req.triangle.unwrap_or(Triangle::B))
        .ok_or("triangle must be a or b for a crack")?;
    let snap = req.snap.unwrap_or_default();
    let (mut cracked, mut healed, mut missing) = (0usize, 0usize, 0usize);
    match (req.delta, req.height, heal) {
        (Some(_), Some(_), _) => {
            return Err("give delta, height or heal, not several".to_string());
        }
        (None, None, false) => return Err("give delta, height or heal".to_string()),
        _ => {}
    }
    for cell in rect.cells() {
        let face = FaceRef::new(
            room,
            psxed_project::grid::LeafKey::sector(cell),
            psxed_project::grid::edit::FaceTarget::Surface(kind),
        );
        if !face.exists(world) {
            missing += 1;
            continue;
        }
        if heal {
            healed += usize::from(natural::heal(world, room, cell, kind));
        } else if let Some(delta) = req.delta {
            cracked += usize::from(natural::crack(
                world,
                room,
                cell,
                kind,
                tri,
                snap.apply(delta).0,
            ));
        } else if let Some(height) = req.height {
            let h = snap.apply(height).0;
            cracked += usize::from(natural::set_triangle_heights(
                world, room, cell, kind, tri, [h; 4],
            ));
        }
    }
    let r = &world.rooms[room];
    let mut diag = 0usize;
    for cell in rect.cells() {
        diag += derive::cell_walls(r, cell)
            .iter()
            .filter(|w| w.edge.is_diagonal() && w.host == cell)
            .count();
    }
    let mut out = if heal {
        format!(
            "room {room}: {healed} {} surface(s) merged back into one plane",
            kind.label()
        )
    } else {
        format!(
            "room {room}: {cracked} {} triangle {} changed over {} sector(s)",
            kind.label(),
            if tri == 0 { "A" } else { "B" },
            rect.count()
        )
    };
    if missing > 0 {
        let _ = write!(
            out,
            "\n{missing} sector(s) had no {} and were skipped",
            kind.label()
        );
    }
    let _ = write!(
        out,
        "\n{diag} derived diagonal wall face(s) now stand in this rectangle{}",
        if r.wall_mode == psxed_project::grid::WallMode::Derived {
            " (grid_walls lists them)"
        } else {
            " (an authored room draws none: convert it with grid_convert_room)"
        }
    );
    Ok(out)
}

/// [`crack`] on the cells (leaves of any size) that touch the rectangle.
fn crack_leaves(world: &mut GridWorld, room: usize, req: &CrackReq) -> Result<String, String> {
    let (lo, hi) = click_rect(&req.cells, req.unit.unwrap_or_default())?;
    let kind = req.kind.unwrap_or(Surface::Floor).kind();
    let heal = req.heal.unwrap_or(false);
    let tri = tri_index(req.triangle.unwrap_or(Triangle::B))
        .ok_or("triangle must be a or b for a crack")?;
    let snap = req.snap.unwrap_or_default();
    match (req.delta, req.height, heal) {
        (Some(_), Some(_), _) => return Err("give delta, height or heal, not several".to_string()),
        (None, None, false) => return Err("give delta, height or heal".to_string()),
        _ => {}
    }
    let keys = touching_leaves(&world.rooms[room], lo, hi);
    if keys.is_empty() {
        return Err("no cell lies in that rectangle".to_string());
    }
    let (mut changed, mut missing, mut refused) = (0usize, 0usize, 0usize);
    for key in &keys {
        let has = world.rooms[room].leaf(*key).is_some_and(|l| match kind {
            psxed_project::grid::edit::SurfaceKind::Floor => l.ground_floor().is_some(),
            psxed_project::grid::edit::SurfaceKind::Ceiling => l.ground_ceiling().is_some(),
        });
        if !has {
            missing += 1;
            continue;
        }
        // Split-level surfaces live on cells of a sector or smaller; a larger
        // cell is split to sectors first (lossless).
        if key.level < psxed_project::grid::leaf::SECTOR_LEVEL && !heal {
            psxed_project::grid::tree::subdivide_all_to(
                &mut world.rooms[room],
                &[*key],
                psxed_project::grid::leaf::SECTOR_LEVEL,
            );
        }
    }
    let keys = touching_leaves(&world.rooms[room], lo, hi);
    for key in &keys {
        if key.level < psxed_project::grid::leaf::SECTOR_LEVEL && !heal {
            refused += 1;
            continue;
        }
        let ok = if heal {
            natural::heal_leaf(world, room, *key, kind)
        } else if let Some(delta) = req.delta {
            natural::crack_leaf(world, room, *key, kind, tri, snap.apply(delta).0)
        } else if let Some(height) = req.height {
            let h = snap.apply(height).0;
            natural::set_triangle_heights_leaf(world, room, *key, kind, tri, [h; 4])
        } else {
            false
        };
        changed += usize::from(ok);
    }
    psxed_project::grid::tree::normalize(&mut world.rooms[room]);
    let r = &world.rooms[room];
    let diag: usize = keys
        .iter()
        .map(|k| psxed_project::grid::leaf_diag::diagonal_face_count(r, *k, None))
        .sum();
    let mut out = if heal {
        format!(
            "room {room}: {changed} {} surface(s) merged back into one plane",
            kind.label()
        )
    } else {
        format!(
            "room {room}: {changed} {} triangle {} changed over {} cell(s) (cells of any size touching the rectangle)",
            kind.label(),
            if tri == 0 { "A" } else { "B" },
            keys.len()
        )
    };
    if missing > 0 {
        let _ = write!(
            out,
            "\n{missing} cell(s) had no {} and were skipped",
            kind.label()
        );
    }
    if refused > 0 {
        let _ = write!(
            out,
            "\n{refused} cell(s) could not be split to a sector and were skipped"
        );
    }
    let _ = write!(
        out,
        "\n{diag} derived diagonal wall face(s) now stand on these cells (grid_walls lists them)"
    );
    Ok(out)
}

/// The distinct leaves of a room that touch a rectangle of clicks, in map
/// order.
fn touching_leaves(
    room: &psxed_project::grid::GridRoom,
    lo: [i32; 2],
    hi: [i32; 2],
) -> Vec<psxed_project::grid::LeafKey> {
    let mut keys = std::collections::BTreeSet::new();
    for z in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            if let Some((key, _)) = room.leaf_at([x, z]) {
                keys.insert(key);
            }
        }
    }
    keys.into_iter().collect()
}

/// Void or restore triangles: half a cell with nothing in it.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoidReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `floor`, `ceiling` or omit for both.
    pub kind: Option<Surface>,
    /// `a`, `b` or `both` to void; `none` restores both.
    pub triangle: Triangle,
    /// What the corners count: `sector` (default) or `click`. In a room with
    /// cells of several sizes either way names the cells that touch the
    /// rectangle.
    pub unit: Option<Unit>,
}

/// Apply [`VoidReq`].
pub fn void(world: &mut GridWorld, req: &VoidReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    if req.unit == Some(Unit::Click) || !world.rooms[room].leaves.is_sectors_only() {
        return void_leaves(world, room, req);
    }
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let (which, set) = match req.triangle {
        Triangle::A => (Triangles::A, true),
        Triangle::B => (Triangles::B, true),
        Triangle::Both => (Triangles::Both, true),
        Triangle::None => (Triangles::Both, false),
    };
    let kinds: Vec<_> = match req.kind {
        Some(k) => vec![k.kind()],
        None => vec![
            psxed_project::grid::edit::SurfaceKind::Floor,
            psxed_project::grid::edit::SurfaceKind::Ceiling,
        ],
    };
    let mut changed = 0usize;
    for cell in rect.cells() {
        for &kind in &kinds {
            changed += usize::from(natural::set_void(world, room, cell, kind, which, set));
        }
    }
    let r = &world.rooms[room];
    let closures: usize = rect
        .cells()
        .map(|c| {
            derive::cell_walls(r, c)
                .iter()
                .filter(|w| w.edge.is_diagonal() && w.kind == derive::DerivedKind::Closure)
                .count()
        })
        .sum();
    Ok(format!(
        "room {room}: {changed} surface(s) {} over {} sector(s); {closures} diagonal closure wall(s) stand in this rectangle\nA void triangle draws nothing and the other half of the cell closes itself along the diagonal (derived rooms). Collision keeps the floor under it; the closure wall stops bodies.",
        if set { "voided" } else { "restored" },
        rect.count()
    ))
}

/// [`void`] on the cells (leaves of any size) that touch the rectangle.
fn void_leaves(world: &mut GridWorld, room: usize, req: &VoidReq) -> Result<String, String> {
    let (lo, hi) = click_rect(&req.cells, req.unit.unwrap_or_default())?;
    let (which, set) = match req.triangle {
        Triangle::A => (Triangles::A, true),
        Triangle::B => (Triangles::B, true),
        Triangle::Both => (Triangles::Both, true),
        Triangle::None => (Triangles::Both, false),
    };
    let kinds: Vec<_> = match req.kind {
        Some(k) => vec![k.kind()],
        None => vec![
            psxed_project::grid::edit::SurfaceKind::Floor,
            psxed_project::grid::edit::SurfaceKind::Ceiling,
        ],
    };
    let keys = touching_leaves(&world.rooms[room], lo, hi);
    if keys.is_empty() {
        return Err("no cell lies in that rectangle".to_string());
    }
    if set {
        // A void triangle needs a cell of a sector or smaller.
        psxed_project::grid::tree::subdivide_all_to(
            &mut world.rooms[room],
            &keys,
            psxed_project::grid::leaf::SECTOR_LEVEL,
        );
    }
    let keys = touching_leaves(&world.rooms[room], lo, hi);
    let mut changed = 0usize;
    for key in &keys {
        if key.level < psxed_project::grid::leaf::SECTOR_LEVEL && set {
            continue;
        }
        for &kind in &kinds {
            changed += usize::from(natural::set_void_leaf(world, room, *key, kind, which, set));
        }
    }
    psxed_project::grid::tree::normalize(&mut world.rooms[room]);
    let r = &world.rooms[room];
    let closures: usize = keys
        .iter()
        .map(|k| {
            psxed_project::grid::leaf_diag::diagonal_face_count(
                r,
                *k,
                Some(derive::DerivedKind::Closure),
            )
        })
        .sum();
    Ok(format!(
        "room {room}: {changed} surface(s) {} over {} cell(s) of any size; {closures} diagonal closure wall(s) stand on them\nA void triangle draws nothing and the other half of the cell closes itself along the diagonal (derived rooms). Collision keeps the floor under it; the closure wall stops bodies.",
        if set { "voided" } else { "restored" },
        keys.len()
    ))
}

/// Set an authored wall's four heights, or crumble the top of a derived one.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetWallReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall: host cell, edge (any of north, east, south, west, nwse,
    /// nesw) and `index` for an authored wall or `kind` for a derived one.
    #[serde(flatten)]
    pub wall: WallRef,
    /// `[bottom a, bottom b, top b, top a]` in grid units relative to the
    /// room's y_offset, walking a -> b along the edge (grid_sector prints
    /// them). A slanted top or bottom is unequal ends.
    pub heights: Option<[i32; 4]>,
    /// Add to the top at the two ends `[a, b]` of an authored wall, or (for a
    /// derived closure of a floor with no ceiling) offset its top.
    pub top_delta: Option<[i32; 2]>,
    /// Add to the bottom at the two ends `[a, b]` of an authored wall.
    pub bottom_delta: Option<[i32; 2]>,
    /// `click` (default) or `fine`.
    pub snap: Option<Snap>,
}

/// Apply [`SetWallReq`].
pub fn set_wall(world: &mut GridWorld, req: &SetWallReq) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let snap = req.snap.unwrap_or_default();
    let edge = parse_edge(&req.wall.edge)?;
    match face.target {
        psxed_project::grid::edit::FaceTarget::Wall { index, .. } => {
            let current = face.wall_of(world).ok_or("the wall vanished")?.heights;
            let mut heights = match req.heights {
                Some(h) => h.map(|v| snap.apply(v).0),
                None => current,
            };
            if let Some([a, b]) = req.top_delta {
                heights[3] += snap.apply(a).0;
                heights[2] += snap.apply(b).0;
            }
            if let Some([a, b]) = req.bottom_delta {
                heights[0] += snap.apply(a).0;
                heights[1] += snap.apply(b).0;
            }
            if req.heights.is_none() && req.top_delta.is_none() && req.bottom_delta.is_none() {
                return Err("give heights, top_delta or bottom_delta".to_string());
            }
            natural::set_wall_heights(world, face.room, face.cell(), edge, index, heights);
            Ok(format!(
                "room {} {:?} {} wall {index}: heights [bottom a, bottom b, top b, top a] {:?} -> {:?}",
                face.room,
                face.cell(),
                edge.label(),
                current,
                heights
            ))
        }
        psxed_project::grid::edit::FaceTarget::Derived { .. } => {
            let delta = req.top_delta.ok_or(
                "a derived wall follows the floors; only top_delta (the crumbled top of an open-topped closure) can be set on it, and heights or bottom_delta need an authored wall",
            )?;
            let to_i16 = |v: i32| {
                i16::try_from(snap.apply(v).0).map_err(|_| format!("top_delta {v} is out of range"))
            };
            let kind = face.derived_wall(world).map(|w| w.kind);
            if kind != Some(derive::DerivedKind::Closure) {
                return Err(
                    "top_delta only applies to a closure wall (grid_walls lists them)".to_string(),
                );
            }
            psxed_project::grid::edit::derived::set_top_delta(
                world,
                face,
                [to_i16(delta[0])?, to_i16(delta[1])?],
            );
            Ok(format!(
                "room {} {:?} {}: top offset {:?} at the two ends of the segment",
                face.room,
                face.cell(),
                edge.label(),
                delta
            ))
        }
        _ => Err("name a wall".to_string()),
    }
}

/// Fill a polygon or a winding corridor with floors and cut its outline at 45
/// degrees.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PolygonReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Points in SECTOR coordinates `[x, z]` (numbers; an integer is a
    /// sector corner, so cell `[3, 2]` has its centre at `[3.5, 2.5]`). A
    /// `room` or `region` is the closed polygon through them; a `corridor`
    /// is the open polyline.
    pub points: Vec<[f64; 2]>,
    /// `room` (fill the polygon, default), `corridor` (a path `width` wide) or
    /// `region` (compute and report the cells, change nothing).
    pub kind: Option<String>,
    /// Corridor width in sectors (default 2).
    pub width: Option<f64>,
    /// Floor height, grid units relative to the room's y_offset (default 0).
    pub floor_height: Option<i32>,
    /// Ceiling height. Omit for open sky (a canyon); give it for a cave.
    pub ceiling_height: Option<i32>,
    /// `corners` (default) cuts every convex staircase corner into a 45
    /// degree line with a void triangle and a diagonal wall; `none` leaves
    /// the staircase.
    pub chamfer: Option<String>,
    /// Floor material (name or id).
    pub material: Option<String>,
    /// Ceiling material.
    pub ceiling_material: Option<String>,
    /// Set the height of floors that already exist in the footprint too
    /// (default false: they are kept).
    pub overwrite: Option<bool>,
    /// `click` (default) or `fine`.
    pub snap: Option<Snap>,
}

/// Apply [`PolygonReq`].
pub fn polygon(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PolygonReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let kind = req.kind.as_deref().unwrap_or("room");
    let cells = match kind {
        "room" | "region" => polygon_cells(&req.points),
        "corridor" => corridor_cells(&req.points, req.width.unwrap_or(2.0)),
        other => {
            return Err(format!(
                "unknown kind {other:?}; use room, corridor or region"
            ))
        }
    };
    if cells.is_empty() {
        return Err("the outline contains no cell centre (a room needs three points)".to_string());
    }
    if cells.len() > MAX_CELLS_PER_CALL {
        return Err(format!(
            "that outline covers {} sectors; one call handles at most {MAX_CELLS_PER_CALL}",
            cells.len()
        ));
    }
    if kind == "region" {
        let list: Vec<String> = cells.iter().take(40).map(|c| format!("{c:?}")).collect();
        return Ok(format!(
            "region: {} sector(s){}: {}{}",
            cells.len(),
            "",
            list.join(" "),
            if cells.len() > 40 { " ..." } else { "" }
        ));
    }
    let chamfer = match req.chamfer.as_deref().unwrap_or("corners") {
        "corners" => true,
        "none" => false,
        other => return Err(format!("unknown chamfer {other:?}; use corners or none")),
    };
    let snap = req.snap.unwrap_or_default();
    let spec = FillSpec {
        floor: snap.apply(req.floor_height.unwrap_or(0)).0,
        ceiling: req.ceiling_height.map(|h| snap.apply(h).0),
        floor_material: mats.resolve_opt(req.material.as_deref())?,
        ceiling_material: mats.resolve_opt(req.ceiling_material.as_deref())?,
        overwrite: req.overwrite.unwrap_or(false),
    };
    let faces_before = face_count(world);
    let filled = fill_cells(world, room, &cells, spec).map_err(|e| format!("{e:?}"))?;
    let list: Vec<[i32; 2]> = cells.iter().copied().collect();
    let cut = if chamfer {
        cut_corners(world, room, &list, None, CornerCut::Void)
    } else {
        outline::CutReport::default()
    };
    let faces_after = face_count(world);
    let r = &world.rooms[room];
    let mut out = format!(
        "room {room}: {} {} over {} sector(s) at {} (floor){}: {} floor(s) and {} ceiling(s) made, {} already had one{}\n{} convex corner(s) cut into 45 degree lines ({} left alone), faces {} -> {} ({:+})",
        kind,
        if kind == "corridor" { format!("{} wide", req.width.unwrap_or(2.0)) } else { String::new() },
        cells.len(),
        spec.floor,
        spec.ceiling.map_or(String::new(), |c| format!(", ceiling {c}")),
        filled.floors,
        filled.ceilings,
        filled.kept,
        if filled.reset > 0 { format!(", {} height(s) reset", filled.reset) } else { String::new() },
        cut.cut,
        cut.skipped,
        faces_before,
        faces_after,
        faces_after as i64 - faces_before as i64
    );
    if r.wall_mode != psxed_project::grid::WallMode::Derived {
        out.push_str("\nThis room has authored walls: the cut corners draw no closure wall until you convert it (grid_convert_room).");
    }
    Ok(out)
}

/// Cut convex staircase corners of existing floors into 45 degree lines.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChamferReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The rectangle of WORLD sectors whose corners may be cut. For `void` the
    /// outline is the room's floors; for `crack` it is the rectangle itself (a
    /// plateau: the floors around it exist, only lower).
    #[serde(flatten)]
    pub cells: Cells,
    /// `void` (default): the outer triangle disappears and the cell closes
    /// itself with a diagonal wall (a room or cave outline). `crack`: the
    /// outer triangle of the floor drops by `drop` (a plateau corner cut at
    /// 45 degrees, with a diagonal riser). `restore` brings voids back.
    pub mode: Option<String>,
    /// For `crack`: how far the outer triangle drops, grid units (default one
    /// click).
    pub drop: Option<i32>,
}

/// Apply [`ChamferReq`].
pub fn chamfer(world: &mut GridWorld, req: &ChamferReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let list: Vec<[i32; 2]> = rect.cells().collect();
    let before = face_count(world);
    let mode = req.mode.as_deref().unwrap_or("void");
    let out = match mode {
        "void" | "crack" => {
            let cut = if mode == "void" {
                CornerCut::Void
            } else {
                CornerCut::Crack {
                    drop: req.drop.unwrap_or(psxed_project::grid::units::CLICK_UNITS),
                }
            };
            // A plateau corner is a corner of the rectangle given (the floors
            // around it exist, only lower); a room outline is the room's own
            // floors.
            let footprint: std::collections::BTreeSet<[i32; 2]> = list.iter().copied().collect();
            let report = cut_corners(
                world,
                room,
                &list,
                (mode == "crack").then_some(&footprint),
                cut,
            );
            format!(
                "room {room}: {} convex corner(s) cut ({mode}), {} left alone (a split-level surface with the other diagonal)",
                report.cut, report.skipped
            )
        }
        "restore" => format!(
            "room {room}: {} surface(s) brought back from void",
            restore_voids(world, room, &list)
        ),
        other => {
            return Err(format!(
                "unknown mode {other:?}; use void, crack or restore"
            ))
        }
    };
    Ok(format!("{out}\nfaces {} -> {}", before, face_count(world)))
}
