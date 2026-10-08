# Combat duel batch: baseline-heavy

- scenario `heavy`, 20 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]
- source `bdf245901387ed3c36ef50348bf204ad62c53eea`, dirty false, polls 11400, disc sha256 `92bf9d5dcf430eadefa0c4f8fab1b36b44123571c52ee7c373345196f1a649b2`
- outcomes {"enemy_won":5,"player_won":8,"stalled_no_damage":5,"timeout":2}

| metric | mean | median | min | max | sum |
|---|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 3.05 | 2.5 | 0.0 | 8.0 | 61.0 |
| damage_to_enemy.heavy | 177.55 | 201.5 | 76.0 | 313.0 | 3551.0 |
| damage_to_enemy.light | 108.95 | 106.0 | 50.0 | 168.0 | 2179.0 |
| damage_to_enemy.shot | 12.25 | 0.0 | 0.0 | 35.0 | 245.0 |
| damage_to_enemy.total | 298.75 | 365.0 | 126.0 | 400.0 | 5975.0 |
| damage_to_player.cannon | 70.8 | 63.0 | 35.0 | 148.0 | 1416.0 |
| damage_to_player.claw_heavy | 104.1 | 115.0 | 0.0 | 207.0 | 2082.0 |
| damage_to_player.claw_light | 76.1 | 62.5 | 0.0 | 197.0 | 1522.0 |
| damage_to_player.total | 251.0 | 279.5 | 35.0 | 454.0 | 5020.0 |
| duration_ticks | 6182.35 | 6357.0 | 3122.0 | 10800.0 | 123647.0 |
| enemy_evades.attempted | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_evades.avoided_shot | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_evades.failed | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_flinches | 5.0 | 5.5 | 2.0 | 8.0 | 100.0 |
| enemy_hp_end | 101.25 | 35.0 | 0.0 | 274.0 | 2025.0 |
| energy.enemy_gained | 94.8 | 100.0 | 0.0 | 180.0 | 1896.0 |
| energy.enemy_spent | 151.0 | 160.0 | 100.0 | 240.0 | 3020.0 |
| energy.player_gained | 142.4 | 156.0 | 64.0 | 220.0 | 2848.0 |
| energy.player_spent | 183.0 | 200.0 | 100.0 | 280.0 | 3660.0 |
| hits_on_enemy.heavy | 4.7 | 5.5 | 2.0 | 8.0 | 94.0 |
| hits_on_enemy.light | 4.25 | 4.0 | 2.0 | 6.0 | 85.0 |
| hits_on_enemy.shot | 0.4 | 0.0 | 0.0 | 1.0 | 8.0 |
| hits_on_enemy.total | 9.35 | 11.0 | 4.0 | 13.0 | 187.0 |
| hits_on_player.cannon | 2.3 | 2.0 | 1.0 | 5.0 | 46.0 |
| hits_on_player.claw_heavy | 3.4 | 4.0 | 0.0 | 7.0 | 68.0 |
| hits_on_player.claw_light | 3.05 | 2.5 | 0.0 | 8.0 | 61.0 |
| hits_on_player.total | 8.75 | 9.0 | 1.0 | 16.0 | 175.0 |
| iframe_avoids.cannon | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.claw_heavy | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.claw_light | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.total | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| opposite_colour_hits_on_enemy | 1.35 | 1.0 | 0.0 | 5.0 | 27.0 |
| opposite_colour_hits_on_player | 4.7 | 3.5 | 1.0 | 11.0 | 94.0 |
| player_hp_end | 101.65 | 125.0 | 0.0 | 200.0 | 2033.0 |
| poise_breaks_inflicted | 5.0 | 5.5 | 2.0 | 8.0 | 100.0 |
| poise_breaks_suffered | 0.6 | 0.5 | 0.0 | 3.0 | 12.0 |
| poise_damage_to_enemy | 341.25 | 387.5 | 150.0 | 525.0 | 6825.0 |
| poise_damage_to_player | 269.25 | 307.5 | 10.0 | 515.0 | 5385.0 |
| shots.enemy_fired | 7.55 | 8.0 | 5.0 | 12.0 | 151.0 |
| shots.enemy_hit | 2.3 | 2.0 | 1.0 | 5.0 | 46.0 |
| shots.player_fired | 9.15 | 10.0 | 5.0 | 14.0 | 183.0 |
| shots.player_hit | 0.4 | 0.0 | 0.0 | 1.0 | 8.0 |
| stance_swaps.enemy | 9.65 | 9.0 | 4.0 | 20.0 | 193.0 |
| stance_swaps.player | 12.0 | 13.0 | 3.0 | 26.0 | 240.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |
|---:|---|---:|---:|---:|---:|---:|
| 1 | stalled_no_damage | 3122 | 193 | 274 | 2 | 0 |
| 2 | stalled_no_damage | 3275 | 178 | 268 | 2 | 0 |
| 3 | player_won | 6411 | 125 | 0 | 6 | 1 |
| 4 | player_won | 6121 | 166 | 0 | 6 | 0 |
| 5 | enemy_won | 4374 | 0 | 148 | 5 | 3 |
| 6 | enemy_won | 6304 | 0 | 92 | 5 | 1 |
| 7 | player_won | 5855 | 151 | 0 | 8 | 0 |
| 8 | timeout | 10800 | 75 | 29 | 7 | 1 |
| 9 | player_won | 6410 | 200 | 0 | 8 | 1 |
| 10 | player_won | 7449 | 1 | 0 | 6 | 1 |
| 11 | stalled_no_damage | 3122 | 193 | 274 | 2 | 0 |
| 12 | enemy_won | 7322 | 0 | 148 | 4 | 0 |
| 13 | stalled_no_damage | 3275 | 178 | 268 | 2 | 0 |
| 14 | stalled_no_damage | 3275 | 178 | 268 | 2 | 0 |
| 15 | player_won | 8879 | 68 | 0 | 6 | 0 |
| 16 | timeout | 10800 | 75 | 29 | 7 | 1 |
| 17 | enemy_won | 4164 | 0 | 186 | 4 | 1 |
| 18 | enemy_won | 7004 | 0 | 41 | 5 | 1 |
| 19 | player_won | 7046 | 127 | 0 | 7 | 0 |
| 20 | player_won | 8639 | 125 | 0 | 6 | 1 |
