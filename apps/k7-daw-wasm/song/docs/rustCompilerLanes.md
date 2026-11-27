# Lanes, ownership and the reference check in Rust (2e)

Sub-step 2e of [rustCompiler.md](rustCompiler.md), whose decisions 1 to 120, output rules, hostile-input rules and stage gate it keeps; 2f's and 2g's
decisions are in [rustCompilerClips.md](rustCompilerClips.md) and [rustCompilerCurves.md](rustCompilerCurves.md). It joins what 2d, 2f and 2g
answer: it reads a project's compile inputs together, names which nodes are lanes and which engine track each chain's steps land on, says what each
input plays against this machine's rig, reads the assemblies (a drawing, nothing to do with sound), resolves the console and reports every
reference that points at nothing. It builds no `ProjectDoc`, and nothing here is wired to a route. Paths are relative to `apps/k7-daw-frontend/packages/song`.

## Corrections to the 2e row of `rustCompiler.md`

- The row omits `graphValidate.ts`' `checkChainInputs`, `graphTypes.ts`' landing and source-type functions, `preferences.ts`' channel resolution,
  `monitorMode.ts`, `entities.ts`' `loadVideoTracks` and `readsAsPicture`, song-schema's assembly files and `enginePlugin`.
- `checkChainInputs` is a function of the reference check, so it is 2e's though it stands in `graphValidate.ts`, which is 2k's otherwise.
- The rig row of "What the compile reads" says "stereo pairs": the compile reads none; `inputPairs` is the engine session's.
- The `Nullable<T>` line of the output rules: no 2e file holds a `.nullable()` (decision 281).
- The 1,500 lines are low: the TypeScript is about 900 lines and the Rust and its tests about 5,300 beside a corpus of 122 cases, the bulk being the negative corpus and the hostile table.

## What lanes and ownership decide (2e)

Numbered on from the curves' table. 240 to 283 are the work order's, 285 on are the builder's (286 is not used); a row marked DEPARTS differs from the TypeScript
and says why. Each is named by its number in the test that holds it, and each DEPARTS is `ts: false` in the corpus or a unit test, so the gate's allow list is empty.

| # | decision | why |
|---|---|---|
| 240 | the code lives in `rust/src/lanes/`, `rust/src/assemblies/`, `rust/src/console.rs`; 2e imports no `ProjectDoc`, `PluginDoc` or `TrackDoc` | every function takes typed inputs and 2h assembles |
| 241 | `Lane<'m>` borrows its `Resolved`; `lane_of` is the one projection (group, then strudel, then picture, else single); `events` is empty for a group and a pattern lane, `tracks` only for a group, `video_source` only on single and picture, `frame` only on a picture lane | a resolved module may hold 100,000 placements per lane and every later stage reads them; a test checks the slice is the module's own |
| 242 | `is_pattern_lane` is a strudel lane with a path that is not disabled; `pattern_lanes` hands 2f `(lane, strudel)` in the map's order | 2f left it here |
| 243 | `lane_materials` and `lane_signal` are ported as they stand: a generated lane answers neither, an absent or empty `ref` is `Missing` | `String(p?.ref ?? "")` |
| 244 | which nodes are lanes is decided on `Inputs.local` alone; a chain step naming `@meta/track` is not a lane | `lanesOf` runs before the chains exist |
| 245 | DEPARTS in the work only. Each module is resolved once: the local nodes, then only the vocabulary modules the chains name, as `Inputs.extra` | the TypeScript's second pass is idempotent and doubles every warning |
| 246 | `load_inputs` runs the TypeScript's order; it returns `Err` only for `Missing` | a bundle that lacks a file must answer `needs_files`; any other trouble with a file is a finding |
| 247 | a family folder the Source refuses as a whole is one WARNING named by its folder and an empty family; one file refused is that file's warning alone | decisions 24 and 116 |
| 248 | DEPARTS. `read_family`: a file that answers `Io` is an ERROR finding for that file and its siblings read; the file then counts as broken | the TypeScript throws and the compile is lost |
| 249 | `Inputs::stops()` is the ERRORs of the head, the routing, the nodes, either resolve and `timelines/`; a family finding on `clips/` or `sequences/` stops nothing, and is a fault only when an event names that file; a WARNING never stops | `checkReferences`' reading: a broken clip nothing places is no fault, a broken timeline is |
| 250 | `Rig::from_json` never fails: a non-object is the empty rig; a channel is read as JavaScript's `Number(value)` (`"3"`, `" 4 "`, `"0x10"`, `true` and `[3]` all count) and kept when a whole number from 1 to `u32::MAX`; a label is read as `String(value)` (`5` is `"5"`, `["a"]` is `"a"`, an object `"[object Object]"`); `cueOutput` is a list read like channels or a label trimmed as JavaScript trims; at most 64 channels per label and 4,096 labels per direction. DEPARTS on the two caps and on a channel past `u32::MAX`, which the TypeScript keeps | unreadable preferences never fail a compile; the caps bound what each live input copies; no device has a channel past a `u32`, and a channel is never `f64` |
| 251 | labels resolve through a first-wins index built once per direction (`Rig::index`); `resolve_input` and `resolve_cue_output` answer `{channels, unknown_label}` | `find`'s first hit; a walk per input is a linear find in a loop |
| 252 | `Ownership` holds the owner of every chain, computed once; `owner_of` answers `None` for the TypeScript's `""` | `ownerOf` walks `out` on every call and 2h calls it per step |
| 253 | `bus_chains` and `input_chains` are `IndexSet`s and `tap_chains` an `IndexMap`, in the TypeScript's order; a later tap on one run replaces the value and keeps the position | their order is the order of the return tracks |
| 254 | DEPARTS. a tap routed to no subchain is one `TapUnrouted` fault per tap, at the node's file | a compile reports what it finds |
| 255 | a laneless run owns a tap's track through an index of first step to first chain with no `from`; `side_input_track` answers chain, lane, then the first chain holding the step, through another index that holds the chain's position and never a copy of its name | both are a linear `find` in a loop in the TypeScript, and a chain called by 512 KB with 6,000 steps is 3 GB of names |
| 256 | DEPARTS in order only. A chain named like an integer is met in file order, not JavaScript's numeric-first order | decision 106 |
| 257 | `live_inputs_of`: the first `io: in` step of a chain, then the track its owner or landing names, the first input on a track winning; a boolean `monitor` is on or off, anything unreadable is `Auto`; a gain not finite is 0 | `liveInputsOf`, `monitorMode`, `normGain` |
| 258 | `input_landing_track` answers `from`, else the first `to` that is a lane, the `to` unsplit | a port on a lane name is not a lane |
| 259 | a check answers `Fault`s and each is worded apart: `Fault::key()` is what a test and the gate compare; a message is at most 300 bytes and quotes a file word cut to 40 characters; the strings a fault holds are cut to 200 | a 1 MB ref copied into 100,000 faults is 100 GB |
| 260 | DEPARTS. `MAX_FAULTS = 1,000`: past it the walk stops with one `More` fault; each fault is one ERROR finding at 1:1 | a hostile project must not cost its size in messages |
| 261 | DEPARTS in wording only. Each lane's and each timeline's events are checked once; cycles are found once by the strongly connected components of the timeline graph, iteratively; an event in `x` naming `y` is a cycle when both are in one component, a fixed field is reported beside it when `x` is not `y`; a lane's event never closes a cycle | the TypeScript walks every simple path; its answer set is this one, a fault being named by its holder |
| 262 | a ref is looked up in maps built once from the three families, never `FamilyRead::get`; a timeline wins over a clip and a clip over a sequence; one stat per distinct clip ref | 100,000 events naming clips |
| 263 | an audio ref is checked through the path `audio_paths` answers: `""` is outside, else one stat: `NotFound` or `Io` are missing, `Missing` is `Err` | `audio_paths` keeps them for this check to name |
| 264 | DEPARTS. a `strudel` path is stat'ed through the Source: `NotFound` or `Io` is missing; `Outside`, or a path that will not normalize, is no fault | a file the compile must not read is not "missing" |
| 265 | `Vocabulary::broken` is sorted when the vocabulary is made and `absent` binary-searches the `ref/` prefix. DEPARTS: a clip or sequence is broken when a family finding names exactly its file | a prefix that names a neighbour says the wrong file broke |
| 266 | `extends` is read of the resolved module of each step, once per distinct module; a step with no module is silent | a missing module compiles to nothing |
| 267 | picture lanes are not checked; disabled sound lanes are, strudel files of disabled lanes included | the picture compiles separately |
| 268 | `check_chain_inputs` is pure over `WiringInput` and `inputs_of` is a closure; it charges nothing: each step's sources are compared with its ports up to the shorter list, and `mandatory` is counted once per module | `inputs.forEach` over every port per step is the product a module of 5,000 ports makes |
| 269 | `source_type` answers sequence, clip (picture when `reads_as_picture`), then lane, then picture lane; a lane's signal is memoised per lane name | a lane of 100,000 events named by 100,000 ports |
| 270 | DEPARTS. the reference faults and the wiring faults are both reported | a compile reports what it finds |
| 271 | a finding's file: a lane or a node by `own_label`; a vocabulary module, as a holder or as a node with a tap, by `<ref>/module.k7x`; a timeline by `timelines/<name>`; a chain by the node its key, else its first step, names, else `graph.k7x` or `chains`; a console by `project.k7x`; each at 1:1. The walk names the file from the full name it holds, because a fault holds its strings cut (259) | a finding names a file; a `Placement` carries no place; a cut id looked up names a file that is not there |
| 272 | `video_tracks` is the picture lanes with the mute `bypassed || disabled`, borrowed | its one input is what `load_inputs` read |
| 273 | `assemblies/` is read with `read_family` through `family_or_warning` and two cache slots; the decoders are strict, every refinement a `Check`, an empty file is a finding | a vanished, refused or unreadable file is `read_family`'s |
| 274 | a template ref goes through `library_ref_parts`, each `/` segment must be plain with a run of `/` inside the path one separator (an empty first or last segment is refused), `@project/` is refused, then `Files::library_file` and `assembly.k7x` through the cache. On Windows a segment holding `\` or `:` is refused. DEPARTS on Unix: a `\` is a character of a name, where the TypeScript splits on it | the host's libraries are the only roots |
| 275 | an absent template, or one the Source refuses, is `missing` and a WARNING on the instance's file; one that does not decode, reads with `Io` or is not UTF-8 is an empty template with one ERROR on the instance's file (DEPARTS for `Io`) | an unreadable template costs that box only |
| 276 | `assemblies::BUDGET = 1,000,000` per call weighs what is copied: the label, a member, a plug and a param each `1 + bytes / 64`, charged before the copy; past it one ERROR names the instance's file, `capped`, the rest absent. The turntable costs 29 an instance | a template of 100,000 plugs instanced 1,000 times is 10^8 copies |
| 277 | not ported: `libraryAssemblies` and `assembliesNaming` | no compile route reaches them |
| 278 | nothing of `assemblies/` joins `Inputs`, `Lanes` or `Ownership`; a test proves a project with and without it gives equal chains, lanes and owners | an assembly is a drawing |
| 279 | `console_systems` reads the regexp by hand and sorts by label in code points; `console_of` resolves both halves and answers `HalfPlugin`s with number params only; 2h clones one per track with `with_id`, places it at `channel_slot` and skips a track for which `takes_channel` is false | `withConsole` edits a `ProjectDoc`, which is 2h's |
| 280 | a system with a half missing is one `ConsoleAbsent` fault, `channel` before `buss`, on `project.k7x`; the resolve's findings pass through | `withConsole`'s refusal |
| 281 | no `Nullable<T>` here: its only readers are the surfaces (2j) | a wrapper with no reader is dead code |
| 282 | no ts-rs twin; the gate is in files of its own | decisions 43 and 171 |
| 283 | no budget on the passes with no product: each visits each chain, step, out, lane, event, timeline, tap and label at most once through an index | a budget on a pass with no multiplication is a limit nobody can hit honestly |
| 285 | a clip or sequence file a family finding names, a WARNING for a refused file included, is a broken ref when an event places it; a timeline that did not read is no clip or sequence, so an event placing it is `ref_missing` beside the timeline's own `timeline_broken` | a file the Source refused exists and is not "missing"; the TypeScript looks for a broken file only among clips and sequences |
| 287 | `Inputs::warnings` holds every WARNING the reads raised, the vocabulary's included | 2h reports them; none stops a compile |
| 288 | a plug or param dropped for want of a mapped member weighs one in the assemblies' budget | otherwise 1,000 instances of an unmapped 100,000-plug template are 10^8 free visits |
| 289 | `resolve_input` and `resolve_cue_output` build no index: they walk their one direction to the first hit; a pass over many inputs asks `Rig::index()` once | the signatures the order fixed, and a call that does not name a label must not cost 8,192 inserts |
| 290 | the faults of the ownership and of the wiring, a group source included, are capped like the reference check's (1,000 and one `More`) | a node of 100,000 taps, or 5,000 chains fed by a group, must not cost their number in faults |
| 291 | `Fault` and its kinds live in `lanes/fault.rs` and are re-exported by `references.rs` | the reference check passed 500 lines with them |
| 292 | the gate compares a lane's events as their refs | `Placement` serializes its parsed form, where `laneOf` hands the authored one |
| 293 | DEPARTS. A syntax error in a clip or sequence file that no event places loses the compile in the TypeScript (it throws at load) and stops nothing in the Rust, which names the file only when an event places it (249); it is `ts: false`. A broken timeline stops the Rust at the read and the TypeScript at its reference check, naming the same file | one file costs that file alone (247, 248); the gate accepts a Rust stop the TypeScript answers with that fault |
| 294 | `More` is held by `Holder::File("")` and named by `project.k7x` | a limit has no place of its own |
| 295 | DEPARTS. A template ref `@project/<x>` is refused even when a library is really called `project` | `@project/` names the project's own folders everywhere else, and a library cannot take the name |
| 296 | a fault is built from the holder by reference and cut first; a chain's fault is built from its name as it stands; `Fault::new` takes a holder by value or by reference | a holder cloned whole per fault is a 512 KB name copied 100,000 times |

## The seams

| neighbour | what 2e gives it | what 2e takes from it |
|---|---|---|
| 2d | | `Segments`, `derive_chains`, `Chain`, `lane_names`, `tap_pairs`, `apply_link_levels`, `with_chain_module_refs`, `load_routing` |
| 2c | | `Resolved`, `resolve_map`, `resolve_ref`, `Vocabulary::absent`, `Files::library_file` |
| 2f | `pattern_lanes`; `&[Placement]` through `Lane` | `audio_paths`, `fixed_fields_of`, `FamilyRead`s via `read_family` |
| 2g | nothing | nothing |
| 2h | `Inputs` and `stops()`, `Lanes`, `Ownership`, `live_inputs_of`, `Checked`, `Console`, `channel_slot`, `takes_channel`, `engine_plugin`, `video_tracks`; 2h builds the `TrackDoc`s, places the console and weighs each `HalfPlugin` it clones per track | no 2h type |
| 2i | `Lanes::picture`, `video_tracks`, `Lane` | |
| 2j | `Rig` and `resolve_input`, `assembly_issues`, `console_systems`; the `Nullable<T>` for `mix.k7x` is 2j's | |
| 2k | `Lane`, `lane_materials`, `lane_signal`, `source_type`, `input_landing_track`, `check_chain_inputs` over its own `inputs_of`, `resolve_assemblies`, `monitor_mode` | |
| 2l | `Rig::from_json`; the risks below | the JSON of the host |

## The gate

`scripts/stageDumpLanes.ts` writes the TypeScript's half and `rust/tests/all/stages_lanes.rs` compares, with an empty allow list, in
`scripts/stage-gates.sh` (part of `npm run ci:local`) and by hand (`cargo test --test all -- --ignored stages_lanes::`). A throw's lines are parsed by anchored patterns into
fault keys, and a line that matches none fails the dump, so a change of the TypeScript's wording cannot pass quietly; no Rust reads a message.
Each project is read through a `Recorder` with one `Cache` shared across projects and again through the bundle it made. The staged corpus and 122
negative cases (115 with the TypeScript) give:

| what | count |
|---|---|
| projects compared, bundle replays | 177, 299 |
| sound lanes, group, pattern, picture, with a frame | 367, 5, 8, 9, 4 |
| events on lanes | 369 |
| chains, of them owned by a bus, an input, the master or a tap | 563, 185 |
| master, bus, input, cued chains | 177, 6, 2, 2 |
| side inputs that resolved a track, of them through a step in a run | 933, 376 |
| live inputs, with a non-auto monitor | 2, 2 |
| assemblies, plugs resolved, params promoted | 2, 4, 16 |
| console systems listed | 1,416 |
| projects with a fault | 2 |
| over the negative corpus: tap chains, console halves, live inputs with channels, cue pairs applied | 6, 4, 18, 3 |
| over the negative corpus: fault kinds, wiring source kinds | 20, 11 |
| TypeScript throws at load the Rust stops on | 1 |
| the most any assemblies call spent of the 1,000,000 budget | 58 |

Its floors sit below those counts and above zero, and the floors the golden corpus must meet (live inputs among them) are read off the golden corpus alone. A difference is an explanation or a fix: where the TypeScript threw at load the Rust must stop,
and where it did not the Rust stops only on a broken timeline the TypeScript's reference check names. `rust/tests/all/lanes_negative.rs` is the same
corpus without the TypeScript and runs wherever `cargo test` does; its cases are `ts: false` for each DEPARTS row.

## Hostile input

Each is answered fast, in under five seconds in a debug build, asserting the answer as well as the time (`rust/tests/all/lanes_hostile.rs`,
`rust/tests/all/assemblies.rs`, `rust/tests/all/console.rs`, `rust/tests/all/lanes_rig.rs`).

| pass | worst input and answer |
|---|---|
| `Rig::from_json` | `null`, a string, a number: the empty rig; 100,000 channels on a label: 64; `[1e300, 0, -1, 1.5, null, false]`: none; `["3", " 4 ", "0x10", "1e2", true, [3], 2.0]`: 3, 4, 16, 100, 1, 3, 2; `1e300` and `5e9`: none (the one difference from the TypeScript, 250); 1,000,000 labels: 4,096; a label of 1 MB kept; a label that is a number, a boolean or an array: its `String`; every wrong `cueOutput`: none, and `["3", 4]`: 3, 4; 100,000 resolves on a full rig: no index built (289) |
| `lanes_of` | 100,000 events on a lane: the slice is the module's own; 100,000 lanes of 5 events; 10,000 lanes the resolver filled to 95% of its budget: all borrowed. 100,000 lanes of 100 events (10 million, 7 GB of memory) runs by hand: 3.5 s in a debug build |
| `chain_ownership` | 100,000 taps over 100,000 chains; a node of 100,000 taps, 50,000 routed: 1,000 faults and `More`; a bus fan-in of 100,000; 100,000 `side_input_track` calls over a chain of 100,000 steps; a chain called by 512 KB with 6,000 steps: the live memory grows by under 16 MB (`lanes_memory.rs`) |
| `live_inputs_of` | 100,000 inputs each on a label of 64 channels; a label absent; every input run to one lane: the first wins |
| `check_chain_inputs` | a module of 5,000 ports stepped 100,000 times; a lane of 100,000 events named by 100,000 ports: one walk, 1,000 faults and `More`; 5,000 chains fed by a group: 1,000 faults and `More`; a ring of 100,000 chains: none |
| `check_references` | a lane called by 2 MB with 200,000 events on one absent ref: one fault, under two seconds; a timeline whose id is past 200 characters: named by its real file; six levels of 100 events: each event once; a chain of 100,000 timelines: no stack overflow; a cycle of 10,000 and a timeline in itself; 100,000 absent refs: 1,000 faults and `More`; 100,000 refs to one broken clip: one fault; a 1 MB ref: a fault under 200 characters and a message under 300 bytes; 8,000 absent clips through a Recorder: one listing of their folder; a pattern climbing out, with a NUL or a `\` |
| capacity | 2,000 lanes of 200 events and 20,000 chains of 8 steps: no fault, about 0.2 s in a debug build |
| assemblies | 1,000 instances of a 100,000-plug template: stopped after 9; a plug name or a label of 1 MB instanced 10,000 or 3,000 times: stopped by bytes; `uses` of `@k7-core/a//b`: resolved; a nest of 300 in either file; hostile `uses`; two plugs of one name; 100,000 members; an instance linked out of the mounts |
| console | a ref with no half; a half of 100,000 params, built once; a vocabulary of 100,000 keys listed in one pass |

## Cases of the TypeScript's tests left out

| case | why |
|---|---|
| `trackModule.test.ts` apart from the five named, `masterChain.test.ts`'s plugin placement, `tapOuts.test.ts`'s pair numbering into a `PluginDoc` | 2h's and 2k's |
| `liveInput.test.ts` through `loadGraph`: the warnings, "refuses the same arrow from a module that is not an input", "no longer refuses a lane holding sequences" | the graph's warnings are 2k's |
| `console.test.ts` "is read off project.k7x by the compile itself" and the placement of the halves on a document | placing is 2h's; `channel_slot` and `takes_channel` are tested |
| `assemblies.test.ts` "lists what the library holds" and "what validate() says" | 277; the validation is 2k's |
| `assemblyYaml.test.ts`'s writers | the edits are not ported |
| `turntableAssembly.test.ts` "holds exactly the members a scaffolded deck maps" and "wires only members" | they read song-schema's stems and the template's graph |
| every message pinned by wording | a finding is matched by file and kind, never by its text |

## Risks

| risk | answer |
|---|---|
| a template behind a link out of the mounts, or not UTF-8, answers `Missing` for ever from a bundle | 2l, with the `refused` paths list and "not text" of `rustCompiler.md` |
| an audio file under a path that is a file (`ENOTDIR`) keeps its path on disk and is `Err(Missing)` in a bundle | 2l, as 2f's |
| `resolve_input` walks its direction to the first hit | a pass over many inputs asks `Rig::index()` once; a caller that loops takes the index |
| a family file that answers `Io` (248), and a template read with `Io` (275) | on disk it is a finding (`clips.findings` holds `[clips/kick.k7x, Error]`) and the file counts as broken; from the Recorder's bundle the read is `Err("not in the bundle")`, so the bundle answers `needs_files` for ever | 2l, which cannot change the bundle's shape here |
| `Inputs` keeps the whole resolved project; 2h clones `HalfPlugin`s per track | 2h weighs the clones (279) |
| the gate's explanation for a Rust stop on a broken timeline names the TypeScript's reference check | a second explained stop would need its own row |
