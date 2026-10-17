//! Combat tuning tools (unchanged by the grid work).

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;

use psxed_mcp::{play, shot};

use super::{text_result, EditorServer};

#[derive(Debug, Deserialize, JsonSchema)]
struct CombatDuelReq {
    /// Repeatable seed (1..255; 128 is reserved for the physical default seed 1).
    seed: Option<u8>,
    /// Total input polls, including loading. Default 11400 covers the 180-second duel limit.
    polls: Option<u32>,
    /// Reuse the built normal Play disc. Default false rebuilds from saved project data.
    skip_build: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CombatTimelineReq {
    /// Animation Set resource id (Graybox Reach player: 61).
    animation_set: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CombatWindowsReq {
    /// Animation Set resource id.
    animation_set: u64,
    /// Complete RON list replacing this set's windows. Example: [(action: HitReact, kind: DodgeBuffer, start: 35, end: 80), (action: HitReact, kind: Dodge, start: 60, end: 81)]. Kinds: AttackBuffer, Attack, DodgeBuffer, Dodge, Movement, Invulnerable, Armored. Source frames, end exclusive; [] removes overrides. Staged until save.
    windows_ron: String,
}

#[rmcp::tool_router(router = combat_router, vis = "pub(super)")]
impl EditorServer {
    #[rmcp::tool(
        description = "Read the prototype combat rules: shared Energy gains/costs, floating duration/rearm, poise openings, damage multipliers and controls. Rates are 60 Hz simulation values, not measured frame rate. Use combat_duel for ground coverage; arches require a separate aerial replay."
    )]
    async fn combat_flow(&self) -> Result<CallToolResult, ErrorData> {
        Ok(text_result(psxed_mcp::combat::flow_rules().to_string()))
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
        Ok(text_result(text))
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
        Ok(text_result(text))
    }
}
