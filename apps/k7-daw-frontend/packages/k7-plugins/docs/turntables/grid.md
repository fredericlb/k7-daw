# Correcting the grid

A track's grid is **inferred**: the engine measures a tempo, and [beats.ts](../../src/turntables/beats.ts)
places the first beat on the circular mean of the transients, with a Rayleigh test saying whether
that phase is a measurement or a shrug. It often works. When it does not —
[bpm.md](bpm.md) shows a track (the Bashung) whose detected tempo is plain wrong — there has to be
a way to say so, and to say it in the right place.

## The correction holds for the SESSION

It is kept in the desk's load of the record, not in the file's tags and not in the playlist:

```
Osez Joséphine.mp3     the analysis said 174
bpm  87                the correction, until the record is taken off or the tab closes
```

**Nothing is written back.** A record is an audio file of the workspace, and a desk that edited
its tags on a halving gesture would change a file other projects read. The cost is that the
correction is made again next time; the gain is that a gesture at the desk never leaves a trace
behind it. It is the file service's analysis, cached beside the record, that stays as measured.

MARKERS are another matter: two bands covering the same record drop the needle in different
places, which is why those live in `mix.k7x` ([mixFile.md](../../../../../k7-daw-wasm/song/docs/mixFile.md)).

## `bpm` replaces what the record IS

Not the label: `Rate = target / natural`. Correcting only the displayed number would leave the
platter spinning at the wrong speed while showing the right value — the kind of fault nobody
finds.

## What an already-moved fader keeps

Correcting the tempo **does not move a fader someone has set**. A fader pulled to 176 to run
against the other deck is a decision; the analysis correcting itself is no reason to throw it
away. So:

- fader at rest (`target === natural`): it **follows** the correction;
- fader already moved: `natural` changes, `target` stays.

A test freezes it, because that is the line whose opposite would be heard mid-set.

## The octave, which is the error a detector actually makes

Under the fader, `÷2` and `×2`. Not shortcuts among others: a detected tempo is right **or wrong
by a factor of two**, almost never wrong by 3 %. A track at 87 read as 174 is the ordinary case,
and two buttons settle it without typing.

Greyed out while no tempo is measured: there is nothing to correct then, and a button answering
on `0` would set a tempo of `0`.

Once a correction is held, a `BPM ✎` appears under the two buttons. Only then: a badge that is
always there says nothing.

## The key, and the wheel

A key such as `C# minor` is **entered and never detected**: the detector was written and measured
against seventeen labelled tracks, and comes in at 53 % — enough to lie, not enough to serve
([key-detection-plan.md](../../../../../../docs/key-detection-plan.md)). When a record's load carries
one, the desk shows its **Camelot** code under the fader — `12A` for C sharp minor. The wheel is
a RENDERING and the key keeps the mode's name; the conversion is in `BpmFader`, and a string it
cannot read comes back out as it went in rather than disappearing.

## What this does not do, and must be said

- **The downbeat cannot be corrected.** It remains the circular mean of the transients.
  It belongs to the record too and could join its tags the day someone needs it; a field
  nothing reads is a field filled in for nothing.
- **4/4 stays assumed.** `BEATS_PER_BAR` is a constant, and a time signature is not corrected here
  — same reason: nobody reads it.
- **Nothing more is detected.** Correcting is not measuring. Better tempo detection is engine
  work, in the pass that writes the sidecar.
- **Nothing DETECTS the key.** The desk displays one it is handed and guesses none.
