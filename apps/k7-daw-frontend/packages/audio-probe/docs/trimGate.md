# `trim.rs` — where a take starts and stops SOUNDING

A recorded take begins with the tail of a count-in and the player settling, and ends with a room
and a hand leaving a string. `event trim` cuts both by asking where the envelope crosses a gate,
and `trim_window` (`apps/k7-daw-backend/packages/waveform/src/trim.rs`, reached by the page as `trimWindow` of `@studio/k7-wasm/media`) is the whole of that decision — pure, and nothing else.

## Bucket-accurate, and one bucket of margin

The answer is bucket-accurate and no finer, which is the honest resolution: the envelope IS
buckets, and pretending to a sample would be inventing precision the picture never had. A bucket
the interval only partly covers still counts, for the same reason.

The window is widened by one bucket at each end. A gate is a decision about where sound BEGINS,
and clipping the attack that crossed it would be audible where a few silent milliseconds are not.

## `within` is a cage, not a hint

`within` is the interval the clip already PLAYS — its current `from`/`to` — and the gate looks
only inside it: a re-trim refines the window and never reaches back into material an earlier trim
removed. The answer is clamped to it, the widening included, so a sound in the interval's first
bucket cannot push the result before the clip's own `from`. Omitted, the whole file is the
interval, which is what a first trim is.

## Null rather than a trim to nothing

Nothing in the interval above the gate answers null. A take entirely below the gate is either
silence or a gate set too high, and both are things to SAY rather than to apply — applying them
would leave a clip of zero length that plays nothing. A single bucket over the gate still has to
come back as a window and not a point.

## The floor

`amp_to_db` puts a zero bucket at −100 dB rather than at −infinity, so comparisons are TOTAL: a NaN
and an infinity both compare false, and one of those is a bug waiting to happen. The same floor is
`PEAK_FLOOR_DB` of `@studio/audio-analysis`'s `peak.ts`, which spawns a decoder and so is not shared.

The default gate is −40 dBFS: below a room's noise floor on any usable take, above true digital
silence, so a take recorded with the preamp up does not trim to nothing.
