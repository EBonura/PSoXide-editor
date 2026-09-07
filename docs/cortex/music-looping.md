# Music looping

The combat cue is authored with looping enabled. It starts at a random point
for a new encounter, then restarts from the beginning at each track boundary.

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
