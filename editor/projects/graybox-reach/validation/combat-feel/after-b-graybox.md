# Combat duel batch: after-b-player-poise-x1.19

- scenario `graybox`, 20 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]
- source `1f6a3ce604b50f9c10c09623aaa0d298bcd9fe6f`, dirty false, polls 11400, disc sha256 `da0f4e9bf98bf8cd4da31e9ccaec4212c8f1ed03047845672caa6bfb4ea6f41f`
- outcomes {"enemy_won":9,"player_won":10,"stalled_no_damage":1}

| metric | mean | median | min | max | sum |
|---|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 2.85 | 3.0 | 1.0 | 5.0 | 57.0 |
| damage_to_enemy.heavy | 197.6 | 211.0 | 38.0 | 295.0 | 3952.0 |
| damage_to_enemy.light | 143.1 | 140.0 | 50.0 | 237.0 | 2862.0 |
| damage_to_enemy.shot | 156.1 | 171.5 | 28.0 | 280.0 | 3122.0 |
| damage_to_enemy.total | 496.8 | 599.5 | 166.0 | 600.0 | 9936.0 |
| damage_to_player.cannon | 8.4 | 0.0 | 0.0 | 43.0 | 168.0 |
| damage_to_player.claw_heavy | 164.5 | 175.0 | 60.0 | 275.0 | 3290.0 |
| damage_to_player.claw_light | 93.05 | 88.5 | 32.0 | 194.0 | 1861.0 |
| damage_to_player.total | 265.95 | 271.0 | 100.0 | 410.0 | 5319.0 |
| duration_ticks | 6881.0 | 7191.0 | 3958.0 | 8307.0 | 137620.0 |
| enemy_evades.attempted | 2.75 | 3.0 | 0.0 | 6.0 | 55.0 |
| enemy_evades.avoided_shot | 2.3 | 2.0 | 0.0 | 6.0 | 46.0 |
| enemy_evades.failed | 0.45 | 0.0 | 0.0 | 2.0 | 9.0 |
| enemy_flinches | 5.8 | 6.0 | 1.0 | 9.0 | 116.0 |
| enemy_hp_end | 103.2 | 0.5 | 0.0 | 434.0 | 2064.0 |
| energy.enemy_gained | 85.0 | 80.0 | 32.0 | 156.0 | 1700.0 |
| energy.enemy_spent | 111.0 | 110.0 | 60.0 | 160.0 | 2220.0 |
| energy.player_gained | 136.4 | 164.0 | 20.0 | 220.0 | 2728.0 |
| energy.player_spent | 171.0 | 180.0 | 20.0 | 260.0 | 3420.0 |
| hits_on_enemy.heavy | 5.25 | 6.0 | 1.0 | 8.0 | 105.0 |
| hits_on_enemy.light | 5.35 | 5.0 | 2.0 | 9.0 | 107.0 |
| hits_on_enemy.shot | 5.4 | 6.0 | 1.0 | 10.0 | 108.0 |
| hits_on_enemy.total | 16.0 | 18.0 | 5.0 | 21.0 | 320.0 |
| hits_on_player.cannon | 0.35 | 0.0 | 0.0 | 2.0 | 7.0 |
| hits_on_player.claw_heavy | 3.45 | 3.5 | 1.0 | 6.0 | 69.0 |
| hits_on_player.claw_light | 2.85 | 3.0 | 1.0 | 5.0 | 57.0 |
| hits_on_player.total | 6.65 | 6.5 | 2.0 | 10.0 | 133.0 |
| iframe_avoids.cannon | 1.15 | 1.0 | 1.0 | 2.0 | 23.0 |
| iframe_avoids.claw_heavy | 0.45 | 0.0 | 0.0 | 2.0 | 9.0 |
| iframe_avoids.claw_light | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.total | 1.6 | 1.5 | 1.0 | 3.0 | 32.0 |
| opposite_colour_hits_on_enemy | 3.1 | 2.0 | 0.0 | 9.0 | 62.0 |
| opposite_colour_hits_on_player | 3.4 | 3.0 | 0.0 | 6.0 | 68.0 |
| player_hp_end | 80.75 | 71.0 | 0.0 | 200.0 | 1615.0 |
| poise_breaks_inflicted | 5.8 | 6.0 | 1.0 | 9.0 | 116.0 |
| poise_breaks_suffered | 3.2 | 3.0 | 1.0 | 6.0 | 64.0 |
| poise_damage_to_enemy | 396.25 | 437.5 | 150.0 | 525.0 | 7925.0 |
| poise_damage_to_player | 438.8 | 444.0 | 148.0 | 719.0 | 8776.0 |
| shots.enemy_fired | 5.55 | 5.5 | 3.0 | 8.0 | 111.0 |
| shots.enemy_hit | 0.35 | 0.0 | 0.0 | 2.0 | 7.0 |
| shots.player_fired | 8.55 | 9.0 | 1.0 | 13.0 | 171.0 |
| shots.player_hit | 5.4 | 6.0 | 1.0 | 10.0 | 108.0 |
| stance_swaps.enemy | 10.8 | 11.0 | 5.0 | 16.0 | 216.0 |
| stance_swaps.player | 13.3 | 14.0 | 7.0 | 17.0 | 266.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |
|---:|---|---:|---:|---:|---:|---:|
| 1 | enemy_won | 6704 | 0 | 164 | 4 | 5 |
| 2 | enemy_won | 7252 | 0 | 42 | 9 | 2 |
| 3 | player_won | 8199 | 170 | 0 | 7 | 2 |
| 4 | player_won | 6097 | 168 | 0 | 6 | 1 |
| 5 | player_won | 7561 | 152 | 0 | 9 | 4 |
| 6 | enemy_won | 7216 | 0 | 48 | 5 | 5 |
| 7 | enemy_won | 7166 | 0 | 202 | 6 | 5 |
| 8 | enemy_won | 5298 | 0 | 328 | 2 | 1 |
| 9 | player_won | 8307 | 21 | 0 | 7 | 6 |
| 10 | player_won | 7440 | 189 | 0 | 7 | 1 |
| 11 | enemy_won | 8170 | 0 | 221 | 4 | 6 |
| 12 | player_won | 8270 | 134 | 0 | 6 | 3 |
| 13 | enemy_won | 3958 | 0 | 434 | 1 | 3 |
| 14 | enemy_won | 7788 | 0 | 1 | 6 | 6 |
| 15 | player_won | 7070 | 200 | 0 | 8 | 1 |
| 16 | player_won | 7852 | 121 | 0 | 9 | 4 |
| 17 | enemy_won | 6722 | 0 | 190 | 5 | 4 |
| 18 | player_won | 5871 | 133 | 0 | 7 | 2 |
| 19 | player_won | 6229 | 127 | 0 | 5 | 2 |
| 20 | stalled_no_damage | 4450 | 200 | 434 | 3 | 1 |
