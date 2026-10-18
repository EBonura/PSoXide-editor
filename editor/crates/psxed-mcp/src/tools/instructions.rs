//! The server's instructions, generated from the tool registry.
//!
//! The prose says how to work (units, the read-act-verify loop, the storey
//! and natural terrain loops). The list of tools is not written by hand: it
//! is read from the routers that registered them, so it cannot lag the
//! registry. A test checks that every tool the prose names exists.

use super::EditorServer;

/// How to work: units, the loop, and the two loops that need explaining.
/// Tool names written here must exist (see the test below); the full list is
/// generated after it.
pub(super) const PROSE: &str = "PSoXide grid-world level building. Work in a loop: READ, ACT, VERIFY.\n\
\n\
UNITS. A sector is 1024 grid units; cells are addressed by WORLD sector [x, z] (north is -Z, east is +X), the same lattice in every room. Heights are grid units relative to their room's y_offset: a click is 256 (the default snap), the fine step is 64 (snap: fine), and heights always sit on the 64 lattice. The world's scale (5/4 for a new project) turns grid units into EDITOR units, 1280 per sector: nodes (player spawn, lights, entities), screenshot poses and grid_plan markers are editor units; planes and everything else in the grid_* tools are grid units. metrics and grid_rooms print the exact figures. The player is 188 wide and 1024 tall in editor units, and cannot step up more than 640.\n\
\n\
READ first (metrics, grid_rooms for the room indices every tool takes, materials with palette_only: true for the world palette, scene_info). After any edit read geometry back (grid_heights, grid_sector, grid_walls, grid_budget, cook_check, grid_plan).\n\
\n\
ACT. In a derived room (every new one) walls come from height differences: a platform is a floor at another height and gets its risers by itself; grid_wall_override hides, ghosts or repaints one. A building seen from outside needs grid_set_room closure_facing both. Static meshes are not grid geometry: an asset (arch, column, rubble or an imported .obj) is a recipe the world keeps once and a room places copies of. Take a checkpoint before a risky stretch and restore it if the result is wrong.\n\
\n\
STOREYS. A column of the world can hold several floors. A storey is a floor height eight or more sectors share, read from the document (grid_storeys). Surface and level tools take y or storey to edit one floor of a column; a column of several floors refuses an edit with no aim. A room with storeys stays whole sectors: its cells cannot be divided.\n\
\n\
NATURAL TERRAIN (caves, canyons, rock). Build the outline, shape the floor profile, break, rough, then grid_walkability, screenshot from several poses, and playtest a route.\n\
\n\
VERIFY. screenshot renders the staged document (saved or not) from up to 8 poses: an eye position plus look_at, or an orbit around a point or node. playtest cooks a real disc and plays it headlessly; with a route (legs of stick, camera and button input) it walks a path, returns frames along it and summarises frame cost. Use grid_plan for layout and screenshot for mood and scale.\n\
\n\
Edits stage in memory. Nothing reaches project.ron until save, which refuses if the editor saved over the file meanwhile.";

/// The areas, in the order the catalogue lists them: the router's name, then
/// the label the instructions give it.
pub(super) const AREAS: [(&str, &str); 14] = [
    ("project", "PROJECT (files, checkpoints, undo)"),
    ("query", "READ"),
    (
        "geometry",
        "SHAPE (rooms, floors, ceilings, walls, heights)",
    ),
    ("surfaces", "LOOK (paint, tiling, shading)"),
    ("bands", "WALL BANDS"),
    ("cells", "CELLS OF SEVERAL SIZES"),
    ("storeys", "STOREYS"),
    ("planes", "FREE PLANES (bridges, ramps, slanted walls)"),
    ("meshes", "STATIC MESHES"),
    ("verbs", "LEVEL VERBS (raise, ramp, stairs, spiral)"),
    ("natural", "NATURAL TERRAIN"),
    ("nodes", "SCENE NODES (spawns, lights, entities)"),
    ("combat", "COMBAT"),
    ("verify", "VERIFY"),
];

/// The prose and the generated catalogue of `areas` (router name, tool
/// names, in registration order). A tool in no known area is listed under
/// OTHER, so a new router is never silently left out.
pub(super) fn compose(areas: &[(&'static str, Vec<String>)]) -> String {
    let mut out = String::from(PROSE);
    let count: usize = areas.iter().map(|(_, names)| names.len()).sum();
    out.push_str(&format!("\n\nTOOLS ({count}), by area:\n"));
    for (router, label) in AREAS {
        let Some((_, names)) = areas.iter().find(|(r, _)| *r == router) else {
            continue;
        };
        if !names.is_empty() {
            out.push_str(&format!("{label}: {}.\n", names.join(", ")));
        }
    }
    let other: Vec<&String> = areas
        .iter()
        .filter(|(r, _)| !AREAS.iter().any(|(known, _)| known == r))
        .flat_map(|(_, names)| names)
        .collect();
    if !other.is_empty() {
        let names: Vec<&str> = other.iter().map(|n| n.as_str()).collect();
        out.push_str(&format!("OTHER: {}.\n", names.join(", ")));
    }
    out
}

impl EditorServer {
    /// The instructions the server sends on initialize.
    pub(crate) fn instructions(&self) -> String {
        compose(&self.areas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_the_prose_names_is_registered() {
        let server = EditorServer::for_listing();
        let registered: Vec<String> = server
            .areas
            .iter()
            .flat_map(|(_, names)| names.iter().cloned())
            .collect();
        assert!(registered.len() > 50, "{} tools", registered.len());
        // Words of the shape `grid_name` or a bare tool name from the READ
        // line; the `grid_*` wildcard and `grid_` prefixes are not tools.
        let mut missing = Vec::new();
        for word in PROSE.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            let tool_like = word.starts_with("grid_") && word.len() > 5;
            if tool_like && !registered.iter().any(|n| n == word) {
                missing.push(word.to_string());
            }
        }
        assert!(missing.is_empty(), "named but not registered: {missing:?}");
        for bare in [
            "metrics",
            "materials",
            "scene_info",
            "screenshot",
            "playtest",
            "save",
        ] {
            assert!(registered.iter().any(|n| n == bare), "{bare}");
        }
    }

    #[test]
    fn the_catalogue_lists_every_tool_once_and_names_no_area_twice() {
        let server = EditorServer::for_listing();
        let text = server.instructions();
        for (_, names) in server.areas.iter() {
            for name in names {
                assert!(text.contains(name.as_str()), "{name} missing");
            }
        }
        assert!(
            !text.contains("OTHER:"),
            "a router is not in AREAS:\n{text}"
        );
        assert!(text.contains("NATURAL TERRAIN") && text.contains("grid_break"));
    }
}
