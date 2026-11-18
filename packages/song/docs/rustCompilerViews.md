# The views in Rust (2j)

Sub-step 2j of `rustCompiler.md`: what the frontend READS of a project, apart from the document the engine plays. It holds the
surface file, the field vocabulary, the WAV header and `Measure`, one clip for the browser, the catalogue and the base modules, the
surface issues, the I/O ports, the hints, the web project, the error list and `answer`, which gives a host any of them from one
compile. Nothing here is wired to a route (2l). Paths below are relative to `packages/song`.

## Corrections to the 2j row of `rustCompiler.md`

- The row omits `diagnostics.ts`, `deadAutomations.ts`, `compileError.ts`, `moduleRefs.ts`' `listBaseModuleRefs` and `panelOf`,
  `surfaceFile.ts`, `surfaceSchema.ts`, `mixViewSchema.ts`, `wavLength.ts`, `wav.ts`' `wavLayout`, `nodePlace.ts`,
  `moduleAssetChoices.ts`, `listModuleAssets`, `listFiles.ts` and `scannedPluginNames`.
- `workspaceDocs.ts` and `webTracks.ts` are taken in part: the picture half and `projectInfoOf` answer no compile route.
- `Request.want` is an argument of `views::answer`, and the flat `Compiled` splits into 2h's `Compiled` plus `Views`.
- `graph` stays out of `Want`; `schemars` is not used (decision 607); `Nullable<T>` lands here for a surface's `mapping` alone.
- A bundle holds a binary by its length alone, so a replay measures through a table (decision 611).
- `Failure.format_too_new` is a bool; the two levels the frontend shows are read off the head (decision 641).
- The surfaces, hints, I/O and catalogue answers exist when the compile fails, which a field of the compile's answer cannot say, so
  `Views` holds them beside `Compiled`, which carries no surface head: the recorder reads `project_surfaces` itself (decisions 642, 643).

## What the views decide (2j)

A row marked DEPARTS differs from the TypeScript; each such case is a unit test and not a stage-gate comparison.

| # | decision | why |
|---|---|---|
| 600 | the code lives in `rust/src/views/`; Part A names no `ProjectDoc`, `TrackDoc`, `PluginDoc`, `Compiled`; A1 names no type of 2e, A2 stands on `Inputs` | the seams between the parts |
| 601 | `Nullable<T>` reads `null` as `Nullable(None)` through `deserialize_any` and gives every other shape to `T` through the matching `serde::de::value` deserializer; it serializes `null` | an `Option` refuses `null` where a value goes (`files/de.rs`), and `null` is a value for a mapping entry |
| 602 | `decode_surface` is a derive with `deny_unknown_fields` plus one check after it, in the TypeScript's order: an empty file, the schema, an empty mapping key or string, the layout pair on `layout`, `playlist` on `playlist`, `stems` on `stems` | `parseSurface`; a refinement is placed by the key it names |
| 603 | a surface is read through a `surfaces` slot of the `Cache`; `id` is the base name, `file` is `surfaces/<name>`; a finding is an ERROR at its real place; the surfaces come in the order of the `[base, name]` pairs sorted as strings with `jsorder::pair_cmp` (`10` before `9`, `timeline` before `timeline-2`, `a` before `a.b`), which is not the order `read_family` lists them in | `projectSurfaces`, whose `sort()` compares `"base,name"` |
| 604 | DEPARTS. a surface behind a link out is a WARNING naming it and absent; a file that will not read is an ERROR and absent; a `surfaces/` refused whole is one WARNING | the TypeScript reads a link out unconfined; a file costs that file |
| 605 | `project_surfaces` fails on `Missing` alone | the surfaces view exists when the compile refuses |
| 607 | the editor's keys come from the derived decoders themselves, asked through a private `Deserializer` that captures the `fields` argument of `deserialize_struct`; hand-dispatched decoders are a `const` beside the probe | one declaration of a file's shape serves the decoder and the editor; wasm32 stays clean |
| 608 | `fields_at` and `values_at` answer as the TypeScript does, omissions included; a curve's `type` offers `lfo`, `custom`, `adsr`, `onoff` on both sides | the completion lists; the lattice compares every answer with the TypeScript's |
| 609 | `file_kind_for` is the TypeScript's regular expressions by hand, in order, `\` first read as `/`; the folders are case-sensitive, the chain-file, `chains.k7x` and `project.k7x` rules are not (ASCII) | no regex crate; a line end ends a `.` as in a regular expression |
| 610 | `wav_seconds` is `wavLayout` plus the division over a `&[u8]`, every offset checked in `u64`; a hostile header answers `None`; a 0 declared `data` size is 0 seconds; a chunk id is compared with the high bit of each byte dropped, as Node's `"ascii"` decode reads it | wasm32's `usize` is 32 bits |
| 611 | `web_clip` asks a `Measure`, never a Source. `ByHeader` stats and range-reads `HEAD_BYTES` through the Source; `ByTable` answers from the `Lengths` a server made, `None` for a path not in it, never `Missing`. `ByHeader` over a bundle answers `Missing` | a bundle holds a binary by its length alone |
| 612 | `answer` measures every `Inputs.audio_paths` value and every audio `source` of the document the first time, `ByHeader::measure_all`, not only the clips of length 0 | a draft that removes a `to:` or places a clip must find its length in the table; the header is 8 KiB, held by stamp across compiles (613) |
| 613 | DEPARTS. a length is held in the `Cache` by path and SETTLED stamp only; the map is dropped whole past 100,000 entries | `docs/moduleCache.md`'s rule |
| 614 | `MAX_HEADERS = 20,000` reads per `ByHeader` that the cache did not answer; past it a path answers `None` and `finish` adds one WARNING named `clips` | a limit counts the walk |
| 615 | `web_clip` is `toWebClip` to the letter: `length` is the clip's, else the natural beats of an audio clip with a source and a tempo above 0 (held to the largest double, since `inf` has no JSON); `sourceName` the last `/` segment; the flags `true` or absent; a zero fade absent; `vel` absent at 1; `repeat` and `skip` absent at 0 | the truthiness of each line |
| 616 | `fold_comps` groups pieces by `comp.of` in an `IndexMap` once, the group at the first piece's place, pieces sorted by a stable `total_cmp` of `start`; `start` is the number after the last `@`, else the first piece's | `foldComps`, without its quadratic regrouping |
| 617 | `fold_nests` is the TypeScript's: a host at its first clip's place, `midi` only when all it holds is, the inner clips folded before the lane | `foldNests` |
| 618 | no budget on `web_clip`, `fold_comps`, `fold_nests`: the output is one clip per input clip and no copy multiplies | a pass with no product says why |
| 619 | the catalogue's roots are `Roots::module_roots()` without `@meta`, a later root of a namespace winning | `sharedRoots` has no meta, and `new Map(...)` keeps the last |
| 620 | a catalogue row: a `@` ref, not a node, no `engine_of`; with a root its `path` is the absolute `module.k7x` when a stat says a file, else the row is dropped; with none the ref must be one segment; the host relativizes `path` | `moduleFileFor` |
| 621 | `base_modules` is `base_module_refs` plus `panel_of`, which is `Some` only when both sides are above 0 | `listBaseModuleRefs` |
| 606 | `surface_issues` starts from what `project_surfaces` found; `has_material` is a stat of `clips` and of `sequences` (`Ok` or `Outside` is true, `NotFound` or `Io` false, `Missing` is `Err`) or a picture lane of `Inputs`; the two warnings are named `surfaces/` and keep the TypeScript's sentences | `existsSync` is true through a link out; the sentence is a contract of `surfaces.md` |
| 622 | `io_ports` lists the raw vocabulary's `io: in` and `io: out` modules, then a project node whose RESOLVED module is `kind: io` unless the vocabulary has its key; a node that does not resolve is read as written; inputs first, then `ref` by code point | `ioPorts`; `byCodePoint` is `str` Ord |
| 623 | `places.rs` holds `place_of` and the chain-project walk of `nodePlaces`, private to `views`; 2k's `node_places` is the same walk and the combine folds one into the other | `io_ports` must not wait on 2k |
| 624 | DEPARTS. a port weighs its `ref`, `name`, `label` and 8 bytes per channel; the channels of a numbered `channel` are read from its first 256 entries and cut at the rig's 64; past the budget one WARNING named `project.k7x`, `capped`, the rest absent | a limit counts the output; 10,000 nodes on a parent of 100,000 channels is `10^9` copies |
| 625 | DEPARTS. `hints_of` stands on `Inputs`, read tolerantly: a broken node, clip, sequence, curve or graph costs what it costs on its own; `Err` only for `Missing` | the TypeScript loses the whole answer on a syntax error in `clips/` and every module on one bad node; completion on a half-written file is the moment for it |
| 626 | `chains` is the keys of `derive_chains` with no lane predicate, derived again, sorted | `deriveChains(loadChains(dir, {}))` passes no `isLane`, and the key of a run depends on it |
| 627 | `modules` is `completion_order` over the sorted node names then the shared refs, status from the authored node else the vocabulary; `ioSinks` is `MIX`, `OUT`, `CUE`, `RENDER` and `VIDEO_OUT` with a picture lane, then the vocabulary's io modules that are not inputs, sorted | favourites first is the point of `status` |
| 628 | `paramsByBase` indexes the extenders' param names once over the nodes and charges a base's work (own, built-in and extender names) before it builds its set; `paramMetaByBase` only for a base with a built-in, charged per entry | `byExtends` is the one index; a built-in of 767 params on 1,000 bases is a product |
| 629 | `paths` is `<kind>:<name>` to the relative file when one stat says it is there; a name holding `/`, or on Windows `\` or `:`, has none; a link out has none; every chain shares the one `chains.k7x` stat | `addPath`; a name must not climb out of its folder |
| 630 | DEPARTS. `media` is a walk of 32 levels skipping dot names, a module's `assets/` one of 6; the Source reports a link as its target, so a link to a folder is entered, and a folder that is one on the path to it (or one of the 32 folders above the project, up to the mount) is not: `current -> .` and a link to the workspace list nothing twice. `Source::folder_id` says which folder a path is (a disk answers its canonical path, so equal mtimes and equal listings make no loop); a source that answers none (memory) compares stamp and listing, and a match there is a loop only when every folder below it was never listed (`Missing`, not a refusal or an I/O error), so `p/a` and `p/a/a` listing only `a` are two folders and a bundle lacking one child asks for its files. A folder the Source refuses is walked past. A link to a FILE is offered as a file, where the TypeScript's walk is `regularOnly`. A listing costs 8 units asked and 200 when the folder is there; each entry is a step and each kept name its bytes | the TypeScript's Dirents do not enter a link and its depth is unbounded; a listing is a disk read; the Source cannot tell a link from its target |
| 631 | `walk.rs` memoises a module's assets per folder; `moduleAssets` climbs the RAW `extends` of the nodes through the raw vocabulary, 16 levels, every entry examined a step and every kept name its bytes | ancestry here is the authored one, not 2k's resolved one |
| 632 | `racks` is every resolved node of `Inputs` whose type is `rack` with at least one symbol; `source` when it is not empty | `resolveModuleMap` |
| 633 | `pluginNames` and `projectRoot` are filled by `hints_of`; `audioClips`, `videoClips` and `curves` are `Option`s present on an answer and absent on `Hints::empty()`, whose `keywords` is empty | one answer; `EMPTY_HINTS` carries neither |
| 634 | every sorted list of names sorts by `str` Ord | decision 165 |
| 635 | one `Spend` per catalogue, per base list and per I/O list; `hints_of` has two, one for its stages and one for the media walk. `BUDGET` is 1,000,000 units, a name `1 + bytes / 64`, a walk step 1; past it `capped`, one WARNING named `project.k7x` for each spend that capped (`the completion vocabulary`, `the media folders`). What was paid for is kept: a list the budget ends inside holds what was copied, a later stage is empty, and the media walk keeps the folders it listed whatever the stages spent | a limit counts the output and every walk; a project with a large media folder keeps its picker |
| 636 | `web_project_of` is `webProjectOf` over 2h's types, field for field; a track's `mixer` is the LAST plugin whose `type` is `Mixer`, with `Volume` and `Pan` from its params (`0` when absent) and absent when the track has none; a plugin carries `id`, `type`, `name` and a non-empty `sfzFile`; a timeline lane is drawn as a track of either kind with no strip, group or plugin, borrowed and not copied; `timelines` is absent when empty; `disabled` of a `WebTrack` is never written | the frontend reads presence; a lane's clips are not copied twice |
| 637 | `dead_automations` indexes a track's plugins by id once (the first of an id is the one `find` meets), reads each plugin's declared params once, and judges each lane in the TypeScript's order; its file is `chains/<chain>.k7x` when one stat says a file is there, else `project.k7x`; a message quotes at most `NAME_CUT` characters of a name and of the declared list, and is at most 300 bytes | a search and a parse per lane would be a product |
| 638 | `dead_project_fields` answers `modules_from` and `libraries_from`, each a WARNING named `project.k7x` at 1:1; a root `type` key cannot reach it, since 2a refuses the key (decision 5) | the two the spec leaves alive |
| 639 | `project_errors`: on a `Failure` the flat `errors` are `<file>: <message>` of its findings in their order and the diagnostics the same; on a compile `errors` is empty and the diagnostics are the plugin findings, the jsdsp graph findings of every resolved project module (wired or not, as `pluginFindings` walks them), the compile's own findings (which already hold the reads' warnings, the vocabulary's and the surfaces'), the dead automations, the vocabulary's findings, the dead project fields, the surface issues and the assembly issues, in that order, each once by (file, line, col, message); the project's reads are no step of their own, since the compile lists them | `projectErrorsIn`; several of those read the same warnings twice, and the compile's own dedupe has no line or column |
| 640 | DEPARTS. a list of diagnostics is cut at `MAX_DIAGNOSTICS` with a last WARNING naming how many were left out, and a `file` or `message` is cut to 300 bytes, its ellipsis included; the flat `errors` of a refusal is cut at the same count with the same last line | the TypeScript lists them all; a hostile project must not cost its size in messages |
| 641 | `formatTooNew` is `{fileLevel, buildLevel}` from `read_head` through the cache, only when `Failure.format_too_new`; the finding is already on the `format` line | `formatLineIn` and its regular expression were the TypeScript's way back to a place the Rust never lost |
| 642 | `answer` is one call: a compile only when `engine`, `web` or `errors` is set, the project's reads once (the compile's own, handed back by `compile_keeping`) and only when a view needs them, one `ByHeader` over the compile's `audio_paths` and the document's audio sources; `Request` is 2h's. The hints, surfaces, I/O and catalogue exist whether or not the compile refuses, and `web` is `Err(Failure)` when it does; `graph` is 2l's | a field of `Compiled` could not say a view exists when the compile failed |
| 643 | `records_output` reads `project_surfaces`; a timeline surface whose `mapping` is malformed does not decode, so the recorder is placed for it | one reader of a surface, as the TypeScript has |
| 644 | every view type derives `ts_rs::TS` behind `ts`; `tests/views_twin.rs` writes each `T::decl()` and `scripts/viewsTwinCheck.ts` runs `tsc --strict`: each generated type must be assignable to the hand-written one, the `keyof`s equal up to two named lists, and two wrong fields must be refused. The run-time half is `compare.rs` over every JSON the gate makes, and `no_null` over all of it | a `ts(type = …)` proves nothing about its field, so `event`, `comp` and `crossfade` rest on the run-time half |
| 645 | known-absent keys of the `keyof` check: `WebTrack.disabled`, which nothing sets; `Hints.audioClips`, `videoClips`, `curves`, which the route answers and the editor's type does not list | a list of what is known, not a filter |
| 646 | the gate is in files of its own: `stageDumpViews.ts`, `views_negative.json`, `stages_views.rs` | one gate to read |
| 647 | `compare.rs` is not edited: a diagnostic set is compared by its `file|severity` projection, a rewritten message being no difference | the "equal enough" rule |

### Mine (A1)

| # | decision | why |
|---|---|---|
| 650 | DEPARTS. `fields_at(project, [])` omits `type` and `project_type`, which the TypeScript offers; the gate lists exactly that pair as its one known difference | 2a refuses both keys (decision 5), so completion would offer what the compiler rejects |
| 651 | the curve kinds' keys are one `const`, not a probe: their derived structs are private to `curves/`; the gate holds the list | an edit of `curves/file.rs` is not this sub-step's |
| 652 | the frozen suffix `__frozen` is a private `const` of `views/web`; at the combine it becomes 2h's `FROZEN_CLIP_SUFFIX` | A1 names no name of 2h |
| 653 | `project_surfaces` reads through `read_family`: its indexed listing serves 100,000 files, then orders the result itself (603), and its kinds are decision 604's (an `Outside` file a WARNING, an `Io` file an ERROR, each costing that file); the function adds the one WARNING for a `surfaces/` the Source refuses whole | one reader of a family folder |
| 654 | a `decode_surface` message is cut to 280 bytes, on a character | a 1 MB key quoted by an unknown-key message |
| 655 | `fold_comps` ports JavaScript's `slice` on an id with no `@`: the name drops the last character and the number read is the whole id | `foldComps` literally; `placement_id` always writes an `@` |
| 656 | `fold_nests` takes the clip-to-web function as a closure, as the TypeScript does | the pass stays testable without a Source |

### Mine (A2)

| # | decision | why |
|---|---|---|
| 657 | a walk is charged where it happens: `Spend::steps(n)` charges work before it is done, a listing 8 units asked and 200 when the folder exists, each entry a step | a folder behind a link that loops listed 3 entries at each of a million places in 5 seconds |
| 658 | `ByHeader` weighs every distinct path it is asked, a stat and a copy, against `BUDGET`; past it a new path answers `None` with no stat and no copy, and `finish` adds the same WARNING named `clips`; a path is memoised whatever it answers | `MAX_HEADERS` bounds the reads, and 1,200,000 distinct paths were 1,200,000 stats |
| 659 | `hints_of` fills its lists in the order of the TypeScript's `Hints`, each copy charged as it is made into the list the answer holds; the budget ends the stage it is in, which keeps what it copied, and every later one stays empty | a partial answer is the lists already paid for |
| 660 | the scan's plugin names are the `Scan`'s (sorted by name), compared as sets by the gate | the TypeScript lists them in the file's order |
| 661 | the gate gives the TypeScript's `hints.channels` the case's rig, and the negative cases are read with the TypeScript's `@meta` root | `projectHints` reads the machine's preferences, which a case cannot hand it, and `case_roots` has no `@meta` tree |
| 662 | the account library is not in the TypeScript-compared negative cases | the TypeScript finds it under the machine's home, the Rust cases under their own `lib/` |

### Mine (B)

| # | decision | why |
|---|---|---|
| 663 | DEPARTS. a chain name that holds a separator names no chain file and is never stat'ed (on Windows a name that `windows_hazard` flags is the same): the finding is at `project.k7x`; a dot name or an empty name is an ordinary file name (`chains/..k7x`, `chains/.k7x`) | a stat of `chains/../x.k7x` could name a file that is not a chain's; the TypeScript joins the text as it stands |
| 664 | `dead_automations` has a `Spend`: a stat per distinct chain is a step, each message weighs its bytes and its `file`; past `BUDGET` the list so far is kept with one WARNING named `project.k7x` | a limit counts the output and every walk |
| 665 | `FormatTooNew`'s levels are `u32`, the file's saturating at `u32::MAX` | an integer is never `f64`; the file's level is read as a `u64` |
| 666 | the symbols of a rack in the hints come in `Object.entries` order, integer-like names first and ascending | `"4"` before `"36"`; found by the gate over 2h's `order-rack-symbols-integer` case |
| 667 | the gate also reads 2h's `doc_negative.json` cases that are `ts: true` and need no render, named `doc-…`, with the TypeScript compile of each (the VSTs as the case says, the rig as the preferences it saves) | the golden corpus holds almost no frozen lane, timeline, comp or refusal; 2h already wrote them |
| 668 | a folder with no `graph.k7x`, `modules/` or `chains/` is no project to the Rust and a folder of files to the TypeScript (2h's decision 305): the gate expects the Rust's refusal, compares no web project or error list for it but its `formatTooNew` (the TypeScript's "update k7" for a newer-format `project.k7x` alone, which the Rust says too), and counts it apart from the refusals and diagnostics | the difference is 2h's and is held there |
| 669 | the gate's roots carry the `also_readable` the host's `songRoots` gives (decisions 736, 737), so a library behind a link out (`e2e/library-rack.k7`) is read here as the TypeScript reads it, and every project is replayed from its bundle | a link out is a mount of the host's, and the bundle says `Outside` and `Io` |
| 670 | a replay draws its web project through `ByTable` over the lengths its first pass measured, and compares equal | decision 611 |

## The field tables

`WebClip`: `id`, `type`, `name`, `start`, `length`, `notes` always; `offset`, `loopStart`, `loopEnd` numbers on a clip of
`web_clip` and absent on a comp or nest host; `sourceFrom`, `sourceTo`, `event`, `comp`, `crossfade` when set; `sourceName` for a
non-empty source; `warped`, `warpOff`, `disabled`, `frozen` `true` or absent; `fadeIn`, `fadeOut` above 0; `pieces` on a comp;
`timeline` on a nest. A comp host and a nest host hold `notes: []` and none of `sourceFrom`, `sourceName`, `offset`, `loopStart`,
`loopEnd`, `warped`.

`WebTrack`: `id`, `name`, `group`, `generated`, `contentType`, `color`, `mute`, `solo`, `clips`, `plugins` always; `mixer` when a `Mixer`
ends the chain; `highlight` when true; `frozenMaterial` when it holds a clip. `WebProject`: its eleven keys always, `timelines` when
there is one. `ProjectErrors`: `errors`, `diagnostics` always, `formatTooNew` on a newer format.

## The gate

`scripts/stageDumpViews.ts` dumps, over a copy of the staged corpus (the TypeScript's compile builds jsdsp graphs into the tree it reads,
so the Rust reads the copy) and over the negative cases the TypeScript can answer, the catalogue, the base modules, the surfaces,
`surfaceIssues`, `ioPorts`, the route's hints, `webProjectOf` over the compile and `projectErrorsIn` around it, each with its throw
recorded; `scripts/stageDumpSweeps.ts` dumps the fields lattice (every kind over every path of up to three segments of a pool, the
named deeper paths, a pool of file names), 2,000 WAV headers and 1,500 lists of clips with what `toWebTracks` makes of each.
`rust/tests/all/stages_views.rs` (`PARTS` names what has landed) reads each project through a `Recorder` with one `Cache` shared across
projects, again through the bundle it made (the web project drawn through `ByTable`, decision 670), and compares through `compare.rs`; a
diagnostic set by its `file|severity` projection (decision 647). A refusal must name every file the TypeScript's diagnostics name, or, where they name none, a file, node or setting its throw's text names; a clean
compile lists the same set, each side with its counts (the TypeScript's by place and message once, as decision 639 lists it). The allow list is empty; the two standing exceptions are a count of what they skip: a folder that is no
project (668) and a TypeScript throw, which fails unless named in `TS_THROWS` (none is).

On the golden corpus alone it counts 177 projects; 66,052 catalogue rows (24,103 with a path), 61,096 base modules, 181 surfaces,
1 issue warning, 533 I/O ports (2 of a placed node), hints with 40,982 `paramsByBase`, 966 paths, 111 media folders and 3 racks; 175 web
projects of 373 tracks (3 mixers, 15 plugins with an `sfzFile`, 8 generated lanes) and 367 clips (278 drawn from a measured length, 87
midi, 310 notes), and 2 refused compiles with 4 diagnostics. A `null` anywhere in a Rust answer fails the gate but for a surface's
mapping entry (decision 644). Each of the work order's golden floors for the web project and the errors is met by the golden corpus alone.

Over the golden and the 201 negative cases (193 with the TypeScript: 111 of this corpus and 82 of 2h's) it counts 110,711 catalogue
rows, 104,199 base modules, 206 surfaces (24 findings, a null mapping entry, 2 playlists, 2 stems), 36 issue warnings, 1,158 ports (6
with a resolved label, 2 unknown), hints with a favourite, 2 hidden modules, a module's assets, 4 slice racks, 4 plugin names, 80,540
lattice entries, 2,000 WAVs and 1,500 clip lists, each kind of fold at least 20 times; 275 web projects (a comp, a placed timeline and
4 timelines, 2 frozen clips and 3 frozen materials, 2 crossfades, 13 mixers, 10 highlights, 4 markers) and 127 measured lengths; 11
refused compiles (4 compared by the files the TypeScript's diagnostics name, 7 by what its throw says) and 84 folders that are no project, counted apart; 45 diagnostics (3 `formatTooNew`, a not-supported plugin, 2 unknown, a
dead automation of each kind, 2 dead project fields, 2 jsdsp findings). 369 bundle replays equal the disk. The largest spend of any
view over all of it is 10,621 units (hints), under a tenth of the budget, and none was capped.

`tests/views_negative.rs` holds the cases without the TypeScript, each by file and key, and runs `no_null` over every answer: 119 cases of
this corpus (111 `ts: true`), those of 2h's with no expectation but the answer, and the order of the surfaces pinned. The 8 that are not
`ts: true`: a broken clip, sequence, curve or node in hints (decision 625), a broken clip in a folder of 40, an empty source, and the
account library (662). Not in the corpus: a jsdsp graph that is stale, which needs a file older than its source and which the
TypeScript rebuilds (the unit test in `views_errors_list.rs` holds the invalid one).

The twin check (decision 644) is run by hand: `cargo test --locked --features ts --test all -- --ignored views_twin::` with `K7_SONG_TWIN`
set, then `npx tsx scripts/viewsTwinCheck.ts "$K7_SONG_TWIN"` from `packages/song`. It passes 28 pairs, the three enums both ways and
its two refusals; a changed `volume`, a renamed `sfzFile`, a dropped `countIn`, a widened `dir`, a `BindTarget` without `invert` and a `ViewBinding` with a key more each fail it.

## The hostile table

`tests/views_surfaces.rs`, `views_fields.rs`, `views_wav.rs`, `views_lengths.rs`, `views_web.rs` and `views_catalogue.rs` hold the
rows of A1: a nest of 300, a mapping of 200,000 keys and a key of 1 MB, 100,000 surface files, a path of 1,000,000 segments, a
header with a chunk of `0xFFFFFFFF` and a walk of 1,000 chunks, one path asked 100,000 times, 25,000 files over the header limit,
1,200,000 distinct paths, 100,000 pieces of one comp, 100,000 nests, 100,000 clips over one source.

`tests/views_hostile.rs` holds the rows of A2: 100,000 refs of one namespace with 100,000 nodes of one module (catalogue and bases, the vocabulary built by hand), 100,000 ports (inputs then outputs), a channel list of 100,000 numbers cut at 64, a
port named with 1 MB (capped by bytes); a project of 100,000 nodes over 100,000 bases, a base with 1,200 extenders of 1,000 params,
10,000 bases on a built-in of 767 params (each capped, with the lists already paid for kept), 100,000 clips and sequences, 100,000
curves, a folder of 200,000 audio names (walked, not capped), nodes named with 1 MB, 1,000 nodes over a chain of 17 parents with
10,000 assets each (capped), a broken clip among 1,000, a link back to its own folder, to the workspace and to a sibling (each listed once), a folder link out of the mounts (walked past on disk and in a replay),
a name that could climb out of its folder. `tests/views_partial.rs` holds what the budget keeps: 5,200 media folders, 600,000 base refs, 12,000 long node names, 400,000 clips. `tests/source_mem.rs` holds 40,000 modules asked for an absent `engines` folder. Each runs under a second here; a bound is five seconds, thirty on musl.

`tests/views_errors.rs` and `views_errors_list.rs` hold the rows of B: 100,000 automations on one plugin whose 1 MiB graph is read once, a first param name of 1 MiB under 100,000 lanes (cut once per message), 100,000 plugins with a lane
each, a param and a type of 1 MB (no message past 300 bytes), 5,000 lanes on a chain file named with 64 KiB (its `file` charged, the check capped), a chain named `..\..\x` and `a/b` (each at `project.k7x`; a dot name is a file name),
100,000 findings of one kind (a thousand and the last, in a compile and in a refusal). `tests/views_project.rs` draws 200 tracks of
2,000 clips from a table; `tests/views_answer.rs` the lengths measured once on disk, a bundle that cannot measure, and a refused
compile that still answers the lists.

`tests/views_capacity.rs` prints the room a real project has: 5,000 modules (200 panels), 2,000 nodes, 500 clips and 2,000 audio
files read in 0.2 s, hints spend 168,573 units, I/O ports 7,240, the catalogue 15,471, the bases 10,452, none capped; 2,000 WAVs
measured on disk, one read each, then none; 200 lanes of 2,000 clips drawn from a table of 2,000 lengths in 0.07 s.

## Risks

- `MemSource` answers a folder question about held paths from a set of every ancestor, built once, so a vocabulary of N modules
  read from memory is linear in what is held; a draft is still scanned on each ask, so a session with thousands of drafts pays
  for them on every absent path. The 100,000-ref catalogue case is built by hand over a source that holds the files.
- A replay through a bundle cannot measure a header: `answer` returns `Lengths` beside the web project, 2l carries them beside the bundle, and
  the replay draws through `ByTable` (decision 670); `answer` over a bundle itself answers `Missing`.
- A surface, a module file or an asset folder behind a link out of the mounts answers `Missing` for ever in a replay; a folder
  the media walk lists is held as an empty listing, so the walk goes past it.
- `ByHeader` stats a file and then reads its head, two operations where `wavSeconds` holds one descriptor: a file replaced between
  them is measured by the new head against the old length.
- `hints.channels` comes from the rig the host hands over; the TypeScript's route reads the machine's preferences itself.
- A project whose files are read through a link out of the mounts (a node's rack, a library behind a link) is a WARNING here unless the host
  mounts the link's real path (736, 737), which the gate's roots do (669).
- A folder with no graph, modules or chains is refused by the Rust and compiled by the TypeScript (668, 2h's 305); a host that opens such
  a folder meets the refusal.
- A jsdsp graph older than its source is a WARNING on a disk and no finding in a bundle (2h's 352). The TypeScript rebuilds it: a stale graph whose
  source no longer builds is an ERROR there ("plays the graph it last built") and a WARNING here, which 2l's `jsdsp_failed` (decision 731) is to bring back.
- A bundle that lacks every folder below a folder with its parent's listing reads as a link loop (the memory rule of decision 630); a bundle that lacks only some of them asks for its files.
- `project_errors` lists the compile's own findings beside the TypeScript's sources (639): the gate holds the two sets equal on every
  project it has, and a source the compile reports that `projectErrorsIn` does not would show there first.
- `formatTooNew` and the refusal's lines read the head through the cache: a head that changed between the compile and the read is
  read by its new stamp.
