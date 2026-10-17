//! Project-level tools: reads, save and revert, snapshots, checkpoints.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::edit::Workspace;
use psxed_mcp::inspect::{materials as material_table, materials_filtered};
use psxed_mcp::{metrics, scene_info, shot};

use super::{text_result, EditorServer};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SceneReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MaterialsReq {
    /// List only the materials a wall, floor or ceiling is likely to use (the
    /// grid editor's palette: what the world already uses, level textures and
    /// the Graybox set) instead of all of the project's, which include HUD,
    /// sky and character images. Default false.
    palette_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CheckpointReq {
    /// Checkpoint name. Omit for the next free `cp-N`. An existing name is
    /// replaced.
    name: Option<String>,
}

#[rmcp::tool_router(router = project_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Grid units, the world scale, the player's dimensions and the traversal limits. Read this before placing anything so sizes are looked up rather than guessed."
    )]
    async fn metrics(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let project = workspace.document()?;
            Ok(metrics(Some(project)))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "The grid world of a scene: rooms, sectors, faces against the face budget, materials in use, and the scene's node counts."
    )]
    async fn scene_info(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(scene_info(project, scene)? + &note)
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "List the project's materials with their texel dimensions, how many world units one tile covers, colour depth, blend mode and tint. The tile size is what decides how often a texture repeats on a surface, and it is not stored in the project file. Pass palette_only to see just the world palette."
    )]
    async fn materials(
        &self,
        Parameters(MaterialsReq { palette_only }): Parameters<MaterialsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let project = workspace.document()?;
            if palette_only.unwrap_or(false) {
                let wanted: std::collections::HashSet<u64> =
                    psxed_project::grid::palette::palette_materials(project, false)
                        .into_iter()
                        .map(|(id, _)| id.raw())
                        .collect();
                return Ok(materials_filtered(project, &root, Some(&wanted)));
            }
            Ok(material_table(project, &root))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Health check: which project is open, whether edits are staged, the scene's world and node counts, how many checkpoints exist, and whether a renderer binary was found. Call this first in a session, and whenever something behaves unexpectedly."
    )]
    async fn status(&self) -> Result<CallToolResult, ErrorData> {
        let renderer =
            match shot::find_frontend(self.frontend.as_deref().map(std::path::PathBuf::as_path)) {
                Ok(path) => format!("{} (screenshot and playtest available)", path.display()),
                Err(error) => format!("NONE: {error}"),
            };
        let text = self.with(|workspace| {
            let staged = workspace.log().len();
            let dirty = workspace.is_dirty();
            let backups = workspace.backups().len();
            let checkpoints = workspace.checkpoint_names();
            let path = workspace.root().to_path_buf();
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, None)?;
            let scene = &project.scenes[index];
            let world = scene.world.grid().map_or_else(
                || "no grid world".to_string(),
                |world| format!("grid world of {} rooms", world.rooms.len()),
            );
            Ok(format!(
                concat!(
                    "project: {}\n",
                    "scene {}: {}, {} nodes\n",
                    "staged edits: {}{}\n",
                    "checkpoints: {}\n",
                    "snapshots: {}\n",
                    "renderer: {}"
                ),
                path.display(),
                format!("{index} {:?}", scene.name),
                world,
                scene.nodes().len(),
                staged,
                if dirty {
                    " -- unsaved (screenshot and playtest still show them: they render a staged copy)"
                } else {
                    ""
                },
                if checkpoints.is_empty() {
                    "none".to_string()
                } else {
                    checkpoints.join(", ")
                },
                backups,
                renderer
            ))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Snapshot the staged document in memory under a name. Cheap; take one before a risky stretch of edits and restore it if the result is wrong. Nothing is written to disk."
    )]
    async fn checkpoint(
        &self,
        Parameters(CheckpointReq { name }): Parameters<CheckpointReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| workspace.checkpoint(name.as_deref()))?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Put the staged document back to a checkpoint (the newest when no name is given). Later checkpoints stay, so a restore can itself be undone. Nothing is written to disk; `save` does that."
    )]
    async fn restore(
        &self,
        Parameters(CheckpointReq { name }): Parameters<CheckpointReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| workspace.restore(name.as_deref()))?;
        Ok(text_result(text))
    }

    #[rmcp::tool(description = "List the in-memory checkpoints, oldest first.")]
    async fn checkpoints(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let names = workspace.checkpoint_names();
            Ok(if names.is_empty() {
                "no checkpoints; call checkpoint to take one".to_string()
            } else {
                format!(
                    "{} checkpoint(s), oldest first: {}",
                    names.len(),
                    names.join(", ")
                )
            })
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Write the staged edits to project.ron. Refuses if the file changed on disk since the edits were staged, which means the editor saved over it. The previous file is snapshotted first."
    )]
    async fn save(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::save)?;
        Ok(text_result(text))
    }

    #[rmcp::tool(description = "Throw away every staged edit and re-read project.ron from disk.")]
    async fn revert(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::revert)?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Roll project.ron back to the snapshot taken before the last save. The version it replaces is snapshotted first, so the undo is itself undoable. For edits that have not been saved yet, use restore or revert instead."
    )]
    async fn undo_last_edit(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::undo_last_edit)?;
        Ok(text_result(text))
    }

    #[rmcp::tool(description = "List the pre-save snapshots, newest first.")]
    async fn list_backups(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let backups = workspace.backups();
            Ok(if backups.is_empty() {
                "no snapshots yet; one is written each time save replaces the file".to_string()
            } else {
                format!(
                    "{} snapshot(s), newest first:\n{}",
                    backups.len(),
                    backups.join("\n")
                )
            })
        })?;
        Ok(text_result(text))
    }
}
