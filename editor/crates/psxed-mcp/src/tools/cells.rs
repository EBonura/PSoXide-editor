//! Cells of several sizes: subdivide, merge and the read that maps them.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::cells::{self, LeavesReq, MergeReq, SubdivideReq};

use super::EditorServer;

#[rmcp::tool_router(router = cells_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Subdivide the cells of a rectangle of WORLD sectors (or, with unit click, of clicks: quarter sectors) into four finer ones: the editor's Cells tool. size one_level (default) splits every cell once, half takes cells down to half a sector and quarter to the finest. A subdivision looks and stands exactly as the cell did (heights are interpolated; a folded sector keeps its fold); the tree is balanced afterwards, so a neighbour more than one level coarser is split too, and the hanging corners on a coarse edge are constrained to its average. Use it to put detail (steps, a small slope) in one corner of a big flat floor, then grid_raise with unit click to shape it. The reply gives the counts by size; grid_leaves maps them."
    )]
    async fn grid_subdivide(
        &self,
        Parameters(req): Parameters<SubdivideReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_subdivide", |world, _| {
            cells::subdivide(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Merge the fine cells of a rectangle of WORLD sectors (or clicks) back into coarser ones, as far as is exact: four cells that share a parent become it only when the parent looks and stands exactly as the four did (one plane, the same materials, no wall between them). up_to sector (default), two or four sectors across. Nothing is approximated; when nothing merges the reply says why."
    )]
    async fn grid_merge(
        &self,
        Parameters(req): Parameters<MergeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_merge", |world, _| {
            cells::merge(world, &req)
        })
    }

    #[rmcp::tool(
        description = "The cells of a room: how many of each size (4 sectors across down to a quarter sector), whether the tree is sound (balanced, big cells planar, hanging corners constrained), and a map with one character per click (a quarter sector) giving the level of the cell there: 0 = 4 sectors across, 1 = 2, 2 = a whole sector, 3 = half, 4 = a quarter. grid_plan draws the same borders in colour."
    )]
    async fn grid_leaves(
        &self,
        Parameters(req): Parameters<LeavesReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| cells::leaves(world, &req))
    }
}
