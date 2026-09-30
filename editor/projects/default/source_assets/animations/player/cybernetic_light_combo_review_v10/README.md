# Light combo v10 — FK backhand

Studio candidate after the user rejected the second strike in v9. The first strike master and GLB remain byte-identical to the accepted v6 studio opener. Two descending diagonals still form the X; there is one sword cast.

The second strike is rebuilt with authored shoulder and elbow poses. The hand keeps its neutral local grip throughout the chamber, cut and follow-through. The sword follows the forearm; there is no independent wrist path or per-frame arm IK. The elbow opens through the strike, torso rotation and forward weight transfer are stronger, and the free arm counterbalances the turn. The head follows the torso. The contact breakdown uses continuous timing instead of stopping and restarting between poses.

Beat frames at 30 Hz: 0 entry, 9 chamber, 11 anticipation, 14 contact, 17 follow-through, 22 settle, 27 recovery, 39 neutral. The 74-frame studio sequence joins opener frames 0–33 to return frames 0–39. The video shows front and three-quarter views at real time then half speed.

Validation checks the matching entry, unchanged opener, opposite descending blade travel, blade-facing alignment, joint motion and ground contact. Maximum local rotation per source frame is approximately 15.2 degrees at the upper arm, 31.0 at the forearm and 2.6 at the wrist (the latter includes the entry transition). These are continuity checks, not evidence of aesthetic approval.

NOT baked or installed. The default project still contains the earlier v3 three-hit configuration. After approval, retain only LightAttack → LightAttackFollowup and remove the follow-up → finisher chain. Align damage/trail events with the new return stroke (source frames 11–17), verify the compact bake in native playback, and preserve historical finisher records.

Authoring script: `review/light-combo-v10/author.py`. Preview: `review/light-combo-v10/studio-two-hit-x.mp4`. Paths are relative to the default project root.
