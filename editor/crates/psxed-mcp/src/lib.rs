//! Authoring tool bodies for the PSoXide Editor MCP server.
//!
//! Every public function here is a pure function of a [`ProjectDocument`],
//! deliberately: Phase 1 runs offline against a project file, and the same
//! bodies move into the live editor's MCP bridge later without changing
//! shape (see `docs/editor-mcp-plan-2026-09-18.md`).

pub mod combat;
pub mod cost;
pub mod duel;
pub mod edit;
pub mod grid;
pub mod inspect;
pub mod nodes;
pub mod play;
pub mod route;
pub mod shot;
pub mod track;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use psxed_project::grid::{limits, GridWorld, SECTOR_UNITS};
use psxed_project::{NodeKind, ProjectDocument};

/// Authored units per runtime unit. The cook divides by this.
pub const WORLD_UNIT_DIVISOR: i32 = 16;
/// Authored units per texel, so a 64x64 texture tiles every 1024 units.
pub const UNITS_PER_TEXEL: i32 = 16;
/// Player capsule from the shipped default project's Character Controller.
pub const PLAYER_RADIUS: i32 = 188;
/// See [`PLAYER_RADIUS`].
pub const PLAYER_HEIGHT: i32 = 1024;
/// World sector size in the shipped default project.
pub const SECTOR: i32 = 1024;
/// Tallest ledge the character motor will climb, in authored units.
///
/// `STEP_UP_HEIGHT` is 40 engine units in `psx-engine/src/character_motor.rs`;
/// authored is that times [`WORLD_UNIT_DIVISOR`]. There is no jump, so this is
/// the hard ceiling on every riser and lip in a level.
pub const STEP_UP_AUTHORED: i32 = 40 * WORLD_UNIT_DIVISOR;

/// Pick the scene to operate on: the caller's index, else the first scene
/// with a grid world, else the first scene.
pub fn resolve_scene(project: &ProjectDocument, index: Option<usize>) -> Result<usize, String> {
    if let Some(index) = index {
        return (index < project.scenes.len())
            .then_some(index)
            .ok_or_else(|| {
                format!(
                    "scene {index} out of range: the project has {}",
                    project.scenes.len()
                )
            });
    }
    project
        .scenes
        .iter()
        .position(|scene| scene.world.grid().is_some())
        .or(if project.scenes.is_empty() {
            None
        } else {
            Some(0)
        })
        .ok_or_else(|| "the project has no scenes".to_string())
}

/// Scale constants and the reference dimensions of the shipped content, so
/// sizes are looked up rather than guessed. This is the analogue of the
/// Blender MCP's `bpy_api_lookup`: its value is that it stops the agent
/// inventing plausible numbers.
pub fn metrics(project: Option<&ProjectDocument>) -> String {
    let mut out = String::new();
    out.push_str("# PSoXide authored-unit metrics\n\n");
    let _ = writeln!(
        out,
        "Nodes are placed in EDITOR units. The cook divides every length by\n\
         {WORLD_UNIT_DIVISOR} to reach runtime units, so 1 runtime unit =\n\
         {UNITS_PER_TEXEL} editor units = 1 texel. A 64x64 texture therefore\n\
         tiles every {} editor units -- but materials are NOT all 64x64, so call\n\
         `materials` for the real per-material tile size instead of assuming.\n",
        64 * UNITS_PER_TEXEL
    );
    out.push_str("\n## Grid world\n\n");
    let _ = writeln!(
        out,
        "- a sector is {SECTOR_UNITS} grid units; a height click is {} and the fine snap is {}\n\
         - each world carries a rational scale from grid units to editor units",
        psxed_project::grid::CLICK_UNITS,
        psxed_project::grid::FINE_UNITS
    );
    out.push_str("\n## Reference bodies\n\n");
    let _ = writeln!(
        out,
        "- player: radius {PLAYER_RADIUS}, height {PLAYER_HEIGHT} (diameter {} wide)",
        PLAYER_RADIUS * 2
    );
    out.push_str("- heavy enemy: radius 320, height 1741\n");
    out.push_str("- gameplay camera: distance 3500, height 1800, target height 1160\n");
    out.push_str("- gravity 96 units/tick^2; draw distance 25000\n");

    out.push_str("\n## Traversal\n\n");
    let _ = writeln!(
        out,
        "- max step-up: {STEP_UP_AUTHORED} authored units. A ledge taller than this \n\
         \x20 stops the player dead, because there is no jump.\n\
         - that is {:.2} player heights, so the tallest climbable ledge is roughly\n\
         \x20 knee height on the character.",
        f64::from(STEP_UP_AUTHORED) / f64::from(PLAYER_HEIGHT)
    );

    if let Some(project) = project {
        if let Ok(index) = resolve_scene(project, None) {
            let scene = &project.scenes[index];
            match scene.world.grid() {
                Some(world) => {
                    let _ = writeln!(
                        out,
                        "\n## This project\n\nScene {index} {:?}: grid world of {} rooms, scale {}/{}, sector {} editor units.",
                        scene.name,
                        world.rooms.len(),
                        world.scale.num,
                        world.scale.den,
                        world.scale.sector_editor_units()
                    );
                }
                None => {
                    let _ = writeln!(
                        out,
                        "\n## This project\n\nScene {index} {:?}: no grid world.",
                        scene.name
                    );
                }
            }
        }
    }
    out
}

/// Structured read of a scene: the grid world and the node census.
pub fn scene_info(project: &ProjectDocument, scene_index: Option<usize>) -> Result<String, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let mut out = String::new();
    let _ = writeln!(out, "# Scene {index}: {:?}\n", scene.name);

    match scene.world.grid() {
        Some(world) => grid_summary(&mut out, project, world),
        None => out.push_str("No grid world.\n"),
    }

    out.push_str("\n## Nodes\n\n");
    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    for node in scene.nodes() {
        if matches!(node.kind, NodeKind::World { .. }) {
            continue;
        }
        *kinds.entry(node.kind.label()).or_default() += 1;
    }
    for (kind, count) in &kinds {
        let _ = writeln!(out, "- {kind}: {count}");
    }
    Ok(out)
}

fn grid_summary(out: &mut String, project: &ProjectDocument, world: &GridWorld) {
    let budget = limits::GridBudget::of(world);
    let _ = writeln!(
        out,
        "grid world: {} rooms, scale {}/{} (sector = {} editor units)",
        world.rooms.len(),
        world.scale.num,
        world.scale.den,
        world.scale.sector_editor_units()
    );
    let _ = writeln!(
        out,
        "{} sectors, {} floors, {} ceilings, {} wall segments; faces {} of {}",
        budget.sectors,
        budget.floors,
        budget.ceilings,
        budget.walls,
        budget.faces,
        limits::MAX_WORLD_FACES
    );
    for (index, room) in world.rooms.iter().enumerate() {
        let _ = writeln!(
            out,
            "- room {index} {:?}: {}x{} sectors, {} with geometry",
            room.name,
            room.width,
            room.depth,
            room.sector_count()
        );
    }
    out.push_str("\n## Materials in use\n\n");
    let mut used: BTreeMap<u64, usize> = BTreeMap::new();
    let mut untextured = 0usize;
    for face in psxed_project::grid::faces::all_faces(world) {
        match face.material {
            Some(id) => *used.entry(id.raw()).or_default() += 1,
            None => untextured += 1,
        }
    }
    let mut rows: Vec<_> = used.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));
    for (id, count) in rows.iter().take(20) {
        let name = project
            .resources
            .iter()
            .find(|resource| resource.id.raw() == **id)
            .map_or("<missing>", |resource| resource.name.as_str());
        let _ = writeln!(out, "- {name} (id {id}): {count} faces");
    }
    if rows.len() > 20 {
        let _ = writeln!(out, "- ... and {} more materials", rows.len() - 20);
    }
    let _ = writeln!(out, "\nfaces with no material: {untextured}");
}

/// Load a project from a `project.ron` path or the directory holding one.
pub fn load_project(path: &std::path::Path) -> Result<ProjectDocument, String> {
    let file = if path.is_dir() {
        path.join("project.ron")
    } else {
        path.to_path_buf()
    };
    let text = std::fs::read_to_string(&file)
        .map_err(|error| format!("read {}: {error}", file.display()))?;
    ProjectDocument::from_ron_str(&text)
        .map_err(|error| format!("parse {}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid summary names the rooms and counts faces against the cap, and
    /// the metrics point at the grid scale instead of brush-era numbers.
    #[test]
    fn scene_info_and_metrics_describe_the_grid_world() {
        let project = ProjectDocument::starter();
        let info = scene_info(&project, None).expect("the starter reports");
        assert!(info.contains("grid world: 1 rooms"), "{info}");
        assert!(info.contains("Starter room"), "{info}");
        assert!(info.contains("faces "), "{info}");
        let text = metrics(Some(&project));
        assert!(text.contains("## Grid world"), "{text}");
        assert!(text.contains("grid world of 1 rooms"), "{text}");
    }

    #[test]
    fn a_scene_index_out_of_range_is_an_error() {
        let project = ProjectDocument::starter();
        assert!(resolve_scene(&project, Some(9)).is_err());
        assert_eq!(resolve_scene(&project, None).unwrap(), 0);
    }
}
