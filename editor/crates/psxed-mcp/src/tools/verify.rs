//! VERIFY: screenshots from chosen poses and headless playtests.

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::route::{self, Leg};
use psxed_mcp::shot::{self, ShotSpec};
use psxed_mcp::{cost, play, track};

use super::{png_block, EditorServer};

/// Most shots one screenshot call renders.
const MAX_SHOTS: usize = 8;

#[derive(Debug, Deserialize, JsonSchema)]
struct ScreenshotReq {
    /// Scene index. Omit for the first scene with a grid world.
    scene: Option<usize>,
    /// A single shot, given inline (position/look_at, or target/node with
    /// distance, yaw, pitch). Ignored when `shots` is given.
    #[serde(flatten)]
    single: ShotSpec,
    /// Several shots in one call (at most 8), each a pose of its own.
    shots: Option<Vec<ShotSpec>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PlaytestReq {
    /// A scripted pad route: legs of `polls` ticks with a left stick (or a
    /// `dir`), a camera stick and held buttons, played after a warm-up while
    /// the level loads. With a route the run reports frame cost and returns
    /// selected frames. Without one, the older fixed-schedule run is used.
    route: Option<Vec<Leg>>,
    /// Route only: polls (ticks, 60 a second) before the first leg, while the
    /// level loads. A CROSS tap clears any screen that waits for one. Default
    /// 450, about when a small grid level starts; raise it for a big level.
    warmup_polls: Option<u32>,
    /// Route only: which frames to return, as polls AFTER the route starts
    /// (0 is the first leg's first poll). Default: four evenly spaced through
    /// the route. The nearest captured frame is used (within a few ticks).
    frames: Option<Vec<u32>>,
    /// Route only: record the player's position every update (the guest's
    /// camera-pose-log feature) and report where the player was at each
    /// returned frame, plus the start, end and height range of the path. It
    /// is how you check the player actually crossed a bridge or climbed a
    /// ramp rather than guessing from a picture. The logging costs a little
    /// guest time, so for a precise cost read run once without it. Default
    /// false.
    track_player: Option<bool>,
    /// Route only: build the disc with the guest's emulator-telemetry feature
    /// so the cost summary has render and world cycles per frame. Default
    /// true for a route. It costs guest RAM, so a level near the cap may not
    /// link with it: when you did not ask for it explicitly the build falls
    /// back to a plain disc and the summary is frame cadence from the
    /// emulator's route log, with a note. Pass false to skip the attempt.
    telemetry: Option<bool>,
    /// Controller polls to run for (no route). One poll is one simulation
    /// tick. Under about 3000 the run is still in the menu and loading flow;
    /// 7000 reaches gameplay. Default 7000.
    polls: Option<u32>,
    /// Reuse the disc from the last build instead of building (saved projects
    /// only: a staged document always rebuilds). Default false; the build
    /// takes about half a minute and the run a further one to two.
    skip_build: Option<bool>,
    /// No route: button schedule, `tick:button[:hold]` comma separated.
    /// Defaults to tapping cross through the title, the world message and
    /// the mid-load splash, all of which wait on it.
    press: Option<String>,
    /// No route: hold the left stick forward. Default true.
    walk: Option<bool>,
}

#[rmcp::tool_router(router = verify_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Render the editor's 3D preview (the PS1 renderer) from one or several camera poses. Poses: an eye position plus look_at (or yaw_deg and pitch_deg), or an orbit around a target point or a node with distance, yaw and pitch. Up to 8 shots in one call via shots. It renders the STAGED document, saved or not. All positions are editor units (a new grid project's sector is 1280); use grid_plan for layout and this for materials, lighting, scale and whether a pose actually sees what you meant."
    )]
    async fn screenshot(
        &self,
        Parameters(req): Parameters<ScreenshotReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let frontend = self.frontend_path()?;
        let specs: Vec<ShotSpec> = match req.shots {
            Some(list) if !list.is_empty() => list,
            _ => vec![req.single],
        };
        if specs.len() > MAX_SHOTS {
            return Err(ErrorData::invalid_params(
                format!("at most {MAX_SHOTS} shots per call"),
                None,
            ));
        }
        let (solved, root) = self.with(|workspace| {
            let root = workspace.render_root()?;
            let project = workspace.document()?;
            let mut solved = Vec::new();
            for (i, spec) in specs.iter().enumerate() {
                let shot =
                    shot::solve(project, req.scene, spec).map_err(|e| format!("shot {i}: {e}"))?;
                solved.push((
                    spec.label
                        .clone()
                        .unwrap_or_else(|| format!("shot {}", i + 1)),
                    shot,
                ));
            }
            Ok((solved, root))
        })?;
        let mut blocks = Vec::new();
        for (label, shot) in solved {
            let png = shot::render(&frontend, &root.path, shot, self.scratch.as_path())
                .map_err(|error| ErrorData::internal_error(format!("{label}: {error}"), None))?;
            let eye = shot::eye_of(&shot);
            blocks.push(ContentBlock::text(format!(
                "{label}: eye {eye:?}, target {:?}, yaw {} pitch {} radius {}{}",
                shot.target,
                shot.yaw_q12,
                shot.pitch_q12,
                shot.radius,
                if root.is_staged() {
                    " (staged edits, not saved)"
                } else {
                    ""
                }
            )));
            blocks.push(png_block(&png));
        }
        Ok(CallToolResult::success(blocks))
    }

    #[rmcp::tool(
        description = "Cook the (staged) project to a real PS1 disc, boot it in the emulator and play it headlessly: the only check that the level RUNS rather than merely cooking. With `route` (legs of stick, camera and button input over polls, e.g. walk forward 300 polls) it plays that path, returns frames along it, and summarises frame cost: fps, presentation periods, and with telemetry true the render and world cycle mean, p90, max and frames over the 1,128,960-cycle 30 fps budget. Without a route it runs the older fixed schedule. Slow: about half a minute to build, then roughly 8 seconds per thousand polls. Read port1-polls first: a run far short of the request stalled on a screen waiting for input."
    )]
    async fn playtest(
        &self,
        Parameters(req): Parameters<PlaytestReq>,
    ) -> Result<CallToolResult, ErrorData> {
        let frontend = self.frontend_path()?;
        let root = self.with(|workspace| workspace.render_root())?;
        let internal = |e: String| ErrorData::internal_error(e, None);
        // A level the cook refuses fails here in a second, in the cook's words.
        self.with(|workspace| {
            let project = workspace.document()?;
            psxed_mcp::grid::read::cook_gate(project, &root.path)
        })?;
        let reuse = req.skip_build.unwrap_or(false) && !root.is_staged();
        let track = req.track_player.unwrap_or(false);
        let routed = req.route.is_some();
        let telemetry_asked = req.telemetry;
        let mut telemetry = telemetry_asked.unwrap_or(routed);
        let mut notes = String::new();
        let cue = if reuse {
            play::last_cue(&root.path).map_err(internal)?
        } else {
            let first = play::build_disc_with(
                &frontend,
                &root.path,
                play::GuestFeatures { telemetry, track },
            );
            match first {
                Ok(cue) => cue,
                Err(error) if telemetry && telemetry_asked.is_none() => {
                    // Not asked for by name: fall back to the plain disc.
                    telemetry = false;
                    notes.push_str(&format!(
                        "the telemetry build failed, so this is a plain disc (cost is frame cadence only): {}\n",
                        error.lines().find(|l| !l.trim().is_empty()).unwrap_or("")
                    ));
                    play::build_disc_with(
                        &frontend,
                        &root.path,
                        play::GuestFeatures {
                            telemetry: false,
                            track,
                        },
                    )
                    .map_err(internal)?
                }
                Err(error) => return Err(internal(error)),
            }
        };

        let Some(legs) = &req.route else {
            return self.legacy_playtest(&frontend, &cue, &req, &root.path);
        };
        let warmup = req.warmup_polls.unwrap_or(route::DEFAULT_WARMUP);
        let built = route::build(warmup, legs).map_err(internal)?;
        let work = self.scratch.join("playtest");
        std::fs::create_dir_all(&work)
            .map_err(|e| internal(format!("create {}: {e}", work.display())))?;
        let tape = work.join("route.csv");
        std::fs::write(&tape, &built.csv)
            .map_err(|e| internal(format!("write {}: {e}", tape.display())))?;
        let route_len = built.polls - built.start;
        let interval = (built.polls / 50).clamp(10, 60);
        let run = play::run_route(&frontend, &cue, &work, &tape, built.polls, interval, track)
            .map_err(internal)?;

        let mut text = notes;
        text.push_str(&run.report.route_summary(built.polls));
        text.push_str(&format!(
            "route: {} leg(s), {route_len} polls ({:.1} s) after a {warmup} poll warm-up{}\n",
            legs.len(),
            f64::from(route_len) / 60.0,
            if root.is_staged() {
                "; played the staged document (not saved)"
            } else {
                ""
            }
        ));
        match cost::summarize(
            &run.route_csv,
            run.profile_csv.as_deref(),
            u64::from(built.start),
        ) {
            Ok(summary) => text.push_str(&summary),
            Err(e) => text.push_str(&format!("frame cost unavailable: {e}\n")),
        }
        if telemetry && telemetry_asked == Some(true) && !text.contains("guest telemetry") {
            text.push_str(
                "telemetry was asked for but no telemetry rows came back; the disc may not have been built with it (skip_build reuses the last disc)\n",
            );
        }

        let wanted: Vec<u32> = match &req.frames {
            Some(list) if !list.is_empty() => list.clone(),
            _ => (0..4).map(|i| route_len * i / 3).collect(),
        };
        if track {
            let samples = track::parse(&run.log, &run.route_csv);
            let polls: Vec<u64> = wanted.iter().map(|p| u64::from(*p)).collect();
            text.push_str(&track::describe(&samples, u64::from(built.start), &polls));
            text.push_str(&track::describe_events(
                &track::events(&run.log, &run.route_csv),
                u64::from(built.start),
            ));
        }
        let mut blocks = vec![ContentBlock::text(String::new())];
        let mut seen = std::collections::BTreeSet::new();
        for rel in wanted.iter().take(8) {
            let poll = u64::from(built.start) + u64::from(*rel);
            let Some((tick, path)) = play::frame_near_poll(&run.route_csv, &run.frames, poll)
            else {
                continue;
            };
            if !seen.insert(tick) {
                continue;
            }
            let png = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|raw| shot::png_from_ppm(&raw))
                .map_err(internal)?;
            blocks.push(ContentBlock::text(format!(
                "frame near route poll {rel} (route tick {tick})"
            )));
            blocks.push(png_block(&png));
        }
        let final_png = std::fs::read(&run.final_ppm)
            .map_err(|e| e.to_string())
            .and_then(|raw| shot::png_from_ppm(&raw))
            .map_err(internal)?;
        blocks.push(ContentBlock::text("final frame".to_string()));
        blocks.push(png_block(&final_png));
        blocks[0] = ContentBlock::text(text);
        // The frames and logs are scratch; the disc is the big file.
        let _ = std::fs::remove_dir_all(work.join("frames"));
        Ok(CallToolResult::success(blocks))
    }
}

impl EditorServer {
    fn frontend_path(&self) -> Result<PathBuf, ErrorData> {
        shot::find_frontend(self.frontend.as_deref().map(PathBuf::as_path))
            .map_err(|error| ErrorData::internal_error(error, None))
    }

    /// The fixed-schedule run: tap through the menus, optionally walk forward.
    fn legacy_playtest(
        &self,
        frontend: &std::path::Path,
        cue: &std::path::Path,
        req: &PlaytestReq,
        _project_dir: &std::path::Path,
    ) -> Result<CallToolResult, ErrorData> {
        let internal = |e: String| ErrorData::internal_error(e, None);
        let polls = req.polls.unwrap_or(7000).clamp(300, 60_000);
        let schedule = req
            .press
            .clone()
            .unwrap_or_else(|| play::menu_press_schedule(polls.min(5200)));
        let dump = self.scratch.join("playtest.ppm");
        let report = play::run_disc(
            frontend,
            cue,
            &dump,
            polls,
            &schedule,
            req.walk.unwrap_or(true),
        )
        .map_err(internal)?;
        let png = std::fs::read(&dump)
            .map_err(|e| format!("read the dumped frame: {e}"))
            .and_then(|raw| shot::png_from_ppm(&raw))
            .map_err(internal)?;
        let _ = std::fs::remove_file(&dump);
        Ok(CallToolResult::success(vec![
            ContentBlock::text(format!(
                "{}\ndisc: {}",
                report.summary(polls),
                cue.display()
            )),
            png_block(&png),
        ]))
    }
}
