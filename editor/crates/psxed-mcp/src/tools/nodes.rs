//! Scene-node tools: entities, lights, spawn points.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::nodes::{entity_types, get_node};

use super::project::SceneReq;
use super::{text_result, EditorServer};

#[derive(Debug, Deserialize, JsonSchema)]
struct NodeLookupReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node name (exact, or a unique substring), or its numeric id.
    node: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PlaceNodeReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node to clone, by name or id. Its whole subtree comes with it.
    source: String,
    /// World position `[x, y, z]` for the clone, EDITOR units (a sector is
    /// `scene_info`'s sector size, 1280 for a new grid project).
    position: [i32; 3],
    /// Name for the clone. Defaults to "<source> copy".
    name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SetNodeReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node to edit, by name or id.
    node: String,
    /// The replacement kind payload as RON, in the form get_node prints. It
    /// must be the same variant the node already is.
    kind: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MoveNodeReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node to move, by name or id.
    node: String,
    /// New world position `[x, y, z]`, EDITOR units.
    position: [i32; 3],
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LightReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// World position `[x, y, z]`, EDITOR units.
    position: [i32; 3],
    /// Reach in WORLD units (the tool converts to the sectors the format
    /// stores). A 1024 radius lights roughly one player-height sphere.
    radius: i32,
    /// RGB, default warm white.
    color: Option<[u8; 3]>,
    /// Brightness multiplier, 0..=8, default 1.
    intensity: Option<f32>,
    /// Node name in the scene tree.
    name: Option<String>,
}

#[rmcp::tool_router(router = nodes_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "List the node kinds present in the scene with counts and example names. Start here for entities: enemies, spawn points, cameras, triggers, points of interest and lights are all scene nodes."
    )]
    async fn entity_types(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(entity_types(project, scene)? + &note)
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Full detail for one node: kind, world position, rotation, parent, children, and its kind payload as RON. The RON is what set_node takes back. Node positions are EDITOR units; a grid sector is 1024 grid units = the world's sector_editor_units (grid_rooms prints it)."
    )]
    async fn get_node(
        &self,
        Parameters(NodeLookupReq { scene, node }): Parameters<NodeLookupReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(get_node(project, scene, &node)? + &note)
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Clone an existing node, with its whole subtree, to a world position. This is how you place an enemy or any other kind: an enemy is a host Entity plus Model Renderer, Animator and Character Controller children, so copying a working one beats building it from parts."
    )]
    async fn place_node(
        &self,
        Parameters(PlaceNodeReq {
            scene,
            source,
            position,
            name,
        }): Parameters<PlaceNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.place_node(scene, &source, position, name.as_deref())?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Replace a node's kind payload from RON, in the form get_node prints. Covers every field of every node kind; it must stay the same variant."
    )]
    async fn set_node(
        &self,
        Parameters(SetNodeReq { scene, node, kind }): Parameters<SetNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.set_node(scene, &node, &kind)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Move a node to a world position (editor units). Use this for the player spawn: the player node must stand on a floor."
    )]
    async fn move_node(
        &self,
        Parameters(MoveNodeReq {
            scene,
            node,
            position,
        }): Parameters<MoveNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.move_node(scene, &node, position)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(description = "Remove a node and its subtree.")]
    async fn delete_node(
        &self,
        Parameters(NodeLookupReq { scene, node }): Parameters<NodeLookupReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.delete_node(scene, &node)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Place a static point light. Radius is given in world units here and converted to the sectors the format stores. Position is editor units."
    )]
    async fn add_light(
        &self,
        Parameters(LightReq {
            scene,
            position,
            radius,
            color,
            intensity,
            name,
        }): Parameters<LightReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.add_light(
                scene,
                position,
                radius,
                color.unwrap_or([255, 236, 208]),
                intensity.unwrap_or(1.0),
                name.as_deref(),
            )?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "List the scene's point lights with their world positions and radii in world units."
    )]
    async fn lights(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let lights = workspace.lights()?;
            if lights.is_empty() {
                return Ok("no point lights in the scene".to_string());
            }
            let mut out = format!("{} point light(s):\n", lights.len());
            for (name, position, radius) in &lights {
                out.push_str(&format!(
                    "- {name:?} at [{:.0}, {:.0}, {:.0}], radius {radius:.0} units\n",
                    position[0], position[1], position[2]
                ));
            }
            Ok(out)
        })?;
        Ok(text_result(text))
    }
}
