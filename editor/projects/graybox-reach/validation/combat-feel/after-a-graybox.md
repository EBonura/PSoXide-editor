# Combat duel batch: after-a-player-poise-x2

- scenario `graybox`, 20 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]
- source `52c5ebe2ded72a1390ecb2ba43d617f1c8d8890e`, dirty false, polls 11400, disc sha256 `8a4fbee02f0bc4f86a926685569f99805b31638e512fcf36cb53a0951d40efb2`
- outcomes {"enemy_won":4,"player_won":15,"stalled_no_damage":1}

| metric | mean | median | min | max | sum |
|---|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 1.3 | 1.0 | 0.0 | 3.0 | 26.0 |
| claw_light_poise.breaks | 1.05 | 1.0 | 0.0 | 2.0 | 21.0 |
| claw_light_poise.hits | 2.9 | 3.0 | 1.0 | 5.0 | 58.0 |
| damage_to_enemy.heavy | 214.6 | 228.0 | 76.0 | 313.0 | 4292.0 |
| damage_to_enemy.light | 165.0 | 153.0 | 50.0 | 305.0 | 3300.0 |
| damage_to_enemy.shot | 170.95 | 168.0 | 56.0 | 280.0 | 3419.0 |
| damage_to_enemy.total | 550.55 | 600.0 | 232.0 | 600.0 | 11011.0 |
| damage_to_player.cannon | 7.15 | 0.0 | 0.0 | 43.0 | 143.0 |
| damage_to_player.claw_heavy | 153.9 | 150.0 | 48.0 | 310.0 | 3078.0 |
| damage_to_player.claw_light | 96.35 | 87.5 | 32.0 | 184.0 | 1927.0 |
| damage_to_player.total | 257.4 | 256.0 | 100.0 | 440.0 | 5148.0 |
| duration_ticks | 7450.25 | 7576.0 | 5776.0 | 9724.0 | 149005.0 |
| enemy_evades.attempted | 2.8 | 3.0 | 0.0 | 6.0 | 56.0 |
| enemy_evades.avoided_shot | 2.4 | 2.0 | 0.0 | 6.0 | 48.0 |
| enemy_evades.failed | 0.4 | 0.0 | 0.0 | 2.0 | 8.0 |
| enemy_flinches | 6.65 | 6.5 | 3.0 | 10.0 | 133.0 |
| enemy_hp_end | 49.45 | 0.0 | 0.0 | 368.0 | 989.0 |
| energy.enemy_gained | 80.8 | 74.0 | 32.0 | 156.0 | 1616.0 |
| energy.enemy_spent | 110.0 | 100.0 | 60.0 | 160.0 | 2200.0 |
| energy.player_gained | 147.2 | 158.0 | 20.0 | 236.0 | 2944.0 |
| energy.player_spent | 190.0 | 180.0 | 60.0 | 260.0 | 3800.0 |
| hits_on_enemy.heavy | 5.7 | 6.0 | 2.0 | 8.0 | 114.0 |
| hits_on_enemy.light | 6.25 | 6.0 | 2.0 | 11.0 | 125.0 |
| hits_on_enemy.shot | 5.95 | 6.0 | 2.0 | 10.0 | 119.0 |
| hits_on_enemy.total | 17.9 | 19.0 | 8.0 | 21.0 | 358.0 |
| hits_on_player.cannon | 0.3 | 0.0 | 0.0 | 2.0 | 6.0 |
| hits_on_player.claw_heavy | 3.25 | 3.0 | 1.0 | 8.0 | 65.0 |
| hits_on_player.claw_light | 2.9 | 3.0 | 1.0 | 5.0 | 58.0 |
| hits_on_player.total | 6.45 | 6.0 | 2.0 | 13.0 | 129.0 |
| iframe_avoids.cannon | 1.2 | 1.0 | 1.0 | 2.0 | 24.0 |
| iframe_avoids.claw_heavy | 0.25 | 0.0 | 0.0 | 1.0 | 5.0 |
| iframe_avoids.claw_light | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| iframe_avoids.total | 1.45 | 1.0 | 1.0 | 2.0 | 29.0 |
| opposite_colour_hits_on_enemy | 3.6 | 3.0 | 0.0 | 9.0 | 72.0 |
| opposite_colour_hits_on_player | 3.2 | 3.0 | 0.0 | 7.0 | 64.0 |
| player_hp_end | 107.55 | 115.0 | 0.0 | 198.0 | 2151.0 |
| poise_breaks_inflicted | 6.65 | 6.5 | 3.0 | 10.0 | 133.0 |
| poise_breaks_suffered | 4.2 | 4.0 | 1.0 | 9.0 | 84.0 |
| poise_damage_to_enemy | 441.25 | 462.5 | 200.0 | 575.0 | 8825.0 |
| poise_damage_to_player | 575.55 | 550.0 | 200.0 | 1150.0 | 11511.0 |
| shots.enemy_fired | 5.5 | 5.0 | 3.0 | 8.0 | 110.0 |
| shots.enemy_hit | 0.3 | 0.0 | 0.0 | 2.0 | 6.0 |
| shots.player_fired | 9.5 | 9.0 | 3.0 | 13.0 | 190.0 |
| shots.player_hit | 5.95 | 6.0 | 2.0 | 10.0 | 119.0 |
| stance_swaps.enemy | 11.85 | 12.0 | 6.0 | 21.0 | 237.0 |
| stance_swaps.player | 14.2 | 15.0 | 9.0 | 22.0 | 284.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |
|---:|---|---:|---:|---:|---:|---:|
| 1 | enemy_won | 9354 | 0 | 48 | 6 | 9 |
| 2 | player_won | 7623 | 112 | 0 | 10 | 3 |
| 3 | player_won | 8199 | 170 | 0 | 7 | 2 |
| 4 | player_won | 6097 | 168 | 0 | 6 | 1 |
| 5 | player_won | 6901 | 86 | 0 | 9 | 5 |
| 6 | player_won | 7273 | 148 | 0 | 5 | 5 |
| 7 | player_won | 9724 | 88 | 0 | 10 | 8 |
| 8 | enemy_won | 7928 | 0 | 162 | 4 | 5 |
| 9 | player_won | 8207 | 90 | 0 | 7 | 6 |
| 10 | player_won | 6279 | 181 | 0 | 6 | 3 |
| 11 | enemy_won | 8170 | 0 | 221 | 4 | 6 |
| 12 | player_won | 7529 | 118 | 0 | 6 | 4 |
| 13 | stalled_no_damage | 6171 | 153 | 368 | 3 | 4 |
| 14 | player_won | 7361 | 108 | 0 | 6 | 5 |
| 15 | player_won | 7833 | 111 | 0 | 7 | 2 |
| 16 | player_won | 7669 | 133 | 0 | 9 | 4 |
| 17 | enemy_won | 6722 | 0 | 190 | 5 | 4 |
| 18 | player_won | 5871 | 133 | 0 | 7 | 2 |
| 19 | player_won | 8318 | 154 | 0 | 7 | 4 |
| 20 | player_won | 5776 | 198 | 0 | 9 | 2 |
