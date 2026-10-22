//! The MCP server and its tools, one file per area.
//!
//! Each file adds a `#[rmcp::tool_router]` block to [`EditorServer`] under its
//! own router name; [`EditorServer::new`] adds the routers together. The tool
//! bodies themselves live in the library crate as pure functions of a
//! `ProjectDocument`, so they could move into a live editor bridge unchanged.

mod bands;
mod cells;
mod combat;
mod flood;
mod geometry;
mod instructions;
mod meshes;
mod natural;
mod nodes;
mod planes;
mod project;
mod query;
mod storeys;
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
    /// The tool names each router registered, for the instructions.
    pub(crate) areas: Arc<Vec<(&'static str, Vec<String>)>>,
}

impl EditorServer {
    /// A server over `workspace`.
    pub fn new(workspace: Workspace, frontend: Option<PathBuf>, scratch: PathBuf) -> Self {
        let routers = Self::routers();
        let areas: Vec<(&'static str, Vec<String>)> = routers
            .iter()
            .map(|(name, router)| {
                (
                    *name,
                    router
                        .list_all()
                        .iter()
                        .map(|t| t.name.to_string())
                        .collect(),
                )
            })
            .collect();
        let tool_router = routers
            .into_iter()
            .map(|(_, router)| router)
            .fold(ToolRouter::new(), |all, router| all + router);
        Self {
            workspace: Arc::new(Mutex::new(workspace)),
            frontend: frontend.map(Arc::new),
            scratch: Arc::new(scratch),
            tool_router,
            areas: Arc::new(areas),
        }
    }

    /// Every router with the name the instructions list it under (see
    /// `instructions::AREAS`).
    fn routers() -> Vec<(&'static str, ToolRouter<Self>)> {
        vec![
            ("project", Self::project_router()),
            ("nodes", Self::nodes_router()),
            ("combat", Self::combat_router()),
            ("query", Self::query_router()),
            ("geometry", Self::geometry_router()),
            ("surfaces", Self::surfaces_router()),
            ("bands", Self::bands_router()),
            ("cells", Self::cells_router()),
            ("flood", Self::flood_router()),
            ("storeys", Self::storeys_router()),
            ("planes", Self::planes_router()),
            ("meshes", Self::meshes_router()),
            ("verbs", Self::verbs_router()),
            ("natural", Self::natural_router()),
            ("verify", Self::verify_router()),
        ]
    }

    /// A server over nothing, to list the tools (tests).
    #[cfg(test)]
    pub(crate) fn for_listing() -> Self {
        let workspace = Workspace::open(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../projects/default"),
        )
        .expect("the default project opens");
        Self::new(workspace, None, std::env::temp_dir())
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
