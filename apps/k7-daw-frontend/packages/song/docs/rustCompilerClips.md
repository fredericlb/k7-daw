# Placements to clips in Rust (2f)

Sub-step 2f of `rustCompiler.md`: one event of a lane read into the `ClipDoc` the engine plays, a comped event cut into its
pieces, a timeline placed as one event expanded into what it holds, and a pattern lane's plugin. Nothing here is wired to a
compile: 2h calls `song::clips::…`. Paths below are relative to `apps/k7-daw-frontend/packages/song`.

## Corrections to the 2f row of `rustCompiler.md`

- Of `takes.ts` and `warpPoints.ts` only three functions are on the compile's path (`compPieces`, `seamCrossfades`, `readClipWarp`).
- The row omits `placementId.ts`, `timelineLane.ts`, `startBeats` and `SHOT_KEYS`.
- `confineTo` becomes a stat through the Source (decision 154).
- No row owns `timelineAudition.ts` or `loadAudioClipDoc`: step 1 does.
- The row's 1,600 lines are too few: the Rust and its tests are about 6,000 lines.

## What placements decide (2f)

A row marked DEPARTS differs from the TypeScript; each such case is `ts: false` in `rust/tests/all/clips_negative.json` or a unit test.

| # | decision | why |
|---|---|---|
| 150 | the `ClipDoc` family lives in `clips/doc.rs`, serialized as `project.ts` spells it; no ts-rs twin | decision 43; 2h derives the twins with the document |
| 151 | the core is pure over typed inputs; `audio_paths` and `read_pattern` are the only functions that call the Source | testable without files; the compile's reads stay in two places a bundle can account for |
| 152 | `placement_id` writes a start as `Number.prototype.toString` does (`js_number`), an exact tie between two candidates that both read back taking the even digit | an id is a string compared exactly, and Rust's `{:e}` rounds a tie up |
| 153 | every beat computation keeps the TypeScript's order of operations; `Grid::bar()` is `(num * 4) / den`; `===` stays exact and `EPS` stays `EPS` | the last bit of a start reaches an id |
| 154 | an audio path is `spell` of the source (absolute) or of `join(project, source)`, then one stat per clip: `Ok`, `NotFound`, `Io` keep it, `Outside` or a NUL answer `""`, `Missing` is `Err`; a Recorder lists the folder above absent files once | the Source is the confinement; a missing file is 2e's to name; 8,000 absent clips are not 8,000 listings |
| 155 | DEPARTS. a shot's key on sound, a transition-map fade or no `ref` is one ERROR per event named by its file, the rest compile | the TypeScript throws at the first; findings name a file |
| 156 | DEPARTS. a clip holding a number that is not finite is dropped with an ERROR; a timeline whose length is not finite has no `TimelineDoc` and an ERROR on its file | JSON writes `null` for them |
| 157 | DEPARTS. `BUDGET = 1,000,000` per `Placer`, weighing the output: each event or lane visited, each clip as `1 + notes + markers + the event's words` charged BEFORE the copy (a comped event's words again for each piece, which copies them), each length visit and each lane, event and picture it walks, a memo hit and a refusal included, and `prefix.len() / 64 + 1` per clip placed inside a timeline for its longer id; past it one ERROR, `capped`, nothing further placed | a timeline of 1,000 events of a timeline of 1,000 events is 10^12 clips from 60 KB; the copies and the ids are the output |
| 158 | DEPARTS. a nest deeper than `MAX_NEST = 32` is cut with one ERROR on the timeline whose entry was refused; a timeline on the path is cut silently | recursion over a file's data needs a bound |
| 159 | DEPARTS in a cycle only. a length enters the memo only when nothing below it was cut by the path or the depth | the answer must not depend on which timeline was asked first |
| 160 | DEPARTS. every piece of a comped event carries the event's `disabled` | a muted comped event kept playing its takes |
| 161 | the event's `gain` stays on its own material's pieces, as the TypeScript | open for the user: one line in `comp_clips` |
| 162 | `comp_pieces`' cut and `seam_crossfades`' finds charge one per piece visited; `mark_crossfades` finds each candidate by binary search near `a.end - a.fade_out`, charging one per pair | 100,000 clips on a lane are 5 x 10^9 naive pairs |
| 163 | a picture or caption clip on a sound lane is an audio `ClipDoc` of its source with no window and no map | `cm.from` and `cm.warp` are absent there |
| 164 | DEPARTS. a pattern is read through the Source: `NotFound` or a folder answer `""` silently, `Missing` is `Err`; `Outside`, past `MAX_PATTERN = 1 MiB`, a NUL or not UTF-8 answer `""` and a WARNING named by the `strudel` path | the TypeScript read any path, unconfined; a limit counts the output |
| 165 | `pattern_shares` keys files by the normalized path and sorts lanes by `str`'s `Ord` | `algo/a.strudel` and `algo/x/../a.strudel` are one file; code point order |
| 166 | `pattern_plugins` returns `PatternPlugin`; `notes === audio` compares positions in `halves` | `PluginDoc` is 2h's |
| 167 | `sample_banks` is `join(project, "samples")` then `Roots::sample_banks`; neither is read | the host knows the machine's folders |
| 168 | DEPARTS in key order only. `timeline_docs` is in family order | the engine ignores them and the view finds them by id |
| 169 | `fixed_fields_of` and `FIXED_FIELDS` live in `clips/timelines.rs`; a timeline event carrying a fixed field still expands | the refusal is `checkReferences`' |
| 170 | `Placement` derives `Default`; a take is compiled as `Placement { ref_, start, ..Default::default() }` through the same function | no second code path to drift |
| 171 | the gate is its own: `scripts/stageDumpClips.ts`, `tests/stages_clips.rs`, `tests/clips_negative.json`; its lane projection is a test-only mirror of `laneOf` | `stageDump.ts` and `negative.json` are 2d's |
| 172 | a chain project is `chain: true` in the gate and not compared | its lanes are 2d's reading |
| 173 | DEPARTS. a NaN start sorts last in `comp_pieces` and `mark_crossfades` | JavaScript's subtraction calls a NaN equal to everything, which is no order and panics a Rust sort |
| 174 | `Materials`, `Placer`, `Leaf`, `Within` live in `clips/placer.rs`, not `mod.rs` | `mod.rs` keeps the grid and the budget |
| 175 | a length cut by the depth is silent and stays out of the memo; the expansion names it | one finding per cut, not one per timeline that asked for a length |
| 176 | `comp_pieces`, `seam_crossfades` and `comp_clips` answer `None` past the budget; `comp_clips` and `mark_crossfades` take the budget, and `comp_clips` a closure that compiles a take | the caller lends the one budget a pass spends |
| 177 | the unit tests of `audio_paths` are in `clips_paths.rs` | `clips_placement.rs` stays under 500 lines |
| 178 | `pattern_shares` answers the shares and the findings: a file serves at most `MAX_FILE_LANES = 32` pattern lanes and a project `MAX_LANES = 128`; a lane past either has no share, so no plugin, and its file gets one ERROR naming its strudel path | each lane carries the file's text (up to 1 MiB) and every claimed name per half, so the cost is the square of the lanes; a realistic project has a handful |

## The gate

`tests/stages_clips.rs` reads each project through a `Recorder` with one `Cache` shared across projects, again through the
bundle it made, and compares lanes, lengths, timelines, shares, banks and pattern plugins with `compare.rs` against
`scripts/stageDumpClips.ts`, with an empty allow list. A lane the TypeScript threw on must carry an ERROR on its file.
The staged corpus and 66 negative cases (60 with the TypeScript) hold 172 projects (5 chain projects skipped),
427 lanes, 455 events, 415 audio clips, 113 sequences, 11 pattern lanes, 31 takes, 32 comp pieces, 4 crossfade marks,
58 timeline docs, 52 nested clips, 2 seconds starts, 4 fractional ids, 13 loop ranges, 9 event windows, 47 clip windows,
7 gains, 3 warps, 4 declined warps, 8 disabled, 1 pad effect. The most any pass spends is 1,662 of the budget; the gate fails past a tenth.
An expected string of a negative case may name `{project}` and `{workspace}`, spelled as the run spelled them.
`tests/clips_negative.rs` is the same corpus without the TypeScript and runs wherever `cargo test` does.

## Hostile input

Each is refused fast (`tests/clips_hostile.rs`), asserting the answer as well as the time.

| pass | worst input and answer |
|---|---|
| `expand_events` | six levels of 100 events: capped, one ERROR; a chain of 100,000 timelines: cut at 32, one ERROR; a timeline in itself and a cycle of 10 events each way: cut silently; a cycle of 100 each way, which has no memo: capped; 30 nested timelines named by 200 characters holding 64,000 clips: capped by the ids' bytes |
| `timeline_length` | a fan-out of 40 levels over a cycle: capped; 12 levels over a cycle ending in 2,000 pictures: capped; `"0:0"` is 0; a 400-digit bar is not finite and has no view |
| `placement_to_clip` | a sequence of 100,000 notes placed 1,000 times: about 10 placed, then the budget; a loop near 2^53 on a window reaching 1e308 is refused with one ERROR, on no window it is finite |
| `comp_pieces`, `seam_crossfades`, `comp_clips` | 100,000 spans and 100,000 crossfades: `None` at the budget; a take list of one ref 100,000 times answers; 20,000 takes and 200 spans, whose pieces copy 4,000,000 words: capped |
| `mark_crossfades` | 100,000 clips overlapped by their fades: linear, all marked; 50,000 against 50,000 at two starts: stopped at the budget |
| `read_pattern`, `pattern_shares` | over 1 MiB (never read), not UTF-8, a folder, a climb out, a NUL; 3,000 lanes on one file and 200 files: 32 and 128 shares, one ERROR per file left out |
| `audio_paths` | a climb, `/etc/passwd`, a NUL, a `\`, 100,000 clips; 2,000 absent clips through a Recorder list their folder once |

## Risks

| risk | answer |
|---|---|
| an audio file's length is not read here | the browser's lanes get it from the view (2j) |
| a pattern lane's text is copied per half, up to 128 lanes of 1 MiB | decision 178's limits; weigh the text in bytes if a real project nears them |
| the Recorder keeps nothing for `Io`, nor for `Outside` through a link, and `MemSource` holds a non-UTF-8 file by its length only, so a bundle replay answers differently from the disk and asks for more files for ever | the bundle's shape is 2l's (the `refused` paths list and "not text" in `rustCompiler.md`); four cases wait for it |
| audio `clips/a.wav/sub.wav` (`ENOTDIR`) keeps its path on disk and is `Err(Missing)` in the bundle | 2l |
| a symlink out of the mounts (`clips/link.wav`) answers `""` on disk and `Err(Missing)` in the bundle | 2l |
| a symlink out of the mounts (`algo/link.strudel`) answers `""` and a WARNING on disk and `Err(Missing)` in the bundle | 2l |
| a non-UTF-8 `algo/bad.strudel` answers `""` and a WARNING on disk and `Err(Missing)` in the bundle | 2l |
