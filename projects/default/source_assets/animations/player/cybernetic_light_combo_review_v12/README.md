# Light combo v12 — blade plane correction

Approved and installed on 30 September 2026, responding to the user's request to align the blade in the second swing. Preserves the accepted first strike master and GLB byte-for-byte and preserves v11's body motion, timing and wrist path.

Inspection of the actual sword mesh exposed an axis error in previous alignment checks. The blade is thin in local X and wide in Y, extending along Z: the broad blade plane is YZ. Previous checks used local -X as the facing reference, which measured flat-first motion rather than edge alignment. Those historical blade-facing scores must not be used as evidence of correct edge alignment.

The correction rolls the right forearm -90 degrees about its own long axis, easing in from the unchanged entry and out during recovery. Local -Y now leads the cut, and the blade's broad face lies along the cutting plane. The hand's local grip remains unchanged. All other joint channels and the wrist world path match v11 within floating-point tolerance. No body, timing, hit emphasis, or first-strike changes were made.

The revised edge-leading alignment score is approximately 0.987 during the main cut; sampled edge/travel deviation is at most 12.5 degrees across frames 12–17. The small residual reflects a curved swing and the slight difference between the forearm and blade axes. Validation now measures the edge and the broad-face normal separately. Preservation validation verifies the narrow scope of the change.

The return remains 40 frames at 30 Hz, with its main cut at 12–16. The 74-frame studio video shows front and three-quarter views at real time then half speed. The approved version is now baked and installed in the default project. The earlier three-hit v3 configuration is retired; historical sources and the unbound finisher resource remain available.

Authoring script: `review/light-combo-v12/author.py`. Video: `review/light-combo-v12/studio-two-hit-x.mp4`. Paths are relative to the default project root.


## Installed controls and bake

One R1 press starts one attack in the Horizon stance. A new R1 edge from source frames 12–34 (0.40–1.133 seconds after attack start) queues one follow-up. Frame 12 is full sword materialization; frame 34 is the matching outgoing pose, where the return begins with a four-tick blend. Holding R1 does not queue; out-of-window presses are discarded during the action; there is no follow-up-to-finisher link. Interruptions and evade intent clear pending continuation. R2 and Zenith retain their existing behaviour.

Opener: clip 84/source 203, 61 frames at 30 Hz. Return: clip 204/source 206, 21 samples at 15 Hz with runtime interpolation, frames 0–20, speed 256, not in-place. All three decisive source strike keys (12,14,16) are retained as samples (6,7,8); the RAM-conscious bake interpolates intermediate source frames. Return damage/trail window is 6–8, damage/poise 25/25, trail history two samples, sword visible from 0 and dissolving over 17–20. Source handoff poses match; retargeted rotations match, while translation differences are bridged by the existing blend. The first performance includes the head revision approved during the studio iterations.

Native emulator input replay: single tap, valid second tap at 600 ms, continuous hold, early second tap at 167 ms, late second tap at 1333 ms, and repeated presses. Observed outcomes are one/two/one/one/one/two strikes respectively. Runtime tests: 242; playtest tests: 78; project chain/cooker tests: 2. Zero guest faults and zero load-delay hazards. Hardware was not tested. Detailed memory/build hashes are in `runtime-validation.json`. Native video: `review/light-combo-install-v12/native-single-vs-combo.mp4`.

## Timing research and adaptation

[PlayStation's Bloodborne combat tips](https://blog.playstation.com/2015/03/23/bloodborne-24-tips-for-survival/) describe moves chaining into contextual follow-ups. [The Saw Cleaver documentation](https://www.bloodborne-wiki.com/2015/03/saw-cleaver.html) documents weapon/form-specific R1 chains and differing speeds; it is community documentation, not a universal timing specification. [MushroomShogun's firsthand Dark Souls III frame-testing explanation](https://www.reddit.com/r/darksouls3/comments/j29o3t/) describes queuing R1 during attack/cancel frames and starting the next attack at the animation's allowed transition. [Souls animation-event format research](https://soulsmodding.wikidot.com/format:tae) describes time-windowed animation events and their 30 Hz authoring convention.

No authoritative universal Bloodborne or Dark Souls combo-buffer duration was established. Cortex's 0.40–1.133-second acceptance window is our tuning choice for its longer sword-cast opener, not a claimed frame-exact reproduction of either game. It follows the buffered-continuation structure, preserves the approved choreography, and rejects extra terminal inputs rather than accumulating a long action queue.
