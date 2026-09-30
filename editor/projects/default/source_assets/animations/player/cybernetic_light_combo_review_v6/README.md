# Light combo v6 — blade facing and returning slash

Studio review candidate, not installed in the default project. V6 addresses the second strike facing backwards and the third strike reading as a sideways thrust.

Strike two turns the grip during preparation and keeps the blade facing into the reverse cut, calibrated against the approved first strike. The turn is shared between upper-arm twist and forearm pronation; it does not add wrist counter-twisting.

Strike three now uses the approved first strike’s actual world-space upper-arm, forearm and hand orientations, sampled from its preparation through its diagonal slash. These are anchored to the finisher’s existing shoulder and planted stance. Its source timing follows the same pose progression as the existing finisher body; it blends from strike two during preparation and has a separate recovery.

V5 head motion, other body channels and footwork are preserved. The opener is copied unchanged from the v5 studio candidate, which contains the head-only review variant. The installed approved opener and walk are untouched. Both combo handoffs match within floating-point precision. validation.json records source contact and joint motion; blade-validation.json compares blade facing against transverse movement during the main cut and checks unchanged non-arm channels.

All masters are at 30 Hz: light_attack is 61 frames; followup and finisher are 40 each. This version has not been baked or tested in the native engine. Current default remains v3. Local authoring script and video are review/light-combo-v6/author.py and review/light-combo-v6/studio-two-angles.mp4 relative to the default project root. The video shows the same front/side studio angles at real time, then half speed.
