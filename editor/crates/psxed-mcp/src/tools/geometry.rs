//! Rooms, floors and ceilings, heights, walls: the shape of a level.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::build::{self, AddSurfaceReq, RemoveSurfaceReq, SetHeightsReq, SetSplitReq};
use psxed_mcp::grid::rooms::{
    self, AddRoomReq, ConvertRoomReq, DeleteRoomReq, SetRoomReq, ShadingReq, StreamReq,
};
use psxed_mcp::grid::walls::{self, PlaceWallReq, RemoveWallReq};
use psxed_mcp::grid::Materials;

use super::EditorServer;

#[rmcp::tool_router(router = geometry_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Add an empty room (derived walls), like the Rooms panel's +. Give origin (world sector of its first cell) and y_offset (vertical offset in grid units, 256 per click, snapped to 64). Rooms may overlap in plan, which is how floors stack. Then give it floors with grid_add_surface."
    )]
    async fn grid_add_room(
        &self,
        Parameters(req): Parameters<AddRoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_add_room", |world, _| {
            rooms::add_room(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Delete a room by index (later rooms are renumbered; the last room of a world stays). Nodes are not touched."
    )]
    async fn grid_delete_room(
        &self,
        Parameters(req): Parameters<DeleteRoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_delete_room", |world, _| {
            rooms::delete_room(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Change a room: name, raise (y_offset in grid units, snapped to 64), ambient light (128 = textures unmodified), the room's default wall material for derived walls, the open wall height for floors with no ceiling, and clean_overrides to drop wall overrides whose wall is gone."
    )]
    async fn grid_set_room(
        &self,
        Parameters(req): Parameters<SetRoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_room", |world, project| {
            rooms::set_room(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Convert a legacy (authored-wall) room to derived walls. preserve_look keeps the room looking and playing exactly as it did; fill also shows the walls it never had. New rooms are already derived."
    )]
    async fn grid_convert_room(
        &self,
        Parameters(req): Parameters<ConvertRoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_convert_room", |world, _| {
            rooms::convert_room(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Set the world's facing shade: how much darker a face is by the way it looks (off, subtle, strong, or four Q8 factors for floors, ceilings, east-west walls, north-south walls). It makes steps and platforms read under one lamp."
    )]
    async fn grid_set_shading(
        &self,
        Parameters(req): Parameters<ShadingReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_shading", |world, _| {
            rooms::set_shading(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Set the world's streaming radii in sectors, walked over the portals: draw_far (a room farther than this from the camera is not drawn and is not needed in memory; 0 is unbounded; default 48), active_radius (entities within this walk distance of the player think; default 32) and lead_radius (sectors past draw_far a streamer prefetches; default 8). ring: true streams and draws the observer's room and every room one portal away instead (for worlds cut into even chunks). They are written into the cooked level."
    )]
    async fn grid_set_stream(
        &self,
        Parameters(req): Parameters<StreamReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_stream", |world, _| {
            rooms::set_stream(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Add a flat floor or ceiling over a rectangle of WORLD sectors at a height (grid units relative to the room's y_offset; 1024 per sector, 256 per click, snapped to 256 by default or 64 with snap fine). The room grows to hold it; sectors that already have one are left alone. In a derived room the walls follow from the height differences, so a platform is just a floor at another height."
    )]
    async fn grid_add_surface(
        &self,
        Parameters(req): Parameters<AddSurfaceReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_add_surface", |world, project| {
            build::add_surface(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Remove floors or ceilings over a rectangle (a pit is a missing floor). Emptied sectors go and the room shrinks around what is left."
    )]
    async fn grid_remove_surface(
        &self,
        Parameters(req): Parameters<RemoveSurfaceReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_remove_surface", |world, _| {
            build::remove_surface(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Set (height) or move (delta) corner heights of floors or ceilings over a rectangle: the whole face, chosen corners (nw, ne, se, sw), or one edge's two corners (north, east, south, west) to build a slope. Heights snap to 256 (click) or 64 (fine). link moves welded corners of neighbours too (the editor's linked drag); default off so a platform keeps its edge and gets a riser."
    )]
    async fn grid_set_heights(
        &self,
        Parameters(req): Parameters<SetHeightsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_heights", |world, _| {
            build::set_heights(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Set or flip the diagonal split of floors or ceilings (nw_se, ne_sw or flip). A bent quad folds along its split, so this decides which way a twisted surface reads."
    )]
    async fn grid_set_split(
        &self,
        Parameters(req): Parameters<SetSplitReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_split", |world, _| {
            build::set_split(world, &req)
        })
    }

    #[rmcp::tool(
        description = "Place a run of authored walls along an edge with the editor's gap-filling wall tool (floor to ceiling or to the next wall, following slopes). In a derived room an edge a floor already closes answers 'covered': author a wall only for what heights cannot say (a freestanding wall, a parapet). Edges are north (-Z), east (+X), south (+Z), west (-X), nwse, nesw."
    )]
    async fn grid_place_wall(
        &self,
        Parameters(req): Parameters<PlaceWallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_place_wall", |world, project| {
            walls::place_wall(world, &Materials::new(project), &req)
        })
    }

    #[rmcp::tool(
        description = "Remove authored walls along an edge, one per cell (the topmost, or the one nearest pick_y). Derived walls are not records: hide one with grid_wall_override."
    )]
    async fn grid_remove_wall(
        &self,
        Parameters(req): Parameters<RemoveWallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_remove_wall", |world, _| {
            walls::remove_walls(world, &req)
        })
    }
}
