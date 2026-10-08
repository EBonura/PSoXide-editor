# Combat duel batch: before60-graybox

- scenario `graybox`, 60 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60]
- source `119c938880eafc74f8132506d44edc09d7dac93e`, dirty false, polls 11400, disc sha256 `993ffe8d9d28e51c1fb16c2e977b098b1ae0dcd549e472dddc9c4cfdc3078942`
- outcomes {"enemy_won":21,"player_won":36,"stalled_no_damage":3}
- player win rate 0.6 (Wilson 95%: 0.474 to 0.714)
- distinct early streams 58 and whole fights 60 of 60 runs; unfinished-fight causes {"moving_standoff":3}

| metric | mean | 95% CI half-width | median | min | max | sum |
|---|---:|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 3.93 | 0.45 | 4.0 | 1.0 | 8.0 | 236.0 |
| damage_to_enemy.heavy | 226.3 | 18.14 | 246.0 | 47.0 | 360.0 | 13578.0 |
| damage_to_enemy.light | 159.8 | 14.48 | 162.0 | 50.0 | 305.0 | 9588.0 |
| damage_to_enemy.shot | 136.18 | 13.8 | 147.0 | 0.0 | 210.0 | 8171.0 |
| damage_to_enemy.total | 522.28 | 34.77 | 600.0 | 135.0 | 600.0 | 31337.0 |
| damage_to_player.cannon | 9.38 | 4.06 | 0.0 | 0.0 | 75.0 | 563.0 |
| damage_to_player.claw_heavy | 165.43 | 20.03 | 156.0 | 0.0 | 384.0 | 9926.0 |
| damage_to_player.claw_light | 128.87 | 13.54 | 120.0 | 32.0 | 224.0 | 7732.0 |
| damage_to_player.total | 303.68 | 26.08 | 307.5 | 40.0 | 488.0 | 18221.0 |
| duration_ticks | 6680.78 | 415.57 | 6908.0 | 2050.0 | 9855.0 | 400847.0 |
| enemy_evades.attempted | 3.07 | 0.42 | 3.0 | 0.0 | 7.0 | 184.0 |
| enemy_evades.avoided_shot | 2.87 | 0.43 | 3.0 | 0.0 | 6.0 | 172.0 |
| enemy_evades.failed | 0.2 | 0.12 | 0.0 | 0.0 | 2.0 | 12.0 |
| enemy_flinches | 6.52 | 0.58 | 7.0 | 1.0 | 12.0 | 391.0 |
| enemy_hp_end | 77.72 | 34.77 | 0.0 | 0.0 | 465.0 | 4663.0 |
| energy.enemy_gained | 39.87 | 6.43 | 40.0 | 0.0 | 100.0 | 2392.0 |
| energy.enemy_spent | 55.67 | 8.56 | 60.0 | 0.0 | 140.0 | 3340.0 |
| energy.player_gained | 140.27 | 14.03 | 160.0 | 0.0 | 216.0 | 8416.0 |
| energy.player_spent | 175.0 | 17.22 | 190.0 | 0.0 | 280.0 | 10500.0 |
| hits_on_enemy.heavy | 5.88 | 0.49 | 6.0 | 1.0 | 9.0 | 353.0 |
| hits_on_enemy.light | 6.12 | 0.55 | 6.0 | 2.0 | 11.0 | 367.0 |
| hits_on_enemy.shot | 4.75 | 0.48 | 5.0 | 0.0 | 7.0 | 285.0 |
| hits_on_enemy.total | 16.75 | 1.15 | 19.0 | 4.0 | 21.0 | 1005.0 |
| hits_on_player.cannon | 0.4 | 0.17 | 0.0 | 0.0 | 3.0 | 24.0 |
| hits_on_player.claw_heavy | 3.68 | 0.45 | 4.0 | 0.0 | 8.0 | 221.0 |
| hits_on_player.claw_light | 3.93 | 0.45 | 4.0 | 1.0 | 8.0 | 236.0 |
| hits_on_player.total | 8.02 | 0.76 | 8.0 | 1.0 | 14.0 | 481.0 |
| iframe_avoids.cannon | 0.17 | 0.11 | 0.0 | 0.0 | 2.0 | 10.0 |
| iframe_avoids.claw_heavy | 0.2 | 0.11 | 0.0 | 0.0 | 2.0 | 12.0 |
| iframe_avoids.claw_light | 0.12 | 0.08 | 0.0 | 0.0 | 1.0 | 7.0 |
| iframe_avoids.total | 0.48 | 0.19 | 0.0 | 0.0 | 3.0 | 29.0 |
| opposite_colour_hits_on_enemy | 3.35 | 0.64 | 3.0 | 0.0 | 11.0 | 201.0 |
| opposite_colour_hits_on_player | 3.77 | 0.4 | 4.0 | 1.0 | 7.0 | 226.0 |
| perfect_swaps.attackers_staggered | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.attempted | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.bolt_negated | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.claw_heavy | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.claw_light | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.damage_avoided | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.energy_refunded | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.succeeded | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| perfect_swaps.succeeded_after_attempt | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| player_hp_end | 80.6 | 20.03 | 70.5 | 0.0 | 200.0 | 4836.0 |
| poise_breaks_inflicted | 6.52 | 0.58 | 7.0 | 1.0 | 12.0 | 391.0 |
| poise_breaks_suffered | 3.57 | 0.44 | 3.0 | 0.0 | 8.0 | 214.0 |
| poise_damage_to_enemy | 447.08 | 31.92 | 475.0 | 150.0 | 650.0 | 26825.0 |
| poise_damage_to_player | 512.98 | 50.61 | 547.5 | 59.0 | 900.0 | 30779.0 |
| shots.enemy_fired | 2.78 | 0.43 | 3.0 | 0.0 | 7.0 | 167.0 |
| shots.enemy_hit | 0.4 | 0.17 | 0.0 | 0.0 | 3.0 | 24.0 |
| shots.player_fired | 8.75 | 0.86 | 9.5 | 0.0 | 14.0 | 525.0 |
| shots.player_hit | 4.75 | 0.48 | 5.0 | 0.0 | 7.0 | 285.0 |
| stance_swaps.enemy | 10.57 | 0.85 | 10.0 | 4.0 | 18.0 | 634.0 |
| stance_swaps.player | 12.53 | 0.87 | 13.0 | 4.0 | 24.0 | 752.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken | end |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | player_won | 6947 | 5 | 0 | 7 | 5 |  |
| 2 | player_won | 9651 | 200 | 0 | 8 | 4 |  |
| 3 | enemy_won | 6556 | 0 | 53 | 8 | 4 |  |
| 4 | enemy_won | 7682 | 0 | 60 | 7 | 8 |  |
| 5 | enemy_won | 5468 | 0 | 120 | 7 | 3 |  |
| 6 | enemy_won | 2050 | 0 | 453 | 1 | 3 |  |
| 7 | player_won | 7191 | 56 | 0 | 5 | 6 |  |
| 8 | player_won | 5824 | 173 | 0 | 7 | 0 |  |
| 9 | player_won | 6531 | 151 | 0 | 7 | 4 |  |
| 10 | enemy_won | 5906 | 0 | 245 | 6 | 4 |  |
| 11 | enemy_won | 7044 | 0 | 21 | 4 | 5 |  |
| 12 | enemy_won | 3812 | 0 | 412 | 3 | 3 |  |
| 13 | player_won | 8067 | 191 | 0 | 6 | 4 |  |
| 14 | enemy_won | 8804 | 0 | 4 | 8 | 7 |  |
| 15 | player_won | 7791 | 200 | 0 | 7 | 2 |  |
| 16 | player_won | 7345 | 180 | 0 | 8 | 2 |  |
| 17 | player_won | 8906 | 141 | 0 | 10 | 5 |  |
| 18 | enemy_won | 3350 | 0 | 449 | 2 | 5 |  |
| 19 | enemy_won | 6890 | 0 | 7 | 6 | 4 |  |
| 20 | player_won | 7620 | 137 | 0 | 7 | 2 |  |
| 21 | enemy_won | 6890 | 0 | 57 | 7 | 2 |  |
| 22 | player_won | 6807 | 200 | 0 | 9 | 1 |  |
| 23 | player_won | 7849 | 3 | 0 | 9 | 6 |  |
| 24 | enemy_won | 6358 | 0 | 139 | 6 | 4 |  |
| 25 | enemy_won | 3786 | 0 | 301 | 3 | 2 |  |
| 26 | player_won | 9855 | 129 | 0 | 12 | 3 |  |
| 27 | player_won | 9641 | 70 | 0 | 9 | 6 |  |
| 28 | enemy_won | 4188 | 0 | 275 | 3 | 3 |  |
| 29 | player_won | 6435 | 200 | 0 | 6 | 2 |  |
| 30 | player_won | 6062 | 116 | 0 | 9 | 2 |  |
| 31 | player_won | 7468 | 71 | 0 | 7 | 3 |  |
| 32 | player_won | 7840 | 24 | 0 | 7 | 7 |  |
| 33 | player_won | 5775 | 106 | 0 | 6 | 2 |  |
| 34 | player_won | 8703 | 128 | 0 | 7 | 5 |  |
| 35 | enemy_won | 6074 | 0 | 140 | 4 | 3 |  |
| 36 | enemy_won | 5776 | 0 | 55 | 7 | 3 |  |
| 37 | player_won | 7812 | 49 | 0 | 6 | 5 |  |
| 38 | player_won | 7989 | 187 | 0 | 10 | 1 |  |
| 39 | player_won | 5247 | 200 | 0 | 6 | 2 |  |
| 40 | player_won | 6298 | 136 | 0 | 9 | 2 |  |
| 41 | player_won | 6421 | 168 | 0 | 6 | 2 |  |
| 42 | stalled_no_damage | 6519 | 125 | 184 | 6 | 2 | moving_standoff |
| 43 | player_won | 6993 | 110 | 0 | 7 | 3 |  |
| 44 | player_won | 7431 | 49 | 0 | 5 | 4 |  |
| 45 | player_won | 6410 | 100 | 0 | 6 | 3 |  |
| 46 | enemy_won | 4322 | 0 | 353 | 2 | 5 |  |
| 47 | stalled_no_damage | 6939 | 200 | 86 | 8 | 1 | moving_standoff |
| 48 | player_won | 6973 | 14 | 0 | 7 | 3 |  |
| 49 | player_won | 6255 | 168 | 0 | 8 | 3 |  |
| 50 | enemy_won | 3372 | 0 | 465 | 2 | 3 |  |
| 51 | enemy_won | 7356 | 0 | 115 | 7 | 5 |  |
| 52 | player_won | 7828 | 37 | 0 | 9 | 5 |  |
| 53 | player_won | 8030 | 120 | 0 | 7 | 5 |  |
| 54 | enemy_won | 6060 | 0 | 130 | 5 | 4 |  |
| 55 | enemy_won | 4636 | 0 | 221 | 3 | 4 |  |
| 56 | player_won | 8664 | 149 | 0 | 9 | 5 |  |
| 57 | stalled_no_damage | 4440 | 200 | 318 | 4 | 0 | moving_standoff |
| 58 | player_won | 7361 | 160 | 0 | 7 | 2 |  |
| 59 | player_won | 6926 | 87 | 0 | 10 | 6 |  |
| 60 | player_won | 7623 | 96 | 0 | 7 | 5 |  |
