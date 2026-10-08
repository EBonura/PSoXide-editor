# Combat feel: poise baseline and after

Batches of 20 seeded duels (seeds 1 to 20, polls 11400, two emulators at a time) taken with `duel-batch`. Each `*.json` carries the full contract: source revision, dirty state, disc and frontend sha256, build flags. `before-after.md` is the comparison, `break-rates.txt` splits poise breaks by source and colour, `frames/` holds the approval contact sheets.

- `baseline-*`: instrumentation only, no rule change (enemy poise 25, enemy melee untyped).
- `after-a-graybox`: enemy poise 50, opposite-colour poise x2 on enemies, typed enemy melee with x2 poise on the player.
- `after-b-*`: the same with the player-side poise multiplier at 4874/4096 (x1.19). This is what the branch ships.

Notes for reading them:

- The baseline discs were built before the docs-only commits that follow them, so a baseline contract's `source_rev` is a later revision with identical code. The disc sha256 identifies the build.
- Twenty seeds is a small sample. Win counts move by several runs between variants whose only difference is a poise multiplier, so treat them as noise unless a mechanism explains them.
- In the heavy scenario the enemy AI barely uses the seed, so several seeds replay the same fight (for example 1 and 11, 2, 13 and 14, 8 and 16 in the baseline). Its effective sample is smaller than 20.
- Frames were replayed from the `after-b-graybox` seeds with `frontend launch --stop-at-poll`, one run per frame (duel tick + 400 = poll).

## Perfect swap (parry) batches

`before60-graybox` and `after60-graybox`: 60 seeds each on base 39f40cd2 (tree 119c9388), the same tree built with and without the `no-perfect-swap` feature (before = 72-tick swap lock, no perfect window, no bot attempts). `parry-before-after.md` is the comparison. They predate the 2faa980f merge, so they must be re-run on that base before being quoted as final. `check0-*` are 7 heavy and 3 graybox seeds that confirmed the stall and seed-collapse fixes.
