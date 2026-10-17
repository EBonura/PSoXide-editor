//! Natural terrain: cracks, voids, wall heights, outlines, break and rough.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData;

use psxed_mcp::grid::natural::{self, ChamferReq, CrackReq, PolygonReq, SetWallReq, VoidReq};
use psxed_mcp::grid::rock::{self, BreakReq, PaintRulesReq, RoughReq, BREAK_CHECKPOINT};
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

    #[rmcp::tool(
        description = "Break a region: add natural detail to walls, floors and ceilings in one call, like the Cathedral's hand-broken tiles and 45 degree plateau edges. targets (walls, floor, ceiling; default walls and floor): walls crumbles the lip of a plateau, bites its edge (the half of a cell on the low side drops, a diagonal wall appears) and ragges the top of open-topped rock walls; floor tilts interior vertices and cracks tiles along a diagonal; ceiling does the same to the roof. intensity 0 to 100 (default 40) sets the chance and size of every change, seed makes it repeatable, amplitude is the largest change at intensity 100 in fine steps (default 4, one click). keep_walkable (default true) checks every change against the walkability analysis with the motor's own numbers: nothing that adds a floor too steep to stand on, a step the character cannot climb or too little headroom survives, and what the mask drops is listed. A checkpoint is taken first; reroll true restores it and breaks again with the next seed, dry_run true only reports. The region is from/to, brush, path or polygon. Writes ordinary corner heights, split-level cells, void triangles and open wall tops, so it is hand-editable after."
    )]
    async fn grid_break(
        &self,
        Parameters(req): Parameters<BreakReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let reroll = req.reroll.unwrap_or(false);
            let dry = req.dry_run.unwrap_or(false);
            // The plan reads the staged document; a reroll reads the one the
            // checkpoint holds, so restore it first.
            if reroll && !dry {
                workspace.restore(Some(BREAK_CHECKPOINT)).map_err(|_| {
                    "nothing to re-roll: no grid_break has been applied this session".to_string()
                })?;
            }
            let (plan, seed, note) = {
                let project = workspace.document()?;
                let index = psxed_mcp::resolve_scene(project, req.scene)?;
                let world = project.scenes[index]
                    .world
                    .grid()
                    .ok_or_else(|| format!("scene {index} has no grid world"))?;
                let plan = rock::plan_break(world, project, &req)?;
                let last = workspace.last_break_seed();
                let (seed, note) = match (req.seed, reroll, last) {
                    (Some(seed), _, _) => (seed, None),
                    (None, true, Some(last)) => (
                        last.wrapping_add(1),
                        Some(format!("re-rolled with seed {}", last.wrapping_add(1))),
                    ),
                    (None, _, last) => {
                        let seed = last.map_or(1, |s| s.wrapping_add(1));
                        (seed, None)
                    }
                };
                (plan, seed, note)
            };
            if dry {
                let project = workspace.document()?.clone();
                let index = psxed_mcp::resolve_scene(&project, req.scene)?;
                let mut world = project.scenes[index]
                    .world
                    .grid()
                    .cloned()
                    .ok_or("no grid world")?;
                let text = rock::run_break(&mut world, &plan, seed, Some("dry run: nothing was changed"))?;
                return Ok(text);
            }
            if !reroll {
                workspace.checkpoint(Some(BREAK_CHECKPOINT))?;
            }
            let text = workspace.grid_edit(req.scene, "grid_break", |world, _| {
                rock::run_break(world, &plan, seed, note.as_deref())
            })?;
            workspace.set_last_break_seed(Some(seed));
            Ok(format!(
                "{text}\n(checkpoint {BREAK_CHECKPOINT:?} holds the document before this break: restore it, or call grid_break with reroll true)"
            ) + &Self::staged_note(workspace))
        })?;
        Ok(super::text_result(text))
    }

    #[rmcp::tool(
        description = "Make rock rough: render-only jitter that turns straight walls into rock. level 0 to 3 over a region (from/to, brush, path or polygon) displaces, by up to the room's amplitude for that level, the plan position of every face corner that meets at a lattice point touching those sectors, the same way in the preview and in Play and watertight (heights are never moved, a vertical edge shifts rigidly). Collision is not moved, so amplitudes above a third of the character's radius (4 runtime units, the default top level) are a validation warning. seed changes the pattern, amp sets the amplitudes at levels 1, 2, 3 in grid units, tint adds brightness noise (percent) to rough faces. Omit the region to change only the room's settings."
    )]
    async fn grid_rough(
        &self,
        Parameters(req): Parameters<RoughReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_rough", |world, _| rock::rough(world, &req))
    }

    #[rmcp::tool(
        description = "Set a room's smart paint rules, the ones the verbs (raise, ramp, stairs, break) apply to floors not painted by hand: floor_by_class [flat, slope, steep], floor_by_height (up to three thresholds in grid units and a material per band: scree low, rock high), wall (the derived wall material of shaped sectors), wall_by_height (strata: the verbs make it the room's default band set with an absolute cut line per threshold) and trim (the foot of every wall). Give a region (from/to, brush, path or polygon) to apply the floor rules to it now."
    )]
    async fn grid_paint_rules(
        &self,
        Parameters(req): Parameters<PaintRulesReq>,
    ) -> Result<CallToolResult, ErrorData> {
        self.grid_edit(req.scene, "grid_paint_rules", |world, project| {
            rock::paint_rules(world, &Materials::new(project), &req)
        })
    }
}
