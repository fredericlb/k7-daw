# A picture lane's sound and a timeline's pictures in Rust (2i)

Sub-step 2i of [rustCompiler.md](rustCompiler.md), whose decisions 1 to 120, output rules, hostile-input rules and stage gate it keeps; 2f's decisions are in
[rustCompilerClips.md](rustCompilerClips.md) and 2e's in [rustCompilerLanes.md](rustCompilerLanes.md). A picture lane's shots are sound as well as picture: 2i turns a
lane's shots into the audio clips the engine plays, heard exactly where each shot is seen, and expands a placed timeline's `pictures:` into shots on the song's picture lane
of the same name, so those sound too. It makes `ClipDoc`s and findings and no track: 2h places them (decision 333). Nothing here is wired to a route. Paths are relative
to `packages/song`.

## Corrections to the 2i row of `rustCompiler.md`

- The row's `timelinePictures.ts` is two things. `timelineShots` and `withTimelineShots` are the compile's; `loadTimelinePictures` is `song-video`'s alone, step 4's, and is not ported (524).
- The walk needs 2f's `Placer` and no name of 2e; what needs 2e is the join (`pictures/join.rs`), which reads `Inputs` and `Lanes`.
- `video_tracks` is offered by 2e and not used here: the sound reads a picture lane's events alone (525).
- `Materials`' tables are `pub(super)`, so two accessors are added (`Materials::timeline`, `Placer::materials`) and `timeline_file` is public.
- The row omits the glue of `nodeProject.ts` lines 218 to 244, which fixes the order of the walk, the merge and the two sound maps; it is 2i's, and lines 245 to 272 are 2h's.
- The 800 lines are low for the whole: the source is about 1,100 lines, and the tests, the gate and the corpus about 3,500.

## What pictures decide (2i)

500 to 528 are the work order's, 540 on are the builder's. A row marked DEPARTS differs from the TypeScript and says why; each such case is `ts: false` in
`rust/tests/all/pictures_negative.json` or a unit test, so the gate's allow list is empty. Each decision is named by its number in the test that holds it.

| # | decision | why |
|---|---|---|
| 500 | the code lives in `src/pictures/`; it imports no `ProjectDoc`, `TrackDoc` or `PluginDoc`, and `Pictures` stays 2h's type | 2h owns the document (333) |
| 501 | a shot is `ShotIn`: a borrow of its `Placement`, its seconds and whether a timeline inside another placed it. A shot a timeline places copies no field of the event | `{...s, start, length, timeline}` clones `animate`, `takes` and the rest per placement, and the sound reads eight fields |
| 502 | `seconds_of` is `Number(x ?? 0)` over a `WindowEdge`: absent is 0, `Seconds` as it stands, a `Timecode` with no `:` is its number, with a `:` NaN | `TIMECODE` admits a bare number string; `Number("2:1")` is NaN |
| 503 | `fade_over`: a `Fade::Map` is its `over` when finite and positive, else 0; a `Fade::Timecode` is 0 | a string has no `over` |
| 504 | `shots_of` is the TypeScript's: a shot ends at `start + length` when `length > 0`, else at the NEXT EVENT's start (an activation included), else open. An activation is dropped only after that, keeping each shot's index. A `speed` not finite is 1; a fade is `js_min`'d to the span | every event times its neighbour; `Math.min` lets a NaN win |
| 505 | DEPARTS. a shot whose `start` or end is not a number has no sound and covers nothing | the TypeScript's `subtract` over a NaN bound answers a piece from a later shot's edge with a `sourceTo` of `null`; only a `:` timecode on a shot reaches it. A shot with a good start and no length, whose next event has a `:` start, loses its sound silently here, where the TypeScript can play a finite piece of it |
| 506 | the covering is a backward sweep: shots from last to first, each asking a `Cover` (a `BTreeMap` of disjoint half-open spans, keyed through `f64::total_cmp`, `-0` normalized) for the free pieces of `[start, end)`, then adding its own `[start + fadeIn, end)`. The answer is in file order, a shot's pieces ascending | the TypeScript copies every later shot per shot: 100,000 shots are 5 x 10^9 steps. `pictures_cover.rs` holds the sweep equal to a naive `subtract` over 20,000 seeded lists |
| 507 | `crossAt` is held by the same sweep: `from -> fadeIn` for each later shot with `fadeIn > 0`, the shot LAST in the file winning a tie, which walking backward means the first inserted stays. A fade spanning its whole shot adds to it and covers nothing | `crossAt.set` in file order |
| 508 | `bps` is `grid.tempo / 60.0`, every beat is `seconds * bps`, every second `beats / bps`: never `Grid::beats`, `Grid::seconds` | those multiply first, and the last bit reaches `placement_id`, a string compared exactly (153) |
| 509 | a piece's `ClipDoc` is the TypeScript's, field for field: `id` `placement_id(ref, a * bps)`, `sourceFrom` only when `> 0`, `sourceTo` only when closed, `fadeIn` the shot's only at `a == start`, `fadeOut` the cross at `b` else the shot's only at `b == end`, a warp only when `speed != 1` (`elastique` if `keep`, else `varispeed`) | the engine plays these |
| 510 | DEPARTS. a piece with a number not finite is dropped with one ERROR per clip file, through `ClipDoc::finite` | decision 156; JSON writes `null` |
| 511 | a shot has sound when its clip is `Clip::Video` and its `sound` word is non-empty, its `speed > 0` and it is not `silent`; a silent, freeze or reverse shot still covers. `ClipSounds` is built once, never a `FamilyRead::get` in a loop | `clipsMap[ref]?.sound`; 100,000 shots over 100,000 clips |
| 512 | a word's path is `spell(word)` when absolute, else `spell(join(project, word))`; `stat` runs once per distinct word and only when a piece is about to be made. `Ok` of any kind counts, a folder included | `resolve` then `wsExists`, called for names that made a clip only |
| 513 | `NotFound` is one `SoundMissing` fault per (lane, ref) and one ERROR on the clip's file `clips/<its file name>`, which stops the compile | `missingSounds`; the word is on the clip file, so that is what the finding names |
| 514 | DEPARTS. a word the Source refuses (`Outside`, a NUL, no spelling) or cannot stat (`Io`) gives no pieces and one WARNING on the clip's file per word. `Missing` is `Err` | the TypeScript hands the engine any path, unconfined; a refused file costs that sound, not the song |
| 515 | a `SoundFault` holds strings cut to 200 characters; its message quotes a word cut to 40 and is at most 300 bytes; past `MAX_FAULTS` the walks stop with one `More` | decisions 259, 260; a 1 MB word copied 100,000 times is 100 GB |
| 516 | `video_sound_lanes` is the TypeScript's: a lane with no `video_source` is skipped; `events` beside it is `VideoSourceBoth`, a name that is no picture lane is `VideoSourceAbsent`, each one ERROR on the lane's file and that lane absent from the answer. DEPARTS: a name spelling an `Object.prototype` key is a plain name | `videoLanes["constructor"]` is truthy in JavaScript (357) |
| 517 | `picture_lane_sounds`: `sounded` is one `HashSet` of `video_source`s built once, a disabled lane's included; `wired` one `HashSet` of chains' `from` with `from_port == "audio"`; a picture lane sounds when not `sounded` and it has clips or is wired; a disabled picture lane still sounds | `audioChainsOf` walks every chain per lane; the mute is the picture's |
| 518 | `timeline_shots`: a ref in no timeline, a `disabled` event or a timeline on the path is skipped; `start` is `within.offset + grid.position(p.start)`, `end` is `js_min(start + timeline_length, within.end)`; each picture lane not disabled is placed, then each sound lane not disabled recursed into. DEPARTS: depth is `MAX_NEST`, past which it is cut silently | recursion over a file's data needs its bound, and two findings for one cut is noise |
| 519 | `place_shots` is the TypeScript's: `room = end - start`, skip when `room <= 1e-6`; `length` the stated one when `> 0`, else the next entry's start minus `own`, else `room`; skip unless `length > 0`; the shot's length is `js_min(length, room)` | a shot with no length gets the one the picture would give it, in its own lane |
| 520 | a lane's placed shots are appended, in first-seen lane order, with `IndexMap::entry` | `[...old, ...shots]` is quadratic over placements of one lane |
| 521 | `with_timeline_shots` serves the picture lanes that exist, the placed shots first; a timeline lane with no song lane of its name is dropped here | the picture compile says so (step 4); first, so the lane's own open shot does not end at a placed one |
| 522 | `BUDGET` weighs what the walks visit and copy, charged before the copy: an event or a lane visited (a disabled one included), a shot placed, a merged shot, a span the `Cover` touches, a stat, a piece `8 + (2 * ref.len() + source.len()) / 64` | the sound's fan-out: 5,000 fade-only shots over 5,000 short solid ones are 25 million pieces from 300 KB |
| 523 | past the budget: one ERROR named the file it stopped in (a timeline's, a lane's, the picture lane a merge reached), `capped`, nothing more placed, and that stops the compile | 2h's 304: a limit is a `Failure` |
| 524 | `loadTimelinePictures` is not ported | its one reader is `song-video`, step 4 |
| 525 | the sound reads `Lanes::picture`'s `events` directly | `video_tracks` adds the rect and the bypass, which no sound rule reads |
| 526 | the join runs inside each pass, on that pass's `Placer`, in `nodeProject`'s order: placed, merged, video sounds, picture sounds; 2h's `Pictures::none()` becomes `picture_sounds(...)?` | the same `Placer` shares its length memo with the lanes' own expansion, as `timelineCtx` does |
| 527 | no ts-rs twin, no `HashMap` in a serialized type, no `Rc` or `RefCell`; a sound file is stat'ed and never read | spec; a bundle holds it by its length |
| 528 | a lane named by an integer-like key is met in file order | decision 256 |
| 540 | a lookup weighs `1 + len / 64` for a shot's `ref`, a timeline event's `ref` and a lane name | a 1 MB name looked up 100,000 times is 100 GB of hashing |
| 541 | `Spend::in_file(&str)` and `Spend::file()`: the cap ERROR names the file the caller set, and the join calls `in_file` before each lane's walk | the walk has no `Inputs` to say where a lane lives |
| 542 | in a timeline's pictures lane an entry visited weighs 1 and a shot placed weighs 1 more | a timeline of 100,000 shots placed 1,000 times is 10^8 |
| 543 | past the budget `Sounder::pieces` returns `Ok(vec![])`, after the one ERROR | the answer of a stopped compile is not read |
| 544 | the timeline half of the hostile table is in `tests/pictures_hostile_placed.rs` | `pictures_hostile.rs` passes 400 lines on its own |
| 545 | `More` has an empty holder and target, key `more||`, and is the 1,001st fault, with one ERROR where the walk stopped | the gate compares it as any key |
| 546 | findings are deduplicated on (file, message) | one clip file named by 100,000 shots is one finding |
| 547 | `video_source: ""` counts as absent and is left out of the `sounded` set | `if (!from) continue` and `l.videoSource ? [...] : []` |
| 548 | `ClipSounds` interns the sound words once; a repeated clip id keeps the first | one verdict per word, not per clip |
| 549 | `placed_shots_of` is the walk over `nodes/<lane>.k7x` labels; the join walks with each lane's own label (`own_label`: `nodes/`, `tracks/` or `chains/`) | a stop is named by the file it is in |
| 550 | a stop while merging is named by the file of the picture lane it stopped at: `with_timeline_shots` takes each lane's file | 523 names the file it stopped in |
| 551 | the join's walk over the sound lanes charges `weigh(name)` per lane visited, a lane with no events included | 100,000 empty lanes are a walk |
| 552 | the join builds a `SoundLaneIn` only for a lane with a `video_source` | the other lanes need nothing the sound reads, and a lane's file name is not copied for them |
| 553 | the `video_source_absent` message carries the TypeScript's backticks around `@meta/picture-track` | the wording is the user's to read; the gate never matches it |
| 554 | a timecode's text weighs `len / 64` more wherever it is parsed: a timeline event's `start`, a placed shot's `start` and the next entry's, a lane's own `start`s, a shot's `from` | a 1 MB `start` placed 20,000 times is 20 GB of parsing |
| 555 | the join runs at the top of each pass, on a `Placer` made there and handed on to the lanes; `Pictures` is the pass's own value, made before the plugins' claims are counted | the claims count the picture sound's halves, and a second pass joins again on its own `Placer` and its own length memo |
| 556 | the join's ERRORs stop the compile with the lane-wiring ones and count in decision 311's 1,000 (2h's list): the wiring walk starts at their number | the user reads one list; a WARNING goes to the diagnostics and stops nothing |
| 557 | the document gate compares picture sound through `docPictures.ts` (240 projects of the picture sweep, seed `0x2e7f1b34`, each given a graph) and the 17 `pictures-` cases of `doc_negative.json`; its counts are taken off the tracks and the project's files, the staged corpus, the negative cases and the sweep kept apart | the staged corpus and `docSweep.ts` hold no `sound:` and no `video_source` |

## The seams

| neighbour | what 2i gives it | what 2i takes from it |
|---|---|---|
| 2f | | `ClipDoc`, `ClipKind`, `WarpDoc`, `placement_id`, `js_min`, `Budget`, `Grid`, `Placer::timeline_length`, `MAX_NEST`, `audio_paths`' rule, `Materials::timeline`, `Placer::materials` |
| 2e | | `Lanes`, `Lane`, `LaneKind`, `Inputs` (`project`, `chains`, `clips`, `local`), `stops` (the gate only). Not `video_tracks` (525) |
| 2d | | `Chain::from`, `from_port` |
| 2h | `PictureSounds`, whose two maps are `Pictures`' fields and whose findings join decision 311's list (526) | `Pictures`, `Pictures::none`, the pass's `Placer` |
| 2l | the risk list | the bundle's shape |
| step 3 | `SoundPiece::shot`, what `picture sound`'s verbs address | |
| step 4 | `timeline_shots`, `with_timeline_shots`, `ShotIn::timeline` | `loadTimelinePictures` and the picture compile stay there |

## The gate

`scripts/stageDumpPictures.ts` runs `loadCompileInputs` and then exactly lines 218 to 244 of `nodeProject.ts`, catching a throw as `threw`, and writes `video_sounds`, `picture_sounds`,
`placed` and `faults`. The faults are `laneErrs` lines parsed by anchored patterns to `kind|holder|target`; a line none matches fails the dump, and no Rust reads a message.
`tests/stages_pictures.rs` (`#[ignore]`, `K7_SONG_STAGED`, `K7_SONG_DUMPS`) reads each project through a `Recorder` and again through the bundle it made, one `Cache` shared, and compares
with an empty allow list. Where the TypeScript threw at load the Rust must stop; where it did not, the Rust must not. `tests/pictures_negative.rs` runs the corpus wherever `cargo test` does;
its `expect` holds the exact files with an ERROR, the exact fault keys and pointers into the answers, every case proving its effect by one (a case of silence carries a control shot that sounds) and the files a stop names empty unless it lists them.

The corpus is 68 cases, 63 with the TypeScript; the TypeScript's values are written from its own dump, the five DEPARTS cases (505, 510, 514, 516, 518) by hand. The sweep
(`scripts/pictureSweep.ts`, seeded `0x2e7f1b33`, written with `testSupport.ts`) is 600 projects, none writing a `:` timecode on a shot or a number that overflows.

First green run, counted over the Rust's own answers that compared equal:

| | staged corpus | negative cases | sweep | floor (negative and sweep together) |
|---|---|---|---|---|
| projects, both answers empty and equal | 177 | | | 170 |
| picture lanes; with a frame; events on them; activations | 9; 4; 8; 3 | 70; 0; 96; 2 | 1,163; 600; 17,472; 1,360 | staged 8; 3; 6; 2 |
| cases; with the TypeScript; sweep projects | | 68; 63 | 600 | 60; 55; 500 |
| pieces | 0 | 99 | 6,232 | 4,000 |
| shots that re-entered after a cover; cut by more than one later shot | 0 | 7; 1 | 387; 34 | 250; 25 |
| crossfades taken; with the last in the file winning | 0 | 3; 1 | 944; 251 | 600; 150 |
| open-ended pieces; with a `sourceFrom` | 0 | 5; 14 | 147; 2,120 | 100; 1,400 |
| warps `varispeed`; `elastique` | 0 | 5; 2 | 611; 180 | 400; 120 |
| silent, freeze, reverse shots, each covering something | 0 | 1, 1, 1 | 865, 322, 349 | 550; 200; 220 |
| lanes sounded through `video_source`; pictures sounded by their own half; wired with no clips | 0 | 7; 60; 1 | 432; 578; 38 | 300; 400; 25 |
| faults: both; absent; missing | 0 | 1; 2; 3 | 70; 44; 64 | 40; 30; 40 |
| placed shots; on a nest; cut at a timeline's end; given a length by the rule; merged before a lane's own open shot | 0 | 53; 35; 3; 8; 1 | 2,849; 506; 706; 1,025; 465 | 1,800; 400; 450; 650; 300 |
| placed shots that then sounded | 0 | 18 | 269 | 180 |
| the most any call spent | 36 | 334 | 970 | asserted ten times below `BUDGET` |

"On a nest" is a shot a timeline placed from inside another timeline (`ShotIn::nested`); "merged" is a placed shot on a lane whose own last shot is open, which a placed shot would end if it came after; "cut" is a shot placed shorter than it states; "given a length" has none stated; "re-entered" is a shot with two
pieces or more, "cut by more than one later shot" three or more, and "last in the file winning" a crossfade whose fade is the last of two differing ones. The staged corpus holds no
sound, no timeline on a picture lane and no fault of these kinds, so every number below its first row is counted over the negative cases and the sweep, and printed. The gate was
shown to bite: keeping the last crossfade of a tie instead of the first fails it with 269 differences.

In a document (`song::compile`, `tests/pictures_doc.rs`): the three cases of `videoSound.test.ts` that compile and the six of `pictureLaneSound.test.ts` are ported, with a case
for a refused word, one for the shared list of 311 and two hostile ones. A `toThrow` is asserted by the file the failure names, never by its wording:

| TypeScript's wording | held as |
|---|---|
| `video_source names 'nope'`, `and events: both` | a failure naming `nodes/dialog.k7x`, each beside a control that compiles |
| `sounds clips/b.k7x`'s missing file | a failure naming `clips/b.k7x`, beside a control with the file |

The document gate counts what picture sound did in each document that compared equal to the TypeScript's: 188 picture projects and 91 negative cases compared, 52 and 9 refused by both. The staged corpus holds
3 picture lanes and no sound, so its floors are the lanes alone; the 16 negative cases with the TypeScript hold 14 sound clips on 10 own tracks, and the 240 picture projects 1,046 clips on 200 tracks
and 717 on lanes sounding a picture lane. Each floor was below the first green run of the dropped document gate. The gate bites: no crossfade marks fail it with 174 differences, a tempo off by one
part in 10 million in the join with 1,541.

## The hostile table

Every case is answered under five seconds in a debug build (thirty on musl) and asserts the answer as well as the time: `pictures_hostile.rs` for the sound, `pictures_hostile_placed.rs` for
the timelines, `pictures_join.rs` for the join over a real project.

| pass | cases and answers |
|---|---|
| `Cover` | 100,000 spans adjacent, nested, identical and each overlapping the next: linear; `+inf` ends |
| `Sounder::pieces` | 100,000 shots all sounding: 100,000 clips; 100,000 of which one sounds: one stat; 5,000 fade-only over 5,000 solid: stopped at the budget, one ERROR, `capped`; a 1 MB `sound:` named by 100,000 shots: stopped by bytes; a 1 MB `ref`; `start` near `1e308` at 300 bpm: dropped with an ERROR, no `null`; 100,000 activations |
| the sound files | 100,000 distinct absent words: 1,000 faults and `More`, the folder listed once; one word on 100,000 shots: one stat; `../../x.wav`, an absolute path out, a NUL, `a\b.wav` (a character on Unix, a separator on Windows), `C:/x.wav` (a folder of the project on Unix, absolute on Windows), a link out, a folder, a dangling link |
| `video_sound_lanes`, `picture_lane_sounds` | 100,000 lanes naming one lane of 100,000 shots: stopped at the budget; 100,000 naming nothing: 1,000 faults and `More`; a 1 MB `video_source`; 100,000 picture lanes |
| `timeline_shots` | six levels of 100 events: stopped, one ERROR; a nest of 100,000 cut at 32; a timeline in itself; a cycle of ten each way; one lane appended 100,000 times; a timeline of 100,000 shots placed 1,000 times: stopped; 100,000 disabled `pictures` entries or `lanes` entries placed 300,000 times: stopped, one ERROR; a `start` of 1 MB on a shot or on an event, placed 20,000 times: stopped; a name of 1 MB |
| the join | 20,000 lanes naming nothing: 1,001 faults; a walk that runs out: one ERROR on the timeline it stopped in |
| the document | 5,000 fade-only shots over 5,000 solid ones through `song::compile`: one ERROR on the lane's file, which stops it; 1,000 lanes naming nothing beside an unwired event: the pictures' 1,000 errors fill 311's list and the event is not named; 200 sounding lanes of 100 shots: whole |
| capacity | 8 picture lanes of 2,000 shots and a timeline of 500 shots placed 32 times: 32,000 shots, 30,080 pieces, 422,806 of 2,000,000 spent (a fifth, under the third asked), in 56 ms |

## New risks

| risk | answer |
|---|---|
| a `sound:` behind a link out of the mounts, or whose stat is `Io`, replays as `Missing` from a bundle | 2l adds its `refused` list; the bundle's shape is not changed here |
| the sweep's project shape is what the generator knows, not what a person edits | 2l's shadow on the dev account; every real shot and timeline comes through there |
| the gate's counts of re-entries and crossfades re-ask the shots of the Rust's own lanes, not the dump's | they describe what the Rust did; the comparison with the TypeScript is the dump's |
