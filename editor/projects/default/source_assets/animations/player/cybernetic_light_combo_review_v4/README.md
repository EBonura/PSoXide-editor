# Light combo v4 — studio review candidate

This candidate replaces the follow-ups’ frame-by-frame IK adjustments with authored FK arm poses. Strike two sweeps horizontally across the front; strike three raises into a descending cut. The approved opener, body weight and grounded footwork are retained. The wrist follows the forearm throughout the strikes. Both 40-frame masters run at 30 Hz; strike two frame 27 matches strike three frame 0 within floating-point precision.

These sources are for studio review and are **not installed in the default project**. The current baked default remains candidate v3. Do not label this version approved or register it as the current replacement without a subsequent decision to install it.

`validation.json` records joint rotation changes, floor contact and the connecting pose. No v4 native-engine validation has run. Local authoring script and two-angle real-time / half-speed video: `review/light-combo-v4/author.py` and `review/light-combo-v4/studio-two-angles.mp4` relative to the default project root.
