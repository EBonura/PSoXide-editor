//! Read-only grid queries: rooms, sectors, height maps, walls, budget.

use std::fmt::Write as _;

use psxed_project::grid::edit::{CellRect, FaceRef};
use psxed_project::grid::limits;
use psxed_project::grid::uv::ProjectUvDefaults;
use psxed_project::grid::{derive, GridRoom, GridSector, GridSurface, GridWorld, WallEdge};
use psxed_project::ProjectDocument;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, clicks, issue_texts, Cells, Materials};

/// Corner names in storage order `[NW, NE, SE, SW]`.
pub const CORNERS: [&str; 4] = ["nw", "ne", "se", "sw"];

fn surface_line(surface: &GridSurface, mats: &Materials<'_>) -> String {
    let mut out = format!(
        "corners nw/ne/se/sw {:?}, split {}",
        surface.heights,
        match surface.split {
            psxed_project::grid::GridSplit::NwSe => "nw_se",
            psxed_project::grid::GridSplit::NeSw => "ne_sw",
        }
    );
    if let Some(b) = surface.heights_b {
        let _ = write!(out, ", second triangle at {b:?}");
    }
    if surface.void != 0 {
        let _ = write!(
            out,
            ", VOID triangle(s) {}",
            match surface.void & 3 {
                1 => "A",
                2 => "B",
                _ => "A and B",
            }
        );
    }
    let a = surface.tri(0);
    let _ = write!(out, ", material {}", mats.label(a.material));
    if let Some(b) = &surface.tri_b {
        let _ = write!(out, " / {}", mats.label(b.material));
    }
    if !a.uv.is_identity() {
        let _ = write!(out, ", uv {:?}", a.uv.mode);
    }
    if !surface.walkable {
        out.push_str(", NOT walkable");
    }
    out
}

/// Every room: where it sits, how big it is, its walls and settings.
pub fn rooms(world: &GridWorld, mats: &Materials<'_>) -> String {
    let mut out = String::new();
    let sector = world.sector_editor_units();
    let _ = writeln!(
        out,
        "{} room(s). Scale {}/{} (1 grid unit = {:.3} editor units), sector = 1024 grid units = {sector} editor units.",
        world.rooms.len(),
        world.scale.num,
        world.scale.den,
        f64::from(world.scale.num) / f64::from(world.scale.den)
    );
    let _ = writeln!(
        out,
        "Facing shade: up {} down {} side_x {} side_z {} (Q8, 256 = unshaded)\n",
        world.shading.up, world.shading.down, world.shading.side_x, world.shading.side_z
    );
    for (index, room) in world.rooms.iter().enumerate() {
        let name = if room.name.is_empty() {
            format!("Room {index}")
        } else {
            room.name.clone()
        };
        let _ = writeln!(
            out,
            "room {index} {name:?}: {}x{} sectors from world sector {:?}, {} with geometry",
            room.width,
            room.depth,
            room.origin,
            room.sector_count()
        );
        let _ = writeln!(
            out,
            "  y_offset {} grid units ({} clicks), ambient {:?}, walls {}",
            room.y_offset,
            clicks(room.y_offset),
            room.ambient,
            room.wall_mode.label()
        );
        let _ = writeln!(
            out,
            "  default wall material {}, open wall height {}, {} band set(s), {} plane(s), {} mesh(es), {} edge override(s)",
            mats.label(room.defaults.wall_material),
            room.defaults.open_wall,
            room.band_sets.len(),
            room.planes.len(),
            room.meshes.len(),
            room.edge_overrides.len()
        );
        if room.defaults.closure_facing != psxed_project::grid::ClosureFacing::Front {
            let _ = writeln!(
                out,
                "  closure walls drawn {}",
                room.defaults.closure_facing.label()
            );
        }
        if let Some((lo, hi)) = floor_span(room) {
            let _ = writeln!(
                out,
                "  floors span {lo}..{hi} room-relative, {}..{} world grid units",
                room.y_offset + lo,
                room.y_offset + hi
            );
        }
        let min = room.origin;
        let max = [
            room.origin[0] + room.width as i32 - 1,
            room.origin[1] + room.depth as i32 - 1,
        ];
        let to_editor = |v: i32| world.scale.to_editor(v * 1024);
        let _ = writeln!(
            out,
            "  editor-unit extent: x {}..{}, z {}..{}",
            to_editor(min[0]),
            to_editor(max[0] + 1),
            to_editor(min[1]),
            to_editor(max[1] + 1)
        );
    }
    out
}

fn floor_span(room: &GridRoom) -> Option<(i32, i32)> {
    let mut span: Option<(i32, i32)> = None;
    for (_, _, sector) in room.iter() {
        if let Some(floor) = sector.ground_floor() {
            let (lo, hi) = floor.height_range();
            span = Some(span.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    span
}

/// Everything in one sector: its surfaces, authored walls, derived walls.
pub fn sector(
    world: &GridWorld,
    project: &ProjectDocument,
    room: usize,
    cell: [i32; 2],
) -> Result<String, String> {
    let mats = Materials::new(project);
    let r = world
        .rooms
        .get(room)
        .ok_or_else(|| format!("room {room} does not exist"))?;
    let local = r.local(cell).ok_or_else(|| {
        format!(
            "world sector {cell:?} is outside room {room} (origin {:?}, {}x{})",
            r.origin, r.width, r.depth
        )
    })?;
    let mut out = format!(
        "room {room}, world sector {cell:?} (local {local:?}), y_offset {}\n",
        r.y_offset
    );
    let sector = r.sector(local[0], local[1]);
    if sector.is_none() && !r.leaves.in_sector(cell).is_empty() {
        return Ok(out + &cells_in_sector(world, project, r, room, cell));
    }
    let Some(sector) = sector else {
        out.push_str("empty: no floor, no ceiling, no walls\n");
        return Ok(out);
    };
    out.push_str("leaf: level 2, a whole sector\n");
    match sector.ground_floor() {
        Some(s) => {
            let _ = writeln!(out, "floor: {}", surface_line(s, &mats));
        }
        None => out.push_str("floor: none (a pit or void)\n"),
    }
    match sector.ground_ceiling() {
        Some(s) => {
            let _ = writeln!(out, "ceiling: {}", surface_line(s, &mats));
        }
        None => out.push_str("ceiling: none (open sky)\n"),
    }
    let defaults = ProjectUvDefaults(project);
    for edge in WallEdge::ALL {
        for (index, wall) in sector.walls(edge).iter().enumerate() {
            let _ = writeln!(
                out,
                "authored wall {} {index}: heights bottom-a/bottom-b/top-b/top-a {:?}, facing {:?}, material {}, solid {}, band set {:?}",
                edge.label(),
                wall.heights,
                wall.facing,
                mats.label(wall.material),
                wall.solid,
                wall.band_set
            );
        }
    }
    for wall in derive::cell_walls(r, cell) {
        let face = FaceRef::of_derived(room, &wall);
        let tiling = psxed_project::grid::uv::report_for(world, &defaults, face, None)
            .map_or_else(String::new, |rep| {
                format!(", tiling {}", rep.effective_text())
            });
        let _ = writeln!(
            out,
            "derived wall {} {}{}: heights {:?}, owner {:?}, material {}, hidden {}, solid {:?}{tiling}",
            wall.edge.label(),
            wall.kind.label(),
            if wall.back { " (seen from the back)" } else { "" },
            wall.heights,
            wall.owner,
            mats.label(wall.material),
            wall.hidden,
            wall.solid,
        );
    }
    let _ = writeln!(
        out,
        "wall material of this sector (for the derived walls it owns): {}",
        mats.label(sector.ground_wall_material())
    );
    Ok(out)
}

/// A sector that is not one cell: each leaf in it (or the bigger one over
/// it) with its size, corners and walls, and the derived walls on its edges.
fn cells_in_sector(
    world: &GridWorld,
    project: &ProjectDocument,
    r: &GridRoom,
    room: usize,
    cell: [i32; 2],
) -> String {
    let mats = Materials::new(project);
    let mut out = String::new();
    let sizes = [
        "4 sectors",
        "2 sectors",
        "1 sector",
        "1/2 sector",
        "1/4 sector",
    ];
    for (key, leaf) in r.leaves.in_sector(cell) {
        let _ = writeln!(
            out,
            "leaf level {} ({} across) at clicks ({}, {}) {}",
            key.level,
            sizes[usize::from(key.level.min(4))],
            key.x,
            key.z,
            if key.level < 2 {
                format!(
                    "(covers sectors {:?} to {:?})",
                    key.sector_range().0,
                    key.sector_range().1
                )
            } else {
                String::new()
            }
        );
        match leaf.ground_floor() {
            Some(s) => {
                let _ = writeln!(out, "  floor: {}", surface_line(s, &mats));
            }
            None => out.push_str("  floor: none\n"),
        }
        match leaf.ground_ceiling() {
            Some(s) => {
                let _ = writeln!(out, "  ceiling: {}", surface_line(s, &mats));
            }
            None => out.push_str("  ceiling: none\n"),
        }
        for edge in WallEdge::ALL {
            for (index, wall) in leaf.walls(edge).iter().enumerate() {
                let _ = writeln!(
                    out,
                    "  authored wall {} {index}: heights {:?}, material {}",
                    edge.label(),
                    wall.heights,
                    mats.label(wall.material),
                );
            }
        }
    }
    for seg in psxed_project::grid::leaf_derive::cell_wall_segments(r, cell) {
        let _ = writeln!(
            out,
            "derived wall {} from grid ({}, {}) to ({}, {}), hidden {}",
            seg.kind.label(),
            seg.a[0],
            seg.a[1],
            seg.b[0],
            seg.b[1],
            seg.hidden
        );
    }
    let _ = world;
    let _ = room;
    out
}

fn x_cell(r: &GridRoom, lx: usize, lz: usize) -> [i32; 2] {
    r.world_cell(lx, lz)
}

/// What a height map shows.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MapShow {
    /// Floor heights (the default).
    #[default]
    Floor,
    /// Ceiling heights.
    Ceiling,
    /// Floor and ceiling, `floor|ceiling`.
    Both,
}

/// One cell of the map as text, `width` characters wide.
fn cell_text(
    world: &GridWorld,
    room: &GridRoom,
    sector: Option<&GridSector>,
    show: MapShow,
    width: usize,
) -> String {
    let pick = |surface: Option<&GridSurface>| -> String {
        match surface {
            None => ".".to_string(),
            Some(s) => {
                let sum: i32 = s.heights.iter().sum();
                let flat = s.heights.iter().all(|h| *h == s.heights[0]);
                let mean = (room.y_offset * 4 + sum) / 4;
                let _ = world;
                if flat {
                    clicks(room.y_offset + s.heights[0])
                } else {
                    format!("/{}", clicks(mean))
                }
            }
        }
    };
    let text = match (show, sector) {
        (_, None) => ".".to_string(),
        (MapShow::Floor, Some(s)) => pick(s.ground_floor()),
        (MapShow::Ceiling, Some(s)) => pick(s.ground_ceiling()),
        (MapShow::Both, Some(s)) => {
            format!("{}|{}", pick(s.ground_floor()), pick(s.ground_ceiling()))
        }
    };
    format!("{text:>width$}")
}

/// An ASCII height map of a rectangle of world sectors.
///
/// North (-Z) is at the top and east (+X) to the right, as the Top view draws
/// it. Heights are WORLD heights (room offset included) in clicks of 256 grid
/// units; `/` marks a sloped surface (its mean), `.` no surface. With several
/// rooms the highest floor shows, with `*` after it when rooms overlap.
pub fn height_map(
    world: &GridWorld,
    room: Option<usize>,
    cells: Option<&Cells>,
    show: MapShow,
) -> Result<String, String> {
    let rect = match cells {
        Some(c) => c.rect(),
        None => {
            let mut lo = [i32::MAX; 2];
            let mut hi = [i32::MIN; 2];
            for (i, r) in world.rooms.iter().enumerate() {
                if room.is_some_and(|only| only != i) {
                    continue;
                }
                lo = [lo[0].min(r.origin[0]), lo[1].min(r.origin[1])];
                hi = [
                    hi[0].max(r.origin[0] + r.width as i32 - 1),
                    hi[1].max(r.origin[1] + r.depth as i32 - 1),
                ];
            }
            if lo[0] > hi[0] {
                return Err("no rooms to map".to_string());
            }
            CellRect::from_corners(lo, hi)
        }
    };
    check_rect(&rect)?;
    if rect.width() > 64 {
        return Err(format!(
            "the map would be {} sectors wide; ask for 64 or fewer (give `from`/`to`)",
            rect.width()
        ));
    }
    // A room with cells of other sizes is read through the sector view; a
    // sector that holds finer cells is marked with %.
    let views: Vec<Option<GridRoom>> = world
        .rooms
        .iter()
        .map(|r| {
            (!r.leaves.is_sectors_only())
                .then(|| psxed_project::grid::sector_view::sector_room(r).0)
        })
        .collect();
    let width = if show == MapShow::Both { 7 } else { 5 };
    let mut out = String::new();
    let _ = write!(out, "{:>6} ", "z\\x");
    for x in rect.min[0]..=rect.max[0] {
        let _ = write!(out, "{x:>width$}");
    }
    out.push('\n');
    for z in rect.min[1]..=rect.max[1] {
        let _ = write!(out, "{z:>6} ");
        for x in rect.min[0]..=rect.max[0] {
            let mut best: Option<(i32, String)> = None;
            let mut covered = 0;
            for (i, r) in world.rooms.iter().enumerate() {
                if room.is_some_and(|only| only != i) {
                    continue;
                }
                let Some([lx, lz]) = r.local([x, z]) else {
                    continue;
                };
                let sector = match &views[i] {
                    Some(view) => view.sector(lx, lz),
                    None => r.sector(lx, lz),
                };
                let Some(sector) = sector else {
                    continue;
                };
                let fine = views[i].is_some()
                    && r.leaves
                        .in_sector(x_cell(r, lx, lz))
                        .iter()
                        .any(|(k, _)| k.level > 2);
                let surface = match show {
                    MapShow::Ceiling => sector.ground_ceiling(),
                    _ => sector.ground_floor(),
                };
                let Some(surface) = surface else { continue };
                covered += 1;
                let h = r.y_offset + surface.heights.iter().sum::<i32>() / 4;
                let mut text = cell_text(world, r, Some(sector), show, width);
                if fine {
                    text = format!("{:>width$}", format!("%{}", text.trim()));
                }
                if best.as_ref().is_none_or(|(b, _)| h > *b) {
                    best = Some((h, text));
                }
            }
            match best {
                Some((_, mut text)) => {
                    if covered > 1 {
                        text = format!("{:>width$}", format!("{}*", text.trim()));
                    }
                    out.push_str(&text);
                }
                None => {
                    let _ = write!(out, "{:>width$}", ".");
                }
            }
        }
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "\nheights in clicks (1 click = 256 grid units; 0.25 = one fine step), world height \
         including each room's y_offset. '/' = sloped (mean), '.' = none, '*' = rooms overlap here, \
         '%' = a sector made of finer cells (the mean of its corners; grid_leaves maps them)."
    );
    Ok(out)
}

/// Floor height at a world point, per room and for the whole world.
pub fn height_at(world: &GridWorld, point: [i32; 2], grid_units: bool) -> Result<String, String> {
    let scale = world.scale;
    let (gx, gz) = if grid_units {
        (point[0], point[1])
    } else {
        (scale.to_grid(point[0]), scale.to_grid(point[1]))
    };
    let (ex, ez) = if grid_units {
        (scale.to_editor(point[0]), scale.to_editor(point[1]))
    } else {
        (point[0], point[1])
    };
    let cell = [gx.div_euclid(1024), gz.div_euclid(1024)];
    let mut out =
        format!("point: grid [{gx}, {gz}] = editor [{ex}, {ez}], world sector {cell:?}\n");
    let mut found = false;
    for (index, room) in world.rooms.iter().enumerate() {
        let Some([lx, lz]) = room.local(cell) else {
            continue;
        };
        let Some(sector) = room.sector(lx, lz) else {
            continue;
        };
        found = true;
        let solo = GridWorld {
            scale,
            rooms: vec![room.clone()],
            ..GridWorld::default()
        };
        let floor = solo.floor_height_at(ex, ez, i32::MAX);
        let _ = writeln!(
            out,
            "room {index}: floor {} editor units ({} grid), floor corners {:?}, ceiling corners {:?}, y_offset {}",
            floor.map_or_else(|| "none".to_string(), |v| v.to_string()),
            floor.map_or_else(|| "none".to_string(), |v| scale.to_grid(v).to_string()),
            sector.ground_floor().map(|s| s.heights),
            sector.ground_ceiling().map(|s| s.heights),
            room.y_offset
        );
    }
    if !found {
        out.push_str("no room has a sector here\n");
    } else if let Some(v) = world.floor_height_at(ex, ez, i32::MAX) {
        let _ = writeln!(out, "highest floor in the world here: {v} editor units");
    }
    Ok(out)
}

/// The derived and authored walls of a room or a rectangle of it.
pub fn walls(
    world: &GridWorld,
    project: &ProjectDocument,
    room: usize,
    cells: Option<CellRect>,
    edge: Option<WallEdge>,
) -> Result<String, String> {
    let mats = Materials::new(project);
    let r = world
        .rooms
        .get(room)
        .ok_or_else(|| format!("room {room} does not exist"))?;
    let rect = cells.unwrap_or_else(|| {
        CellRect::from_corners(
            r.origin,
            [
                r.origin[0] + r.width as i32 - 1,
                r.origin[1] + r.depth as i32 - 1,
            ],
        )
    });
    check_rect(&rect)?;
    let cook_units = psxed_project::grid::cook::RuntimeUnits::new(world.scale).ok();
    let mut derived = Vec::new();
    let mut authored = Vec::new();
    for cell in rect.cells() {
        for wall in derive::cell_walls(r, cell) {
            if edge.is_none_or(|e| e == wall.edge) {
                derived.push((cell, wall));
            }
        }
        let Some([lx, lz]) = r.local(cell) else {
            continue;
        };
        let Some(sector) = r.sector(lx, lz) else {
            continue;
        };
        for e in WallEdge::ALL {
            if edge.is_some_and(|only| only != e) {
                continue;
            }
            for (index, wall) in sector.walls(e).iter().enumerate() {
                authored.push((cell, e, index, wall.clone()));
            }
        }
    }
    let mut out = format!(
        "room {room}: {} derived wall face(s), {} authored wall record(s) in {}x{} sectors\n",
        derived.len(),
        authored.len(),
        rect.width(),
        rect.depth()
    );
    if !derived.is_empty() {
        out.push_str("\nDERIVED (computed from height differences; edit with grid_wall_override, grid_paint, grid_tiling, grid_band_*)\n");
        out.push_str("cell edge kind | heights [bottom-a, bottom-b, top-b, top-a] | owner | run | material | flags\n");
        for (cell, wall) in &derived {
            let run = derive::run_members(r, wall).len();
            let mut flags = Vec::new();
            if wall.hidden {
                flags.push("hidden".to_string());
            }
            match wall.solid {
                Some(true) => flags.push("forced solid".to_string()),
                Some(false) => flags.push("ghost".to_string()),
                None => {}
            }
            if let Some(set) = wall.band_set {
                flags.push(format!("band set {set}"));
            }
            if wall.uv.is_some() {
                flags.push("own tiling".to_string());
            }
            if !wall.blocks() {
                let cliff = cook_units.as_ref().is_some_and(|u| {
                    psxed_project::grid::cook::collision::is_cliff_wall(u, r.y_offset, wall)
                });
                flags.push(
                    if cliff {
                        "one-sided cliff wall (taller than a step)"
                    } else {
                        "no wall record (step rule)"
                    }
                    .to_string(),
                );
            }
            if wall.back {
                flags.push("seen from the back of the diagonal".to_string());
            }
            let _ = writeln!(
                out,
                "{:?} {} {} | {} | {:?} | {} | {} | {}",
                cell,
                wall.edge.label(),
                wall.kind.label(),
                wall.heights
                    .map_or_else(|| format!("ring {:?}", wall.ring), |h| format!("{h:?}")),
                wall.owner,
                run,
                mats.short(wall.material),
                flags.join(", ")
            );
        }
    }
    if !authored.is_empty() {
        out.push_str("\nAUTHORED (wall records)\n");
        for (cell, e, index, wall) in &authored {
            let _ = writeln!(
                out,
                "{:?} {} #{index} | {:?} | facing {:?} | {} | solid {} | band set {:?}",
                cell,
                e.label(),
                wall.heights,
                wall.facing,
                mats.short(wall.material),
                wall.solid,
                wall.band_set
            );
        }
    }
    Ok(out)
}

/// The budget readout and every validation finding.
pub fn budget(world: &GridWorld) -> String {
    let b = limits::GridBudget::of(world);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "rooms {}, sectors {}, floors {}, ceilings {}, authored walls {}, planes {}, meshes {}",
        b.rooms, b.sectors, b.floors, b.ceilings, b.walls, b.planes, b.meshes
    );
    let _ = writeln!(
        out,
        "drawable faces {}; busiest room {} of {} ({}%; the editor turns orange from 85%)",
        b.faces,
        b.largest_room_faces,
        limits::MAX_ROOM_FACES,
        b.faces_percent()
    );
    let _ = writeln!(
        out,
        "caps: {} rooms, {} sectors a room side, {} walls an edge, {} planes a room, {} meshes a room",
        limits::MAX_ROOMS,
        limits::MAX_ROOM_SIDE,
        limits::MAX_WALLS_PER_EDGE,
        limits::MAX_PLANES_PER_ROOM,
        psxed_project::grid::mesh::MAX_MESHES_PER_ROOM
    );
    let issues = issue_texts(world);
    if issues.is_empty() {
        out.push_str("validate: no findings\n");
    } else {
        let _ = writeln!(out, "validate: {} finding(s)", issues.len());
        for text in &issues {
            let _ = writeln!(out, "- {text}");
        }
    }
    out
}

/// Cook the project in memory and report its errors, warnings and exact
/// size. Errors are the cook's own messages, one per line.
pub fn cook_check(project: &ProjectDocument, root: &std::path::Path) -> String {
    let (package, report) = psxed_project::playtest::build_package(project, root);
    let mut out = String::new();
    if report.is_ok() {
        out.push_str("cook: OK\n");
    } else {
        let _ = writeln!(out, "cook: FAILED with {} error(s)", report.errors.len());
        for error in &report.errors {
            let _ = writeln!(out, "- error: {}", error.message);
        }
    }
    for warning in &report.warnings {
        let _ = writeln!(out, "- warning: {warning}");
    }
    if let Some(package) = package {
        let budgets = psxed_project::playtest::cooked_playtest_budgets(project, &package);
        let _ = writeln!(out, "{}", budgets.concise_summary());
    }
    out
}

/// Cook in memory and return the cook's errors as one readable message, or
/// `Ok` when it would build. Run before a slow disc build so a level the cook
/// refuses fails in a second, with the cook's own words, not after a guest
/// toolchain run with a tail of its log.
pub fn cook_gate(project: &ProjectDocument, root: &std::path::Path) -> Result<(), String> {
    let (_, report) = psxed_project::playtest::build_package(project, root);
    if report.is_ok() {
        return Ok(());
    }
    let mut out = format!(
        "the cook refuses this project ({} error(s)); no disc was built:\n",
        report.errors.len()
    );
    for error in &report.errors {
        let _ = writeln!(out, "- {}", error.message);
    }
    Err(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::edit::{create_surfaces, SurfaceKind};

    fn world() -> GridWorld {
        crate::grid::test_support::new_grid_project()
            .active_scene()
            .world
            .grid()
            .cloned()
            .expect("the starter has a grid world")
    }

    #[test]
    fn the_height_map_labels_flat_and_raised_cells() {
        let mut w = world();
        let rect = CellRect::from_corners([6, 0], [7, 1]);
        create_surfaces(&mut w, 0, SurfaceKind::Floor, rect, 512, None);
        let map = height_map(&w, None, None, MapShow::Floor).unwrap();
        // Rows 0 and 1 carry the raised platform (2 clicks) past the 6x6 room.
        let line = map
            .lines()
            .find(|l| l.trim_start().starts_with("0 "))
            .unwrap();
        assert!(line.contains('2'), "{map}");
        assert!(map.contains("click"), "{map}");
    }

    #[test]
    fn a_sector_report_lists_derived_walls() {
        let p = crate::grid::test_support::new_grid_project();
        let w = world();
        let text = sector(&w, &p, 0, [0, 0]).unwrap();
        assert!(text.contains("floor:"), "{text}");
        assert!(text.contains("derived wall north closure"), "{text}");
        assert!(sector(&w, &p, 0, [99, 99]).is_err());
    }

    #[test]
    fn height_at_reports_the_starter_floor() {
        let w = world();
        let text = height_at(&w, [2000, 2000], false).unwrap();
        assert!(text.contains("room 0"), "{text}");
        assert!(text.contains("highest floor"), "{text}");
    }

    #[test]
    fn the_cook_gate_passes_a_good_project_and_names_a_refusal() {
        let root = psxed_project::projects_dir().join("graybox-reach");
        let mut project = crate::grid::test_support::new_grid_project();
        assert!(cook_gate(&project, &root).is_ok());
        // A plane outside its room is a cook error.
        let world = project.active_scene_mut().world.grid_mut().unwrap();
        let mut plane = psxed_project::grid::GridPlane::new([-9000, 0, 500], [1024, 1024]);
        plane.collision = psxed_project::grid::PlaneCollision::Solid;
        world.rooms[0].planes.push(plane);
        let refusal = cook_gate(&project, &root).unwrap_err();
        assert!(refusal.contains("no disc was built"), "{refusal}");
        assert!(refusal.contains("- "), "{refusal}");
    }

    #[test]
    fn budget_lists_the_face_count() {
        let w = world();
        let text = budget(&w);
        assert!(text.contains("drawable faces"), "{text}");
        assert!(text.contains("no findings"), "{text}");
    }
}
