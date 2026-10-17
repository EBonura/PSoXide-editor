//! Materials, tiling and wall overrides.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::paint::{self, MaterialTilingReq, PaintReq, PaintWallReq, TilingReq};
use psxed_mcp::grid::walls::{self, WallOverrideReq};
use psxed_mcp::grid::Materials;

use super::{text_result, EditorServer};

#[rmcp::tool_router(router = surfaces_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Paint a material on faces over a rectangle of WORLD sectors: floor, ceiling, walls (authored and derived), or all. Derived walls are picked by the sector that owns the step (a platform's cells own its sides), so painting a platform's rectangle with faces [derived] paints its sides; by_host picks them by the sector that sees them. Material is a name (exact or unique substring) or id; see `materials`."
    )]
    async fn grid_paint(
        &self,
        Parameters(req): Parameters<PaintReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_paint", |world, project| {
            paint::paint(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Paint one wall (a derived wall by host cell, edge and kind, or an authored one by index) or, with run, the whole run of collinear faces it belongs to."
    )]
    async fn grid_paint_wall(
        &self,
        Parameters(req): Parameters<PaintWallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_paint_wall", |world, project| {
            paint::paint_wall(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Set the texture tiling of faces: mode world (texels per sector, the texture follows the world), fit (N x M tiles across the whole face), or inherit (the material's default); plus quarter turns, flips, offset, scale and rotation. Targets a rectangle's faces or one wall (run: the whole run). Reports the effective result ('2 x 2 on this face') and warns when the cook has to cut the face."
    )]
    async fn grid_tiling(
        &self,
        Parameters(req): Parameters<TilingReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_tiling", |world, project| {
            paint::tiling(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "Set a material's grid tiling default (world density or fit repeats). Faces whose own mode is inherit follow it; a face that names its own mode ignores it. This edits the material resource, not the world."
    )]
    async fn grid_material_tiling(
        &self,
        Parameters(req): Parameters<MaterialTilingReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace
                .project_edit("grid_material_tiling", |p| paint::material_tiling(p, &req))?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Override one wall: hide or show a derived face, make it solid or passable (a ghost you can walk through), paint it, set an authored wall's facing, or reset a derived segment to automatic. A wall is addressed by host cell, edge and kind (riser, drop, closure) or authored index; grid_walls prints them."
    )]
    async fn grid_wall_override(
        &self,
        Parameters(req): Parameters<WallOverrideReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_wall_override", |world, project| {
            walls::wall_override(world, &Materials::new(project), &req)
        })
    }
}
