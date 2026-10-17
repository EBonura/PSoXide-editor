//! Natural terrain: cracks, voids, wall heights, outlines, break and rough.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::natural::{self, ChamferReq, CrackReq, PolygonReq, SetWallReq, VoidReq};
use psxed_mcp::grid::Materials;

use super::EditorServer;

#[rmcp::tool_router(router = natural_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Crack a floor or ceiling: give one triangle of each sector in a rectangle its own height (delta in grid units, or an absolute flat height), making the surface split-level if it was not. In a derived room a diagonal wall (a riser, or a drop for a ceiling) fills the step between the two halves along the diagonal, seen from the lower half, and a step taller than the character climbs also cooks a one-sided cliff wall so rays and the camera cannot pass under it. This is the Cathedral's broken tile and 45 degree plateau edge. triangle a or b (grid_sector says which half each is); heal true merges the halves back."
    )]
    async fn grid_crack(
        &self,
        Parameters(req): Parameters<CrackReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_crack", |world, _| {
            natural::crack(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Void half a cell: triangle a, b or both of a floor, ceiling or both has no face, and in a derived room the other half closes itself along the diagonal with a solid wall (floor to ceiling, or the room's open wall height). none restores. Voiding both triangles empties the cell and its neighbours close their sides of the hole. Collision keeps the floor under a void triangle; the closure wall is what stops a body."
    )]
    async fn grid_void(
        &self,
        Parameters(req): Parameters<VoidReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_void", |world, _| {
            natural::void(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Set the heights of an authored wall directly: heights [bottom a, bottom b, top b, top a] (slanted tops and bottoms are unequal ends), or top_delta / bottom_delta added at the two ends. For a derived closure wall of a floor with no ceiling, top_delta crumbles its top. Edges may be diagonal (nwse, nesw)."
    )]
    async fn grid_set_wall(
        &self,
        Parameters(req): Parameters<SetWallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_wall", |world, _| {
            natural::set_wall(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Fill an arbitrary polygon (kind room, closed through points) or a winding corridor (kind corridor, width in sectors) with floors at floor_height, and a roof at ceiling_height for a cave (omit it for a canyon with sky), then cut every convex staircase corner of the outline into a 45 degree line: the corner cell's outer triangle is voided and the other half closes itself with a diagonal wall, so a run of unit steps becomes one straight diagonal wall instead of a staircase. Points are sector coordinates (numbers; cell [3,2] has its centre at [3.5,2.5]). chamfer none keeps the staircase; kind region only reports the cells. The room grows to hold the footprint; cells that already have a floor keep it unless overwrite is true."
    )]
    async fn grid_polygon(
        &self,
        Parameters(req): Parameters<PolygonReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_polygon", |world, project| {
            natural::polygon(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Cut the convex staircase corners of existing floors in a rectangle into 45 degree lines. mode void (default) voids the outer triangle so the cell closes itself with a diagonal wall (a room or cave outline); crack drops the outer triangle of a plateau corner by drop (a diagonal riser, the Cathedral's cut corner); restore brings voids back. A corner is a cell with exactly two adjacent neighbours missing; one-wide corridors and tips keep their square ends."
    )]
    async fn grid_chamfer(
        &self,
        Parameters(req): Parameters<ChamferReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_chamfer", |world, _| {
            natural::chamfer(world, &req)
        })
    }
}
