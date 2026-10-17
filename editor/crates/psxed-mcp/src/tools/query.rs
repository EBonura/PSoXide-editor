//! Read-only grid queries.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::grid::plan::{self, PlanReq};
use psxed_mcp::grid::read::{self, MapShow};
use psxed_mcp::grid::{self, Cells, Materials};

use super::project::SceneReq;
use super::{png_block, EditorServer};

#[derive(Debug, Deserialize, JsonSchema)]
struct SectorReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    room: Option<usize>,
    /// WORLD sector `[x, z]`.
    cell: [i32; 2],
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HeightsReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Only this room. Omit to map every room (the highest floor shows where
    /// rooms overlap).
    room: Option<usize>,
    /// One corner of the rectangle of WORLD sectors to map. Omit for the
    /// whole world (at most 64 sectors wide).
    from: Option<[i32; 2]>,
    /// The opposite corner, inclusive.
    to: Option<[i32; 2]>,
    /// `floor` (default), `ceiling` or `both`.
    show: Option<MapShow>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HeightAtReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// The point `[x, z]`. EDITOR units unless `grid_units` is true.
    point: [i32; 2],
    /// Read the point as grid units (1024 per sector) instead of editor
    /// units.
    grid_units: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WallsReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    room: Option<usize>,
    /// One corner of the rectangle of WORLD sectors. Omit for the whole room.
    from: Option<[i32; 2]>,
    /// The opposite corner, inclusive.
    to: Option<[i32; 2]>,
    /// Only walls on this edge: north, east, south or west.
    edge: Option<String>,
}

#[rmcp::tool_router(router = query_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "The grid world's rooms: index, name, origin, size, vertical offset (y_offset, 256 per click), ambient, wall mode, default wall material, band sets, planes, and each room's extent in editor units. The first call when building: room indices are what every geometry tool takes."
    )]
    async fn grid_rooms(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(scene, |world, project| {
            Ok(read::rooms(world, &Materials::new(project)))
        })
    }

    #[rmcp::tool(
        description = "Everything in one sector: floor and ceiling corner heights, split, materials and tiling, authored walls, and the derived walls (risers, drops, closures) with owner, material, flags and effective tiling. Use the host cell and edge it prints to address a wall in other tools."
    )]
    async fn grid_sector(
        &self,
        Parameters(req): Parameters<SectorReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            let room = grid::pick_room(world, req.room)?;
            read::sector(world, project, room, req.cell)
        })
    }

    #[rmcp::tool(
        description = "ASCII height map of a rectangle of sectors: world floor (or ceiling) heights in clicks of 256 grid units, north (-Z) at the top, '/' for sloped cells, '.' for none. The fast way to READ geometry back after editing, and to see which cells are at what height."
    )]
    async fn grid_heights(
        &self,
        Parameters(req): Parameters<HeightsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| {
            let cells = req.from.map(|from| Cells { from, to: req.to });
            read::height_map(
                world,
                req.room,
                cells.as_ref(),
                req.show.unwrap_or_default(),
            )
        })
    }

    #[rmcp::tool(
        description = "Floor height at a point, per room and for the world: the interpolated surface height in editor and grid units (the same function the runtime stands on), plus each room's floor and ceiling corners there. Use it to place nodes on a floor."
    )]
    async fn grid_height_at(
        &self,
        Parameters(req): Parameters<HeightAtReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, _| {
            read::height_at(world, req.point, req.grid_units.unwrap_or(false))
        })
    }

    #[rmcp::tool(
        description = "The derived walls (risers, drops, closures, with heights, owner, run length, material, hidden/solid flags) and authored walls of a room or a rectangle of it. Derived walls are what a height difference makes; address one by its host cell, edge and kind in grid_wall_override, grid_paint_wall, grid_tiling and the band tools."
    )]
    async fn grid_walls(
        &self,
        Parameters(req): Parameters<WallsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(req.scene, |world, project| {
            let room = grid::pick_room(world, req.room)?;
            let cells = req.from.map(|from| Cells { from, to: req.to }.rect());
            let edge = req.edge.as_deref().map(grid::parse_cardinal).transpose()?;
            read::walls(world, project, room, cells, edge)
        })
    }

    #[rmcp::tool(
        description = "The budget readout (rooms, sectors, floors, ceilings, walls, planes, drawable faces against the 16383 cap) and every limits::validate finding with its room and sector. Run it after a batch of edits."
    )]
    async fn grid_budget(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_read(scene, |world, _| Ok(read::budget(world)))
    }

    #[rmcp::tool(
        description = "Cook the (staged) project in memory exactly as Play does and report the cook's own errors and warnings, one per line, plus its exact size summary. Errors here are what would stop a playtest build. Fast: no disc is built."
    )]
    async fn cook_check(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(read::cook_check(project, &root) + &note)
        })?;
        Ok(super::text_result(text))
    }

    #[rmcp::tool(
        description = "A top-down plan of the grid world as a PNG: floor heights as colours (blue low, orange high), derived and authored walls as coloured edge lines, numbered room outlines, plane footprints, and numbered node markers (player spawn, lights with their reach, entities), with sector coordinates on the axes. The picture for LAYOUT; screenshot is the picture for mood."
    )]
    async fn grid_plan(
        &self,
        Parameters(req): Parameters<PlanReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let (png, text) = self.with(|workspace| {
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, req.scene)?;
            plan::render(project, index, &req)
        })?;
        Ok(CallToolResult::success(vec![
            ContentBlock::text(text),
            png_block(&png),
        ]))
    }
}
