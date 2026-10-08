# Music looping

The combat cue is authored with looping enabled. A new encounter starts at a
random point within the opening 15 seconds, then restarts from the beginning
at each track boundary. The one-second TOC rounding guard and short-track end
margin still apply. Re-engaging before the fade-out finishes keeps the song
playing without another seek. Ordinary menu music starts from the beginning.

This avoids starting near the end of a song; it does not make CD-DA looping
gapless. The drive still needs to seek back when a full playthrough ends.

The old player issued GetStat and waited for 1,024 polls. On timeout, the
following query discarded the late answer. If every response arrived outside
that budget, the player could never confirm that the track had ended.

The player now dispatches one query and collects its response on subsequent
frames, without spinning. It allows one second before abandoning a missing
reply and retrying. Two healthy stopped responses still confirm a restart;
playing, seeking, reading, errors and an open lid do not. Track changes and
data-read handoffs cancel pending queries and clear old stopped confirmations.

## Validation

The original build looped twice in a 208-second controlled combat test; the
user's exact emulator stop was not reproduced. A separate diagnostic copy
with a one-poll response budget reproduced indefinite silence after the first
track ending. This tests the timeout failure, not the user's precise cause.

The candidate passed 58 host tests, including replies delayed by ten frames,
missing replies and cancellation at track changes. Its 208-second combat
probe restarted the song twice and remained audible through the last ten
seconds. The probe only forces the combat music gate on; it does not ship.
The normal disc passed a separate gameplay/menu run with zero guest faults,
zero load-delay hazards and a passing arithmetic symbol gate.

If the stop recurs, capture an emulator save state while it is silent so the
combat gate, CD status and volume state can be inspected at the actual failure.


## Opening-window calibration — 2026-10-07

Combat random starts are now capped at 15 seconds. The new regression fails
against the former whole-track selection and passes with the cap. All 60
GameApp tests pass, including short/invalid tracks, re-engagement without a
reseek, and a full-start loop without another random offset. A normal Graybox
Reach disc replay issued Setloc at 00:37:00, approximately 13.32 seconds into
combat track 2. The rebuilt disc passed all 8 stack guards and had zero
remaining load-delay hazards. Logs are in
`build/graybox-reach/music-start-window/`. This run verifies the start offset;
it does not establish gapless looping or console seek timing.
