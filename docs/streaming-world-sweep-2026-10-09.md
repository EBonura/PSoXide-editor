# Stress world sweep, 2026-10-09

What the 50-seed stress world sweep says once the RAM budget is read from a
current link map and the generator fits the drive gate. Run with
`cargo run --release -p psxed-project --bin stream-world-sweep` from `editor/`
(seeds 1 to 50, config `editor/projects/stream-world.config.ron`). Evidence
labels: [M] measured, [D] derived, [E] estimate.

## RAM budget

The 2026-10-08 budget (headroom 17,332 B) predates both animation passes. It is
re-derived from the editor-playtest link map of tree a2d5039d (SDK pin
479d33934, the tree editor main 254f7842 builds):

| input | 10-08 | now | source |
|---|---|---|---|
| static headroom | 17,332 | 332,956 | cap 1,998,848 less image 1,665,892 (`__bss_end` 0x801a6764) [M, link map] |
| baked PXBSP | 90,892 | 93,808 | `PXBSP_WORLD` is 0x16e70 [M, link map] |
| persistent asset arena | 643,072 | 307,200 | 150 pages [M, cooked manifest] |
| heap in use, poll 1200 | 3,764 | 3,764 | not re-measured, needs an emulator run |
| safety floor | 16,384 | 16,384 | [E] |
| pool | 88,076 | 406,616 | [D] |

The headroom grew by 315,624 B: 153,360 B from the dense repair and 181,896 B
from the keyed tracks (the "182 KB" figure), plus rounding in the link. So
`arena_reclaim_bytes` stays 0: the reclaim is already inside the measured
headroom, and adding 182 KB on top of the old 17,332 B would count only the
second pass.

## Result

| run | pool | seeds passing | failures |
|---|---|---|---|
| 10-08 sweep, old RAM, 18,432 B regions | 88 KB | 0 of 50 | 50 pool, 17 rho |
| old headroom plus 182 KB reclaim, 18,432 B regions | 264 KB | 28 of 50 | 8 pool, 17 rho (3 both) |
| link-map budget, 18,432 B regions, 16 x 16 | 397 KB | 33 of 50 | 17 rho |
| link-map budget, 14,336 B regions, terrain fallback, 16 x 16 | 397 KB | 50 of 50 | none |
| same, 20 x 20 | 397 KB | 46 of 50 | 4 rho (seeds 4, 41, 47, 49) |
| same plus the cut-search change, 16 x 16 | 397 KB | 49 of 50 | seed 7 rho (1.13) |
| same plus the cut-search change, 20 x 20 (default) | 397 KB | 49 of 50 | seed 27 rho (1.02) |

Peak requirement at 14,336 B regions: 171 to 238 KB including the skeleton at
16 x 16, 209 to 260 KB at 20 x 20, so the pool gate passes at every pool from
264 KB up. That includes the literal reading of the first pass (old headroom
plus 182 KB, 264 KB pool): the same world passes the pool gate there too. The
rho margin is thin: the worst seed sits at 0.94 to 1.13 of the limit.

The 20 x 20 grid is the default because the design asks the stress world for at
least ten times the pool, and the real pool is now 406 KB: a 16 x 16 world is
2.9 MB (7.2 times), 20 x 20 is 4.6 MB (11 times). The per-seed table of the
default run (20 x 20, seeds 1 to 50) is at the end.

## Why rho failed

At 18,432 B regions all 17 failures were the drive gate. The gate prices a
crossing at the median region's sector count (B_eff from 8 sectors, limit 92.5
B per unit) and compares the bytes the crossing newly needs over its length.

* 10 of 17: the destination region sees a hook room within hook range, so the
  hook landing's closure (two regions) joins the requirement: two visible
  regions and two pulled ones, 74 to 100 KB over 770 units.
* 7 of 17: entering a junction whose three neighbours are all new, with
  regions at the heavy end (22 to 25 KB against a 15 KB median).

At 14,336 B (seven sectors) the heavy end drops to about 20 KB and the same
crossings fit. Eight and a half to nine sectors is the cliff: 15,360 B
and 16,384 B still failed 5 of 20 and 4 of 20 seeds, 12,288 B failed 17 of 20
because 90 modules no longer fit the target and were cut in two (a cut module
is seen by more regions).

## What was left after the target change

* Terrain modules. The generator built the finest terrain patch that fits and
  the finest was two cells; a terrain module with three doors does not fit
  14,336 B even then, so the cooker cut it in two and the halves saw 6 or 7
  regions. Terrain now falls back to one cell, then to a flat court (counted as
  trimmed). Three of the first 39 seeds at 16 x 16 failed on this alone.
* The cut search thins its candidate planes by an even spread once there are
  more than 32, which drops the planes between two modules (they split no face)
  and can place the cut inside a module, leaving a sliver region whose
  neighbours all see it. Thinning now keeps the candidates that split the
  fewest faces and spreads only the last tier. Graybox Reach cooks to the same
  five regions with both.

## What is still failing

Both remaining seeds are a module cut inside its walls, not a module over its
budget. A scratch check on 16 x 16 seed 7 found every module's cut-search bytes
at 12.8 KB or less against the 14,336 B target. The cut tree places a plane
near a door wall, the sliver beside it (6.5 KB) is seen by all its neighbours,
and one crossing brings in 46 KB over 480 units (94.8 B/u against 83.8). The
cut-search change moves which seeds this happens to (it fixed 20 x 20 seed 4
and broke 16 x 16 seed 7) and nets two seeds out of a hundred. The cause to fix
is that the tree is top-down and picks the first plane by open area, so a
plane that is not on a module boundary can win; cutting along the module grid
first, or scoring a sliver, would remove the class. Not done here: it changes
every partition.

## Not done

* The heap in use is the 10-08 emulator figure; it needs an emulator run on a
  current build.
* Generated routes are not verified walkable.
* M7's guest-side numbers (stream code 24.7 KB, install scratch) are not in the
  safety floor.

## Default run, per seed

```
# stress world sweep: grid 20x20, module 768 units, cut target 14336 B, detail 1024 B, door degree <= 3, seeds 1..50
# pool 406616 B = 332956 headroom - 3764 heap in use + 93808 baked PXBSP no longer baked + 0 arena reclaimed - 16384 safety floor = 406616 B
# skeleton for 400 regions: 40828 B [D]; available for pages 365788 B
# union of 23 regions (door degree 3): fits regions of 15903 B; regions of 14336 B need a pool of 370556 B, 0 B more than the RAM gives
seed regions payload_KB max|V| ball  rho/limit(B/u)  peak/avail(KB)  window(B,x)  region max/med(B)  failures
   1     399       5130     5   13    65.8/83.8       177/361      30680,1   17871/13371   PASS
   2     403       5185     5   14    66.1/83.8       195/361      30925,1   18969/13397   PASS
   3     398       5147     5   13    66.4/83.8       182/362      51068,1   20806/13534   PASS
   4     402       5246     5   14    74.6/83.8       196/361      79487,1   19827/13601   PASS
   5     401       5228     5   14    71.1/83.8       207/361      34282,1   18906/13575   PASS
   6     403       5170     5   14    68.8/83.8       197/361      64908,1   17658/13407   PASS
   7     402       5163     5   15    70.0/83.8       202/361      66433,1   19101/13491   PASS
   8     402       5232     6   16    79.5/83.8       218/361      57755,1   18137/13540   PASS
   9     400       5145     5   13    68.1/83.8       178/361      31845,1   20659/13340   PASS
  10     399       5181     5   13    60.9/83.8       193/362      47573,1   17998/13466   PASS
  11     401       5153     5   14    61.0/83.8       192/361      64075,1   18450/13389   PASS
  12     404       5196     5   14    73.4/83.8       182/361      79869,1   17346/13408   PASS
  13     403       5278     5   14    70.4/83.8       194/361      75668,1   18000/13578   PASS
  14     402       5202     5   14    81.2/83.8       197/361      37868,1   20562/13399   PASS
  15     398       5210     4   13    63.2/83.8       182/362      68025,1   18788/13491   PASS
  16     398       5140     6   14    77.5/83.8       192/362      36148,1   19331/13473   PASS
  17     401       5224     6   14    65.0/83.8       194/361      31424,1   17942/13519   PASS
  18     400       5121     5   15    70.1/83.8       224/361      30122,1   21551/13232   PASS
  19     397       5164     5   13    72.0/83.8       180/362      78979,1   20107/13407   PASS
  20     400       5157     5   13    78.4/83.8       189/361      85566,1   18607/13381   PASS
  21     400       5221     5   13    68.8/83.8       192/361      65275,1   20231/13591   PASS
  22     401       5267     5   14    77.4/83.8       210/361      83993,1   19247/13578   PASS
  23     400       5163     5   13    66.1/83.8       180/361      31852,1   20410/13418   PASS
  24     405       5243     5   14    70.9/83.8       190/361      34252,1   18910/13537   PASS
  25     400       5201     5   15    75.5/83.8       212/361      71354,1   19431/13605   PASS
  26     399       5175     5   13    68.8/83.8       189/361      32071,1   17109/13523   PASS
  27     400       5215     5   14    85.2/83.8       184/361      65136,1   20127/13499   rho
  28     400       5147     5   14    73.6/83.8       201/361      79106,1   18435/13400   PASS
  29     397       5179     5   13    67.5/83.8       192/362      63672,1   19134/13577   PASS
  30     401       5176     5   14    66.9/83.8       185/361      31296,1   20611/13443   PASS
  31     396       5116     5   14    70.2/83.8       187/362      76231,1   19887/13434   PASS
  32     402       5175     5   14    74.0/83.8       196/361      67710,1   18570/13421   PASS
  33     400       5158     5   14    63.6/83.8       191/361      66624,1   19050/13393   PASS
  34     402       5223     5   14    65.6/83.8       188/361      61133,1   20001/13501   PASS
  35     400       5086     5   13    65.5/83.8       173/361      30641,1   19058/13308   PASS
  36     401       5244     5   14    65.9/83.8       202/361      50661,1   17708/13554   PASS
  37     399       5180     5   13    68.0/83.8       184/362      72958,1   19342/13433   PASS
  38     401       5207     5   14    73.2/83.8       196/361      68585,1   18949/13424   PASS
  39     397       5217     5   15    63.9/83.8       220/362      48927,1   19767/13638   PASS
  40     403       5175     5   14    67.2/83.8       192/361      31340,1   18032/13426   PASS
  41     400       5231     5   14    63.7/83.8       193/361      30680,1   18729/13576   PASS
  42     404       5186     5   14    61.7/83.8       192/361      28743,1   18641/13490   PASS
  43     402       5193     6   15    75.2/83.8       200/361      81674,1   20047/13435   PASS
  44     398       5141     5   14    76.8/83.8       190/362      35801,1   20355/13408   PASS
  45     398       5180     5   14    66.4/83.8       196/361      32002,1   22250/13419   PASS
  46     404       5162     5   14    72.6/83.8       185/361      68450,1   19365/13397   PASS
  47     398       5177     6   14    80.5/83.8       179/362      38950,1   18581/13458   PASS
  48     399       5166     5   13    69.9/83.8       184/361      65206,1   22812/13519   PASS
  49     401       5164     5   13    66.4/83.8       179/361      32091,1   17984/13376   PASS
  50     400       5183     5   13    67.3/83.8       188/361      63887,1   19057/13434   PASS

# 49 of 50 seeds pass every gate at this pool
#   1 of 50 fail the rho gate
# pool a seed needs (peak requirement + skeleton), KB: min 209, median 227, p90 243, max 260
# rho / limit: min 0.73, median 0.82, max 1.02; seeds under 1.0: 49
# mean partition + cook time 48.2s per seed

# pass rate at other pool sizes (the other gates as measured):
# pool_KB pass_rate
#      86    0/50 = 0%
#     146    0/50 = 0%
#     195    0/50 = 0%
#     264   49/50 = 98%
#     293   49/50 = 98%
#     342   49/50 = 98%
#     397   49/50 = 98%
```
