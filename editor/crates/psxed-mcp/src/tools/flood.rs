//! Streaming reads: the cells a world is cut into and the flood that keeps
//! them resident.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::flood::{self, CellsReq, FloodReq};

use super::EditorServer;

#[rmcp::tool_router(router = flood_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "The cells a world streams as (rooms, chunks): how many, how many sectors and pages each, the portals between them (open-air, opening, vertical), and the pool the level's own streaming rule needs. With sweep: true it also floods from a spread of cameras (the guest's own flood, to the pool ceiling) and reports the cells, pages, boundary distance and cost against the ring rule, plus the pool each resident radius needs. rechunk_hex: 4 reads the world cut into hexagons of that size without changing it. The numbers the flood-streaming design is sized from."
    )]
    async fn grid_cells(
        &self,
        Parameters(req): Parameters<CellsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, req.scene)?;
            let world = project.scenes[index]
                .world
                .grid()
                .ok_or_else(|| format!("scene {index} has no grid world"))?;
            Ok(flood::cells(project, &root, world, &req)? + &note)
        })?;
        Ok(super::text_result(text))
    }

    #[rmcp::tool(
        description = "Flood the portal graph from a camera at a WORLD sector, the way the guest streams a flood level: which cells are drawn (within the draw radius), which are kept resident (within the resident radius and the page budget), their pages, the boundary portals, the quads in the plan cone of the heading, and the work the flood took. near, res and budget_pages default to the world's flood settings and the guest's pool; rechunk_hex previews a hexagon cut."
    )]
    async fn grid_flood(
        &self,
        Parameters(req): Parameters<FloodReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, req.scene)?;
            let world = project.scenes[index]
                .world
                .grid()
                .ok_or_else(|| format!("scene {index} has no grid world"))?;
            Ok(flood::flood(project, &root, world, &req)? + &note)
        })?;
        Ok(super::text_result(text))
    }
}
