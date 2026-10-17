//! The MCP server and its tools, one file per area.
//!
//! Each file adds a `#[rmcp::tool_router]` block to [`EditorServer`] under its
//! own router name; [`EditorServer::new`] adds the routers together. The tool
//! bodies themselves live in the library crate as pure functions of a
//! `ProjectDocument`, so they could move into a live editor bridge unchanged.

mod bands;
mod combat;
mod geometry;
mod natural;
mod nodes;
mod planes;
mod project;
mod query;
mod surfaces;
mod verbs;
mod verify;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::ErrorData;

use psxed_mcp::edit::Workspace;
use psxed_project::grid::GridWorld;
use psxed_project::ProjectDocument;

/// The server: a staged workspace plus where the renderer and scratch live.
#[derive(Clone)]
pub struct EditorServer {
    /// Edits stage here and reach disk only on `save`.
    workspace: Arc<Mutex<Workspace>>,
    /// Renderer binary, or None to search the usual target directories.
    frontend: Option<Arc<PathBuf>>,
    /// Directory for intermediate files (frames, tapes, logs).
    scratch: Arc<PathBuf>,
    /// Every tool, combined from the per-area routers.
    pub(crate) tool_router: ToolRouter<Self>,
}

impl EditorServer {
    /// A server over `workspace`.
    pub fn new(workspace: Workspace, frontend: Option<PathBuf>, scratch: PathBuf) -> Self {
        let tool_router = Self::project_router()
            + Self::nodes_router()
            + Self::combat_router()
            + Self::query_router()
            + Self::geometry_router()
            + Self::surfaces_router()
            + Self::bands_router()
            + Self::planes_router()
            + Self::verbs_router()
            + Self::natural_router()
            + Self::verify_router();
        Self {
            workspace: Arc::new(Mutex::new(workspace)),
            frontend: frontend.map(Arc::new),
            scratch: Arc::new(scratch),
            tool_router,
        }
    }

    /// Run `body` against the workspace. Reads see staged edits; with nothing
    /// staged the file is re-read, so the GUI's own saves are picked up.
    fn with<T>(
        &self,
        body: impl FnOnce(&mut Workspace) -> Result<T, String>,
    ) -> Result<T, ErrorData> {
        let mut workspace = self
            .workspace
            .lock()
            .map_err(|_| ErrorData::internal_error("workspace lock poisoned".to_string(), None))?;
        body(&mut workspace).map_err(|error| ErrorData::internal_error(error, None))
    }

    /// Append the staged-edit reminder so a session cannot drift into
    /// believing its work is on disk.
    fn staged_note(workspace: &Workspace) -> String {
        if workspace.is_dirty() {
            format!(
                "\n\n[{} edit(s) staged in memory, not yet written. Call save when the \
                 section looks right, or revert to drop them.]",
                workspace.log().len()
            )
        } else {
            String::new()
        }
    }

    /// A grid edit: runs `body` on a copy of the scene's world, stages it, and
    /// reports.
    fn grid_edit(
        &self,
        scene: Option<usize>,
        label: &str,
        body: impl FnOnce(&mut GridWorld, &ProjectDocument) -> Result<String, String>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.grid_edit(scene, label, body)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    /// A grid read: runs `body` over the scene's world.
    fn grid_read(
        &self,
        scene: Option<usize>,
        body: impl FnOnce(&GridWorld, &ProjectDocument) -> Result<String, String>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, scene)?;
            let world = project.scenes[index]
                .world
                .grid()
                .ok_or_else(|| format!("scene {index} has no grid world"))?;
            Ok(body(world, project)? + &note)
        })?;
        Ok(text_result(text))
    }
}

/// A plain text reply.
pub fn text_result(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// A PNG image block.
pub fn png_block(png: &[u8]) -> ContentBlock {
    ContentBlock::image(
        base64::engine::general_purpose::STANDARD.encode(png),
        "image/png".to_string(),
    )
}
