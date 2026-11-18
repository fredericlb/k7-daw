# The curves in Rust (2g)

Sub-step 2g of the compiler's port. It extends [rustCompiler.md](rustCompiler.md), whose decisions 1 to 87, output rules,
hostile-input rules and stage gate it keeps. Paths are relative to `packages/song`.

A curve file is a function of time; the engine plays a list of breakpoints. This sub-step reads `curves/<id>.k7x`
(`rust/src/curves/file.rs`), resolves each against the song's time signature (`resolved.rs`), and bakes it to `[beat, value]`
points (`bake.rs`): an oscillator on a grid, a drawn curve at its corners, an on/off curve as steps, an envelope at each of its
firings, all under their conditions, their duration and their cut. It compiles nothing and is wired to nothing: the sub-step
that builds the lanes hands `CurveBaker` a curve id and the lane's note-ons (`baker.rs`).

## What the curves decide (2g)

Numbered on from the readers', the module tree's and resolution's tables. A row marked DEPARTS differs from the TypeScript and
says why; the stage gate allows a difference by its number, never by a pattern wider than the case.

| # | choice | why |
|---|---|---|
| 200 | the arithmetic and the timecode are the pilot's (`song-schema`), reused and unchanged; `song` holds the file shape, the conditions and the bake | a change to a crate the facade links forces the checked-in module's rebuild, and conditions are a bake's concern: the editor's preview is drawn without them |
| 201 | `CurveFile` is what `parseCurve` answers: the pilot's `CurveSpec` plus `conditions`, serialized flattened with the TypeScript's keys | the gate compares it with `loadCurves`, and it is what the graph view will ship as `spec` |
| 202 | the decoder is `CurveSchema`, strict, then `parseCurve`'s defaults, read over the k7x tree: the root's `type` dispatched by hand, one struct per kind with unknown keys refused, `conditions` decoded from the list node and each entry dispatched on its own `type`; `parseCurve`'s fallbacks behind the schema (an unknown shape read as sine, an amplitude clamped, an on/off value rounded) are not ported | a derived internally tagged enum buffers the node and loses the place of an error; `parseCurve` only ever sees schema-valid data |
| 203 | the normalisations are made: `polarity` unipolar, `amplitude` 1, `shape` sine, `frequency` 1, `phase` 0, stages 0, `level` 0.5; a custom point's `value` is clamped to 0..1; an on/off curve is always unipolar at 1 with no `cut`; `cut: none`, an empty `conditions` and a `duration` of no length (read in 4/4) are absent; a `time` is kept as written | the shape the bake and the graph read; a `duration` is dropped before the song's signature is known |
| 204 | DEPARTS. a curve file that does not decode (a syntax error included) is a WARNING named `curves/<file>` at its real line and column; that curve is absent, any binding to it drops, every other file still reads. A file the disk will not read (`Io`) is one WARNING named for it, at 1:1, and its siblings read too. The TypeScript says nothing when the schema refuses a file and loses EVERY curve when one has a syntax error or cannot be read | a broken file must not vanish with no word, and one file must not take its siblings with it |
| 205 | DEPARTS. a `curves/` folder the Source refuses as `Outside` is one WARNING named `curves`, and no curve is read; a file in it refused the same way is a WARNING named `curves/<file>`, and the other files still read | decision 24's rule: a refused file costs that file alone |
| 206 | the curves are read through `read_family` and the per-file cache under the caller's Source, per compile; they never join the vocabulary or its key | decision 25: the vocabulary is shared by the projects of a workspace |
| 207 | a curve is resolved once per `CurveBaker`, memoised by id (a refusal too), against the song's `TimeSig`: the spec without its cut resolved by the pilot, the conditions' timecodes parsed to beats, the drawn beats held | a binding count times a point count must not be the cost |
| 208 | DEPARTS. a timecode that parses to a non-finite beat count, or a baked breakpoint that is not finite, is an ERROR named `curves/<id>.k7x` at 1:1, and that curve bakes no lane. The TypeScript writes `null` breakpoints or drops the condition | a non-finite number is a compile error, not a value |
| 209 | `curveRuns` and `envelopeFires` are ported as they stand over conditions already in beats; fires at one beat are merged by a stable sort and a scan | `retrigger \|\|=` and the longer-or-open window commute, so the order the TypeScript's map met them in cannot show |
| 210 | a run's drawn corners are found by binary search over the sorted beats | the same set of breakpoints; 100,000 points under 10,000 retriggers must not be 10^9 tests |
| 211 | DEPARTS. one budget, `BUDGET = 12,000,000` per `CurveBaker`, weighs the OUTPUT: each breakpoint written, each point a cut reads once, each candidate a grid or a run's corners produce, each run, each fire once, each note-on laid. Past it: one ERROR named `curves/<id>.k7x` of the curve it stopped in, the baker capped and answering `None` from then on. The TypeScript is unbounded, and two cases never return: a looped clip of length 0 in `noteOns`, and a `between` so far out that `b += step` stops moving | a limit counts the output. An hour of sixteenths is 28,800 note-ons a lane: one lane with two cut envelopes spends 633,604, so a dozen lanes spend about 7.6 million and fit; the limit is set where that fits and a hostile input still ends in seconds |
| 212 | a lane whose curve exists but bakes no point is `None` (no automation) | the engine plays no empty lane |
| 213 | the seam is pure functions over `Cache`, `Source`, `TimeSig`, `f64` and `&[f64]`; nothing in `curves/` names a routing or a clip type | 2g is built beside 2d and 2f; the only thing the bake reads of them is a list of beats |
| 214 | `MidiWindow` is the bake's input type and `note_ons` weighs every note it walks and every note it lays | `noteOns` is the body of the bake's cost, and it reads a clip through five numbers |
| 215 | `curve_id_from_ref` lives in `curves/mod.rs` and the prefix constant is private | the collection and the graph both read it |
| 216 | `bake_curve` on an envelope answers `[[0, 0]]`, `[[0, -1]]` when bipolar, with its cut applied as the TypeScript's, and `CurveBaker::bake` does not reach it | the union stays total without a fourth path |
| 217 | the gate is in files of its own (`scripts/curveDump.ts`, `rust/tests/all/negative_curves.json`, `curves_negative.rs`, `curves_stage.rs`) | branches that extend the stage gate must not collide on one |
| 218 | the seeded sweep is the referee where the golden corpus cannot be: beats compared within `1e-9`, values within `1.5e-6`, breakpoint counts exact | the bake rounds to six decimals and `sin`'s last bit can flip one tie |
| 219 | no ts-rs twin | decision 43: the graph's `WebCurve` comes with the view that draws it |
| 220 | `rustCompiler.md` is not edited here; this page extends it | the sub-steps that run beside this one edit the same tables |
| 221 | the weight of a walk is paid whether or not it lays anything, and only when it happens: a window's notes before its loop is tested, each repeat of a looped note, the trigger list once when it is first sorted, and a retriggering event once more for the runs it lays; an event that is not a note-on walks nothing and weighs nothing; an envelope condition weighs the note-ons it filters | 100,000 retriggering conditions by a million note-ons is 10^11 steps from a small file, windows sharing a million notes outside their loops lay nothing and still cost, and a loop of nothing never ends |
| 222 | the cap of `note_ons` is an ERROR named `curves`, at 1:1, since no curve is being baked | a finding names a file, and the folder is the one thing a lane's notes belong to |
| 223 | an ERROR is reported once per file: a curve bound 100,000 times, or refused 100,000 times, says it once | a finding per binding would be 100,000 lines of one fault |
| 224 | the message of an ERROR here is a fixed sentence and never carries the timecode or the number that caused it; a WARNING of the decoder may quote a word of the file, cut to 40 characters, and is never past 300 | a timecode or a key of 5,000 characters must not become a 5,000-character message; the decoder's own sentences quote what they refused |

## The gate

`scripts/curveDump.ts` writes the TypeScript's half and `rust/tests/all/curves_stage.rs` compares, in `scripts/stage-gates.sh`
(part of `npm run ci:local`) and by hand (`cargo test --test all -- --ignored curves_stage::`). Over the staged corpus it holds:

| what | count |
|---|---|
| golden curves compared (2 custom, 2 oscillators; no condition, on/off, envelope or cut) | 4 |
| negative cases, of them read by the TypeScript | 66, 65 |
| swept curves compared (the parsed spec field by field, runs, firings, bakes), seeded by `0x2e7f0a11` | 3,200 |
| of them oscillators, drawn, on/off, envelopes | 776, 811, 773, 840 |
| conditions `start`, `end`, `between`, `event`; `retrigger` | 883, 843, 870, 1,815; 1,544 |
| by effect: durations that ended a run; cuts positive, negative that changed a point; curves with more than one run; envelopes with a corner cut short | 228; 77, 222; 548; 266 |
| coverage: grids widened to their 2,048 cap; a position below 1; of four fields; deeper than four (to 13); a fractional tempo | 185; 872; 874; 1,407; 1,617 |
| coverage: triggers off the eighth grid; songs of 1e5 beats or more; signatures of 1/64 or finer (to 1/4096) | 1,098; 508; 581 |
| wired cases, bindings, lanes compared against the real `bakeAutomations` | 220, 509, 402 |
| the most any swept bake spent of the 12,000,000 budget; one full-size lane (28,800 note-ons, two cut envelopes) | 6,389; 633,604 |

Its minimums sit below those counts and above zero: 4 golden curves, 30 negative cases of which 20 with the TypeScript, 3,000
swept curves, 50 of each kind, condition, `retrigger`, `duration`, cut, multi-run, cut-short envelope and coverage row, 150 wired lanes; and
the sweep's maximum and the full-size lane must each stay above zero and ten times below `BUDGET`. A `duration`, a cut and a cut-short envelope count only where the bake shows the effect. The allow list is empty: the Rust and the TypeScript agree on every case.
Where the TypeScript throws (a syntax error, a nest past the parser's limit) the Rust must have a finding for the file.

The negative corpus is `negative_curves.json`, and `curves_negative.rs` reads it everywhere `cargo test` runs, through a Recorder and
again through the bundle it made. A non-UTF-8 file is the one case a bundle cannot replay (decision 14).

## Hostile input

Each is refused fast, in under five seconds in a debug build, and `curves_hostile.rs` asserts the answer as well as the time.

| decoder or pass | the worst input and its answer |
|---|---|
| a curve file | a nest of 300, no `type`, `type: 1`, an unknown `type`, an empty file, `frequency` 0, -1 and `1e400`, `amplitude` 1.5 and -0.1, `phase` 2, `level` 2, `attack` -1 and `1e400`, an unknown `polarity`, `shape` or `cut`, an extra key on each kind, a word of 5,000 characters as a shape, a type, a key, a frequency or a cut (a message under 300), `polarity`, `cut` or `amplitude` and a value of 0.5 on an on/off curve, `points` empty or a number or a point of no `time`, `time` a number or `"x"`, `duration: "x"`, and every shape of malformed condition: each a finding for the file; a million points in one file are read |
| resolving | a position or a `duration` of 5,000 digits is an ERROR (208); a chain of 40 colon fields and a signature of `1/4096` resolve |
| the bake | 200,000 points under 20,000 retriggering events, 100,000 retriggering conditions by a million note-ons, 3,000 windows sharing a million notes outside their loops, an envelope on three million note-ons and a `between` so far out the grid stops moving each end in the budget (211); an oscillator under a `1e300` song is 2,049 points; 100,000 conditions bake; 101 events that are not note-ons on 40,000 note-ons cost nothing; a dozen lanes of 28,800 note-ons with two cut envelopes each fit; an envelope of zero stages on 100,000 note-ons at one beat is finite; `attack: 1e308` at tempo `1e300` is an ERROR (208); one curve bound 100,000 times is resolved once |
| `note_ons` | a loop of `1e-9` in a length of `1e6`, and a loop in a length of 0, end in the budget (211, 222); two million notes are laid within it |
