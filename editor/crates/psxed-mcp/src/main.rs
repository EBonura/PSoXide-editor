//! `psxed-mcp` -- MCP server exposing PSoXide level authoring to an agent.
//!
//! Phase 1 is read-only and runs offline over stdio against a project file,
//! because the tool bodies are pure functions of a `ProjectDocument` and the
//! live editor is a 75-second rebuild. Phase 2 adds writes; the live in-GUI
//! bridge (mirroring `emu/crates/frontend/src/mcp.rs`) comes after that.
//! See `docs/editor-mcp-plan-2026-09-18.md`.
//!
//! ```bash
//! psxed-mcp --project editor/projects/default
//! psxed-mcp --new editor/projects/scratch   # empty project, then exit
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::wrapper::Parameters;
use base64::Engine as _;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

use psxed_mcp::audit::{audit, AuditDepth};
use psxed_mcp::edit::{find_material, RadialArray, Workspace};
use psxed_mcp::nodes::{entity_types, get_node};
use psxed_mcp::play;
use psxed_mcp::shot;
use psxed_mcp::{metrics, plan_view, scene_info, Focus, PlanAxis};
use psxed_project::brush_primitives::{
    BrushCardinalDirection, BrushDrawSettings, BrushDrawShape,
};

#[derive(Clone)]
struct EditorServer {
    /// Edits stage here and reach disk only on `save`.
    workspace: Arc<Mutex<Workspace>>,
    /// Renderer binary, or None to search the usual target directories.
    frontend: Option<Arc<PathBuf>>,
    /// Directory for the renderer's intermediate PPM.
    scratch: Arc<PathBuf>,
    /// Built by `#[rmcp::tool_router]`; read by the generated handler, not
    /// by this file, so the dead-code pass cannot see the use.
    #[allow(dead_code)]
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SceneReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlanReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// `top` for a floor plan, `front` or `side` for a vertical section.
    axis: Option<String>,
    /// World coordinate to cut at, on the axis the view does not plot. Omit
    /// for 512 above the scene floor, then re-aim using the legend's
    /// candidate floor levels.
    slice: Option<i32>,
    /// Image width in pixels (256..2048, default 900).
    width: Option<u32>,
    /// World point `[x, y, z]` to frame on. Without it the whole level is
    /// drawn, where the player is under a pixel. Needs `extent`.
    center: Option<[i32; 3]>,
    /// Half-span in world units around `center`. Roughly 3000 frames one
    /// room at a readable human scale.
    extent: Option<i32>,
}


/// The map's working grid; every generated coordinate snaps to it.
const GRID_STEP: i32 = 64;
/// Floor and ceiling slabs in the shipped level are 256 or 384 thick.
const SLAB_THICKNESS: i32 = 256;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddShapeReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// box, ramp, cylinder (or pillar), doorway arch, curved wall, stairs.
    shape: String,
    /// Lower corner `[x, y, z]` of the box the shape fills.
    min: [i32; 3],
    /// Upper corner `[x, y, z]`, strictly above `min` on all three axes.
    max: [i32; 3],
    /// north, east, south or west. Used by ramp, arch, curved wall, stairs.
    direction: Option<String>,
    /// Material name to put on every face. Omit and the faces cook untextured.
    material: Option<String>,
    /// Grid to snap to, default 64 (what the shipped level uses).
    grid: Option<i32>,
    /// Sides of a cylinder pillar, 3..=32, default 8. Check `metrics` for the
    /// footprint each count needs, or just read the warning it comes back with.
    sides: Option<u8>,
    /// Voussoirs per arch, or segments per curved wall. Default 6.
    segments: Option<u8>,
    /// Band thickness for an arch or curved wall. Must be under the radius.
    thickness: Option<u16>,
    /// Arc a curved wall sweeps, 90..=360 degrees.
    arc_degrees: Option<u16>,
    /// Steps in a stair run, 1..=32.
    steps: Option<u8>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct MakeRoomReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// Lower corner of the INTERIOR the player moves through.
    min: [i32; 3],
    /// Upper corner of the interior.
    max: [i32; 3],
    /// Wall thickness, grown outward from the interior. Default 256.
    thickness: Option<i32>,
    /// Material for every face.
    material: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ArrayReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// First brush index to repeat, as add_shape reported it.
    first: usize,
    /// How many brushes from `first` form one instance.
    count: usize,
    /// Extra instances to add. The original stays put.
    copies: u32,
    /// Step between linear copies, `[x, y, z]`.
    offset: Option<[i32; 3]>,
    /// Sweep about this world point instead of translating.
    center: Option<[i32; 3]>,
    /// Total sweep in degrees, split across the copies. Default 360.
    degrees: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CarveReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// First brush index to cut.
    first: usize,
    /// How many brushes from `first`.
    count: usize,
    /// Lower corner of the box-shaped void to remove.
    min: [i32; 3],
    /// Upper corner of the void.
    max: [i32; 3],
    /// Material for the newly revealed surfaces. Defaults to whatever the
    /// brush being cut already uses.
    material: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetMaterialReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// First brush index.
    first: usize,
    /// How many brushes from `first`.
    count: usize,
    /// Material name.
    material: String,
    /// Only faces pointing this way, e.g. `[0,1,0]` floors, `[0,-1,0]` ceilings.
    normal: Option<[i32; 3]>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlaytestReq {
    /// Controller polls to run for. One poll is one simulation tick. Under
    /// about 3000 the run is still in the menu and loading flow; 7000 reaches
    /// gameplay. Default 7000.
    polls: Option<u32>,
    /// Skip the disc build and run the last one. Default false; the build
    /// takes about a minute and the run a further two.
    skip_build: Option<bool>,
    /// Button schedule, `tick:button[:hold]` comma separated. Defaults to
    /// tapping cross through the title, the world message and the mid-load
    /// splash, all of which wait on it.
    press: Option<String>,
    /// Hold the left stick forward, to walk into the level rather than
    /// standing on the spawn. Default true.
    walk: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ScreenshotReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// World point to look at. Omit and give `first`/`count` instead to frame
    /// a run of brushes, or a `node` to frame an entity.
    target: Option<[i32; 3]>,
    /// First brush index to frame, with `count`.
    first: Option<usize>,
    /// How many brushes from `first`.
    count: Option<usize>,
    /// Node name or id to frame.
    node: Option<String>,
    /// How far back to stand, world units. Pulled in automatically when a
    /// wall is closer than this. Default 3000, or derived from the brushes.
    distance: Option<i32>,
    /// Orbit yaw, 4096 per turn. Omit to let the framing pick the heading
    /// with the most open space, which is what stops the shot landing in a
    /// wall.
    yaw: Option<u16>,
    /// Orbit pitch, 4096 per turn. Small values look slightly upward.
    pitch: Option<u16>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct NodeLookupReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// Node name (exact, or a unique substring), or its numeric id.
    node: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlaceNodeReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// Node to clone, by name or id. Its whole subtree comes with it.
    source: String,
    /// World position `[x, y, z]` for the clone.
    position: [i32; 3],
    /// Name for the clone. Defaults to "<source> copy".
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetNodeReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// Node to edit, by name or id.
    node: String,
    /// The replacement kind payload as RON, in the form get_node prints. It
    /// must be the same variant the node already is.
    kind: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct MoveNodeReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// Node to move, by name or id.
    node: String,
    /// New world position `[x, y, z]`.
    position: [i32; 3],
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LightReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// World position `[x, y, z]`.
    position: [i32; 3],
    /// Reach in WORLD UNITS (the tool converts to the sectors the format
    /// stores). A 1024 radius lights roughly one player-height sphere.
    radius: i32,
    /// RGB, default warm white.
    color: Option<[u8; 3]>,
    /// Brightness multiplier, 0..=8, default 1.
    intensity: Option<f32>,
    /// Node name in the scene tree.
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CookModeReq {
    /// true for Release (bakes lights), false for Draft (fullbright).
    release: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AuditReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// `quick` (geometry, milliseconds), `sealing` (adds the leak check), or
    /// `full` (adds a cook and per-leaf draw cost, seconds). Default quick.
    depth: Option<String>,
    /// Grid to check alignment against, default 64.
    grid: Option<i32>,
    /// First brush index to audit. With `count`, scopes the report to the
    /// work just done instead of the whole level's history.
    first: Option<usize>,
    /// How many brushes from `first`.
    count: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DeleteReq {
    /// Scene index. Omit for the first scene that has brushes.
    scene: Option<usize>,
    /// First brush index.
    first: usize,
    /// How many brushes from `first`.
    count: usize,
}

#[rmcp::tool_router]
impl EditorServer {
    fn new(workspace: Workspace, frontend: Option<PathBuf>) -> Self {
        Self {
            workspace: Arc::new(Mutex::new(workspace)),
            frontend: frontend.map(Arc::new),
            scratch: Arc::new(std::env::temp_dir().join("psxed-mcp")),
            tool_router: Self::tool_router(),
        }
    }

    /// Run `body` against the workspace. Reads see staged edits; with nothing
    /// staged the file is re-read, so the GUI's own saves are picked up.
    fn with<T>(
        &self,
        body: impl FnOnce(&mut Workspace) -> Result<T, String>,
    ) -> Result<T, ErrorData> {
        let mut workspace = self
            .workspace
            .lock()
            .map_err(|_| ErrorData::internal_error("workspace lock poisoned".to_string(), None))?;
        body(&mut workspace).map_err(|error| ErrorData::internal_error(error, None))
    }

    /// Append the staged-edit reminder so a session cannot drift into
    /// believing its work is on disk.
    fn staged_note(workspace: &Workspace) -> String {
        if workspace.is_dirty() {
            format!(
                "\n\n[{} edit(s) staged in memory, not yet written. Call save when the \
                 section looks right, or revert to drop them.]",
                workspace.log().len()
            )
        } else {
            String::new()
        }
    }

    #[rmcp::tool(
        description = "Scale constants, the player's dimensions, the working grid, and the space sizes measured from the shipped level. Read this before authoring geometry so sizes are looked up rather than guessed."
    )]
    async fn metrics(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let project = workspace.document()?;
            Ok(metrics(Some(project)))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Extents, brush and face counts, groups, materials in use, and the candidate floor levels of a scene."
    )]
    async fn scene_info(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(scene_info(project, scene)? + &note)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Orthographic floor plan or vertical section of the level's solid brushes, drawn to scale with a grid and the player disc for comparison. Use this, not a 3D screenshot, to reason about the size and shape of spaces."
    )]
    async fn plan_view(
        &self,
        Parameters(PlanReq {
            scene,
            axis,
            slice,
            width,
            center,
            extent,
        }): Parameters<PlanReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let axis = PlanAxis::parse(axis.as_deref().unwrap_or("top"))
            .map_err(|error| ErrorData::invalid_params(error, None))?;
        let focus = match (center, extent) {
            (Some(center), Some(extent)) => Some(Focus { center, extent }),
            (Some(center), None) => Some(Focus {
                center,
                extent: 3000,
            }),
            (None, Some(_)) => {
                return Err(ErrorData::invalid_params(
                    "extent needs center: give the world point to frame on".to_string(),
                    None,
                ))
            }
            (None, None) => None,
        };
        let view = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            let mut view = plan_view(project, scene, axis, slice, width.unwrap_or(900), focus)?;
            view.legend.push_str(&note);
            Ok(view)
        })?;
        Ok(CallToolResult::success(vec![
            ContentBlock::text(view.legend),
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(&view.png),
                "image/png".to_string(),
            ),
        ]))
    }

    #[rmcp::tool(
        description = "Build a parametric solid inside an axis-aligned box and add it to the scene. Shapes: box, ramp, cylinder (an N-sided pillar), doorway arch, curved wall, stairs. Reports the brush and face counts it actually produced, and warns when grid snapping cost the shape sides or segments. Edits stage in memory until `save`."
    )]
    async fn add_shape(
        &self,
        Parameters(AddShapeReq {
            scene,
            shape,
            min,
            max,
            direction,
            material,
            grid,
            sides,
            segments,
            thickness,
            arc_degrees,
            steps,
        }): Parameters<AddShapeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let shape = BrushDrawShape::parse(&shape).ok_or_else(|| {
            ErrorData::invalid_params(
                format!(
                    "unknown shape {shape:?}: use one of {}",
                    BrushDrawShape::ALL
                        .iter()
                        .map(|s| s.label())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None,
            )
        })?;
        let direction = match direction.as_deref() {
            Some(value) => BrushCardinalDirection::parse(value).ok_or_else(|| {
                ErrorData::invalid_params(
                    format!("unknown direction {value:?}: use north, east, south or west"),
                    None,
                )
            })?,
            None => BrushCardinalDirection::default(),
        };
        let defaults = BrushDrawSettings::default();
        let settings = BrushDrawSettings {
            shape,
            direction,
            cylinder_sides: sides.unwrap_or(defaults.cylinder_sides),
            arch_segments: segments.unwrap_or(defaults.arch_segments),
            arch_thickness: thickness.unwrap_or(defaults.arch_thickness),
            curved_wall_arc_degrees: arc_degrees.unwrap_or(defaults.curved_wall_arc_degrees),
            stair_steps: steps.unwrap_or(defaults.stair_steps),
        };
        let text = self.with(|workspace| {
            let report = workspace.add_shape(
                scene,
                min,
                max,
                settings,
                grid.unwrap_or(GRID_STEP),
                material.as_deref(),
            )?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Add a hollow box room. Dimensions are the INTERIOR the player moves through; the walls grow outward from it. Reports the interior in player heights and widths."
    )]
    async fn make_room(
        &self,
        Parameters(MakeRoomReq {
            scene,
            min,
            max,
            thickness,
            material,
        }): Parameters<MakeRoomReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.make_room(
                scene,
                min,
                max,
                thickness.unwrap_or(SLAB_THICKNESS),
                material.as_deref(),
            )?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Repeat a run of brushes, either along a line (`offset`) or swept about a vertical axis (`center` plus `degrees`). This is how one pillar becomes a colonnade and one arch becomes an arcade. Use the first/count that add_shape reported."
    )]
    async fn array(
        &self,
        Parameters(ArrayReq {
            scene,
            first,
            count,
            copies,
            offset,
            center,
            degrees,
        }): Parameters<ArrayReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let radial = match (center, degrees) {
            (Some(center), Some(degrees)) => Some(RadialArray { center, degrees }),
            (Some(center), None) => Some(RadialArray {
                center,
                degrees: 360,
            }),
            (None, Some(_)) => {
                return Err(ErrorData::invalid_params(
                    "degrees needs center: give the world point to sweep about".to_string(),
                    None,
                ))
            }
            (None, None) => None,
        };
        let text = self.with(|workspace| {
            let report = workspace.array(
                scene,
                first,
                count,
                copies,
                offset.unwrap_or([0, 0, 0]),
                radial,
            )?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Cut a box-shaped void out of a run of brushes, replacing each with the convex remainder. This is how a doorway or window goes through a wall, and how a make_room shell stops being sealed. Brush indices shift, so re-read scene_info afterwards."
    )]
    async fn carve(
        &self,
        Parameters(CarveReq {
            scene,
            first,
            count,
            min,
            max,
            material,
        }): Parameters<CarveReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report =
                workspace.carve(scene, first, count, min, max, material.as_deref())?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Assign a material to a run of brushes. Pass `normal` to hit only faces pointing that way, e.g. [0,1,0] for floors or [0,-1,0] for ceilings."
    )]
    async fn set_material(
        &self,
        Parameters(SetMaterialReq {
            scene,
            first,
            count,
            material,
            normal,
        }): Parameters<SetMaterialReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.set_material(scene, first, count, &material, normal)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Delete a run of brushes. Indices above the range shift down, and the result says by how much."
    )]
    async fn delete(
        &self,
        Parameters(DeleteReq {
            scene,
            first,
            count,
        }): Parameters<DeleteReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.delete(scene, first, count)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Cook the project to a real PS1 disc, boot it in the emulator, and return the final frame. This is the only check that the level actually RUNS rather than merely cooking. Slow: about a minute to build and two to run. Read port1-polls in the result first; a run far short of the request stalled on a screen waiting for input."
    )]
    async fn playtest(
        &self,
        Parameters(PlaytestReq {
            polls,
            skip_build,
            press,
            walk,
        }): Parameters<PlaytestReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let frontend = shot::find_frontend(self.frontend.as_deref().map(PathBuf::as_path))
            .map_err(|error| ErrorData::internal_error(error, None))?;
        let project_dir = self.with(|workspace| {
            if workspace.is_dirty() {
                return Err(
                    "there are unsaved staged edits; the disc is built from project.ron on                      disk, so call save first or you will playtest the old level"
                        .to_string(),
                );
            }
            Ok(workspace.root().to_path_buf())
        })?;
        let polls = polls.unwrap_or(7000).clamp(300, 60_000);
        let schedule = press.unwrap_or_else(|| play::menu_press_schedule(polls.min(5200)));

        let cue = if skip_build.unwrap_or(false) {
            play::last_cue(&project_dir).map_err(|error| ErrorData::internal_error(error, None))?
        } else {
            play::build_disc(&frontend, &project_dir)
                .map_err(|error| ErrorData::internal_error(error, None))?
        };
        let dump = self.scratch.join("playtest.ppm");
        let report = play::run_disc(
            &frontend,
            &cue,
            &dump,
            polls,
            &schedule,
            walk.unwrap_or(true),
        )
        .map_err(|error| ErrorData::internal_error(error, None))?;
        let png = std::fs::read(&dump)
            .map_err(|error| ErrorData::internal_error(format!("read the dumped frame: {error}"), None))
            .and_then(|raw| {
                shot::png_from_ppm(&raw).map_err(|error| ErrorData::internal_error(error, None))
            })?;
        let _ = std::fs::remove_file(&dump);
        Ok(CallToolResult::success(vec![
            ContentBlock::text(format!(
                "{}\ndisc: {}",
                report.summary(polls),
                cue.display()
            )),
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(&png),
                "image/png".to_string(),
            ),
        ]))
    }

    #[rmcp::tool(
        description = "Render the editor's 3D preview, framed automatically so the camera never ends up inside a wall. Give a target point, a brush range (first/count), or a node name. Use this to check materials, lighting and mood; use plan_view to judge sizes and layout."
    )]
    async fn screenshot(
        &self,
        Parameters(ScreenshotReq {
            scene,
            target,
            first,
            count,
            node,
            distance,
            yaw,
            pitch,
        }): Parameters<ScreenshotReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let frontend = shot::find_frontend(self.frontend.as_deref().map(PathBuf::as_path))
            .map_err(|error| ErrorData::internal_error(error, None))?;
        let (solved, project_dir) = self.with(|workspace| {
            if workspace.is_dirty() {
                return Err(
                    "there are unsaved staged edits; the renderer reads project.ron from disk,                      so call save first or the shot will show the old geometry"
                        .to_string(),
                );
            }
            let dir = workspace.root().to_path_buf();
            let project = workspace.document()?;
            let (target, derived) = match (target, first, node.as_deref()) {
                (Some(point), _, _) => (point, distance.unwrap_or(3000)),
                (None, Some(first), _) => {
                    let (center, span) =
                        shot::brush_range_target(project, scene, first, count.unwrap_or(1))?;
                    (center, distance.unwrap_or(span))
                }
                (None, None, Some(needle)) => {
                    let index = psxed_mcp::resolve_scene(project, scene)?;
                    let scene_doc = &project.scenes[index];
                    let id = psxed_mcp::nodes::find_node(scene_doc, needle)?;
                    let position = scene_doc
                        .node(id)
                        .ok_or_else(|| format!("node {needle:?} vanished"))?
                        .transform
                        .translation;
                    (
                        position.map(|value| value.round() as i32),
                        distance.unwrap_or(2400),
                    )
                }
                (None, None, None) => {
                    return Err(
                        "give a target point, a brush range (first/count), or a node".to_string()
                    )
                }
            };
            Ok((
                shot::frame(project, scene, target, derived, yaw, pitch)?,
                dir,
            ))
        })?;
        let png = shot::render(&frontend, &project_dir, solved, self.scratch.as_path())
            .map_err(|error| ErrorData::internal_error(error, None))?;
        let clearance = if solved.clearance.is_finite() {
            format!("{:.0} units", solved.clearance)
        } else {
            "open".to_string()
        };
        Ok(CallToolResult::success(vec![
            ContentBlock::text(format!(
                "target {:?}, yaw {} pitch {} radius {} (nearest surface behind the eye: {clearance})",
                solved.target, solved.yaw_q12, solved.pitch_q12, solved.radius
            )),
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(&png),
                "image/png".to_string(),
            ),
        ]))
    }

    #[rmcp::tool(
        description = "List the node kinds present in the scene with counts and example names. Start here for entities: enemies, spawn points, cameras, triggers, points of interest and lights are all scene nodes."
    )]
    async fn entity_types(
        &self,
        Parameters(SceneReq { scene }): Parameters<SceneReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(entity_types(project, scene)? + &note)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Full detail for one node: kind, world position, rotation, parent, children, and its kind payload as RON. The RON is what set_node takes back."
    )]
    async fn get_node(
        &self,
        Parameters(NodeLookupReq { scene, node }): Parameters<NodeLookupReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let project = workspace.document()?;
            Ok(get_node(project, scene, &node)? + &note)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Clone an existing node, with its whole subtree, to a world position. This is how you place an enemy or any other kind: an enemy is a host Entity plus Model Renderer, Animator and Character Controller children, so copying a working one beats building it from parts."
    )]
    async fn place_node(
        &self,
        Parameters(PlaceNodeReq {
            scene,
            source,
            position,
            name,
        }): Parameters<PlaceNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.place_node(scene, &source, position, name.as_deref())?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Replace a node's kind payload from RON, in the form get_node prints. Covers every field of every node kind; it must stay the same variant."
    )]
    async fn set_node(
        &self,
        Parameters(SetNodeReq { scene, node, kind }): Parameters<SetNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.set_node(scene, &node, &kind)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(description = "Move a node to a world position.")]
    async fn move_node(
        &self,
        Parameters(MoveNodeReq {
            scene,
            node,
            position,
        }): Parameters<MoveNodeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.move_node(scene, &node, position)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(description = "Remove a node and its subtree.")]
    async fn delete_node(
        &self,
        Parameters(NodeLookupReq { scene, node }): Parameters<NodeLookupReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.delete_node(scene, &node)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Place a static point light. Radius is given in world units here and converted to the sectors the format stores. IMPORTANT: point lights do nothing while bsp_cook_mode is Draft, which is the default, because Draft packs every surface fullbright and skips the bake entirely; the tool says so when that is the case."
    )]
    async fn add_light(
        &self,
        Parameters(LightReq {
            scene,
            position,
            radius,
            color,
            intensity,
            name,
        }): Parameters<LightReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.add_light(
                scene,
                position,
                radius,
                color.unwrap_or([255, 236, 208]),
                intensity.unwrap_or(1.0),
                name.as_deref(),
            )?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "List the scene's point lights with their world positions and radii in world units."
    )]
    async fn lights(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let lights = workspace.lights()?;
            if lights.is_empty() {
                return Ok("no point lights in the scene".to_string());
            }
            let mut out = format!("{} point light(s):\n", lights.len());
            for (name, position, radius) in &lights {
                out.push_str(&format!(
                    "- {name:?} at [{:.0}, {:.0}, {:.0}], radius {radius:.0} units\n",
                    position[0], position[1], position[2]
                ));
            }
            Ok(out)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Switch the BSP cook between Draft (fullbright, ignores lights) and Release (bakes point lights over a dark ambient). Lighting work is invisible until this is Release."
    )]
    async fn set_cook_mode(
        &self,
        Parameters(CookModeReq { release }): Parameters<CookModeReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.set_cook_mode(release)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Check the scene for the defects a geometry tool reports as success: coplanar faces that will z-fight, degenerate brushes, coordinates off the working grid, untextured faces, a leak to the void, and the per-leaf PS1 draw cost. Run `quick` after every structural change and `full` before calling a section done."
    )]
    async fn audit(
        &self,
        Parameters(AuditReq {
            scene,
            depth,
            grid,
            first,
            count,
        }): Parameters<AuditReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let depth = AuditDepth::parse(depth.as_deref().unwrap_or("quick"))
            .map_err(|error| ErrorData::invalid_params(error, None))?;
        let text = self.with(|workspace| {
            let note = Self::staged_note(workspace);
            let root = workspace.root().to_path_buf();
            let project = workspace.document()?;
            let range = match (first, count) {
                (Some(first), Some(count)) => Some((first, count)),
                (Some(first), None) => Some((first, usize::MAX)),
                _ => None,
            };
            Ok(audit(project, &root, scene, depth, grid.unwrap_or(GRID_STEP), range)? + &note)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "List the project's materials, so a name can be passed to add_shape or set_material."
    )]
    async fn materials(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let project = workspace.document()?;
            // find_material's error path already lists every candidate, which
            // is exactly this listing.
            Ok(match find_material(project, "\u{0}") {
                Ok(_) => "no materials".to_string(),
                Err(listing) => listing.replace("no material matches \"\\0\". ", ""),
            })
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Write the staged edits to project.ron. Refuses if the file changed on disk since the edits were staged, which means the editor saved over it."
    )]
    async fn save(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::save)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(description = "Throw away every staged edit and re-read project.ron from disk.")]
    async fn revert(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::revert)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[rmcp::tool_handler(
    name = "psoxide-editor",
    version = "0.1.0",
    instructions = "PSoXide level authoring, in authored editor units. Work in a loop: READ, ACT, VERIFY.\n\nREAD first. `metrics` carries the unit scale, the player's size, the 64-unit working grid, the ceiling heights that actually shipped, and the footprint each pillar side-count needs; call it before inventing any dimension. `scene_info` gives the scene's extents and its candidate floor levels. `materials` lists what you can texture with.\n\nACT with `add_shape`, `make_room`, `array`, `set_material` and `delete`. Every shape reports the brushes and faces it really produced, and warns when grid snapping cost it sides or segments, so read the result instead of assuming. Rooms are authored by their INTERIOR. Give every face a material: untextured faces still cook, they just look wrong. Note the first/count each call returns; the later tools take them.\n\nVERIFY two ways, and do both. `audit` scoped to the brushes you just added (pass first and count) catches coplanar faces that will z-fight, off-grid coordinates and untextured faces in milliseconds; run it after every structural change. `plan_view` is how you judge the SPACE: section a floor plan at a candidate floor level plus 512, pass center and extent to frame one room at human scale, and check a front or side section before trusting a height. Do not reach for a 3D render, the level is dark night-time art at fullbright and volumes read as black masses in perspective.\n\nBefore calling a section done, run `audit` at depth full. It cooks the map and reports the per-leaf PS1 draw cost. Watch the worst leaf's packet slots: if that number climbed after your edit, the edit opened a sightline, and the fix is to break the sightline with geometry rather than to delete detail. An n-sided pillar is n+2 faces and an arch is segments+2 brushes, so a long arcade down an open hall is exactly what makes a level unshippable.\n\nEdits stage in memory. Nothing reaches project.ron until `save`, which refuses if the editor saved over the file meanwhile."
)]
impl ServerHandler for EditorServer {}

/// Write an empty project: the starter's resources and settings, no brushes.
///
/// Authoring experiments belong in a project with nothing in it. Building
/// into a copy of the shipped level means its 141 existing coplanar overlaps
/// and its 2390 cooked faces swamp any measurement of the new work.
fn new_project(dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let file = dir.join("project.ron");
    if file.exists() {
        return Err(format!("{} already exists; refusing to overwrite", file.display()).into());
    }
    let mut doc = psxed_project::ProjectDocument::default();
    let mut cleared = 0usize;
    let mut moved = 0usize;
    for scene in doc.scenes.iter_mut() {
        cleared += scene.brushes.len();
        scene.brushes.clear();
        // Clearing the brushes leaves every entity at the shipped level's
        // coordinates, thousands of units from wherever the new geometry will
        // go. That matters more than it sounds: the leak diagnostic floods
        // from the player, so a spawn outside the new map reports the map as
        // leaking no matter how well sealed it is.
        let ids: Vec<_> = scene
            .nodes()
            .iter()
            .filter(|node| node.parent.is_some())
            .map(|node| node.id)
            .collect();
        for id in ids {
            if let Some(node) = scene.node_mut(id) {
                if node.transform.translation != [0.0; 3] {
                    node.transform.translation = [0.0; 3];
                    moved += 1;
                }
            }
        }
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(&file, doc.to_ron_string()?)?;
    println!(
        "wrote {} with {cleared} starter brushes removed, {moved} node(s) moved to the origin, \
         {} resources kept",
        file.display(),
        doc.resources.len()
    );
    println!("link its assets, e.g.: ln -s ../default/assets {}/assets", dir.display());
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = PathBuf::from("editor/projects/default");
    let mut new_at: Option<PathBuf> = None;
    let mut frontend: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                project = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--project needs a path")?;
            }
            "--new" => {
                new_at = Some(args.next().map(PathBuf::from).ok_or("--new needs a path")?);
            }
            "--frontend" => {
                frontend = Some(
                    args.next()
                        .map(PathBuf::from)
                        .ok_or("--frontend needs a path")?,
                );
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    if let Some(dir) = new_at {
        return new_project(&dir);
    }
    // Fail loudly at startup rather than on every tool call.
    let workspace = Workspace::open(&project)?;
    let service = EditorServer::new(workspace, frontend)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
