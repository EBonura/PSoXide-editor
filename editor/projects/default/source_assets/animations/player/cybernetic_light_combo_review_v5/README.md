# Light combo v5 — head motion studio review

The head now follows the torso, with authored anticipation, turns, small tilts and a downward follow-through. This replaces the former head channels that counter-rotated the neck by as much as 140 degrees from its neutral local orientation to keep facing forward. The new head motion stays within 22 degrees of its neutral local orientation, independent of the larger torso rotation. Both combo head handoffs match.

Only the Neck bone changes from the previous motion: v4 follow-up arm/body/footwork channels and the approved opener’s other channels are retained. The rig uses Neck to control the head and has no separate Head joint.

This is a studio candidate, not installed in the default project. The folder includes a **head-only review variant of the opener**, plus the two follow-ups, so all three strikes can be judged together. The installed approved opener and walk are untouched; the default combo remains v3. Do not mark this review variant approved or current until it is chosen for installation.

Editable masters/export: light_attack.blend/.glb (61 frames at 30 Hz), light_attack_followup.blend/.glb and light_attack_finisher.blend/.glb (40 frames each at 30 Hz). Head measurements are in head-validation.json. Local authoring and video: review/light-combo-v5/author.py and review/light-combo-v5/studio-two-angles.mp4, relative to the default project. No v5 native-engine validation has run.
