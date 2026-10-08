# Combat duel batch: after-b-heavy

- scenario `heavy`, 20 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]
- source `1f6a3ce604b50f9c10c09623aaa0d298bcd9fe6f`, dirty false, polls 11400, disc sha256 `178a40699431e6aa7b0cb4619c4b060de33dc20c81b17c9ed0ac581d986c9635`
- outcomes {"enemy_won":6,"player_won":7,"stalled_no_damage":7}

| metric | mean | median | min | max | sum |
|---|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 3.45 | 4.0 | 0.0 | 7.0 | 69.0 |
| damage_to_enemy.heavy | 184.1 | 199.0 | 76.0 | 313.0 | 3682.0 |
| damage_to_enemy.light | 115.0 | 125.0 | 50.0 | 181.0 | 2300.0 |
| damage_to_enemy.shot | 4.9 | 0.0 | 0.0 | 35.0 | 98.0 |
| damage_to_enemy.total | 304.0 | 380.0 | 126.0 | 400.0 | 6080.0 |
| damage_to_player.cannon | 69.35 | 66.5 | 35.0 | 148.0 | 1387.0 |
| damage_to_player.claw_heavy | 95.75 | 92.5 | 0.0 | 202.0 | 1915.0 |
| damage_to_player.claw_light | 90.3 | 106.0 | 0.0 | 188.0 | 1806.0 |
| damage_to_player.total | 255.4 | 272.0 | 35.0 | 454.0 | 5108.0 |
| duration_ticks | 6091.85 | 6845.0 | 3122.0 | 8879.0 | 121837.0 |
| enemy_evades.attempted | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_evades.avoided_shot | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_evades.failed | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| enemy_flinches | 5.35 | 6.0 | 2.0 | 8.0 | 107.0 |
| enemy_hp_end | 96.0 | 20.0 | 0.0 | 274.0 | 1920.0 |
| energy.enemy_gained | 96.2 | 100.0 | 0.0 | 192.0 | 1924.0 |
| energy.enemy_spent | 156.0 | 170.0 | 100.0 | 200.0 | 3120.0 |
| energy.player_gained | 148.6 | 170.0 | 64.0 | 220.0 | 2972.0 |
| energy.player_spent | 195.0 | 220.0 | 100.0 | 280.0 | 3900.0 |
| hits_on_enemy.heavy | 4.85 | 5.0 | 2.0 | 8.0 | 97.0 |
| hits_on_enemy.light | 4.55 | 5.0 | 2.0 | 7.0 | 91.0 |
| hits_on_enemy.shot | 0.15 | 0.0 | 0.0 | 1.0 | 3.0 |
| hits_on_enemy.total | 9.55 | 12.0 | 4.0 | 13.0 | 191.0 |
| hits_on_player.cannon | 2.25 | 2.0 | 1.0 | 5.0 | 45.0 |
| hits_on_player.claw_heavy | 2.95 | 3.0 | 0.0 | 7.0 | 59.0 |
| hits_on_player.claw_light | 3.45 | 4.0 | 0.0 | 7.0 | 69.0 |
| hits_on_player.total | 8.65 | 9.5 | 1.0 | 15.0 | 173.0 |
| iframe_avoids.cannon | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.claw_heavy | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.claw_light | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.total | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| opposite_colour_hits_on_enemy | 1.3 | 1.0 | 0.0 | 5.0 | 26.0 |
| opposite_colour_hits_on_player | 3.05 | 3.5 | 1.0 | 6.0 | 61.0 |
| player_hp_end | 94.7 | 87.0 | 0.0 | 200.0 | 1894.0 |
| poise_breaks_inflicted | 5.35 | 6.0 | 2.0 | 8.0 | 107.0 |
| poise_breaks_suffered | 0.6 | 0.5 | 0.0 | 3.0 | 12.0 |
| poise_damage_to_enemy | 356.25 | 425.0 | 150.0 | 525.0 | 7125.0 |
| poise_damage_to_player | 267.05 | 357.0 | 10.0 | 501.0 | 5341.0 |
| shots.enemy_fired | 7.8 | 8.5 | 5.0 | 10.0 | 156.0 |
| shots.enemy_hit | 2.25 | 2.0 | 1.0 | 5.0 | 45.0 |
| shots.player_fired | 9.75 | 11.0 | 5.0 | 14.0 | 195.0 |
| shots.player_hit | 0.15 | 0.0 | 0.0 | 1.0 | 3.0 |
| stance_swaps.enemy | 9.8 | 12.0 | 4.0 | 20.0 | 196.0 |
| stance_swaps.player | 11.5 | 13.0 | 3.0 | 19.0 | 230.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |
|---:|---|---:|---:|---:|---:|---:|
| 1 | stalled_no_damage | 3122 | 193 | 274 | 2 | 0 |
| 2 | stalled_no_damage | 3275 | 179 | 268 | 3 | 0 |
| 3 | enemy_won | 7786 | 0 | 20 | 6 | 1 |
| 4 | enemy_won | 7872 | 0 | 7 | 7 | 1 |
| 5 | enemy_won | 4374 | 0 | 148 | 5 | 3 |
| 6 | enemy_won | 7872 | 0 | 7 | 7 | 1 |
| 7 | player_won | 6793 | 150 | 0 | 8 | 0 |
| 8 | player_won | 7366 | 69 | 0 | 7 | 1 |
| 9 | player_won | 6410 | 200 | 0 | 8 | 1 |
| 10 | player_won | 6897 | 15 | 0 | 6 | 1 |
| 11 | stalled_no_damage | 3122 | 193 | 274 | 2 | 0 |
| 12 | enemy_won | 7322 | 0 | 148 | 4 | 0 |
| 13 | stalled_no_damage | 3275 | 179 | 268 | 3 | 0 |
| 14 | stalled_no_damage | 3275 | 179 | 268 | 3 | 0 |
| 15 | player_won | 8879 | 56 | 0 | 6 | 0 |
| 16 | enemy_won | 7786 | 0 | 20 | 6 | 1 |
| 17 | stalled_no_damage | 6661 | 105 | 85 | 6 | 1 |
| 18 | player_won | 7161 | 24 | 0 | 6 | 1 |
| 19 | stalled_no_damage | 5006 | 200 | 133 | 5 | 0 |
| 20 | player_won | 7583 | 152 | 0 | 7 | 0 |
