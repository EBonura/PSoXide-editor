//! Horizontal wall bands.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::grid::bands::{self, BandCutReq, BandDeleteReq, BandMoveReq, BandSetReq};
use psxed_mcp::grid::walls::WallRef;
use psxed_mcp::grid::Materials;

use super::EditorServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct BandsReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    wall: WallRef,
}

#[rmcp::tool_router(router = bands_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "The bands of a wall run: its set, cut lines (anchor kind and value, with the heights they come to at both ends) and each band's material, tiling and V rule, top to bottom, plus how many wall segments the set applies to."
    )]
    async fn grid_bands(
        &self,
        Parameters(req): Parameters<BandsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            bands::bands(world, &Materials::new(project), &req.wall)
        })
    }

    #[rmcp::tool(
        description = "Cut a wall run with a horizontal line. Anchor bottom (above the wall's bottom, so a trim follows a sloped floor), top (below its top, so a frieze hangs from a ceiling) or abs (a room-relative datum height); value in grid units. At most six lines (seven bands) a set. Scope run (default) edits just this run, copying a shared set first; room edits the set in place, and for a wall with no bands makes the room default."
    )]
    async fn grid_band_cut(
        &self,
        Parameters(req): Parameters<BandCutReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_band_cut", |world, project| {
            bands::band_cut(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Move a band's cut line to a new anchor value (and optionally another anchor kind)."
    )]
    async fn grid_band_move(
        &self,
        Parameters(req): Parameters<BandMoveReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_band_move", |world, project| {
            bands::band_move(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Delete one cut line (the two bands around it join) or, with no line, take every band off the run."
    )]
    async fn grid_band_delete(
        &self,
        Parameters(req): Parameters<BandDeleteReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_band_delete", |world, project| {
            bands::band_delete(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Style one band: its material, tiling (world density or fit repeats across the run and between its two lines), quarter turns, and where V sits (world, from_bottom so a trim sits on the floor, from_top). Bands count from the bottom, 0 up."
    )]
    async fn grid_band_set(
        &self,
        Parameters(req): Parameters<BandSetReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_band_set", |world, project| {
            bands::band_set(world, &Materials::new(project), &req)
        })
    }
}
