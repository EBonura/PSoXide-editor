//! Storeys: the storey list, a column, the slab verbs and the shift above a
//! height.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::storeys::{self, ColumnReq, CutReq, ShiftReq, SlabReq, StoreysReq};

use super::EditorServer;

#[rmcp::tool_router(router = storeys_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "The storeys of a grid world: the floor heights that many cells share (eight or more, merged within four clicks), with how many leaves hold 1, 2, 3 ... spans per room. A storey is derived, never stored; read it before you place slabs."
    )]
    async fn grid_storeys(
        &self,
        Parameters(req): Parameters<StoreysReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| Ok(storeys::storeys_text(world)))
    }

    #[rmcp::tool(
        description = "One cell's column: each span with its floor and ceiling (clicks, a range when sloped), the solid between spans, and the derived walls the cell hosts with their kind, edge and owner. Heights are relative to the room."
    )]
    async fn grid_column(
        &self,
        Parameters(req): Parameters<ColumnReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| {
            let room = psxed_mcp::grid::pick_room(world, req.room)?;
            storeys::column_text(world, room, req.cell)
        })
    }

    #[rmcp::tool(
        description = "Insert a slab: a floor at height y (grid units, room relative) over rectangles of world sectors. A span holding y is split: the one below keeps the floor and takes the slab's underside, the one above takes the new floor. A cell whose lower part would be under 3 clicks is a sliver: its floor rises to y instead. Cells with no span at y, a sloped surface across y, or too little headroom are skipped and counted. One call is one undo step."
    )]
    async fn grid_insert_slab(
        &self,
        Parameters(req): Parameters<SlabReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_insert_slab", |world, project| {
            storeys::slab(world, project, &req, false)
        })
    }

    #[rmcp::tool(
        description = "Fill a hole: the inverse of grid_cut_hole, a slab inserted at y over cells where the column is open through y. Same arguments as grid_insert_slab."
    )]
    async fn grid_fill_hole(
        &self,
        Parameters(req): Parameters<SlabReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_fill_hole", |world, project| {
            storeys::slab(world, project, &req, true)
        })
    }

    #[rmcp::tool(
        description = "Cut a hole: remove the slab whose floor is at y (which one) or every slab at or above y (which all_above) over rectangles of world sectors. The span above merges into the span below, so a stairwell opens. Cells with no flat floor at y are skipped and counted."
    )]
    async fn grid_cut_hole(
        &self,
        Parameters(req): Parameters<CutReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_cut_hole", |world, _| {
            storeys::cut(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Shift above a height: move every floor, ceiling and authored wall corner at or above y by delta (grid units, positive is up), over rectangles or the whole room. Surfaces that cross y stretch. Refused, with nothing changed, when a column would overlap itself. Nodes are not moved in this phase."
    )]
    async fn grid_shift_above(
        &self,
        Parameters(req): Parameters<ShiftReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_shift_above", |world, _| {
            storeys::shift(world, &req)
        })
    }
}
