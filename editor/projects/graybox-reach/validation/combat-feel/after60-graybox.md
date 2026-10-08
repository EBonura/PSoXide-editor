# Combat duel batch: after60-graybox

- scenario `graybox`, 60 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60]
- source `119c938880eafc74f8132506d44edc09d7dac93e`, dirty false, polls 11400, disc sha256 `b615eb3d2bf158fd680f4b40405262035dc6f2bda15acc66712a826a6e616c51`
- outcomes {"enemy_won":21,"player_won":37,"stalled_no_damage":2}
- player win rate 0.617 (Wilson 95%: 0.49 to 0.729)
- distinct early streams 58 and whole fights 60 of 60 runs; unfinished-fight causes {"moving_standoff":2}

| metric | mean | 95% CI half-width | median | min | max | sum |
|---|---:|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 3.48 | 0.42 | 3.0 | 1.0 | 10.0 | 209.0 |
| damage_to_enemy.heavy | 224.62 | 19.81 | 244.0 | 38.0 | 360.0 | 13477.0 |
| damage_to_enemy.light | 155.6 | 13.61 | 162.0 | 31.0 | 324.0 | 9336.0 |
| damage_to_enemy.shot | 128.73 | 16.09 | 140.0 | 0.0 | 252.0 | 7724.0 |
| damage_to_enemy.total | 508.95 | 37.06 | 600.0 | 160.0 | 600.0 | 30537.0 |
| damage_to_player.cannon | 8.83 | 3.83 | 0.0 | 0.0 | 75.0 | 530.0 |
| damage_to_player.claw_heavy | 140.65 | 19.54 | 134.0 | 0.0 | 360.0 | 8439.0 |
| damage_to_player.claw_light | 115.88 | 14.03 | 110.5 | 32.0 | 289.0 | 6953.0 |
| damage_to_player.total | 265.37 | 25.14 | 264.0 | 40.0 | 505.0 | 15922.0 |
| duration_ticks | 6068.55 | 412.32 | 6279.5 | 2340.0 | 8859.0 | 364113.0 |
| enemy_evades.attempted | 3.12 | 0.45 | 3.0 | 0.0 | 6.0 | 187.0 |
| enemy_evades.avoided_shot | 2.83 | 0.43 | 3.0 | 0.0 | 6.0 | 170.0 |
| enemy_evades.failed | 0.28 | 0.13 | 0.0 | 0.0 | 2.0 | 17.0 |
| enemy_flinches | 6.73 | 0.61 | 7.0 | 1.0 | 10.0 | 404.0 |
| enemy_hp_end | 91.05 | 37.06 | 0.0 | 0.0 | 440.0 | 5463.0 |
| energy.enemy_gained | 39.2 | 6.79 | 40.0 | 0.0 | 124.0 | 2352.0 |
| energy.enemy_spent | 53.33 | 8.0 | 60.0 | 0.0 | 140.0 | 3200.0 |
| energy.player_gained | 135.4 | 16.63 | 160.0 | 0.0 | 248.0 | 8124.0 |
| energy.player_spent | 174.33 | 19.26 | 190.0 | 0.0 | 300.0 | 10460.0 |
| hits_on_enemy.heavy | 5.8 | 0.52 | 7.0 | 1.0 | 9.0 | 348.0 |
| hits_on_enemy.light | 5.95 | 0.52 | 6.0 | 1.0 | 12.0 | 357.0 |
| hits_on_enemy.shot | 4.55 | 0.57 | 5.0 | 0.0 | 9.0 | 273.0 |
| hits_on_enemy.total | 16.3 | 1.21 | 19.0 | 5.0 | 21.0 | 978.0 |
| hits_on_player.cannon | 0.37 | 0.16 | 0.0 | 0.0 | 3.0 | 22.0 |
| hits_on_player.claw_heavy | 3.12 | 0.44 | 3.0 | 0.0 | 7.0 | 187.0 |
| hits_on_player.claw_light | 3.48 | 0.42 | 3.0 | 1.0 | 10.0 | 209.0 |
| hits_on_player.total | 6.97 | 0.72 | 7.0 | 1.0 | 14.0 | 418.0 |
| iframe_avoids.cannon | 0.15 | 0.1 | 0.0 | 0.0 | 2.0 | 9.0 |
| iframe_avoids.claw_heavy | 0.05 | 0.06 | 0.0 | 0.0 | 1.0 | 3.0 |
| iframe_avoids.claw_light | 0.02 | 0.03 | 0.0 | 0.0 | 1.0 | 1.0 |
| iframe_avoids.total | 0.22 | 0.12 | 0.0 | 0.0 | 2.0 | 13.0 |
| opposite_colour_hits_on_enemy | 3.28 | 0.52 | 3.0 | 0.0 | 10.0 | 197.0 |
| opposite_colour_hits_on_player | 3.37 | 0.39 | 3.0 | 0.0 | 7.0 | 202.0 |
| perfect_swaps.attackers_staggered | 0.72 | 0.24 | 0.0 | 0.0 | 4.0 | 43.0 |
| perfect_swaps.attempted | 0.68 | 0.25 | 0.0 | 0.0 | 4.0 | 41.0 |
| perfect_swaps.bolt_negated | 0.12 | 0.1 | 0.0 | 0.0 | 2.0 | 7.0 |
| perfect_swaps.claw_heavy | 0.35 | 0.16 | 0.0 | 0.0 | 3.0 | 21.0 |
| perfect_swaps.claw_light | 0.37 | 0.16 | 0.0 | 0.0 | 2.0 | 22.0 |
| perfect_swaps.damage_avoided | 30.87 | 10.37 | 10.0 | 0.0 | 160.0 | 1852.0 |
| perfect_swaps.energy_refunded | 11.27 | 4.33 | 0.0 | 0.0 | 80.0 | 676.0 |
| perfect_swaps.succeeded | 0.83 | 0.28 | 0.5 | 0.0 | 4.0 | 50.0 |
| perfect_swaps.succeeded_after_attempt | 0.68 | 0.25 | 0.0 | 0.0 | 4.0 | 41.0 |
| player_hp_end | 91.62 | 19.74 | 107.5 | 0.0 | 200.0 | 5497.0 |
| poise_breaks_inflicted | 6.02 | 0.6 | 6.0 | 1.0 | 10.0 | 361.0 |
| poise_breaks_suffered | 2.92 | 0.4 | 3.0 | 0.0 | 7.0 | 175.0 |
| poise_damage_to_enemy | 438.75 | 34.01 | 500.0 | 100.0 | 650.0 | 26325.0 |
| poise_damage_to_player | 443.73 | 49.12 | 415.5 | 59.0 | 873.0 | 26624.0 |
| shots.enemy_fired | 2.67 | 0.4 | 3.0 | 0.0 | 7.0 | 160.0 |
| shots.enemy_hit | 0.37 | 0.16 | 0.0 | 0.0 | 3.0 | 22.0 |
| shots.player_fired | 8.72 | 0.96 | 9.5 | 0.0 | 15.0 | 523.0 |
| shots.player_hit | 4.55 | 0.57 | 5.0 | 0.0 | 9.0 | 273.0 |
| stance_swaps.enemy | 10.13 | 0.82 | 10.0 | 4.0 | 20.0 | 608.0 |
| stance_swaps.player | 12.55 | 0.91 | 13.0 | 5.0 | 20.0 | 753.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken | end |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | enemy_won | 4538 | 0 | 146 | 5 | 3 |  |
| 2 | player_won | 6591 | 143 | 0 | 6 | 2 |  |
| 3 | enemy_won | 8708 | 0 | 28 | 7 | 5 |  |
| 4 | player_won | 7968 | 127 | 0 | 7 | 3 |  |
| 5 | player_won | 6699 | 74 | 0 | 5 | 3 |  |
| 6 | enemy_won | 2340 | 0 | 440 | 2 | 3 |  |
| 7 | player_won | 5163 | 87 | 0 | 6 | 4 |  |
| 8 | enemy_won | 4472 | 0 | 190 | 5 | 3 |  |
| 9 | enemy_won | 5402 | 0 | 148 | 6 | 4 |  |
| 10 | player_won | 7729 | 82 | 0 | 7 | 4 |  |
| 11 | enemy_won | 2496 | 0 | 415 | 2 | 3 |  |
| 12 | enemy_won | 3812 | 0 | 412 | 3 | 3 |  |
| 13 | player_won | 6287 | 185 | 0 | 7 | 2 |  |
| 14 | player_won | 7563 | 144 | 0 | 8 | 2 |  |
| 15 | player_won | 6000 | 179 | 0 | 8 | 2 |  |
| 16 | enemy_won | 7960 | 0 | 79 | 7 | 3 |  |
| 17 | enemy_won | 4970 | 0 | 254 | 5 | 4 |  |
| 18 | player_won | 6985 | 168 | 0 | 7 | 1 |  |
| 19 | player_won | 6633 | 141 | 0 | 6 | 2 |  |
| 20 | enemy_won | 5130 | 0 | 164 | 4 | 3 |  |
| 21 | player_won | 6971 | 105 | 0 | 10 | 1 |  |
| 22 | player_won | 5949 | 152 | 0 | 8 | 2 |  |
| 23 | player_won | 8747 | 40 | 0 | 9 | 6 |  |
| 24 | stalled_no_damage | 8859 | 200 | 6 | 10 | 2 | moving_standoff |
| 25 | player_won | 7728 | 148 | 0 | 6 | 2 |  |
| 26 | player_won | 6374 | 198 | 0 | 8 | 1 |  |
| 27 | enemy_won | 3434 | 0 | 333 | 3 | 4 |  |
| 28 | enemy_won | 4886 | 0 | 206 | 3 | 4 |  |
| 29 | enemy_won | 5968 | 0 | 18 | 8 | 4 |  |
| 30 | player_won | 6556 | 184 | 0 | 9 | 2 |  |
| 31 | player_won | 6351 | 124 | 0 | 8 | 1 |  |
| 32 | enemy_won | 5434 | 0 | 150 | 3 | 5 |  |
| 33 | player_won | 6281 | 152 | 0 | 5 | 2 |  |
| 34 | player_won | 7176 | 60 | 0 | 7 | 4 |  |
| 35 | player_won | 6482 | 156 | 0 | 6 | 2 |  |
| 36 | player_won | 4570 | 200 | 0 | 9 | 0 |  |
| 37 | enemy_won | 6162 | 0 | 247 | 1 | 5 |  |
| 38 | player_won | 7142 | 110 | 0 | 10 | 2 |  |
| 39 | enemy_won | 2502 | 0 | 428 | 1 | 4 |  |
| 40 | player_won | 7709 | 173 | 0 | 8 | 3 |  |
| 41 | enemy_won | 5576 | 0 | 228 | 5 | 4 |  |
| 42 | player_won | 5714 | 164 | 0 | 8 | 2 |  |
| 43 | player_won | 6602 | 164 | 0 | 6 | 3 |  |
| 44 | player_won | 6278 | 138 | 0 | 7 | 1 |  |
| 45 | enemy_won | 3396 | 0 | 353 | 2 | 4 |  |
| 46 | player_won | 8848 | 122 | 0 | 4 | 6 |  |
| 47 | enemy_won | 4864 | 0 | 364 | 3 | 3 |  |
| 48 | player_won | 6419 | 50 | 0 | 7 | 2 |  |
| 49 | player_won | 5547 | 141 | 0 | 7 | 3 |  |
| 50 | player_won | 6063 | 159 | 0 | 8 | 1 |  |
| 51 | player_won | 7901 | 88 | 0 | 7 | 3 |  |
| 52 | player_won | 6520 | 133 | 0 | 6 | 2 |  |
| 53 | player_won | 5955 | 139 | 0 | 8 | 3 |  |
| 54 | player_won | 5177 | 200 | 0 | 8 | 0 |  |
| 55 | enemy_won | 3330 | 0 | 379 | 1 | 4 |  |
| 56 | player_won | 8235 | 200 | 0 | 6 | 4 |  |
| 57 | stalled_no_damage | 4440 | 200 | 318 | 4 | 0 | moving_standoff |
| 58 | enemy_won | 5856 | 0 | 157 | 6 | 7 |  |
| 59 | player_won | 8198 | 104 | 0 | 7 | 7 |  |
| 60 | player_won | 6467 | 163 | 0 | 6 | 1 |  |
