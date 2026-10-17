//! Free planes: bridges, ramps, slanted walls, banners.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::grid::planes::{self, AddPlaneReq, DeletePlaneReq, SetPlaneReq};
use psxed_mcp::grid::Materials;

use super::EditorServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct PlanesReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Only this room. Omit for every room.
    room: Option<usize>,
}

#[rmcp::tool_router(router = planes_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "List free planes with index, centre, size, rotation, material, tiling, sidedness, collision class and what that cooks to (platform you stand on, wall segment, blocker box), and the sectors each covers."
    )]
    async fn grid_planes(
        &self,
        Parameters(req): Parameters<PlanesReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            planes::list(world, &Materials::new(project), req.room)
        })
    }

    #[rmcp::tool(
        description = "Add a free plane: a flat quad off the sector lattice for a bridge, ramp, slanted wall, banner or grate. Place it by centre, size and rotation (yaw, pitch, roll degrees; pitch 90 lays it flat facing up) or, for a bridge or ramp, by the midpoints of its two ends (from, to) and a width. Collision decor (drawn only), walkable (stand on it) or solid. Positions are GRID units (x, z on the world lattice, 1024 per sector; y relative to the room's y_offset). The room grows to hold the plane."
    )]
    async fn grid_add_plane(
        &self,
        Parameters(req): Parameters<AddPlaneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_add_plane", |world, project| {
            planes::add(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Change a plane by index: placement (any of centre, size, rotation, orientation, or a from/to span), material, tiling mode, sidedness, collision class, fullbright, camera pass-through, or enabled."
    )]
    async fn grid_set_plane(
        &self,
        Parameters(req): Parameters<SetPlaneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_plane", |world, project| {
            planes::set(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Delete planes by index (higher indices go first; the rest are renumbered)."
    )]
    async fn grid_delete_plane(
        &self,
        Parameters(req): Parameters<DeletePlaneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_delete_plane", |world, _| {
            planes::delete(world, &req)
        })
    }
}
