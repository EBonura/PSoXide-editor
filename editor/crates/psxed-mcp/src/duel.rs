//! Repeatable dual-stance guest duels and machine-readable outcomes.
use serde_json::{json, Value};
use std::{path::Path, process::Command};

/// Summarize guest records. Completion and flow coverage are separate results.
pub fn summarize(log: &str) -> Value {
    let mut samples: Vec<Vec<u32>> = Vec::new();
    let mut decisions = Vec::new();
    let mut flow_events = Vec::new();
    let mut shot_events = Vec::new();
    let mut tactic_events = Vec::new();
    let mut combat_events: Vec<Vec<u32>> = Vec::new();
    let mut totals: Option<Vec<u32>> = None;
    let mut result = None;
    let mut seed = None;
    for line in log.lines() {
        let Some((label, rest)) = line.split_once(' ') else {
            continue;
        };
        let values: Vec<u32> = rest
            .split_whitespace()
            .filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
            .collect();
        match label {
            "duel:start" if values.len() == 2 => seed = Some(values[0]),
            "duel:sample" if values.len() == 19 || values.len() == 21 || values.len() == 23 => {
                samples.push(values)
            }
            "duel:decision" if values.len() == 5 => decisions.push(values),
            "duel:flow" if values.len() == 3 => flow_events.push(values),
            "duel:shot" if values.len() == 3 => shot_events.push(values),
            "duel:tactic" if values.len() == 3 => tactic_events.push(values),
            "duel:event" if values.len() == 6 => combat_events.push(values),
            "duel:totals" if values.len() == 8 => totals = Some(values),
            "duel:end" if values.len() == 2 => result = Some(values),
            _ => {}
        }
    }
    let mut damage = [[0u32; 2]; 2];
    let mut damage_events = [0u32; 2];
    let mut swaps = [0u32; 2];
    let mut stance_ticks = [[0u32; 2]; 2];
    let mut accepted = [0u32; 3];
    let mut enemy_attacks = 0;
    let mut enemy_windups = [0u32; 3];
    let mut switch_events = Vec::new();
    let mut ranges = [0u32; 3];
    for pair in samples.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let dt = b[0].saturating_sub(a[0]);
        for actor in 0..2 {
            let s = 9 + actor;
            if a[s] < 2 {
                stance_ticks[actor][a[s] as usize] += dt;
            }
            if a[s] != b[s] {
                swaps[actor] += 1;
                switch_events.push(json!({"tick":b[0],"actor":if actor==0 {"player"} else {"enemy"},"from":a[s],"to":b[s],"reason":if actor==0 {Some(b[14])} else {b.get(19).copied()}}));
            }
            let base = 5 + actor * 2;
            let mut hit = false;
            for channel in 0..2 {
                let loss = a[base + channel].saturating_sub(b[base + channel]);
                damage[actor][channel] += loss;
                hit |= loss > 0;
            }
            if hit {
                damage_events[actor] += 1;
            }
        }
        if b[17] != a[17] {
            match b[11] {
                6 | 34 | 35 => accepted[0] += 1,
                7 => accepted[1] += 1,
                33 => accepted[2] += 1,
                _ => {}
            }
        }
        if b[12] != a[12] && b[12] == 3 {
            if let Some(&kind) = b.get(20) {
                if kind < 3 {
                    enemy_windups[kind as usize] += 1;
                }
            }
        }
        // State values are emitted with the schema instead of inferred from screenshots.
        if b[12] != a[12] && b[12] == 4 {
            enemy_attacks += 1;
        }
    }
    for d in &decisions {
        if d[1] == 1 {
            ranges[0] += 1;
        }
        if d[1] == 6 {
            ranges[1] += 1;
        }
        if d[1] == 3 {
            ranges[2] += 1;
        }
    }
    let requested = [
        decisions.iter().filter(|d| d[3] & 0x800 != 0).count(),
        decisions.iter().filter(|d| d[3] & 0x200 != 0).count(),
    ];
    let name = match result.as_ref().map(|v| v[1]) {
        Some(1) => "player_won",
        Some(2) => "enemy_won",
        Some(3) => "double_ko",
        Some(4) => "manual_takeover",
        Some(5) => "timeout",
        Some(6) => "stalled_no_damage",
        _ => "incomplete",
    };
    let emitted: Vec<_> = (0..2)
        .map(|actor| shot_events.iter().filter(|r| r[1] == actor).count())
        .collect();
    let mut warnings = Vec::new();
    if !matches!(name, "player_won" | "enemy_won" | "double_ko") {
        warnings.push("Fight did not reach death.");
    }
    if stance_ticks.iter().any(|a| a[0] == 0 || a[1] == 0) {
        warnings.push("Both fighters did not spend time in both stances.");
    }
    if damage.iter().any(|a| a[0] == 0 || a[1] == 0) {
        warnings.push("Not every vitality channel took observed damage.");
    }
    if accepted[2] == 0 && emitted[0] == 0 {
        warnings.push("Player had no accepted ranged attack.");
    }
    // Time-weighted spatial coverage is independent of stance labels.
    let separation = |r: &[u32]| {
        let dx = f64::from(r[1] as i32) - f64::from(r[3] as i32);
        let dz = f64::from(r[2] as i32) - f64::from(r[4] as i32);
        (dx * dx + dz * dz).sqrt() as u32
    };
    let mut distance_ticks = [0u32; 3];
    let mut weighted_distance = 0u64;
    let mut close_streak = 0u32;
    let mut longest_close = 0u32;
    let mut excursion = 0u8;
    let mut cycles = 0u32;
    for pair in samples.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let dt = b[0].saturating_sub(a[0]);
        let distance = separation(a);
        let band = if distance <= 80 {
            0
        } else if distance < 192 {
            1
        } else {
            2
        };
        distance_ticks[band] += dt;
        weighted_distance += u64::from(distance) * u64::from(dt);
        if band == 0 {
            close_streak += dt;
            longest_close = longest_close.max(close_streak);
            if excursion == 2 {
                cycles += 1;
            }
            excursion = 1;
        } else {
            close_streak = 0;
            if band == 2 && excursion == 1 {
                excursion = 2;
            }
        }
    }
    let total: u32 = distance_ticks.iter().sum();
    let shot_distances: Vec<_> = shot_events
        .iter()
        .filter_map(|shot| {
            samples
                .iter()
                .min_by_key(|row| row[0].abs_diff(shot[0]))
                .map(|row| json!([shot[0], shot[1], separation(row)]))
        })
        .collect();
    if total > 0 && (distance_ticks[2] < total / 5 || cycles == 0) {
        warnings.push("Spatial push/pull coverage is weak: inspect distance bands and completed close-far-close cycles.");
    }
    // Ignore initial selection and unfinished final phases. Count completed
    // stance visits that never reached an attack, independently of distance.
    let mut empty_phases = [0u32; 2];
    let mut entered = [false; 2];
    let mut attacked = [false; 2];
    let mut goal_changes = 0u32;
    for pair in samples.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        goal_changes += u32::from((b[16] as u16).wrapping_sub(a[16] as u16));
        for actor in 0..2 {
            if a[9 + actor] != b[9 + actor] {
                if entered[actor] && !attacked[actor] {
                    empty_phases[actor] += 1;
                }
                entered[actor] = true;
                attacked[actor] = false;
            }
        }
        attacked[0] |= a[17] != b[17] && matches!(b[11], 6 | 7 | 33 | 34 | 35);
        attacked[1] |= a[12] != b[12] && b[12] == 3;
    }
    let mut tactic_ticks = [[0u32; 9]; 2];
    let mut cover_arrivals = [0u32; 2];
    let mut peek_shots = [0usize; 2];
    let mut evades = [0u32; 2];
    let mut defenses = [0u32; 2];
    let mut punishes = [0u32; 2];
    let last_tick = samples.last().map_or(0, |r| r[0]);
    for actor in 0..2 {
        let events: Vec<_> = tactic_events
            .iter()
            .filter(|e| e[1] == actor as u32)
            .collect();
        let mut sheltered = false;
        let mut peeked = false;
        for (n, event) in events.iter().enumerate() {
            let end = events.get(n + 1).map_or(last_tick, |e| e[0]);
            let mode = event[2] as usize;
            if mode < 9 {
                tactic_ticks[actor][mode] += end.saturating_sub(event[0]);
            }
            match mode {
                2 => {
                    cover_arrivals[actor] += 1;
                    sheltered = true;
                    peeked = false;
                }
                3 => {
                    peeked = sheltered;
                }
                4 => {
                    evades[actor] += 1;
                    sheltered = false;
                    peeked = false;
                }
                6 => {
                    defenses[actor] += 1;
                    sheltered = false;
                    peeked = false;
                }
                7 => {
                    punishes[actor] += 1;
                    sheltered = false;
                    peeked = false;
                }
                0 if peeked => {
                    peek_shots[actor] += shot_events
                        .iter()
                        .filter(|s| s[1] == actor as u32 && s[0] >= event[0] && s[0] < end)
                        .count();
                    sheltered = false;
                    peeked = false;
                }
                _ => {
                    sheltered = false;
                    peeked = false;
                }
            }
        }
    }
    if !tactic_events.is_empty() {
        if cover_arrivals.iter().all(|&n| n == 0) {
            warnings.push("No geometry-confirmed cover arrival; a finished fight does not validate cover play.");
        }
        if peek_shots.iter().all(|&n| n == 0) {
            warnings.push("No shot followed a completed cover/peek sequence.");
        }
        if evades.iter().all(|&n| n == 0) {
            warnings.push("No visible-projectile evasion decision was observed.");
        }
    }
    let energy_min: Vec<_> = (21..23)
        .map(|column| samples.iter().filter_map(|r| r.get(column)).min().copied())
        .collect();
    let energy_end: Vec<_> = (21..23)
        .map(|column| samples.last().and_then(|r| r.get(column)).copied())
        .collect();
    let json_samples: Vec<_> = samples
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, &v)| {
                    if (1..=4).contains(&i) {
                        json!(v as i32)
                    } else {
                        json!(v)
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let fingerprints = json!({
        "early": fingerprint(&samples, &combat_events, Some(EARLY_TICKS)),
        "whole": fingerprint(&samples, &combat_events, None),
        "early_ticks": EARLY_TICKS,
        "note": "FNV-1a over sampled positions, health, state and combat events; bot decisions are excluded so two seeds that only differ in a roll the world ignores compare equal.",
    });
    let diagnosis = end_diagnosis(name, &samples, &decisions);
    let mut report = json!({"seed":seed,"outcome":name,"completed_by_death":matches!(name,"player_won"|"enemy_won"|"double_ko"),
        "projectiles_emitted_player_enemy":emitted,"shot_events":shot_events,"shot_event_columns":["tick","actor_0_player_1_enemy","energy_after"],
        "energy_min_player_enemy":energy_min,"energy_end_player_enemy":energy_end,
        "flow_events":flow_events,"flow_event_columns":["tick","attacker_0_player_1_enemy","event_1_shot_interrupt"],
        "spatial_flow":{"band_edges_units":[80,192],"ticks_close_transition_ranged":distance_ticks,
            "mean_distance_units":if total==0 {0} else {(weighted_distance/u64::from(total)) as u32},
            "longest_close_ticks":longest_close,"close_far_close_cycles":cycles,
            "shot_distance_columns":["tick","actor","nearest_sample_distance"],"shot_distances":shot_distances},
        "ranged_exchange":{"cover_arrivals_player_enemy":cover_arrivals,
            "shots_after_cover_peek_player_enemy":peek_shots,"projectile_evade_attempts_player_enemy":evades,
            "melee_defense_episodes_player_enemy":defenses,"melee_punish_windows_player_enemy":punishes,
            "mode_ticks_player_enemy":tactic_ticks,"mode_names":["open","seek_cover","covered","peek","projectile_evade","search_cover","melee_defend","melee_punish","stamina_recovery"],
            "note":"Cover arrival requires occlusion checks. Evasion and punish counts are decisions, not confirmed dodges or hits."},
        "tactic_events":tactic_events,"tactic_event_columns":["tick","actor_0_player_1_enemy","mode"],
        "decision_quality":{"completed_stance_visits_without_attack_player_enemy":empty_phases,
            "enemy_goal_changes":goal_changes,
            "enemy_goal_changes_per_second":if total==0 {0.0} else {f64::from(goal_changes)*60.0/f64::from(total)},
            "note":"Initial and unfinished final stance visits excluded; an attack start is not proof of a hit. Goal changes are diagnostic, not a balance score."},
        "end_tick":result.as_ref().map(|v|v[0]),"sample_count":samples.len(),"stance_ticks_player_enemy":stance_ticks,
        "stance_switches_player_enemy":swaps,"damage_taken_player_enemy_hrz_zth":damage,"damage_events_player_enemy":damage_events,
        "player_accepted_light_heavy_ranged":accepted,"player_requested_r1_r2":requested,"enemy_attack_entries":enemy_attacks,
        "enemy_windups_light_heavy_ranged":enemy_windups,"switch_events":switch_events,
        "approach_retreat_ranged_decisions":ranges,"warnings":warnings,
        "measurement_note":"Attack starts are sampled animation changes; moving-fire layers may not match those counts. Projectile emission events count successful launches. Logs predating duel:shot have no emission counts. Damage is observed health loss, not raw attack damage; regeneration between samples may offset it. A death is a completion result, not a balance or flow pass.",
        "sample_columns":["tick","player_x","player_z","enemy_x","enemy_z","player_hrz","player_zth","enemy_hrz","enemy_zth","player_stance","enemy_stance","player_action","enemy_state","intent","stance_reason","stamina_q12","enemy_goal_generation","player_action_start","enemy_goal","enemy_stance_reason","enemy_attack_kind","player_energy","enemy_energy"],
        "decision_columns":["tick","intent","stance_reason","requested_buttons","distance"],
        "intent_names":["hold","approach","melee","ranged","dodge","swap","retreat"],
        "stance_reasons":["distance","recover_pool","press_after_shots","exposed_channel","forced_break","rebuild_energy","melee_to_ranged","shot_to_melee","overhead_target","ranged_phase","contest_space"],"samples":json_samples,"decisions":decisions});
    report["fingerprints"] = fingerprints;
    report["end_diagnosis"] = diagnosis;
    report["metrics"] = metrics(&MetricInputs {
        outcome: name,
        end_tick: result.as_ref().map(|v| v[0]),
        samples: &samples,
        events: &combat_events,
        totals: totals.as_deref(),
        tactic_events: &tactic_events,
        shot_events: &shot_events,
        swaps,
    });
    report
}

/// Ticks of fight covered by the early fingerprint.
const EARLY_TICKS: u32 = 900;

/// FNV-1a over the world-visible record of a fight, up to `upto` ticks.
fn fingerprint(samples: &[Vec<u32>], events: &[Vec<u32>], upto: Option<u32>) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |value: u32| {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let within = |tick: u32| upto.is_none_or(|limit| tick <= limit);
    for row in samples.iter().filter(|r| within(r[0])) {
        row.iter().take(13).for_each(|v| feed(*v));
    }
    for event in events.iter().filter(|e| within(e[0])) {
        event.iter().for_each(|v| feed(*v));
    }
    format!("{hash:016x}")
}

/// Why a fight that nobody won stopped making progress, from its last
/// 600 ticks. `null` for a fight that ended in a death or takeover.
///
/// Classes: `blocked_approach` (the player pushed forward and did not move:
/// a prop in the way, or an enemy holding its spacing outside the bot's
/// reach), `moving_standoff` (no damage but both actors moving),
/// `chip_loop` (damage kept landing to the end without a kill), `other`.
fn end_diagnosis(outcome: &str, samples: &[Vec<u32>], decisions: &[Vec<u32>]) -> Value {
    if !matches!(outcome, "stalled_no_damage" | "timeout") {
        return Value::Null;
    }
    let Some(last) = samples.last() else {
        return Value::Null;
    };
    let from = last[0].saturating_sub(600);
    let window: Vec<_> = samples.iter().filter(|r| r[0] >= from).collect();
    let span = |column: usize| {
        let values = window.iter().map(|r| r[column] as i32);
        values.clone().max().unwrap_or(0) - values.min().unwrap_or(0)
    };
    let player_range = span(1).max(span(2));
    let recent: Vec<_> = decisions.iter().filter(|d| d[0] >= from).collect();
    let approach = recent.iter().filter(|d| d[1] == 1).count();
    let approach_share = if recent.is_empty() {
        0.0
    } else {
        approach as f64 / recent.len() as f64
    };
    let damaged = window.windows(2).any(|w| (5..9).any(|c| w[1][c] < w[0][c]));
    let class = if approach_share >= 0.8 && player_range < 12 {
        "blocked_approach"
    } else if damaged {
        "chip_loop"
    } else if outcome == "stalled_no_damage" {
        "moving_standoff"
    } else {
        "other"
    };
    let distance = {
        let dx = f64::from(last[1] as i32) - f64::from(last[3] as i32);
        let dz = f64::from(last[2] as i32) - f64::from(last[4] as i32);
        (dx * dx + dz * dz).sqrt() as u32
    };
    json!({
        "class": class,
        "player_intent_approach_share": (approach_share * 100.0).round() / 100.0,
        "player_range_units": player_range,
        "damage_in_window": damaged,
        "enemy_state_last": last[12],
        "enemy_state_names": ["idle","patrol","aggro","windup","attack","recover","staggered","dead"],
        "distance_last": distance,
        "player_stance": last[9],
        "enemy_stance": last[10],
        "player_hp_channels": [last[5], last[6]],
        "enemy_hp_channels": [last[7], last[8]],
        "player_energy": last.get(21),
        "enemy_energy": last.get(22),
        "window_ticks": [from, last[0]],
    })
}

/// Player poise capacity in the runtime (`psx_game_runtime::character::PLAYER_POISE`).
const PLAYER_POISE: u32 = 60;

/// Everything `metrics` reads from one parsed guest log.
struct MetricInputs<'a> {
    outcome: &'a str,
    end_tick: Option<u32>,
    samples: &'a [Vec<u32>],
    events: &'a [Vec<u32>],
    totals: Option<&'a [u32]>,
    tactic_events: &'a [Vec<u32>],
    shot_events: &'a [Vec<u32>],
    swaps: [u32; 2],
}

/// Per-duel metrics that need guest `duel:event` records. Every value is a
/// number so batches can aggregate them without knowing their meaning.
///
/// Event layout: `[tick, kind, a, b, c, d]`.
/// - kind 1, player hit on the enemy: a = source (0 light, 1 heavy, 2 shot),
///   b = health removed, c = poise damage applied, d = flags (1 poise break,
///   2 killed, 4 opposite colour, 8 shot landed in an opening).
/// - kind 2, enemy hit on the player: a = source (0 claw light, 1 claw heavy,
///   2 cannon), b = health removed, c = poise damage applied, d = flags
///   (1 poise break, 2 killed, 4 opposite colour).
/// - kind 3, an enemy attack overlapped the player during i-frames: a = source.
fn metrics(m: &MetricInputs<'_>) -> Value {
    // [count, health removed, poise applied, breaks, opposite colour]
    let mut dealt = [[0u32; 5]; 3];
    let mut taken = [[0u32; 5]; 3];
    let mut avoids = [0u32; 3];
    let mut shot_hit_ticks = Vec::new();
    // Claw light hits whose own poise damage reaches the player's capacity
    // (`character::PLAYER_POISE`), i.e. hits that break a fresh player alone.
    let mut claw_light_alone = 0u32;
    for e in m.events {
        let (kind, source, hp, poise, flags) = (e[1], e[2] as usize, e[3], e[4], e[5]);
        if source > 2 {
            continue;
        }
        let row = match kind {
            1 => &mut dealt[source],
            2 => &mut taken[source],
            3 => {
                avoids[source] += 1;
                continue;
            }
            _ => continue,
        };
        if kind == 2 && source == 0 && poise >= PLAYER_POISE {
            claw_light_alone += 1;
        }
        row[0] += 1;
        row[1] += hp;
        row[2] += poise;
        row[3] += flags & 1;
        row[4] += (flags >> 2) & 1;
        if kind == 1 && source == 2 {
            shot_hit_ticks.push(e[0]);
        }
    }
    // Enemy sidesteps: a mode-4 tactic event for actor 1 starts one. It failed
    // when a player shot landed before the next mode change.
    let enemy_modes: Vec<_> = m.tactic_events.iter().filter(|t| t[1] == 1).collect();
    let end = m
        .end_tick
        .or_else(|| m.samples.last().map(|r| r[0]))
        .unwrap_or(0);
    let (mut evade_attempts, mut evade_failed) = (0u32, 0u32);
    for (n, t) in enemy_modes.iter().enumerate() {
        if t[2] != 4 {
            continue;
        }
        evade_attempts += 1;
        let until = enemy_modes
            .get(n + 1)
            .map_or(end.saturating_add(1), |next| next[0]);
        if shot_hit_ticks.iter().any(|&h| h >= t[0] && h < until) {
            evade_failed += 1;
        }
    }
    // Enemy flinches are read independently, from the sampled behaviour state
    // (6 = staggered), as a cross-check on the poise-break events.
    let flinches = m
        .samples
        .windows(2)
        .filter(|w| w[0][12] != 6 && w[1][12] == 6)
        .count() as u32;
    let sum = |rows: &[[u32; 5]; 3], column: usize| rows.iter().map(|r| r[column]).sum::<u32>();
    let fired = |actor: u32| m.shot_events.iter().filter(|s| s[1] == actor).count() as u32;
    let hp_end = |column: usize| {
        m.totals
            .map(|t| t[column])
            .or_else(|| m.samples.last().map(|r| r[column + 1]))
    };
    let pool_sum = |a: Option<u32>, b: Option<u32>| a.zip(b).map(|(a, b)| a + b);
    let energy = |column: usize| m.totals.map(|t| t[column]);
    json!({
        "outcome": m.outcome,
        "duration_ticks": m.end_tick,
        "player_hp_end": pool_sum(hp_end(4), hp_end(5)),
        "enemy_hp_end": pool_sum(hp_end(6), hp_end(7)),
        "damage_to_enemy": {"light": dealt[0][1], "heavy": dealt[1][1], "shot": dealt[2][1], "total": sum(&dealt, 1)},
        "hits_on_enemy": {"light": dealt[0][0], "heavy": dealt[1][0], "shot": dealt[2][0], "total": sum(&dealt, 0)},
        "damage_to_player": {"claw_light": taken[0][1], "claw_heavy": taken[1][1], "cannon": taken[2][1], "total": sum(&taken, 1)},
        "hits_on_player": {"claw_light": taken[0][0], "claw_heavy": taken[1][0], "cannon": taken[2][0], "total": sum(&taken, 0)},
        "claw_light_poise": {"hits": taken[0][0], "breaks": taken[0][3], "alone_capable": claw_light_alone},
        "poise_damage_to_enemy": sum(&dealt, 2),
        "poise_damage_to_player": sum(&taken, 2),
        "poise_breaks_inflicted": sum(&dealt, 3),
        "poise_breaks_suffered": sum(&taken, 3),
        "enemy_flinches": flinches,
        "opposite_colour_hits_on_enemy": sum(&dealt, 4),
        "opposite_colour_hits_on_player": sum(&taken, 4),
        "iframe_avoids": {"claw_light": avoids[0], "claw_heavy": avoids[1], "cannon": avoids[2], "total": avoids.iter().sum::<u32>()},
        "enemy_evades": {"attempted": evade_attempts, "avoided_shot": evade_attempts - evade_failed, "failed": evade_failed},
        "stance_swaps": {"player": m.swaps[0], "enemy": m.swaps[1]},
        "energy": {"player_spent": energy(0), "player_gained": energy(1), "enemy_spent": energy(2), "enemy_gained": energy(3)},
        "shots": {"player_fired": fired(0), "player_hit": dealt[2][0], "enemy_fired": fired(1), "enemy_hit": taken[2][0]},
        "definitions": "Health removed is what left the pools, so overkill is excluded. Poise is the value applied after colour scaling. i-frame avoids count one per enemy swing that overlapped the player (claw) or per bolt about to cross the player (cannon, estimated by look-ahead) while the player was invulnerable. An enemy evade 'avoided' when no player shot landed before the enemy left the evade mode. Flinches come from sampled behaviour state, breaks from events.",
    })
}

/// Run an existing normal Play disc. A start gesture selects the seeded controller.
pub fn run(frontend: &Path, cue: &Path, out: &Path, seed: u8, polls: u32) -> Result<Value, String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let tape = out.join("input.csv");
    let mut csv=String::from("psoxide-tape,v2,clock=pad_poll,start_poll=0\nframe,buttons,right_x,right_y,left_x,left_y\n");
    for n in 0..=polls {
        let start = (400..404).contains(&n);
        csv.push_str(&format!(
            "{n},{},{},128,128,128\n",
            if start { 257 } else { 0 },
            if start { seed } else { 128 }
        ));
    }
    std::fs::write(&tape, csv).map_err(|e| e.to_string())?;
    let output = Command::new(frontend)
        .args(["launch", "--path"])
        .arg(cue)
        .arg("--embedded-playtest")
        .arg("--input-tape")
        .arg(&tape)
        .arg("--stop-at-poll")
        .arg(polls.to_string())
        .args(["--steps", "10000000000"])
        .arg("--dump-display")
        .arg(out.join("final.ppm"))
        .output()
        .map_err(|e| e.to_string())?;
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(out.join("guest.log"), &log).map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Duel emulator failed; see {}",
            out.join("guest.log").display()
        ));
    }
    let mut report = summarize(&log);
    report["artifacts"] = json!(out);
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    // Full traces remain in report.json; the MCP response stays compact.
    report.as_object_mut().unwrap().remove("samples");
    report.as_object_mut().unwrap().remove("decisions");
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cover_reports_require_arrival_and_an_uninterrupted_peek() {
        let mut row = [0u32; 23];
        row[0] = 120;
        let mut log = format!(
            "duel:sample {}\n",
            row.iter()
                .map(|x| format!("{x:08X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        // Player searches and fails, then fires; enemy reaches cover and peeks.
        for (tick, actor, mode) in [
            (0, 0, 5),
            (10, 0, 0),
            (0, 1, 1),
            (10, 1, 2),
            (20, 1, 3),
            (30, 1, 0),
            (50, 1, 2),
            (60, 1, 3),
            (70, 1, 4),
            (80, 1, 5),
            (90, 1, 0),
        ] {
            log += &format!("duel:tactic {tick:08X} {actor:08X} {mode:08X}\n");
        }
        for (tick, actor) in [(20, 0), (40, 1), (100, 1)] {
            log += &format!("duel:shot {tick:08X} {actor:08X} 00000050\n");
        }
        let report = summarize(&log);
        assert_eq!(
            report["ranged_exchange"]["cover_arrivals_player_enemy"],
            json!([0, 2])
        );
        assert_eq!(
            report["ranged_exchange"]["shots_after_cover_peek_player_enemy"],
            json!([0, 1])
        );
        assert_eq!(
            report["ranged_exchange"]["projectile_evade_attempts_player_enemy"],
            json!([0, 1])
        );
    }
    #[test]
    fn reports_resource_samples_and_timed_interrupts() {
        let row = |tick, player, enemy| {
            let mut r = [0u32; 23];
            r[0] = tick;
            r[21] = player;
            r[22] = enemy;
            format!(
                "duel:sample {}\n",
                r.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let report = summarize(&format!(
            "{}{}{}duel:flow 0000003C 00000001 00000001\nduel:shot 0000001E 00000000 00000050\n",
            row(0, 100, 100),
            row(60, 0, 20),
            row(120, 24, 32)
        ));
        assert_eq!(report["projectiles_emitted_player_enemy"], json!([1, 0]));
        assert_eq!(report["energy_min_player_enemy"], json!([0, 20]));
        assert_eq!(report["energy_end_player_enemy"], json!([24, 32]));
        assert_eq!(report["flow_events"], json!([[60, 1, 1]]));
    }
    #[test]
    fn stance_changes_cannot_substitute_for_physical_push_pull() {
        let row = |tick, distance| {
            let mut r = [0u32; 23];
            r[0] = tick;
            r[1] = distance;
            format!(
                "duel:sample {}\n",
                r.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let contact = summarize(&(row(0, 40) + &row(60, 40) + &row(120, 40)));
        assert_eq!(
            contact["spatial_flow"]["ticks_close_transition_ranged"],
            json!([120, 0, 0])
        );
        assert_eq!(contact["spatial_flow"]["close_far_close_cycles"], 0);
        let cycle = summarize(&(row(0, 40) + &row(60, 256) + &row(120, 40) + &row(180, 40)));
        assert_eq!(
            cycle["spatial_flow"]["ticks_close_transition_ranged"],
            json!([120, 0, 60])
        );
        assert_eq!(cycle["spatial_flow"]["close_far_close_cycles"], 1);
    }
    #[test]
    fn counts_abandoned_stance_visits_separately_from_an_attack_visit() {
        let row = |tick, stance, action, start| {
            let mut r = [0u32; 23];
            r[0] = tick;
            r[9] = stance;
            r[11] = action;
            r[17] = start;
            format!(
                "duel:sample {}\n",
                r.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let log = row(0, 0, 0, 0)
            + &row(60, 1, 0, 0)
            + &row(120, 0, 0, 0)
            + &row(150, 0, 6, 150)
            + &row(180, 1, 0, 150);
        let report = summarize(&log);
        assert_eq!(
            report["decision_quality"]["completed_stance_visits_without_attack_player_enemy"],
            json!([1, 0])
        );
    }
    #[test]
    fn event_records_become_per_source_metrics() {
        let line = |label: &str, v: &[u32]| {
            format!(
                "{label} {}\n",
                v.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let sample = |tick: u32, enemy_state: u32| {
            let mut r = [0u32; 23];
            r[0] = tick;
            r[12] = enemy_state;
            line("duel:sample", &r)
        };
        let mut log = line("duel:start", &[3, 0]);
        // Idle, then staggered at 120, back to idle, staggered again at 300.
        log += &(sample(0, 0) + &sample(120, 6) + &sample(180, 0) + &sample(300, 6));
        for e in [
            // player: light (breaks, opposite), heavy, two shots
            [120, 1, 0, 25, 50, 5],
            [200, 1, 1, 30, 50, 0],
            [210, 1, 2, 28, 0, 4],
            [250, 1, 2, 28, 0, 0],
            // enemy: claw light, cannon (breaks the player), then an i-frame avoid
            [130, 2, 0, 32, 50, 0],
            [140, 2, 2, 25, 20, 5],
            [150, 3, 1, 0, 0, 0],
        ] {
            log += &line("duel:event", &e);
        }
        // Enemy sidesteps at 205 (shot lands at 210, failed) and at 400 (avoided).
        for (tick, mode) in [(0, 0), (205, 4), (235, 0), (400, 4), (430, 0)] {
            log += &line("duel:tactic", &[tick, 1, mode]);
        }
        for (tick, actor) in [(200, 0), (240, 0), (245, 0), (100, 1)] {
            log += &line("duel:shot", &[tick, actor, 80]);
        }
        log += &line("duel:totals", &[60, 36, 20, 0, 90, 100, 150, 140]);
        log += &line("duel:end", &[500, 1]);
        let m = &summarize(&log)["metrics"];
        assert_eq!(m["outcome"], "player_won");
        assert_eq!(m["duration_ticks"], 500);
        assert_eq!(
            m["damage_to_enemy"],
            json!({"light":25,"heavy":30,"shot":56,"total":111})
        );
        assert_eq!(m["hits_on_enemy"]["shot"], 2);
        assert_eq!(m["damage_to_player"]["claw_light"], 32);
        assert_eq!(m["damage_to_player"]["cannon"], 25);
        assert_eq!(m["poise_breaks_inflicted"], 1);
        assert_eq!(m["poise_breaks_suffered"], 1);
        // The claw light hit applied 50 poise: below the player's 60, so it cannot break alone.
        assert_eq!(
            m["claw_light_poise"],
            json!({"hits":1,"breaks":0,"alone_capable":0})
        );
        assert_eq!(m["enemy_flinches"], 2);
        assert_eq!(m["opposite_colour_hits_on_enemy"], 2);
        assert_eq!(m["opposite_colour_hits_on_player"], 1);
        assert_eq!(
            m["iframe_avoids"],
            json!({"claw_light":0,"claw_heavy":1,"cannon":0,"total":1})
        );
        assert_eq!(
            m["enemy_evades"],
            json!({"attempted":2,"avoided_shot":1,"failed":1})
        );
        assert_eq!(
            m["shots"],
            json!({"player_fired":3,"player_hit":2,"enemy_fired":1,"enemy_hit":1})
        );
        assert_eq!(m["energy"]["player_spent"], 60);
        assert_eq!(m["energy"]["player_gained"], 36);
        assert_eq!(m["player_hp_end"], 190);
        assert_eq!(m["enemy_hp_end"], 290);
    }
    #[test]
    fn metrics_degrade_to_nulls_without_the_new_records() {
        let m = &summarize("duel:start 00000001 00000000\nduel:end 00000040 00000001\n")["metrics"];
        assert_eq!(m["hits_on_enemy"]["total"], 0);
        assert!(m["player_hp_end"].is_null());
        assert!(m["energy"]["player_spent"].is_null());
    }
    /// A log of `ticks` ticks in which the player holds still at x = 0 and the
    /// decisions carry `intent`, ending as a no-damage stall.
    fn stalled_log(intent: u32, move_player: bool, damage_late: bool) -> String {
        let line = |label: &str, v: &[u32]| {
            format!(
                "{label} {}\n",
                v.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let mut log = line("duel:start", &[1, 0]);
        for tick in (0..=1800u32).step_by(60) {
            let mut r = [0u32; 23];
            r[0] = tick;
            r[1] = if move_player { tick / 4 } else { 0 };
            r[3] = 61;
            r[5..9].fill(100);
            r[12] = 2;
            if damage_late && tick >= 1500 {
                r[5] = 100 - (tick - 1500) / 12;
            }
            log += &line("duel:sample", &r);
            log += &line("duel:decision", &[tick, intent, 9, 0, 61]);
        }
        log + &line("duel:end", &[1800, 6])
    }
    #[test]
    fn a_stall_is_classified_by_what_the_actors_were_doing() {
        let blocked = summarize(&stalled_log(1, false, false));
        assert_eq!(blocked["end_diagnosis"]["class"], "blocked_approach");
        assert_eq!(blocked["end_diagnosis"]["enemy_state_last"], 2);
        let moving = summarize(&stalled_log(1, true, false));
        assert_eq!(moving["end_diagnosis"]["class"], "moving_standoff");
        let waiting = summarize(&stalled_log(0, false, false));
        assert_eq!(waiting["end_diagnosis"]["class"], "moving_standoff");
        // A death has nothing to diagnose.
        assert!(
            summarize("duel:start 00000001 00000000\nduel:end 00000040 00000001\n")
                ["end_diagnosis"]
                .is_null()
        );
    }
    #[test]
    fn fingerprints_ignore_bot_decisions_but_not_the_fight() {
        let a = summarize(&stalled_log(1, false, false));
        let b = summarize(&stalled_log(2, false, false));
        assert_eq!(a["fingerprints"]["whole"], b["fingerprints"]["whole"]);
        let c = summarize(&stalled_log(1, true, false));
        assert_ne!(a["fingerprints"]["whole"], c["fingerprints"]["whole"]);
        assert_ne!(a["fingerprints"]["early"], c["fingerprints"]["early"]);
    }
    #[test]
    fn never_passes_an_unstarted_or_stalled_fight() {
        assert_eq!(summarize("route-ticks=100")["outcome"], "incomplete");
        let r = summarize("duel:start 00000002 00000000\nduel:end 00000708 00000006\n");
        assert_eq!(r["outcome"], "stalled_no_damage");
        assert_eq!(r["completed_by_death"], false);
    }
    #[test]
    fn death_does_not_automatically_pass_stance_coverage() {
        let r = summarize("duel:start 00000002 00000000\nduel:end 00000400 00000001\n");
        assert_eq!(r["completed_by_death"], true);
        assert!(r["warnings"].as_array().unwrap().len() >= 2);
    }
    #[test]
    fn report_counts_actual_action_starts_channels_and_signed_positions() {
        let mut a = [0u32; 21];
        a[1] = (-32i32) as u32;
        a[5..9].fill(100);
        let mut b = a;
        b[0] = 60;
        b[5] = 80;
        b[8] = 70;
        b[9] = 1;
        b[11] = 33;
        b[17] = 60;
        b[12] = 3;
        b[20] = 2;
        let row = |r: [u32; 21]| {
            format!(
                "duel:sample {}\n",
                r.iter()
                    .map(|x| format!("{x:08X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let log = format!(
            "duel:start 00000001 00000000\n{}{}duel:end 0000003C 00000001\n",
            row(a),
            row(b)
        );
        let report = summarize(&log);
        assert_eq!(
            report["player_accepted_light_heavy_ranged"],
            json!([0, 0, 1])
        );
        assert_eq!(report["enemy_windups_light_heavy_ranged"], json!([0, 0, 1]));
        assert_eq!(
            report["damage_taken_player_enemy_hrz_zth"],
            json!([[20, 0], [0, 30]])
        );
        assert_eq!(report["samples"][0][1], -32);
    }
}
