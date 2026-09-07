# Temporary Cortex UI sound sources

The remaining processed placeholders derive from Brian MacIntosh's **Mechanical
Sounds** collection:

<https://opengameart.org/content/mechanical-sounds>

The original recordings are dedicated to the public domain under CC0 1.0.
Attribution is not required; the author optionally requests credit as Brian
MacIntosh.

| Project cue | Original recording |
| --- | --- |
| `ui_confirm.wav` | `lightclunk2.wav` |
| `ui_back.wav` | `lightclunk2.wav`, reversed |
| `ui_tab_shift.wav` | `mechanical2.wav` |
| `ui_slider_tick.wav` | `typewriter.wav` |
| `ui_limit.wav` | `mechanical2.wav` |
| `ui_socket.wav` | `mechanical1.wav` |
| `ui_unsocket.wav` | `mechanical1.wav`, reversed |

The placeholders are mono 44.1 kHz 16-bit PCM WAV files. They have been
trimmed, pitch-shaped, filtered, compressed and lightly bit-reduced for the
Cortex Ignition interface. The normal project cook converts them to PS1 SPU
ADPCM.

## Current selection sound

`ui_navigate.wav` now comes from the supplied `interfacemove.wav` recording.
It is used for selection changes throughout the main menu, options and inventory,
and for slider nudges. The clip is trimmed to 0.75-1.17 seconds and stored as
mono 16 kHz PCM. Its full-quality master and input hash are in
`source_assets/audio/menu-and-swings`. This replacement is separate from the
CC0 placeholder sources above.

For the dash sound update, the resident confirm, back, limit and socket WAVs
are mono 8 kHz. Their full-quality processed copies are preserved under
`source_assets/audio/ui-placeholders`; this saves executable RAM without
changing their timing or pitch. Other placeholders retain their existing rates.
