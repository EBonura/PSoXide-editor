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

// The instructions are composed from the registry, so `get_info` is written
// here and the macro adds the tool calls only.
#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for EditorServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new("psoxide-editor", "0.2.0"))
        .with_instructions(self.instructions())
    }
}

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
