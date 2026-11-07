# Enemy behaviour polish

The original video is retained in `../video-review`. This run uses its identical
controller tape and normal-speed audio/video capture. The currently linked
15 Hz directional clips and authored 50-percent spacing speed are retained.

Changes in this pass:

- Movement goals no longer restart an unchanged gait animation.
- Gait phase follows committed travel relative to each gait’s nominal speed.
- Circling targets 96 units instead of 64; radius correction uses that target.
- Two consecutive cannon attacks prompt a nearby close approach when stance
  cooldown permits, keeping the wider orbit from becoming a cannon-only loop.
- Brief occlusion during route recovery preserves the target memory.
- A detour ending at the same wall cannot clear the retry count.

Validation: 92 enemy tests and the actual-project cook test passed. Four
diagnostic replays cover all nine goal values and all three attacks. Reset
matches across 75 paired checkpoints. The extended blocked case reaches three
retries and return-home without crossing the sealed pen. Physical return
ends within 12 units of spawn. See `verification.json` and saved logs.

The delivered recording uses the normal build without emulator telemetry.
`build.json` identifies the disc. Captures use private disc copies so another
workspace build cannot change a running replay.
