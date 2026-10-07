//! Inspect authored combat tracks alongside the actual cooked permissions.
use psxed_project::{ProjectDocument, ResourceData};
use serde_json::json;
use std::path::Path;

/// Structured timing report; never guesses FPS from animation frames.
pub fn timeline(project: &ProjectDocument, root: &Path, set_id: u64) -> Result<String, String> {
    let id = project
        .resources
        .iter()
        .find(|r| r.id.raw() == set_id)
        .ok_or("unknown animation set")?
        .id;
    let resource = project.resource(id).ok_or("unknown animation set")?;
    let ResourceData::AnimationSet(set) = &resource.data else {
        return Err("resource is not an Animation Set".into());
    };
    let characters: Vec<_> = project.resources.iter().filter_map(|r| {
        if let ResourceData::Character(c) = &r.data { if c.animation_set == Some(id) {
            return Some(json!({"id":r.id.raw(),"name":r.name,"damage_and_hurtbox_tracks":c.combat_capsules}));
        }} None
    }).collect();
    let (package, report) = psxed_project::playtest::build_package(project, root);
    let cooked: Vec<_> = package.as_ref().map(|p| p.characters.iter().filter(|c|
        characters.iter().any(|v| v["id"].as_u64()==Some(c.source_resource.raw()))).map(|c| {
        let windows: Vec<_> = c.combat_windows.iter().filter(|w|w.action != 255).map(|w| {
            let slot = usize::from(w.action);
            let fallback = c.action_clips[slot] == u16::MAX && matches!(slot, 5 | 15 | 16);
            let local = c.action_clips[if fallback { 4 } else { slot }];
            let clip = p.models.get(usize::from(c.model)).and_then(|m| p.model_clips.get(usize::from(m.clip_first) + usize::from(local)));
            let stats = clip.and_then(|clip| p.assets.get(clip.animation_asset_index)).and_then(|asset|
                psxed_project::model_import::animation_stats_from_bytes("combat timeline", &asset.bytes, 0).ok());
            let rate = stats.as_ref().map(|s| s.sample_rate_hz);
            let seconds = |frame: u16| rate.filter(|r|*r>0).map(|r|
                f64::from(frame.saturating_sub(c.action_frame_ranges[slot].start)) * 256.0 / (f64::from(r)*f64::from(c.action_speeds[slot].max(1))));
            json!({"action_index":w.action,"kind":format!("{:?}",w.kind),"start":w.start,"end_exclusive":w.end,
                "cooked_rate_hz":rate,"speed_q8":c.action_speeds[slot],"nominal_start_seconds":seconds(w.start),
                "nominal_end_seconds":seconds(w.end),"uses_forward_dash_fallback":fallback})
        }).collect();
        json!({"character":c.source_resource.raw(),"windows":windows})
    }).collect()).unwrap_or_default();
    serde_json::to_string_pretty(&json!({
        "animation_set":set_id,"name":resource.name,
        "runtime_scope":"player-controller permissions/protection; enemy damage and combo tracks remain independently authored",
        "time_note":"nominal seconds use cooked sample rate, selected range and authored speed; attack-speed bonuses can change elapsed time",
        "window_units":"source clip frames, [start,end); cooking remaps to sampled frames",
        "inheritance":"missing channel retains legacy behavior; equal endpoints explicitly close it",
        "permissions":"attack/dodge buffer stores one press scoped to this action instance; permission consumes it once; accepted transitions interrupt old motor recovery; movement requires stick intent",
        "legacy_protection":"unconfigured invulnerability uses motor dodge ticks; stance-swap protection is separate; unconfigured armour uses heavy-attack hitbox active frames",
        "bindings_speed_range_and_push":set.action_clips,
        "windows":set.combat_windows,"combo_handoffs":set.action_chains,
        "characters":characters,"cook_valid":report.is_ok(),"cook_errors":report.errors.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "cooked":cooked
    })).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_resolves_cooked_time_and_invalid_edits_are_atomic() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/graybox-reach");
        let mut workspace = crate::edit::Workspace::open(&root).unwrap();
        let report: serde_json::Value =
            serde_json::from_str(&timeline(workspace.document().unwrap(), &root, 61).unwrap())
                .unwrap();
        assert_eq!(report["cook_valid"], true);
        let hit_dodge = report["cooked"][0]["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["action_index"] == 10 && w["kind"] == "Dodge")
            .unwrap();
        assert_eq!(hit_dodge["nominal_start_seconds"], 1.0);
        assert_eq!(hit_dodge["cooked_rate_hz"], 60);
        assert!(workspace
            .set_combat_windows(61, "[(action: HitReact, kind: Dodge, start: 70, end: 60)]")
            .is_err());
        assert!(!workspace.is_dirty());
        assert!(workspace
            .set_combat_windows(60, "[]")
            .unwrap_err()
            .contains("active player's"));
        assert!(!workspace.is_dirty());
    }
}

// Pure shared constants/state tests, without depending on the guest renderer.
#[allow(dead_code)]
#[path = "../../../../engine/crates/psx-game-runtime/src/combat_flow.rs"]
mod flow_rules;

/// Report prototype tuning from the canonical guest resource rules.
pub fn flow_rules() -> serde_json::Value {
    use flow_rules::*;
    json!({"status":"prototype","scope":"ranged-equipped editor playtests; legacy entity rules otherwise unchanged",
        "energy":{"max":ENERGY_MAX,"shot_cost_per_projectile":SHOT_COST,"light_contact_gain":MELEE_GAIN,
            "heavy_contact_gain":HEAVY_GAIN,"floating_gain_per_second":FLOAT_GAIN_PER_SECOND,
            "floating_pauses_while_firing":true,"passive_ground_regen":false,
            "enemy_uses_same_energy":true,"enemies_can_hook":false},
        "ai":{"energy_resume_threshold":AI_RESUME_ENERGY,"followup_memory_ticks":FOLLOWUP_TICKS,
            "ranged_band":"near = 3x preferred distance; far = 6x, bounded by weapon range",
            "breakaway":"turn and run out before firing; collision, attack locks and player stamina remain authoritative",
            "blocked_breakaway_melee_ticks":CONTEST_SPACE_TICKS,
            "spatial_validation":"combat_duel measures distance occupancy and close-far-close cycles, independently of stance changes",
            "ranged_exchange":"two-shot or timed exposure, geometry-tested cover, timed shelter, peek; visible live projectiles can trigger collision-bound evasion",
            "lateral_evade":{"distance_units":EVADE_DISTANCE,"movement_ticks":EVADE_MOVE_TICKS,
                "recovery_ticks":EVADE_RECOVERY_TICKS,"cooldown_ticks":EVADE_COOLDOWN_TICKS,
                "energy_cost":0,"invulnerable":false,"requires":"visible released approaching projectile; free locomotion; clear side lane",
                "animation":"existing left/right strafe clip, travel-driven cadence; vulnerable planted recovery"},
            "melee_exchange":"read committed swings, defend, punish recovery; proximity stays melee without a heavy-contact exit opportunity",
            "spacing_pause_ticks":[36,72],"spacing_replan_after_ticks":24,
            "melee_tell":"first half may turn and step back through collision; second half plants",
            "overhead_empty":"keeps ranged stance and waits for target to become reachable; no free Energy"},
        "arch":{"duration_ticks_60hz":AIR_TICKS,"ground_rearm_ticks":GROUND_REARM_TICKS,
            "detach_on":"timer, poise break, death, collision, voluntary Circle",
            "rehook_does_not_reset":true},
        "damage":{"matching_percent":100,"opposite_percent":125,"poise_is_shared_between_stances":true},
        "interrupt":{"shot_window":"final half of melee windup; NPC ranged windup also vulnerable",
            "ordinary_shot_poise":"min(authored / 4, 10)","grace_after_reaction_ticks":BREAK_GRACE_TICKS,
            "ordinary_shot_forces_hit_react":false},
        "controls":{"Triangle":"manual stance","Zenith_R2":"fire if energy sufficient",
            "L2":"optional precision aim; L2+R2 hooks when arch selected","Select_L2":"AI duel"},
        "animation":"current Aletha 26-joint upper body and Light 22-joint legs compose into the shared visible/socket/hurtbox pose",
        "validation":"combat_duel reports sampled energy and health. An aerial input tape is required to validate arch flow; ground duels alone do not cover it."})
}
