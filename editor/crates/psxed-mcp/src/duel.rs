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
    json!({"seed":seed,"outcome":name,"completed_by_death":matches!(name,"player_won"|"enemy_won"|"double_ko"),
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
        "stance_reasons":["distance","recover_pool","press_after_shots","exposed_channel","forced_break","rebuild_energy","melee_to_ranged","shot_to_melee","overhead_target","ranged_phase","contest_space"],"samples":json_samples,"decisions":decisions})
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
