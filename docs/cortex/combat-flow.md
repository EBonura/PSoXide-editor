# Graybox Reach combat-flow prototype

Energy is the third bar beneath the two vitality bars. Ranged fire spends it; connected melee and attachment to a floating arch restore it at different rates. Grounded waiting does not recharge it. Both fighters use the same ground resource rules; only the player can hook.

The current AI separates ranged and melee decisions. At range, a shared bounded planner searches for wall cover, moves into shelter, waits briefly, then peeks to fire. Live approaching projectiles can trigger lateral evasion. At contact, fighters read committed attacks and recovery instead of leaving simply because their Energy is high. A connected heavy attack can create an exit opportunity; light contacts replenish Energy without requesting a retreat. The earlier distance-only iterations below are retained as measurement history.

| Rule | Initial value |
|---|---:|
| Capacity / starting Energy | 100 |
| Cost per successfully emitted projectile | 20 |
| Light melee contact | +12 |
| Heavy melee contact | +20 |
| Attached recharge, outside firing/recovery | +20 per second |
| Total attachment allowance per excursion | 6 seconds |
| Continuous grounded time to rearm allowance | 2 seconds |

A missed melee swing earns nothing. Each burst projectile costs separately; allocation failures do not spend Energy. Rehooking another arch does not refresh the timer. Timer expiry, poise break or death detaches the player; Circle also drops voluntarily. Ordinary projectile hits still deal damage, but do not force the player into the hit-reaction animation. The small TETHER meter beside Energy shows remaining attachment time.

Triangle changes stance manually. Horizon uses R1 light / R2 heavy. Zenith uses R2 directly to fire; L2 adds precision aiming. L2+R2 hooks when an eligible arch is selected. Firing slows movement even without L2. The enemy can retreat or circle at half its spacing-walk speed while firing, with a leg animation layer beneath the cannon animation.

Matching-colour hits deal 100% damage, opposite-colour hits 125% and double the poise damage against an enemy (`OPPOSED_POISE_Q12`, a starting point). Enemy melee is Horizon-coloured and reads the player's stance the same way: 100% damage when the player is in Horizon, 125% in Zenith, with a smaller poise multiplier (`PLAYER_OPPOSED_POISE_Q12`) chosen so one claw light (50 poise) cannot break a fresh player (capacity 60). A matching hit keeps its authored poise: flow rules have no guard chip, unlike the legacy rules, which shrink guarded damage and poise together. Projects without a ranged weapon keep untyped enemy melee and the legacy rules. The light enemy's poise is 50, so one light player hit (25) never staggers it while a light combo or one heavy (50) does; the count resets after 120 quiet ticks. Poise is shared. Ordinary projectiles apply at most 10 poise damage; a shot connecting in the latter half of an exposed windup breaks poise. The player exposes melee windups; NPCs expose melee and ranged windups. A reaction plus 60 ticks of grace prevents repeated stun refresh. A player's heavy melee contact pushes the opponent through its normal collision mover, creating separation for a shot. AI preferences respond to those openings: replenished Energy favours creating space for ranged fire, a successful shot interrupt favours melee, and insufficient Energy favours melee. Human stance selection remains manual.

The editor MCP `combat_flow` tool reports the canonical resource constants and prototype rules. `combat_duel` exercises the shipping guest through pad input and includes both Energy values in its trace. Death completion does not prove balance. Ground duels cannot verify hook attachment, recharge, the timer or aerial targeting; those need a separate replay and visual inspection.

The pose masks currently target Graybox Reach's 26-joint Aletha and 22-joint Light enemy. They use the same composed pose for drawing, sockets and hurtboxes. Other rigs need authored masks before this becomes a general editor feature. These values are initial prototype tuning, not a final combat balance or a hardware performance guarantee.

## AI polish, October 7

After spending below one shot's cost, AI fighters remain committed to rebuilding Energy until they reach 60. This reserve rule only guides the AI: a human can still shoot whenever they have 20. A melee connection with a large reserve, or a heavy connection, can create a ranged follow-up; a successful shot interruption creates a melee follow-up. The opportunity remains available for eight seconds so recovery and stance cooldowns do not silently erase it. Ordinary action permissions and cooldowns still apply.

Between attacks, prototype enemies reassess after 36–72 ticks rather than circling for several seconds. A stance/resource change can cancel ordinary spacing after 24 ticks, while obstacle recovery and committed attacks retain their own lifecycles. An empty enemy facing an overhead player keeps the cannon stance and waits for a reachable target; it receives no free recharge and cannot hook.

The melee approach stops 16 units farther out. In the first half of its tell, a crowded enemy can turn at its normal bounded rate and step backwards through collision; the second half plants. The current Light rig layers its backwards gait beneath the upper-body tell. This helps the visible claw connect without enlarging damage capsules or shortening authored attacks.

The Energy bar turns orange-red below a shot's cost. `combat_flow` exposes AI thresholds and timing, and duel switch reasons distinguish reserve rebuilding, melee-to-ranged follow-ups, shot-to-melee follow-ups and overhead targeting.

Initial comparison across seeds 1 and 3 increased observed player damage from 45/77 to 161/120. Seed 7 took 120 damage. All three fights reached enemy death; sampled damage can be offset by vitality regeneration, and these results are not a win-rate balance target. Seed 3 did not damage both player vitality channels, which remains a coverage warning. Evidence is stored under the project's `validation/ai-polish/` directory.

## Physical push and pull

The first showcase exposed a gap in those checks: reprocessing seed 1 with the MCP's integer distance bands shows 74.7% of its measured time within 80 units and only 5.4% at 192 units or more. Switching stance and eventually reaching death did not establish spatial combat flow.

AI now retains a resource-driven weapon phase. A charged fighter commits to ranged play even while touching the opponent; below one shot it commits to melee until connected hits rebuild 60 Energy. A successful shot interrupt still creates a melee opportunity. Expiring follow-up memory does not silently revert a charged fighter to melee just because it is nearby.

Both ground controllers use the same firing-band calculation: near distance is the larger of three times preferred distance or three times melee reach; far distance is the larger of six times preferred distance or four times melee reach, capped by weapon range. The near edge is capped at three quarters of the far edge. For the current encounter the authored 256-unit weapon range caps this to a 192–256-unit band. Crowded NPCs turn and run away through normal collision, then turn back before firing. Player automation holds the real sprint input while escaping, subject to stamina and action locks. Shooting retains its movement penalty. Blocked escape does not fake arrival or allow contact-range fire. A blocked retreat, or reaching the edge of the engagement area, commits the NPC to a four-second melee response. This prevents kiting through the home leash and trying to reset while being attacked.

The MCP duel report includes `spatial_flow`: time in close/transition/ranged bands, mean distance, longest uninterrupted close exchange, completed close–far–close excursions, and approximate distance at each emitted shot. Less than 20% ranged time or zero completed excursions raises a coverage warning; these are diagnostic thresholds, not a balance score. Death, damage, Energy, and stance coverage remain separate checks.

Final headless checks (seeds 1 and 7) each completed two close–far–close excursions and reached enemy death, with 37.5%/44.6% ranged occupancy and 38.2%/40.5% close occupancy. The longest close exchange fell from the baseline's 19.1 seconds to 5.4/6.0 seconds. Both fighters used ranged attacks and both player vitality channels took damage. These runs establish spatial coverage, not equal combat strength: the player still won both, and the camera can be obstructed by nearby walls during the chase. The complete first fight and machine-readable reports are in `editor/projects/graybox-reach/validation/push-pull/`.

## Follow-up: movement continuity and wasted decisions

The player bot keeps moving through a stance request and commits an escape to the middle of its firing band. Local width probes retain a clear escape direction instead of repeatedly backing into a wall; an unsuccessful three-second escape uses the shared melee response. NPCs already defending at the engagement boundary stay in melee until there is room to use their cannon. A settled approach persists while turning or waiting to attack, reducing repeated goal restarts. All movement and attack permissions still go through the ordinary controllers.

The next frozen-disc checks (seeds 1, 7 and 23) completed 4/4/6 distance excursions. For the two comparison seeds, enemy goal changes fell from 7.5/9.2 to 5.4/1.6 per second. Seed 1's completed enemy stance visits without an attack fell from two to zero. These are narrower claims than overall balance: close occupancy increased to 57.8%/52.5%, and the longest close exchanges increased to 8.6/7.8 seconds. Seed 23 spent 47.2% close, 31.6% ranged. The player still won all three. Reports, build hashes, checks and the full first fight are in `validation/flow-polish/` within Graybox Reach.

## Cover, projectile evasion and melee reads

Both controllers use `ranged_tactics::RangedExchange`. It begins by searching for cover and checks up to two candidates per 12 simulation ticks. Candidates must have a clear movement lane, conceal both sides of the actor at two body heights, and have a reachable firing position with a clear shot. A failed search returns to open fire; it never records shelter. Cover lasts 54 ticks before peeking. Two shots or 120 ticks of exposure trigger another search. Arrival must be within four units and pass the concealment check again, because the opponent may have moved.

Projectile reactions inspect already-emitted hostile bolts at least six ticks old, with a predicted closest approach within 24 ticks. Occluded bullets do not trigger an evasion. The player's automation sends the normal dodge input, with stamina and action locks; the Light enemy uses a lateral collision-bound step without new invulnerability frames. Its cover movement follows the checked segment directly instead of inheriting a chase heading. Hidden opponents are represented by their last seen position.

In melee, the player bot observes enemy windup, active attack and recovery states. The NPC reads the player's current posed hitbox timing, can backstep during a tell, and releases that defensive retreat when recovery becomes visible. Both retain ordinary attack permissions and cooldowns. These are decisions rather than a guarantee that a dodge or punish succeeds.

The editor MCP `combat_flow` describes these rules. `combat_duel.ranged_exchange` reports actual cover arrivals, shots following a completed cover/peek sequence, projectile-evasion attempts, melee defense and punish decisions, and time in each mode. Interrupted peeks and failed cover searches do not count as peek shots. Distance occupancy remains useful context, but it is not the acceptance criterion for combat flow.

The normal `cd-stream-bench` guest uses size optimization on the AI planner, policy, duel helpers, and startup-only loading functions to fit these additions. Rendering optimization settings, world geometry and model detail are unchanged. Passing the linker alone was insufficient: an earlier build exhausted its small remaining heap during gameplay initialization. The validated build must also boot and finish actual guest fights.

Two staggered cover walls were added through the editor MCP to the opening encounter, in the group `Combat flow / staggered cover walls`. Each is 96 by 24 by 96 runtime units (1536 by 384 by 1536 authored units). Their 64-unit central gap and open ends retain melee approaches. The cook adds 20 BSP faces (1125 to 1145); the conservative PVS packet estimate becomes 1504/1536. That estimate is not a measured frame-rate guarantee.

Final frozen-disc checks (seeds 1, 7 and 23) completed by enemy death in 105.3, 88.5 and 97.4 simulation seconds. Player/enemy cover arrivals were 3/2, 3/3 and 5/3. Shots after completed peeks were 5/1, 5/1 and 7/1. NPC projectile-evasion decisions were 4, 3 and 3; the player projectile-dodge path was not exercised in these runs. Both controllers recorded melee defense and recovery-punish decisions, and both player vitality channels took damage in every run.

These results validate the cover/peek sequence and the existence of distinct melee decisions. They do not establish equal ranged pressure: the enemy emitted only 2, 2 and 1 projectiles versus the player's 11, 10 and 9, and the player won all three. Evasion decisions are not confirmed successful dodges; punish decisions are not confirmed hits. No hardware frame-rate claim is made. The complete first fight, per-seed reports, test counts, build hashes and MCP geometry-edit receipt are in `editor/projects/graybox-reach/validation/encounter-flow/`.

## Enemy lateral evade

The Light enemy can now commit a fast lateral step in either stance, including with zero Energy. It reacts to a visible, already-released approaching projectile independently of the slower cover-search cadence. It checks both side lanes and moves through the ordinary collision motor. A blocked side selects the other lane; two blocked lanes prevent the attempt.

The shared tuning requests 72 units over 18 simulation ticks (0.3 seconds), followed by 12 ticks of planted recovery (0.2 seconds). A 120-tick cooldown runs from the start and survives stance changes. The step uses the existing left/right strafe clips, faces the opponent, costs no Energy and grants no invulnerability. It cannot cancel an attack windup, active attack, attack recovery or stance transition. Actual travel can be shorter because of collision or arrival tolerance. `combat_flow.ai.lateral_evade` exposes these values and requirements.

Runtime tests cover zero-Energy melee evasion, lateral clip selection, travel and planted recovery, blocked-side fallback, cooldown retention, and committed-action protection. Guest replay evidence belongs in `validation/lateral-evade/` within Graybox Reach; an evade decision alone does not prove the incoming projectile missed.

The normal Play build passed 352 runtime unit tests, five integration tests and the PS1 instruction hazard scan. Guest seeds 1 and 7 completed by enemy death after 7202 and 8030 ticks, with four and six enemy evade attempts. The first two steps were inspected in the recorded replay; the enemy moves laterally and plants before resuming combat. These runs verify integration, not dodge success rate or combat balance.

## Stance swap as a parry

A voluntary stance swap grants 25 ticks of i-frames. If an enemy melee hit or bolt would land within the first `PERFECT_SWAP_TICKS` (12) ticks of the swap, it is a perfect swap: the melee attacker is staggered (the bolt is negated), `PERFECT_SWAP_ENERGY` (one shot, 20) is refunded and the swap cooldown resets. A hit that lands after tick 12 but inside the i-frames is still avoided, but pays nothing. The `no-perfect-swap` editor-playtest feature restores the old 72-tick swap lockout with no window and no bot attempts, so a batch can measure the parry against the same tree. The duel bot attempts a perfect swap on 60% of its opportunities (`PERFECT_SWAP_SKILL_PERCENT` in the editor-playtest `duel.rs`).

The starting values are guesses to be judged by feel. To tune them, edit the constants and rebuild the disc:

| Value | Where |
|---|---|
| Perfect swap window, Energy refund | `PERFECT_SWAP_TICKS`, `PERFECT_SWAP_ENERGY` in `engine/crates/psx-game-runtime/src/combat_flow.rs` |
| Aim/shot/hook lock after a swap (keep equal to the window) | `SWAP_COMMIT_TICKS` in `vitality.rs` |
| Swap cooldown, swap animation length | `swap_cooldown_ticks`, `swap_duration_ticks` in `CombatStanceConfig::DEFAULT` (`vitality.rs`) |
| I-frames of the swap and of the sidestep | the player's `roll_invulnerable_frames` in `project.ron` |
| Opposite-colour poise on enemy / on player | `OPPOSED_POISE_Q12`, `PLAYER_OPPOSED_POISE_Q12` in `combat_flow.rs` |

The 60-seed before/after batches under `validation/combat-feel/` were taken on base 39f40cd2 and have not been repeated on the current base.
