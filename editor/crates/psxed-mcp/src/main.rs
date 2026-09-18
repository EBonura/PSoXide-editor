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
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::wrapper::Parameters;
use base64::Engine as _;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

use psxed_mcp::edit::{find_material, RadialArray, Workspace};
use psxed_mcp::{metrics, plan_view, scene_info, Focus, PlanAxis};
use psxed_project::brush_primitives::{
    BrushCardinalDirection, BrushDrawSettings, BrushDrawShape,
};

#[derive(Clone)]
struct EditorServer {
    /// Edits stage here and reach disk only on `save`.
    workspace: Arc<Mutex<Workspace>>,
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
    fn new(workspace: Workspace) -> Self {
        Self {
            workspace: Arc::new(Mutex::new(workspace)),
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
    instructions = "PSoXide level authoring, in authored editor units. Work in a loop: READ, ACT, VERIFY. \n\nREAD first. `metrics` carries the unit scale, the player's size, the 64-unit working grid, the ceiling heights that actually shipped, and the footprint each pillar side-count needs; call it before inventing any dimension. `scene_info` gives the scene's extents and its candidate floor levels. `materials` lists what you can texture with.\n\nACT with `add_shape`, `make_room`, `array`, `set_material` and `delete`. Every shape reports the brushes and faces it really produced, and warns when grid snapping cost it sides or segments, so read the result instead of assuming. Rooms are authored by their INTERIOR. Give every face a material: untextured faces cook, they just look wrong.\n\nVERIFY with `plan_view`, not a 3D render: the level is dark night-time art rendered at fullbright, so volumes read as black masses in perspective. Section a floor plan at a candidate floor level plus 512, pass `center` and `extent` to frame one space at human scale, and check a `front` or `side` section before trusting a height.\n\nEdits stage in memory. Nothing reaches project.ron until `save`, which refuses if the editor saved over the file meanwhile. Watch draw cost as you go: an n-sided pillar is n+2 faces and an arch is segments+2 brushes, so a long arcade in one sightline is what makes a level unshippable on PS1."
)]
impl ServerHandler for EditorServer {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = PathBuf::from("editor/projects/default");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                project = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or("--project needs a path")?;
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    // Fail loudly at startup rather than on every tool call.
    let workspace = Workspace::open(&project)?;
    let service = EditorServer::new(workspace)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}
