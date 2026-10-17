//! Static meshes: arches, columns, rubble and imported props.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::grid::meshes::{self, ArchSpanReq, DeleteMeshReq, PlaceMeshReq, SetMeshReq};
use psxed_mcp::grid::Materials;

use super::EditorServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct MeshesReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Only the instances in this room. Omit for every room.
    room: Option<usize>,
}

#[rmcp::tool_router(router = meshes_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "List static mesh ASSETS (index, name, recipe, faces, collision shapes, sockets, material slots, how often placed) and the placed INSTANCES (stable id, asset, position, rotation, scale, materials, sectors covered). A mesh is not grid geometry: arches, columns, rubble and imported props are assets a room places copies of. Also lists the recipe kinds a new asset can come from."
    )]
    async fn grid_meshes(
        &self,
        Parameters(req): Parameters<MeshesReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            meshes::list(world, &Materials::new(project), req.room)
        })
    }

    #[rmcp::tool(
        description = "Place a static mesh: an existing asset (by index or name) or a recipe that makes one (arch, column, rubble, or obj from a .obj file path). Position is GRID units: x and z on the world lattice (1024 per sector), y relative to the room's y_offset; rotation [yaw, pitch, roll] degrees, scale one number or three, a material per slot. The room grows to hold it. Draws like a free plane run, collides through the native grid collision (piers and columns block, flat tops are surfaces to stand on), and an arch's crown socket cooks to a hook point the player can perch under."
    )]
    async fn grid_place_mesh(
        &self,
        Parameters(req): Parameters<PlaceMeshReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_place_mesh", |world, project| {
            meshes::place(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Change a placed mesh by its stable id: position, rotation or yaw, scale, material (all slots or per slot), collide, fullbright, enabled, another asset, or asset_recipe to change the numbers of its asset in place (every copy of that asset changes: a taller column, a wider arch)."
    )]
    async fn grid_set_mesh(
        &self,
        Parameters(req): Parameters<SetMeshReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_mesh", |world, project| {
            meshes::set(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Delete placed meshes by stable id. Other ids are unchanged and the assets stay in the palette."
    )]
    async fn grid_delete_mesh(
        &self,
        Parameters(req): Parameters<DeleteMeshReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_delete_mesh", |world, _| {
            meshes::delete(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Generate an arch and place it spanning two points: from and to are the floor points under the two springings (grid units), the clear opening is their distance and the arch is turned to run along them, standing at the lower height. Profile round, segmental, pointed or flat; rise, spring (pier height: keep it above the 48 unit body, about 1000 grid units at the default scale, if a body is to walk through), thickness, depth, segments per half. perch (default true) puts a hook socket at the crown: the cook spawns a hook point there and the player can hang from the crown. Collision: piers block, the extrados is a surface to stand on, the haunches block as boxes (the cook says so). Reports the numbers and where the hook grips."
    )]
    async fn grid_arch_span(
        &self,
        Parameters(req): Parameters<ArchSpanReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_arch_span", |world, project| {
            meshes::arch(world, &Materials::new(project), &req)
        })
    }
}
