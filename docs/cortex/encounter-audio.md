# Encounter sounds

A new fight plays one alert line: first `System anomaly detected`, then
`Outside`, alternating on subsequent encounters. Enemies joining the same
fight do not add another line. The existing combat gate waits for three
seconds without hostile enemies before allowing a new encounter cue.

Dialogue uses SPU voice 19, separate from gameplay effects on 16-18 and UI
sounds on 20-23, so footsteps, hits and menus cannot interrupt the line.

Both enemy types use the plasma sound when their projectile is successfully
spawned, at the authored release frame. Charging retains its separate cue.
The launch sound now plays at the recording's normal pitch.

The recordings provided on 7 September 2026 are trimmed without changing
internal timing. Full-quality trimmed WAV masters and source hashes are in
`source_assets/audio/encounter`. The resident swing WAV files are mono 16 kHz; speech and the longer plasma
sample use 11.025 kHz. They cook into PS1 SPU ADPCM. This keeps the new sounds within the
current executable's RAM budget while preserving the masters for later use.

## Menus and swings

Selection changes in all 31 menu buttons and sliders use `interfacemove.wav`.
Slider nudges use the same sample, deduplicated in the resident bank. Shoulder
navigation between inventory and system tabs plays the focus sound once;
the old overlapping tab activation cue is removed. Confirm, back, item socket
and limit sounds keep their separate meanings.

Light attacks and heavy attacks have distinct whooshes at unity pitch in both
stances. Player and enemy dispatch use the authored attack action, rather than
choosing a sound by actor type. Each newly active melee window triggers its
own whoosh, so the existing multi-swing attacks retain all their sounds.
The two existing swing event bits are reused as light/heavy events; recook
cached manifests after updating the source.

The first menu/swing build fit the linker budget but exhausted the remaining
bump heap during world loading. Reducing the plasma runtime copy to 11.025 kHz
recovers about 5 KiB; the full-quality master is unchanged. Both static RAM and
post-load heap headroom must be checked when expanding the sound bank.

## Dash

`warp.wav` plays once when the player enters an accepted forward, backward or
sideways evade, at normal pitch and 90% gain. It does not fire on rejected
inputs, during cruise/recovery, or during stance/cinematic decomposition.
The gameplay event mask is now 32 bits to accommodate the seventeenth event.

The recording retains 1.31–2.61 seconds, including its quiet decay. The stereo
48 kHz master and source hash are in `source_assets/audio/dash`; the resident
copy is mono 11.025 kHz. Four older mechanical UI placeholders (confirm, back,
limit and socket) now use 8 kHz resident copies to make room in main RAM. Their
original 44.1 kHz copies are preserved in `source_assets/audio/ui-placeholders`.
The new menu-selection, speech and swing recordings retain their existing rates.

## Stance change

`phase_change.wav` plays once when the player successfully requests a stance
switch. A request blocked by cooldown or a broken vitality pool stays silent.
The existing cooldown-ready cue remains separate.

The supplied recording retains 1.515–3.28 seconds, with its natural decay.
The stereo 48 kHz master and source hash are in `source_assets/audio/stance-change`.
Its resident copy is mono 10 kHz at unity pitch and 90% gain. Both encounter
voice lines now use mono 11.025 kHz runtime copies to fit the main RAM budget;
their full-quality masters and alternation behavior are unchanged.
