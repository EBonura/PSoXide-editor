//! The level verbs (Raise and Lower, Ramp, Stairs), the walkable flag and the
//! two overlays (walkability, performance heatmap).

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::verbs::{
    self, HeatmapReq, RaiseReq, RampReq, SetWalkableReq, SpiralReq, StairsReq, WalkabilityReq,
};

use super::EditorServer;

#[rmcp::tool_router(router = verbs_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Raise or lower the floors of a rectangle of WORLD sectors by an amount (grid units, 256 a click, negative lowers): the editor's Raise tool. mode offset keeps each sector's shape, level sets every corner to a reference height plus the amount (the default on a flat region). Corners on the border move with the region and the sectors outside keep theirs, so a cliff appears and in a derived room its walls follow. Ceilings follow by default (follow_ceiling false holds a floor one click under its ceiling). Surfaces made by this tool or grid_add_surface choose their own diagonal from their heights; smart paint rules and wall_material apply to the shaped sectors."
    )]
    async fn grid_raise(
        &self,
        Parameters(req): Parameters<RaiseReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_raise", |world, project| {
            verbs::raise(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "Slope a rectangle of floors from one side to the opposite side: the editor's Ramp tool. keep names the side that keeps its height (north, east, south, west); the far side rises by rise grid units (negative falls) and every corner between gets its place along the slope, rounded to 64. The reply gives the slope per sector in runtime units, the angle, whether the character can stand on it and the steepest rise it can (the motor's own limit). walkable true with no rise takes that steepest slope, and clamps a rise to it."
    )]
    async fn grid_ramp(
        &self,
        Parameters(req): Parameters<RampReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_ramp", |world, project| {
            verbs::ramp(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "Lay a flight of stairs along a path of waypoints (WORLD sectors): the editor's Stairs tool. Each path sector is one whole-sector tread, a turn is a landing, width widens the flight sideways. The first tread stands one riser above the floor where the path starts. riser is click (default), max (the tallest riser the character steps up) or a number of grid units held to that limit; the step count follows from rise and riser, and the rest of the path is a flat landing at the top. A flight that would need a riser over the limit is refused with nothing written: use ramp true, a longer path or a smaller rise. Floors are made where the path crosses holes; the room never grows."
    )]
    async fn grid_stairs(
        &self,
        Parameters(req): Parameters<StairsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_stairs", |world, project| {
            verbs::stairs(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "Wind stairs round a ring of sectors, a storey per half or full turn: the editor's Spiral tool. ring is waypoints (WORLD sectors) in walking order, joined by a four-connected walk, the last back to the first (at least four distinct sectors). A straight run of the ring is treads (one riser up each), a turn is a landing (flat). The walk goes round the ring as often as storeys x rise_per_storey needs, then through the landings after the last tread. Each tread is a slab laid with the storey primitive, so a sector the stairs pass twice holds two treads a storey apart, and a tread more than three clicks up leaves a walkable undercroft below it. The climb starts from the floor of the ring's first sector (y or storey picks one where it has several). Build the storeys' own floors with grid_insert_slab and the stairwell with grid_cut_hole. Cells where a tread does not fit (too little headroom, no column, span cap) are skipped and counted."
    )]
    async fn grid_spiral_stairs(
        &self,
        Parameters(req): Parameters<SpiralReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_spiral_stairs", |world, project| {
            verbs::spiral_stairs(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "Make floors (or ceilings) a ghost or solid again: walkable false is a floor the character falls through or a ceiling that does not stop the head, drawn as before. Over a rectangle of WORLD sectors."
    )]
    async fn grid_set_walkable(
        &self,
        Parameters(req): Parameters<SetWalkableReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_set_walkable", |world, _| {
            verbs::set_walkable(world, &req)
        })
    }

    #[rmcp::tool(
        description = "The editor's walkability overlay as text, computed with the character motor's own numbers (step up, body height, steepest floor): sectors too steep to stand on, with headroom under the body, ghost floors and walkable floors no player spawn can reach, plus steps taller than the body climbs and drops to nothing, a map per room and the flagged edges."
    )]
    async fn grid_walkability(
        &self,
        Parameters(req): Parameters<WalkabilityReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            verbs::walkability(world, project, &req)
        })
    }

    #[rmcp::tool(
        description = "The editor's performance heatmap as text: a MODEL of the world render cost at every spot a character can stand (visible quads times 741 cycles plus near-plane quads times 9400, worst of several yaws, constants fitted to the renderer's own counters on the Cathedral route) against the 1,128,960 cycle frame minus the cycles the player and sky take, from the cooked render tables. Reports percentiles, the worst spot, a map in tenths of the budget and the hottest spots. It is a model: playtest with telemetry is the measurement."
    )]
    async fn grid_heatmap(
        &self,
        Parameters(req): Parameters<HeatmapReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| verbs::heat(world, &req))
    }
}
