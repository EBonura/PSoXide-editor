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

use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, ServerHandler, ServiceExt};

use psxed_mcp::edit::Workspace;
use psxed_mcp::inspect::materials as material_table;
use psxed_mcp::nodes::{entity_types, get_node};
use psxed_mcp::play;
use psxed_mcp::shot;
use psxed_mcp::{metrics, scene_info};

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
struct CombatDuelReq {
    /// Repeatable seed (1..255; 128 is reserved for the physical default seed 1).
    seed: Option<u8>,
    /// Total input polls, including loading. Default 11400 covers the 180-second duel limit.
    polls: Option<u32>,
    /// Reuse the built normal Play disc. Default false rebuilds from saved project data.
    skip_build: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CombatTimelineReq {
    /// Animation Set resource id (Graybox Reach player: 61).
    animation_set: u64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CombatWindowsReq {
    /// Animation Set resource id.
    animation_set: u64,
    /// Complete RON list replacing this set's windows. Example: [(action: HitReact, kind: DodgeBuffer, start: 35, end: 80), (action: HitReact, kind: Dodge, start: 60, end: 81)]. Kinds: AttackBuffer, Attack, DodgeBuffer, Dodge, Movement, Invulnerable, Armored. Source frames, end exclusive; [] removes overrides. Staged until save.
    windows_ron: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SceneReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
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
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// World point to look at. Omit and give a `node` to frame an entity, or
    /// neither to frame the middle of the world.
    target: Option<[i32; 3]>,
    /// Node name or id to frame.
    node: Option<String>,
    /// How far back to stand, world units. Default 3000.
    distance: Option<i32>,
    /// Orbit yaw, 4096 per turn. Default 0.
    yaw: Option<u16>,
    /// Orbit pitch, 4096 per turn. Pitches near a full turn look down from
    /// above the target, small values look slightly upward. Default 3776.
    pitch: Option<u16>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct NodeLookupReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node name (exact, or a unique substring), or its numeric id.
    node: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlaceNodeReq {
    /// Scene index. Omit for the first scene with a grid world.
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
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node to edit, by name or id.
    node: String,
    /// The replacement kind payload as RON, in the form get_node prints. It
    /// must be the same variant the node already is.
    kind: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct MoveNodeReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// Node to move, by name or id.
    node: String,
    /// New world position `[x, y, z]`.
    position: [i32; 3],
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LightReq {
    /// Scene index. Omit for the first scene with a grid world.
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
        description = "Grid units, the world scale, the player's dimensions and the traversal limits. Read this before placing anything so sizes are looked up rather than guessed."
    )]
    async fn metrics(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let project = workspace.document()?;
            Ok(metrics(Some(project)))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "The grid world of a scene: rooms, sectors, faces against the face budget, materials in use, and the scene's node counts."
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
            .map_err(|error| {
                ErrorData::internal_error(format!("read the dumped frame: {error}"), None)
            })
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
        description = "Render the editor's 3D preview from an orbit camera aimed at a target point, a node, or the middle of the world. Use this to check materials, lighting and mood."
    )]
    async fn screenshot(
        &self,
        Parameters(ScreenshotReq {
            scene,
            target,
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
                    "there are unsaved staged edits; the renderer reads project.ron from disk, \
                     so call save first or the shot will show the old world"
                        .to_string(),
                );
            }
            let dir = workspace.root().to_path_buf();
            let project = workspace.document()?;
            let target = match (target, node.as_deref()) {
                (Some(point), _) => point,
                (None, Some(needle)) => {
                    let index = psxed_mcp::resolve_scene(project, scene)?;
                    let scene_doc = &project.scenes[index];
                    let id = psxed_mcp::nodes::find_node(scene_doc, needle)?;
                    scene_doc
                        .node(id)
                        .ok_or_else(|| format!("node {needle:?} vanished"))?
                        .transform
                        .translation
                        .map(|value| value.round() as i32)
                }
                (None, None) => shot::world_center(project, scene)?,
            };
            Ok((
                shot::frame(target, distance.unwrap_or(3000), yaw, pitch),
                dir,
            ))
        })?;
        let png = shot::render(&frontend, &project_dir, solved, self.scratch.as_path())
            .map_err(|error| ErrorData::internal_error(error, None))?;
        Ok(CallToolResult::success(vec![
            ContentBlock::text(format!(
                "target {:?}, yaw {} pitch {} radius {}",
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
        description = "Place a static point light. Radius is given in world units here and converted to the sectors the format stores."
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
        description = "Read the prototype combat rules: shared Energy gains/costs, floating duration/rearm, poise openings, damage multipliers and controls. Rates are 60 Hz simulation values, not measured frame rate. Use combat_duel for ground coverage; arches require a separate aerial replay."
    )]
    async fn combat_flow(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            psxed_mcp::combat::flow_rules().to_string(),
        )]))
    }

    #[rmcp::tool(
        description = "Run a seeded AI-controlled player against the flagged training enemy using both Horizon and Zenith and normal player inputs. Returns completion, stance coverage, damage per channel, requests vs accepted attacks and replay artifacts. A timeout/stall is a failure to finish, not an automatic win. Requires a saved project with a ranged-equipped player and training enemy. Slow: builds and runs a real PS1 disc."
    )]
    async fn combat_duel(
        &self,
        Parameters(req): Parameters<CombatDuelReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let frontend = shot::find_frontend(self.frontend.as_deref().map(PathBuf::as_path))
            .map_err(|e| ErrorData::internal_error(e, None))?;
        let root = self.with(|w| {
            if w.is_dirty() {
                return Err("Save staged edits before running a duel".into());
            }
            Ok(w.root().to_path_buf())
        })?;
        let seed = req.seed.unwrap_or(1);
        if seed == 0 || seed == 128 {
            return Err(ErrorData::invalid_params(
                "seed must be 1..255 excluding 128",
                None,
            ));
        }
        let cue = if req.skip_build.unwrap_or(false) {
            play::last_cue(&root)
        } else {
            play::build_disc(&frontend, &root)
        }
        .map_err(|e| ErrorData::internal_error(e, None))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let out = root
            .join("validation/combat-duels")
            .join(format!("seed-{seed}-{stamp}"));
        let report = psxed_mcp::duel::run(
            &frontend,
            &cue,
            &out,
            seed,
            req.polls.unwrap_or(11400).clamp(900, 12000),
        )
        .map_err(|e| ErrorData::internal_error(e, None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&report).unwrap(),
        )]))
    }

    #[rmcp::tool(
        description = "Read a shared combat timeline: source-frame permissions, buffer windows, damage/hurtbox tracks, action speed/range/push, combos and cook-validated runtime intervals. Missing channels inherit legacy behavior; [start,end) intervals exclude end. Lists cook errors rather than pretending invalid data is playable."
    )]
    async fn combat_timeline(
        &self,
        Parameters(req): Parameters<CombatTimelineReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            psxed_mcp::combat::timeline(workspace.document()?, &root, req.animation_set)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Replace an Animation Set's combat windows. Validates the entire candidate cook before staging; invalid intervals do not mutate the document. Use combat_timeline first and save after verification. Timings are source clip frames, end exclusive."
    )]
    async fn set_combat_windows(
        &self,
        Parameters(req): Parameters<CombatWindowsReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let report = workspace.set_combat_windows(req.animation_set, &req.windows_ron)?;
            Ok(report + &Self::staged_note(workspace))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "List the project's materials with their texel dimensions, how many world units one tile covers, colour depth, blend mode and tint. The tile size is what decides how often a texture repeats on a surface, and it is not stored in the project file."
    )]
    async fn materials(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let root = workspace.root().to_path_buf();
            let project = workspace.document()?;
            Ok(material_table(project, &root))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Roll project.ron back to the snapshot taken before the last save. The version it replaces is snapshotted first, so the undo is itself undoable. For edits that have not been saved yet, use revert instead."
    )]
    async fn undo_last_edit(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(Workspace::undo_last_edit)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(description = "List the pre-save snapshots, newest first.")]
    async fn list_backups(&self) -> Result<CallToolResult, ErrorData> {
        let text = self.with(|workspace| {
            let backups = workspace.backups();
            Ok(if backups.is_empty() {
                "no snapshots yet; one is written each time save replaces the file".to_string()
            } else {
                format!(
                    "{} snapshot(s), newest first:\n{}",
                    backups.len(),
                    backups.join("\n")
                )
            })
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[rmcp::tool(
        description = "Health check: which project is open, whether edits are staged, the scene's world and node counts, and whether a renderer binary was found. Call this first in a session, and whenever something behaves unexpectedly."
    )]
    async fn status(&self) -> Result<CallToolResult, ErrorData> {
        let renderer = match shot::find_frontend(self.frontend.as_deref().map(PathBuf::as_path)) {
            Ok(path) => format!("{} (screenshot and playtest available)", path.display()),
            Err(error) => format!("NONE: {error}"),
        };
        let text = self.with(|workspace| {
            let staged = workspace.log().len();
            let dirty = workspace.is_dirty();
            let backups = workspace.backups().len();
            let path = workspace.root().to_path_buf();
            let project = workspace.document()?;
            let index = psxed_mcp::resolve_scene(project, None)?;
            let scene = &project.scenes[index];
            let world = scene.world.grid().map_or_else(
                || "no grid world".to_string(),
                |world| format!("grid world of {} rooms", world.rooms.len()),
            );
            Ok(format!(
                concat!(
                    "project: {}\n",
                    "scene {}: {}, {} nodes\n",
                    "staged edits: {}{}\n",
                    "snapshots: {}\n",
                    "renderer: {}"
                ),
                path.display(),
                format!("{index} {:?}", scene.name),
                world,
                scene.nodes().len(),
                staged,
                if dirty {
                    " -- unsaved, so screenshot and playtest will refuse"
                } else {
                    ""
                },
                backups,
                renderer
            ))
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
    instructions = "PSoXide grid-world authoring support, in authored editor units. Work in a loop: READ, ACT, VERIFY.\n\nREAD first. `metrics` carries the grid units, the world scale, the player's size and the traversal limits; call it before inventing any dimension. `scene_info` summarises the grid world and the scene's nodes. `materials` lists what you can texture with, `entity_types` and `get_node` show the scene nodes.\n\nACT on scene nodes with `place_node`, `set_node`, `move_node`, `delete_node`, `add_light` and `set_combat_windows`. The grid geometry itself is edited in the editor.\n\nVERIFY with `screenshot` for materials, lighting and mood, and with `playtest`, which cooks a real disc and boots it in the emulator: the only check that the level RUNS rather than merely cooking.\n\nEdits stage in memory. Nothing reaches project.ron until `save`, which refuses if the editor saved over the file meanwhile."
)]
impl ServerHandler for EditorServer {}

/// Write a starter project: the default project's resources and settings with
/// the starter room as its world (see `psxed_project::grid::starter`).
fn new_project(dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let file = dir.join("project.ron");
    if file.exists() {
        return Err(format!("{} already exists; refusing to overwrite", file.display()).into());
    }
    let doc = psxed_project::ProjectDocument::starter();
    std::fs::create_dir_all(dir)?;
    std::fs::write(&file, doc.to_ron_string()?)?;
    println!(
        "wrote {} with the starter room and {} resources",
        file.display(),
        doc.resources.len()
    );
    println!(
        "link its assets, e.g.: ln -s ../default/assets {}/assets",
        dir.display()
    );
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
