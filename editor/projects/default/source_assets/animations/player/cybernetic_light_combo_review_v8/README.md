# Light combo v8 — two-hit X

The user changed the design to two strikes: retain the first diagonal swipe, then add one descending swipe in the opposite direction to form an X. The previous second and third choreography is superseded for this proposed combo.

The opener master and GLB are byte-identical to the v6/v7 studio opener. The new return is authored from the first cut’s reflected torso, arm and actual sword motion, sampled with one shared progress curve. It has the same fast cutting cadence as the first. The blade stays aligned with the cut; the measured torso and blade rotational peaks are one 30 Hz frame apart. The right hand continues to hold the sword. The feet remain planted through the return, with separate recovery steps and no second sword cast.

Only two clips are present in this candidate. light_attack has 61 source frames at 30 Hz, and light_attack_followup has 40. The preview joins opener frame 34 to return frame 0, then plays the return through recovery, for 74 frames. x-validation.json verifies the matching entry, unchanged opener, opposite lateral/downward blade travel and blade-facing alignment. validation.json records ground contact and joint motion.

This remains a studio review candidate, not baked or installed. The default currently contains the earlier v3 three-hit configuration. When installing the two-hit design, retain only LightAttack → LightAttackFollowup in action_chains, remove the follow-up → finisher handoff, and update the new return’s hit/trail window to its actual stroke (authored frames 11–15; proposed 10 Hz samples 4–5, to verify in native playback). Preserve the historical finisher sources and register records. Verify the new bake, events and recovery in the native engine before declaring installation complete.

Local script and video: review/light-combo-v8/author.py and review/light-combo-v8/studio-two-hit-x.mp4, relative to the default project root. The video shows frontal and three-quarter views at real time and half speed.
