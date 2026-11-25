# The verbs ported to Rust

Phase 2 of `docs/native-app-plan.md`: k7's editing commands, today TypeScript run in the page's Worker
(`apps/k7-daw-frontend/packages/commands`), become the crate `apps/k7-daw-wasm/verbs`, which the native app reaches
through the JSON C ABI and the web, while it exists, through the same WebAssembly module. This page holds the
inventory, the milestones, how a verb is tested and the choices the port rests on.

## What there is to port

The catalogue (`commands.yaml`) names **191 verbs**. Their TypeScript is 10,000 lines of verb files
(`src/verbs/`), 700 of dispatch (`commandSpec.ts`, `commands.ts`, the host types) and 370 of completion providers,
over 84 test files (10,600 lines). What a verb WRITES is already Rust: the song editor (`song-edit`, 21,000 lines)
answers every `stub(...)` of `@studio/song` and `@studio/song-edit`, and the compile a verb reads (`h.project()`)
is the `song` crate's. What remains TypeScript is the verb itself: its argument parsing, its refusals, its reads of
the project, the order of its writes and the lines it says.

| milestone | area | verbs | TS lines (verb files) | host members it needs | estimate |
|---|---|---|---|---|---|
| M0 | the command language: the catalogue, longest-prefix dispatch, the argument shapes, the format and sounding-lane refusals | — | 700 | none | done with M1 |
| M1 | arrangement: `event move/add/remove/resize/split`, `events move/add/remove`, `track new/rename/delete/order/mute/solo`, `project tempo/meter`, `project marker new/delete` | 18 | 2,450 | `applyProject` (mute, solo) | 4 – 6 agent-days |
| M2 | an event's strip and takes: `event status/fade*/window/gain/loop/crossfade/comp/take/trim/process`, `clip warp/onset/process`, `track process`, `selection consolidate` | 25 | 1,080 | `seconds`, `clipEnvelope`, `render`, `peak`, `runColdStep`, `takeArm` | 4 – 6 |
| M3 | sequences, notes, curves, automation | 27 | 1,250 | none | 3 – 5 |
| M4 | the rest of lanes and the project: groups, duplicates, hide, input, highlight, timelines, time splice, aspect, master, console, tags, surfaces, tasks, libraries, playlists | 36 | 1,250 | `timelinePlay`, `playlistStems` | 5 – 7 |
| M5 | nodes, modules, the graph, assemblies, taps, presets, freeze | 44 | 1,875 | `render`, `applyProject`, `showNativePluginUI`, `saveBenchChain` | 7 – 10 |
| M6 | picture lanes and shots | 22 | 1,860 | `probe`, `prepare`, `extractSound`, `renderPicture`, `background` | 6 – 8 |
| M7 | the machine and the transport: `system *`, `transport *`, `project record save` | 19 | 300 | the engine link, the preferences | 1 – 2 |
| M8 | completion: the providers, labels and `completionItems` | — | 370 | none | 2 – 3 |

In all, 32 to 47 agent-days, the plan's 30 – 45. M1 is first because phase 5's arrangement editing calls it; undo
and history are not verbs: the backend's journal (`apps/k7-daw-backend/docs/file-changes.md`) undoes a change set
whatever wrote it.

## How a verb is tested

| choice | why |
|---|---|
| the TypeScript is a reference for WHAT a verb does, never an oracle to match: no vector is generated from it | the native app is the product; a byte-for-byte match would freeze the TypeScript's accidents into the port |
| each Rust verb is tested on its effects over a real fixture project (`e2e/fixtures/*.k7`, its `_modules/` and k7's `@meta`) held in a `MemFs` (`apps/k7-daw-wasm/verbs/tests/support/mod.rs`) | what a person sees is the files and what plays; a hand-made `ProjectDoc` hides how the compile reads the files |
| a test reads back the files the verb wrote, as text and as k7x data | a verb's contract is the change set the journal lands |
| a test compiles the project after the verb with the song crate's engine compile and asserts on the `ProjectDoc` | that document is what the backend sends the engine; the C++ render harnesses play it, and stay the engine's tests |
| every successful verb is undone in its test: its change set reversed, the files and the compiled document must be the ones before | undo is the journal laying the previous files back; a change set that undoes to another song is a verb bug |
| a refusal is asserted with the files untouched | a verb that says no writes nothing |
| a chain project (`chains/`, `tracks/`) is a fixture of the crate's own, `apps/k7-daw-wasm/verbs/tests/fixtures/chain-lanes.k7` and `chain-rack.k7`, opened with `Project::open_crate` | e2e's fixtures are all graph projects, and the graph, lane and tap verbs write both layouts |
| the catalogue crosses as `verbs/commands.json`, written from `commands.yaml` by `apps/k7-daw-wasm/verbs/scripts/catalogue.mjs`; `--check` runs in `mac-checks.sh` | the crate parses no YAML and takes no new dependency for it |

## The crate

| choice | why |
|---|---|
| `verbs` depends on `song`, `song-schema`, `song-edit` and `k7x`, and runs over song-edit's `Fs` | the writes are song-edit's already; a verb is a function from a project's files to the writes they get |
| `h.project()` is `song::doc::compile` over the same `Fs`, compiled again after a write, through one `song::Cache` a caller may keep from line to line (`run_request_cached`) | the TypeScript's host compiles the staged files too; the cache is keyed by each file's stamp, so a write is decoded again and a kept cache compiles the song a fresh one does |
| a host member the native app owns is an EFFECT (`reload`, `applyProject`) or an ASK in the answer (`{member, args}`: `transport`, `arm`, `takeArm`, `listPlugins`, `rescanPlugins`, `setInputStereo`, `onnxProviders`, `saveTake`), or a refusal by name when the verb cannot go on without its answer (`render`, `clipEnvelope`) | the crate holds no engine and no clock; the caller performs what it is asked, as the Worker's runtime does, and a line that depends on the answer says what was asked |
| one entry, `{"op":"verb.run","args":{"line","project","env"}}` through the existing `k7_call`, answers `{log, refuse, refused, effects, asks?, threw?}` | the native plan's one seam per gesture (`docs/native-app-plan.md`); the change set is the `Fs`'s, which the caller lands |
| `verb.run` sits behind k7-wasm's `verbs` feature, which no browser module is built with | the web keeps its TypeScript verbs until phase 4; the page gains nothing from a second copy of them |
| a verb the crate has not ported answers the dispatcher's own refusal, `command '…' has no handler (sid)` | a native caller learns by the answer which verbs it may offer, with no table of its own to keep |
| an event verb on a lane that sounds a picture refuses, where the TypeScript silences the shot | silencing a shot is M6's picture code; refusing writes nothing wrong |
| a host member whose answer only fills a log line is an ASK (`asks` in the answer): `timelinePlay`, `playlistStems`, `showNativePluginUI`, `saveBenchChain` | the verb has nothing left to decide once it is asked; the caller performs it and says the outcome |
| `playlist stems` asks with the chain's REF, where the TypeScript hands resolved cold steps | the crate has no `resolveChain`; the caller that runs the stems resolves it |
| `node freeze` on a live node refuses naming `render`; un-freezing a frozen one is ported | `frozen:` may only be written once the bounce has landed, and a verb cannot await it |
| `timeline lane copy` of a picture lane refuses by name | copying shots is M6's picture code |
| tasks, playlists and library pins live in `workspace_dir_of(env, project)` | the native host has no `repoDir`; a project always stands in a workspace |

## Where it stands

| milestone | ported | tests (`apps/k7-daw-wasm/verbs/tests/`) |
|---|---|---|
| M0 | the catalogue, the dispatch, the argument shapes, the format and sounding-lane refusals | `project.rs` |
| M1 | 18 of 18; `events add` checks each pasted field with song's own `Placement` types, so its messages are serde's, not zod's | `events.rs`, `events_add.rs`, `tracks.rs`, `project.rs` |
| M2 | 25 of 25; `event trim` refuses by `clipEnvelope`, and `event/clip/track process` and an audio `selection consolidate` by `render` | `strip.rs`, `takes.rs`, `clip.rs` |
| M3 | 27 of 27: the sequences and their note editor, curves and their points, track and node automation | `sequences.rs`, `curves.rs` |
| M4 | 36 of 36: lanes (group, arrange, duplicate, input, hide), timelines, time splice, aspect, master, console, tags, surfaces, tasks, libraries, playlists | `lanes_m4.rs`, `timelines.rs`, `project_m4.rs`, `playlists.rs`, `tracks.rs` |
| M4 | `track highlight` | `tracks.rs` |
| M5 | 44 of 44: nodes, modules, assemblies, the graph's wiring and layout, taps, presets, alternatives, engines, params, freeze | `graph_nodes.rs`, `taps.rs`, `node_state.rs`, `node_values.rs`, `chain_fixture.rs` |
| M7 | 19 of 19; `system stems` and `system instrumental` store the chain unchecked, since `resolveChain` is TypeScript only | `system.rs`, `transport.rs` |
| M8 | completion: the 25 providers, their labels and the command words | `complete.rs` |
| the compile cache | kept from line to line; on `clip-lane`, 1.1 ms a line fresh and 0.7 ms kept, in release | `cache.rs` |

## Completion

`complete(line, ctx)` in `apps/k7-daw-wasm/verbs/src/complete/` answers what `completionItems` answers: the command words while one is
being typed, then the values of the argument under the caret, each as a whole `line` with an optional `label`.

| choice | why |
|---|---|
| the entry reads `&dyn Fs`, never `&mut`, and `{"op":"verb.complete","args":{line, cursor?, project, env?, labelled?}}` answers `{items}` | completion writes nothing, and a command bar asks on every key; a caller keeps one `song::Cache` (`complete_in_env`) so the compile is read again only for what changed |
| an item is `{line, label?}`, and with `labelled: false` (the default) a bare string, as the page's `runtime.complete` returns | the page that switches to the crate reads the list it reads today |
| `cursor` counts UTF-16 units and cuts the line; what follows it is not read | a browser's `selectionStart` is in UTF-16, and the TypeScript completes the whole string it is given |
| `commands.json` carries each argument's `complete:` and `values:` | the dispatch needs no YAML at run time; the provider names are the TypeScript's, so a caller maps both ways |
| a provider whose read fails answers an empty list | a broken curve file or an unloadable project costs its completion, never the command bar |
| `surfaceTypes` is the four built-in types | the plugin registry of surface types is the page's, and a native caller has none |
| `curves` lists `curves/*.k7x` by file id, without parsing each curve | a curve the compile would refuse still has a file to name |
| a chain project's node is offered by uuid, labelled `<module> · <chain> · step n` | the uuid is what every command carries and the label is what a person reads |
| the playlists come off the workspace folder, not the project's | a set list belongs to the workspace |

## How the native app reaches a verb

`POST /verb {line}` on the backend (`apps/k7-daw-backend/docs/file-changes.md`, "A verb run natively"): the verb runs over the
file service's `StagedFs`, its writes land as one journal step labelled with the line, and `/undo` lays them back. The Swift app
calls it through `K7Client`, as it reaches every other route.

| choice | why |
|---|---|
| a backend route, not `k7_call`'s `verb.run` | a verb's writes need the journal's gate, the account's lock, the relay window and the engine's push, which only the front holds; `k7_call` is a pure dispatch with no account |
| `k7-embed` links the verbs through the front, not through k7-wasm | the crate reaches the phone with the file service that lands its writes; k7-wasm's `verb.run` stays the WebAssembly module's, over its host's imports |
