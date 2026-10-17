//! `psxed-mcp` -- MCP server exposing PSoXide level authoring to an agent.
//!
//! The server runs offline over stdio against a project file: the tool bodies
//! are pure functions of a `ProjectDocument`, and the live editor is a
//! multi-minute rebuild. Writes stage in memory and reach `project.ron` only
//! on `save`. See `docs/grid/mcp-guide.md` for how an agent builds a grid
//! level with it.
//!
//! ```bash
//! psxed-mcp --project editor/projects/default
//! psxed-mcp --new editor/projects/scratch   # a New Grid Project, then exit
//! ```

mod tools;

use std::path::{Path, PathBuf};

use rmcp::{ServerHandler, ServiceExt};

use psxed_mcp::edit::Workspace;
use tools::EditorServer;

#[rmcp::tool_handler(
    router = self.tool_router,
    name = "psoxide-editor",
    version = "0.2.0",
    instructions = "PSoXide grid-world level building. Work in a loop: READ, ACT, VERIFY.\n\
\n\
UNITS. A sector is 1024 grid units; cells are addressed by WORLD sector [x, z] (north is -Z, east is +X), the same lattice in every room. Heights are grid units relative to their room's y_offset: a click is 256 (the default snap), the fine step is 64 (snap: fine), and heights always sit on the 64 lattice. The world's scale (5/4 for a new project) turns grid units into EDITOR units, 1280 per sector: nodes (player spawn, lights, entities), screenshot poses and grid_plan markers are editor units; planes and everything else in the grid_* tools are grid units. metrics and grid_rooms print the exact figures. The player is 188 wide and 1024 tall in editor units, and cannot step up more than 640.\n\
\n\
READ first. metrics, grid_rooms (room indices every tool takes), materials (palette_only: true for the world palette), scene_info. After any edit read geometry back: grid_heights (ASCII height map), grid_sector, grid_walls (derived walls: risers, drops, closures), grid_planes, grid_bands, grid_height_at, grid_budget (face budget and validation), cook_check (the real cook's errors), and grid_plan (a top-down PNG with heights as colour, walls, planes and node markers).\n\
\n\
ACT. Shape: grid_add_room, grid_set_room, grid_add_surface (floor or ceiling over a rectangle at a height), grid_set_heights (corners, edges for slopes), grid_set_split, grid_remove_surface, grid_place_wall / grid_remove_wall (authored walls), grid_convert_room. In a derived room (every new one) walls come from height differences: a platform is a floor at another height and gets its risers by itself; grid_wall_override hides, ghosts or repaints one. Look: grid_paint, grid_paint_wall, grid_tiling (world density or fit N x M, turns, flips, offset), grid_material_tiling, grid_band_cut / grid_band_move / grid_band_delete / grid_band_set (horizontal wall bands, run or room scope), grid_set_shading, grid_set_stream (draw distance, activation and prefetch radii in sectors). Free planes (bridges, ramps, slanted walls): grid_add_plane (by centre, or by the midpoints of its two ends), grid_set_plane, grid_delete_plane. Scene nodes: place_node, set_node, move_node, delete_node, add_light, set_combat_windows. Take a checkpoint before a risky stretch and restore it if the result is wrong.\n\
\n\
NATURAL TERRAIN (caves, canyons, rock). Level verbs: grid_raise, grid_ramp, grid_stairs. Outline: grid_polygon fills a polygon or a winding corridor with floors (and a roof for a cave) and cuts every convex staircase corner into a 45 degree line, grid_chamfer does the corner cut on existing floors. Diagonals: grid_crack gives one triangle of a cell its own height (a derived diagonal wall fills the step; a step taller than the character climbs also cooks a one-sided cliff wall), grid_void empties half a cell (the other half closes itself with a diagonal wall), grid_set_wall sets an authored wall's four heights or crumbles an open top. Detail: grid_break (walls, floor, ceiling; intensity and seed; keep_walkable holds the walkability mask; reroll restores its checkpoint and tries the next seed; dry_run only reports), grid_rough (render-only jitter and tint, collision untouched), grid_paint_rules (floor by slope class or height, strata walls). Loop: build the outline, shape the floor profile, break, rough, then grid_walkability, screenshot from several poses, and playtest a route.\n\
\n\
VERIFY. screenshot renders the staged document (saved or not) from up to 8 poses: an eye position plus look_at, or an orbit around a point or node. playtest cooks a real disc and plays it headlessly; with a route (legs of stick, camera and button input) it walks a path, returns frames along it and summarises frame cost. Use grid_plan for layout and screenshot for mood and scale.\n\
\n\
Edits stage in memory. Nothing reaches project.ron until save, which refuses if the editor saved over the file meanwhile."
)]
impl ServerHandler for EditorServer {}

/// Write a New Grid Project: exactly what File > New Grid Project makes (the
/// Graybox Reach base with the starter room), at `dir`.
fn new_project(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = dir.join("project.ron");
    if file.exists() {
        return Err(format!("{} already exists; refusing to overwrite", file.display()).into());
    }
    let name = dir.file_name().map_or_else(
        || "mcp-project".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let projects = psxed_project::projects_dir();
    let base = projects.join(psxed_project::grid::starter::STARTER_BASE);
    let written = psxed_project::grid::starter::create_project(&base, dir, &name)?;
    println!(
        "wrote {} (the starter room, base {})",
        written.display(),
        base.display()
    );
    println!(
        "its assets are linked to the base's; keep {} beside the other projects so ../default/... paths resolve",
        dir.display()
    );
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = PathBuf::from("editor/projects/default");
    let mut new_at: Option<PathBuf> = None;
    let mut frontend: Option<PathBuf> = None;
    let mut scratch: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                project = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--project needs a path")?;
            }
            "--new" => {
                new_at = Some(args.next().map(PathBuf::from).ok_or("--new needs a path")?);
            }
            "--frontend" => {
                frontend = Some(
                    args.next()
                        .map(PathBuf::from)
                        .ok_or("--frontend needs a path")?,
                );
            }
            "--scratch" => {
                scratch = Some(
                    args.next()
                        .map(PathBuf::from)
                        .ok_or("--scratch needs a path")?,
                );
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    if let Some(dir) = new_at {
        return new_project(&dir);
    }
    // Fail loudly at startup rather than on every tool call.
    let workspace = Workspace::open(&project)?;
    let scratch = scratch.unwrap_or_else(|| std::env::temp_dir().join("psxed-mcp"));
    let service = EditorServer::new(workspace, frontend, scratch)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
