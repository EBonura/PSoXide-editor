# Combat duel batch: baseline-graybox

- scenario `graybox`, 20 runs (0 failed), seeds [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]
- source `eb0c6898fb2054f4458b1fa44329158fe129e491`, dirty false, polls 11400, disc sha256 `cc10b8d908b80ecf88a06428476e6bf9774747845225d921d77d8aba71709a21`
- outcomes {"enemy_won":5,"player_won":12,"stalled_no_damage":3}

| metric | mean | median | min | max | sum |
|---|---:|---:|---:|---:|---:|
| claw_light_poise.alone_capable | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.breaks | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| claw_light_poise.hits | 3.2 | 3.0 | 0.0 | 7.0 | 64.0 |
| damage_to_enemy.heavy | 230.05 | 246.0 | 76.0 | 304.0 | 4601.0 |
| damage_to_enemy.light | 152.6 | 160.5 | 50.0 | 261.0 | 3052.0 |
| damage_to_enemy.shot | 134.4 | 147.5 | 28.0 | 224.0 | 2688.0 |
| damage_to_enemy.total | 517.05 | 600.0 | 166.0 | 600.0 | 10341.0 |
| damage_to_player.cannon | 9.75 | 0.0 | 0.0 | 50.0 | 195.0 |
| damage_to_player.claw_heavy | 151.25 | 144.5 | 48.0 | 311.0 | 3025.0 |
| damage_to_player.claw_light | 94.3 | 96.0 | 0.0 | 197.0 | 1886.0 |
| damage_to_player.total | 255.3 | 291.0 | 48.0 | 441.0 | 5106.0 |
| duration_ticks | 7311.7 | 7490.0 | 4450.0 | 9219.0 | 146234.0 |
| enemy_evades.attempted | 2.95 | 3.0 | 0.0 | 5.0 | 59.0 |
| enemy_evades.avoided_shot | 2.75 | 3.0 | 0.0 | 5.0 | 55.0 |
| enemy_evades.failed | 0.2 | 0.0 | 0.0 | 1.0 | 4.0 |
| enemy_flinches | 8.15 | 9.0 | 3.0 | 11.0 | 163.0 |
| enemy_hp_end | 82.95 | 0.0 | 0.0 | 434.0 | 1659.0 |
| energy.enemy_gained | 80.4 | 80.0 | 20.0 | 160.0 | 1608.0 |
| energy.enemy_spent | 103.0 | 100.0 | 60.0 | 160.0 | 2060.0 |
| energy.player_gained | 141.4 | 158.0 | 20.0 | 208.0 | 2828.0 |
| energy.player_spent | 173.0 | 190.0 | 20.0 | 240.0 | 3460.0 |
| hits_on_enemy.heavy | 6.05 | 7.0 | 2.0 | 8.0 | 121.0 |
| hits_on_enemy.light | 5.7 | 6.0 | 2.0 | 9.0 | 114.0 |
| hits_on_enemy.shot | 4.75 | 5.5 | 1.0 | 8.0 | 95.0 |
| hits_on_enemy.total | 16.5 | 19.0 | 5.0 | 20.0 | 330.0 |
| hits_on_player.cannon | 0.4 | 0.0 | 0.0 | 2.0 | 8.0 |
| hits_on_player.claw_heavy | 3.4 | 3.5 | 1.0 | 7.0 | 68.0 |
| hits_on_player.claw_light | 3.2 | 3.0 | 0.0 | 7.0 | 64.0 |
| hits_on_player.total | 7.0 | 8.0 | 1.0 | 12.0 | 140.0 |
| iframe_avoids.cannon | 1.1 | 1.0 | 1.0 | 2.0 | 22.0 |
| iframe_avoids.claw_heavy | 0.05 | 0.0 | 0.0 | 1.0 | 1.0 |
| iframe_avoids.claw_light | 0.1 | 0.0 | 0.0 | 1.0 | 2.0 |
| iframe_avoids.total | 1.25 | 1.0 | 1.0 | 2.0 | 25.0 |
| opposite_colour_hits_on_enemy | 3.15 | 3.0 | 0.0 | 7.0 | 63.0 |
| opposite_colour_hits_on_player | 3.85 | 4.0 | 1.0 | 9.0 | 77.0 |
| player_hp_end | 102.7 | 117.0 | 0.0 | 200.0 | 2054.0 |
| poise_breaks_inflicted | 8.15 | 9.0 | 3.0 | 11.0 | 163.0 |
| poise_breaks_suffered | 3.3 | 3.0 | 1.0 | 7.0 | 66.0 |
| poise_damage_to_enemy | 445.0 | 487.5 | 150.0 | 575.0 | 8900.0 |
| poise_damage_to_player | 417.4 | 421.5 | 75.0 | 731.0 | 8348.0 |
| shots.enemy_fired | 5.15 | 5.0 | 3.0 | 8.0 | 103.0 |
| shots.enemy_hit | 0.4 | 0.0 | 0.0 | 2.0 | 8.0 |
| shots.player_fired | 8.65 | 9.5 | 1.0 | 12.0 | 173.0 |
| shots.player_hit | 4.75 | 5.5 | 1.0 | 8.0 | 95.0 |
| stance_swaps.enemy | 10.75 | 11.0 | 5.0 | 15.0 | 215.0 |
| stance_swaps.player | 14.1 | 14.5 | 7.0 | 19.0 | 282.0 |

| seed | outcome | ticks | player hp | enemy hp | breaks given | breaks taken |
|---:|---|---:|---:|---:|---:|---:|
| 1 | player_won | 8964 | 65 | 0 | 10 | 2 |
| 2 | player_won | 9219 | 111 | 0 | 9 | 5 |
| 3 | player_won | 8030 | 22 | 0 | 8 | 4 |
| 4 | player_won | 6062 | 200 | 0 | 7 | 1 |
| 5 | player_won | 6770 | 200 | 0 | 8 | 3 |
| 6 | enemy_won | 5886 | 0 | 270 | 6 | 5 |
| 7 | player_won | 7779 | 123 | 0 | 11 | 5 |
| 8 | player_won | 6806 | 125 | 0 | 10 | 1 |
| 9 | stalled_no_damage | 8242 | 184 | 149 | 9 | 2 |
| 10 | stalled_no_damage | 6363 | 200 | 56 | 8 | 1 |
| 11 | enemy_won | 8266 | 0 | 221 | 5 | 5 |
| 12 | player_won | 6554 | 181 | 0 | 9 | 1 |
| 13 | enemy_won | 5548 | 0 | 346 | 5 | 3 |
| 14 | enemy_won | 6446 | 0 | 132 | 7 | 6 |
| 15 | player_won | 9025 | 48 | 0 | 10 | 4 |
| 16 | player_won | 7334 | 200 | 0 | 10 | 2 |
| 17 | enemy_won | 8524 | 0 | 51 | 9 | 7 |
| 18 | player_won | 7646 | 142 | 0 | 10 | 3 |
| 19 | player_won | 8320 | 53 | 0 | 9 | 5 |
| 20 | stalled_no_damage | 4450 | 200 | 434 | 3 | 1 |
