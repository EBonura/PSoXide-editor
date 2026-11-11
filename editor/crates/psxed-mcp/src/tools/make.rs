//! The Build group over MCP: rooms, corridors, openings, arcades, bridges,
//! buildings, terrain patches, ruins, styles, doors and levers.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::make::{
    self, ArcadeReq, BridgeReq, BuildingReq, CaptureReq, CorridorReq, DoorReq, LeverReq, MoveReq,
    OpeningReq, RoomReq, RuinReq, TerrainReq, TiltReq, WallReq,
};

use super::{text_result, EditorServer};

impl EditorServer {
    /// Run a project edit that also needs the project folder.
    fn make_edit(
        &self,
        label: &str,
        body: impl FnOnce(
            &mut psxed_project::ProjectDocument,
            &std::path::Path,
        ) -> Result<String, String>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let report = workspace.project_edit(label, |doc| body(doc, &root))?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(text_result(text))
    }
}

#[rmcp::tool_router(router = make_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Make a walled room: the editor's Room tool. A rectangle of WORLD sectors (from, to), a floor height (base) and a floor to ceiling height (default 3072); the walls come from the cells. shape rectangle, octagon (cut corners) or polygon; room puts it inside an existing derived room, else it is a new room; style names a style (grid_style_list) for the vault over each bay. Refused with a plain sentence and nothing written when the room is too low, too large or lands on cells it cannot overwrite."
    )]
    async fn grid_room(
        &self,
        Parameters(req): Parameters<RoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_room", |p, root| make::room(p, root, &req))
    }

    #[rmcp::tool(
        description = "Lay a corridor through waypoints (WORLD sectors, legs along X or Z): the editor's Corridor tool. width in sectors, headroom, leg ramp or stairs where heights change (heights gives the floor at each waypoint, null keeps the running one), style names a style: plain (default), the built-in Rib corridor, or a style or captured bay stored with the project. A bay (Rib corridor or captured) is repeated along every straight leg: its width is fixed (6 for the Rib corridor, taken when width is omitted; another width is refused), the path must be level (every named height equal), and a turn leaves a plain landing the size of the width, so give legs of at least one bay (3 sectors for the Rib corridor)."
    )]
    async fn grid_corridor(
        &self,
        Parameters(req): Parameters<CorridorReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_corridor", |p, root| make::corridor(p, root, &req))
    }

    #[rmcp::tool(
        description = "Cut a door or window in a wall: the editor's Opening tool. room, cell and side name the wall, from and to the run along it in world clicks (4 a sector; a run may cross sectors). kind door or window (sill), profile rectangular, pointed or round, top the lintel height. span picks the storey of a column of several floors. A door next to a room on the other side is cut in both."
    )]
    async fn grid_opening(
        &self,
        Parameters(req): Parameters<OpeningReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_opening", |p, _| make::opening(p, &req))
    }

    #[rmcp::tool(
        description = "Build a run of arches on piers along a line (grid units): the editor's Arcade tool. bays, span, pier, spring, rise, profile (round, pointed, segmental, flat), perch for a hook on each crown."
    )]
    async fn grid_arcade(
        &self,
        Parameters(req): Parameters<ArcadeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_arcade", |p, _| make::arcade(p, &req))
    }

    #[rmcp::tool(
        description = "Build a row of columns along a line (grid units), bays plus one of them: the editor's Colonnade tool."
    )]
    async fn grid_colonnade(
        &self,
        Parameters(req): Parameters<ArcadeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_colonnade", |p, _| make::colonnade(p, &req))
    }

    #[rmcp::tool(
        description = "Build a bridge deck between two points (grid units x, y, z): the editor's Bridge tool. width, thickness, parapet height, pier_spacing. It lives in a room of its own."
    )]
    async fn grid_bridge(
        &self,
        Parameters(req): Parameters<BridgeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_bridge", |p, _| make::bridge(p, &req))
    }

    #[rmcp::tool(
        description = "Build a building on a footprint of WORLD sectors: the editor's Building tool. storeys 1 to 4, storey_height, door_sides, window_every and window_profile, an interior stair, a roof parapet, a seed for the rhythm."
    )]
    async fn grid_building(
        &self,
        Parameters(req): Parameters<BuildingReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_building", |p, _| make::building(p, &req))
    }

    #[rmcp::tool(
        description = "Shape hills and slopes over a region of WORLD sectors from a seed: the editor's Terrain patch tool. seed (another seed is a re-roll), amplitude_clicks, octaves, wavelength, slope_x and slope_z, keep_walkable, edge_blend (0 for a cliff), and base with create to make floors where there are none."
    )]
    async fn grid_terrain(
        &self,
        Parameters(req): Parameters<TerrainReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_terrain", |p, _| make::terrain(p, &req))
    }

    #[rmcp::tool(
        description = "Damage a region, or the whole room when no rectangle is given: the editor's Ruin tool. seed, intensity 0 to 100, keep (sectors no damage touches: the path), off (toggles to turn off: crumble, sink, floor_bays, roof_bays, breaches, fallen_arches, rubble)."
    )]
    async fn grid_ruin(
        &self,
        Parameters(req): Parameters<RuinReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_ruin", |p, _| make::ruin(p, &req))
    }

    #[rmcp::tool(
        description = "List the styles a room or corridor can take: plain, the built-in Rib corridor bay and those stored with the project."
    )]
    async fn grid_style_list(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| Ok(make::style_list(workspace.root())))?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Capture the bay in a rectangle of WORLD sectors as a named style stored with the project (its styles folder), listed by grid_style_list and usable as a corridor style (repeated along the path)."
    )]
    async fn grid_capture_style(
        &self,
        Parameters(req): Parameters<CaptureReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let doc = workspace.document()?;
            make::capture_style(doc, &root, &req)
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Cut a doorway and place its sliding door in one call: the editor's Door tool. The opening fields of grid_opening, plus name, direction (up, down, east, west, south, north), distance, ticks (speed), lock (open, locked, one_way with refuse_from), refusal text and persistent (remembered across death and saves, default true)."
    )]
    async fn grid_door(
        &self,
        Parameters(req): Parameters<DoorReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_door", |p, _| make::door(p, &req))
    }

    #[rmcp::tool(
        description = "Place a lever that operates a door (or other named node): the editor's Lever tool. cell, height, operates (the door's name), behavior toggle or one_shot, unlocks, persistent."
    )]
    async fn grid_lever(
        &self,
        Parameters(req): Parameters<LeverReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_lever", |p, _| make::lever(p, &req))
    }

    #[rmcp::tool(
        description = "Read the walls on one side of a sector, whatever made them: heights, material and collision, each marked automatic or set, the storey and whether the room next door shares it."
    )]
    async fn grid_wall_get(
        &self,
        Parameters(req): Parameters<WallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let doc = workspace.document()?;
            make::wall_get(doc, &req)
        })?;
        Ok(text_result(text))
    }

    #[rmcp::tool(
        description = "Set the four corner heights of a wall (bottom near, bottom far, top far, top near; null keeps one): slide, stretch and slope are this one call, and the floors stay where they are. Reports a gap the heights leave open."
    )]
    async fn grid_wall_set(
        &self,
        Parameters(req): Parameters<WallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_wall_set", |p, _| make::wall_set(p, &req))
    }

    #[rmcp::tool(
        description = "Set the material of one wall (a name or id; `none` takes it back to automatic), on any storey, whatever made the wall."
    )]
    async fn grid_wall_material(
        &self,
        Parameters(req): Parameters<WallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_wall_material", |p, _| make::wall_material(p, &req))
    }

    #[rmcp::tool(
        description = "Cut a door or window in one wall (from and to in clicks along the wall, top and sill in grid units, profile rectangular, pointed or round), or close it with opening none. Works on any storey, whatever made the wall."
    )]
    async fn grid_wall_opening(
        &self,
        Parameters(req): Parameters<WallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_wall_opening", |p, _| make::wall_opening(p, &req))
    }

    #[rmcp::tool(
        description = "Take a wall back to automatic: heights, material, collision, bands or all (the default)."
    )]
    async fn grid_wall_reset(
        &self,
        Parameters(req): Parameters<WallReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_wall_reset", |p, _| make::wall_reset(p, &req))
    }

    #[rmcp::tool(
        description = "Move cells east and south by whole sectors and up or down by lattice steps, with their walls, meshes, planes and the nodes standing on them: the editor's Move. One call is one undo step; overwrite replaces what stands at the destination."
    )]
    async fn grid_move(
        &self,
        Parameters(req): Parameters<MoveReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_move", |p, _| make::move_cells(p, &req))
    }

    #[rmcp::tool(
        description = "Lean cells by an offset ramp that keeps their shape: the editor's Tilt. hinge names the side that stays, rise is the offset at the far side (grid units, rounded to the 64 lattice). A tilt that would leave floors too steep to stand on is refused unless force is true."
    )]
    async fn grid_tilt(
        &self,
        Parameters(req): Parameters<TiltReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.make_edit("grid_tilt", |p, _| make::tilt(p, &req))
    }
}
