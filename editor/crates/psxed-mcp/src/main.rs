//! `psxed-mcp` -- MCP server exposing PSoXide level authoring to an agent.
//!
//! Phase 1 is read-only and runs offline over stdio against a project file,
//! because the tool bodies are pure functions of a `ProjectDocument` and the
//! live editor is a 75-second rebuild. Phase 2 adds writes; the live in-GUI
//! bridge (mirroring `emu/crates/frontend/src/mcp.rs`) comes after that.
//! See `docs/editor-mcp-plan-2026-09-18.md`.
//!
//! ```bash
//! psxed-mcp --project editor/projects/default
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use base64::Engine as _;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

use psxed_mcp::{load_project, metrics, plan_view, scene_info, Focus, PlanAxis};

#[derive(Clone)]
struct EditorServer {
    project_path: Arc<PathBuf>,
    /// Built by `#[rmcp::tool_router]`; read by the generated handler, not
    /// by this file, so the dead-code pass cannot see the use.
    #[allow(dead_code)]
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SceneReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlanReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// `top` for a floor plan, `front` or `side` for a vertical section.
    axis: Option<String>,
    /// World coordinate to cut at, on the axis the view does not plot. Omit
    /// for 512 above the scene floor, then re-aim using the legend's
    /// candidate floor levels.
    slice: Option<i32>,
    /// Image width in pixels (256..2048, default 900).
    width: Option<u32>,
    /// World point `[x, y, z]` to frame on. Without it the whole level is
    /// drawn, where the player is under a pixel. Needs `extent`.
    center: Option<[i32; 3]>,
    /// Half-span in world units around `center`. Roughly 3000 frames one
    /// room at a readable human scale.
    extent: Option<i32>,
}

#[rmcp::tool_router]
impl EditorServer {
    fn new(project_path: PathBuf) -> Self {
        Self {
            project_path: Arc::new(project_path),
            tool_router: Self::tool_router(),
        }
    }

    /// Reload every call: the user edits the same file in the GUI, so a
    /// cached document would quietly go stale.
    fn project(&self) -> Result<psxed_project::ProjectDocument, ErrorData> {
        load_project(&self.project_path).map_err(|error| ErrorData::internal_error(error, None))
    }

    #[rmcp::tool(
        description = "Scale constants, the player's dimensions, the working grid, and the space sizes measured from the shipped level. Read this before authoring geometry so sizes are looked up rather than guessed."
    )]
    async fn metrics(&self) -> Result<CallToolResult, ErrorData> {
        let project = self.project().ok();
        Ok(CallToolResult::success(vec![ContentBlock::text(metrics(
            project.as_ref(),
        ))]))
    }

    #[rmcp::tool(
        description = "Extents, brush and face counts, groups, materials in use, and the candidate floor levels of a scene."
    )]
    async fn scene_info(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.project()?;
        let text =
            scene_info(&project, scene).map_err(|error| ErrorData::internal_error(error, None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Orthographic floor plan or vertical section of the level's solid brushes, drawn to scale with a grid and the player disc for comparison. Use this, not a 3D screenshot, to reason about the size and shape of spaces."
    )]
    async fn plan_view(
        &self,
        Parameters(PlanReq {
            scene,
            axis,
            slice,
            width,
            center,
            extent,
        }): Parameters<PlanReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.project()?;
        let axis = PlanAxis::parse(axis.as_deref().unwrap_or("top"))
            .map_err(|error| ErrorData::invalid_params(error, None))?;
        let focus = match (center, extent) {
            (Some(center), Some(extent)) => Some(Focus { center, extent }),
            (Some(center), None) => Some(Focus {
                center,
                extent: 3000,
            }),
            (None, Some(_)) => {
                return Err(ErrorData::invalid_params(
                    "extent needs center: give the world point to frame on".to_string(),
                    None,
                ))
            }
            (None, None) => None,
        };
        let view = plan_view(&project, scene, axis, slice, width.unwrap_or(900), focus)
            .map_err(|error| ErrorData::internal_error(error, None))?;
        Ok(CallToolResult::success(vec![
            ContentBlock::text(view.legend),
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(&view.png),
                "image/png".to_string(),
            ),
        ]))
    }
}

#[rmcp::tool_handler(
    name = "psoxide-editor",
    version = "0.1.0",
    instructions = "PSoXide level authoring. Call `metrics` first: it carries the authored-unit scale, the player's size, the 64-unit working grid, and the ceiling heights that actually shipped, so you do not invent dimensions. Read the space with `plan_view` rather than a 3D render: the level is dark night-time art rendered at fullbright, so volumes read as black masses in perspective. Section a floor plan at a candidate floor level plus 512, and check a `front` or `side` section before trusting a height."
)]
impl ServerHandler for EditorServer {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = PathBuf::from("editor/projects/default");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                project = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--project needs a path")?;
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    // Fail loudly at startup rather than on every tool call.
    load_project(&project)?;
    let service = EditorServer::new(project)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
