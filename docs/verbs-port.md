# The verbs in Rust

k7's editing commands are the crate `apps/k7-daw-wasm/verbs`: every line typed in the command bar and every gesture that
edits a project is a command line the crate runs over the project's files. The web page runs a line through the backend's
`POST /verb` (`apps/k7-daw-backend/docs/file-changes.md`, "A verb run by the backend"); the page holds no verb of its own.
The catalogue stays `apps/k7-daw-frontend/packages/commands/commands.yaml`, which the palette reads and the crate takes as
`commands.json`. This page holds how a verb runs, how it reaches the machine, how it is tested, and what is ported.

## Where a line runs

| choice | why |
|---|---|
| the page sends the line to `POST /verb` on k7's backend, never to a WebAssembly module of its own | a verb's writes need the journal's gate and step, the account's write lock, the relay window and the engine's push, which only the front holds; the desktop app, the phones and a browser on the device all serve the page from that same backend, so one seam serves every shell |
| the line runs under the lock and lands as one journal step labelled with it; `/undo` lays it back | undo and history are not verbs: the journal undoes a step whatever wrote it |
| the page asks no bundle and compiles nothing to run a verb | a line costs one request on the device's own loopback, where the bundle, its compile and a 3 MB module cost seconds on a phone |
| the Worker of `@studio/commands` holds a project only for its picture views and runs an import; after a line lands the page hands it the files the step changed | the picture compile is still the TypeScript read side, and a view read after an edit must be the project as it is now |
| completion is `POST /verb/complete` over the same files, read-only, with the compile cache the runs keep | the command bar asks on every key, and a kept cache recompiles only what moved |
| k7-wasm's `verb.run` and `verb.complete` stay behind its `verbs` feature, which no browser module is built with | a host that holds files and lands them itself (a native app over its own folder) calls the crate the same way, through the JSON C ABI |

## How a verb reaches the machine

A verb that needs the machine names a route of the front: `h.call("POST /verb-host/render", args)` for an answer it reads,
`h.ask(route, args)` for one asked once its writes have landed. The routes are the existing `/verb-host/*`, `/waveform` and
`/bench/save`, with their bodies as the args (`apps/k7-daw-wasm/verbs/src/verbs/routes.rs` names them).

| choice | why |
|---|---|
| a call with no answer yet ends the run as `pending` (`{route, args}`), writing nothing; the backend asks the route outside the lock and runs the line again from its start with the answers so far, matched in order by route and args | a verb is a function of the files and the answers; a render takes seconds and a separation minutes, which no lock may be held through |
| the backend asks its own routes with the caller's headers (`Call::reenter`), and polls a job to its end | each route keeps its gates, its weight and its job table; nothing is answered twice |
| the verb speaks real paths and the routes virtual ones, so the args are mapped to the bundle's virtual roots and the answer back | the routes judge every path against the person's read gate, which only speaks virtual paths |
| a job's answer carries its `job` and its `dir`, and the verb copies a file it keeps from there into the project | a render lands in the same step as the clip files that name it |
| the ids a run draws and the stamp a render's name carries are the same on every run of a line | the args of a call must not move between runs, or the answer would never match |
| an ask is asked after the landing, and the lines its answer carries (and a failure's sentence) are added to the run's log | a transport, a take arm or a plugin window decides nothing the verb writes |
| a line calls at most 256 routes | a process over many files asks one render per file; a verb that asks forever is a bug the request must end |

## The crate

| choice | why |
|---|---|
| `verbs` depends on `song`, `song-schema`, `song-edit` and `k7x`, and runs over song-edit's `Fs` | the writes are song-edit's; a verb is a function from a project's files to the writes they get |
| `h.project()` is `song::doc::compile` over the same `Fs`, compiled again after a write, through one `song::Cache` a caller may keep from line to line (`run_request_cached`) | the cache is keyed by each file's stamp, so a write is decoded again and a kept cache compiles the song a fresh one does |
| an effect (`reload`, `applyProject`) is the caller's: the backend pushes the engine once after a landing that asked one | the crate holds no engine and no clock |
| one entry, `{"op":"verb.run","args":{"line","project","env","answers"?,"stamp"?}}`, answers `{log, refuse, refused, effects, asks?, pending?, threw?}` | the backend and a native host speak the same answer |
| a verb the crate has no handler for answers the dispatcher's own refusal, `command '…' has no handler (sid)` | a caller learns by the answer which verbs it may offer, with no table of its own to keep |
| an event verb on a lane that sounds a picture refuses, and `timeline lane copy` of a picture lane refuses by name | silencing and copying shots is the picture verbs' code |
| tasks, playlists and library pins live in `workspace_dir_of(env, project)` | a project always stands in a workspace |

## How a verb is tested

| choice | why |
|---|---|
| a verb is held to its own tests, never to vectors from another implementation | a byte-for-byte match would freeze another program's accidents into the verbs |
| each Rust verb is tested on its effects over a real fixture project (`e2e/fixtures/*.k7`, its `_modules/` and k7's `@meta`) held in a `MemFs` (`apps/k7-daw-wasm/verbs/tests/support/mod.rs`) | what a person sees is the files and what plays; a hand-made `ProjectDoc` hides how the compile reads the files |
| a test reads back the files the verb wrote, as text and as k7x data | a verb's contract is the change set the journal lands |
| a test compiles the project after the verb with the song crate's engine compile and asserts on the `ProjectDoc` | that document is what the backend sends the engine; the C++ render harnesses play it, and stay the engine's tests |
| every successful verb is undone in its test: its change set reversed, the files and the compiled document must be the ones before | undo is the journal laying the previous files back; a change set that undoes to another song is a verb bug |
| a refusal is asserted with the files untouched | a verb that says no writes nothing |
| a chain project (`chains/`, `tracks/`) is a fixture of the crate's own, `apps/k7-daw-wasm/verbs/tests/fixtures/chain-lanes.k7` and `chain-rack.k7`, opened with `Project::open_crate` | e2e's fixtures are all graph projects, and the graph, lane and tap verbs write both layouts |
| the catalogue crosses as `verbs/commands.json`, written from `commands.yaml` by `apps/k7-daw-wasm/verbs/scripts/catalogue.mjs`; `--check` runs in `mac-checks.sh` | the crate parses no YAML and takes no new dependency for it |
| a route a verb calls is answered in its test by a closure (`answered` in `tests/support`), the line run again from the files it started on | the backend runs the line again with each answer, so the test does what the backend does |

## Where it stands

The catalogue names 192 verbs. A verb the crate has no handler for answers `command '…' has no handler (sid)`: today the
22 picture-lane and shot verbs (`picture *`, `shot *`) and `track transcribe`, whose port is under way.


| area | ported | tests (`apps/k7-daw-wasm/verbs/tests/`) |
|---|---|---|
| M0 | the catalogue, the dispatch, the argument shapes, the format and sounding-lane refusals | `project.rs` |
| M1 | 18 of 18; `events add` checks each pasted field with song's own `Placement` types, so its messages are serde's, not zod's | `events.rs`, `events_add.rs`, `tracks.rs`, `project.rs` |
| M2 | 25 of 25; `event trim` reads `GET /waveform`, `event/clip/track process` resolve project and library chains and call `peak`, `render` and `separate`, an audio `selection consolidate` calls `render` | `strip.rs`, `takes.rs`, `clip.rs`, `renders.rs` |
| M3 | 27 of 27: the sequences and their note editor, curves and their points, track and node automation | `sequences.rs`, `curves.rs` |
| M4 | 36 of 36: lanes (group, arrange, duplicate, input, hide), timelines, time splice, aspect, master, console, tags, surfaces, tasks, libraries, playlists; `playlist stems` sends the chain's resolved cold steps | `lanes_m4.rs`, `timelines.rs`, `project_m4.rs`, `playlists.rs`, `tracks.rs` |
| M4 | `track highlight` | `tracks.rs` |
| M5 | 44 of 44: nodes, modules, assemblies, the graph's wiring and layout, taps, presets, alternatives, engines, params, freeze (the bounce through `render`, then `frozen:`), `chain save` through `POST /bench/save` | `graph_nodes.rs`, `taps.rs`, `node_state.rs`, `node_values.rs`, `chain_fixture.rs`, `renders.rs` |
| M7 | 19 of 19; `system stems` and `system instrumental` check their chain against the open project, `system onnx` bare says the machine's list, `transport arm` words the engine's answer | `system.rs`, `transport.rs` |
| M8 | completion: the 25 providers, their labels and the command words | `complete.rs` |
| the compile cache | kept from line to line; on `clip-lane`, 1.1 ms a line fresh and 0.7 ms kept, in release | `cache.rs` |

## Completion

`complete(line, ctx)` in `apps/k7-daw-wasm/verbs/src/complete/` answers the command bar: the command words while one is
being typed, then the values of the argument under the caret, each as a whole `line` with an optional `label`.

| choice | why |
|---|---|
| the entry reads `&dyn Fs`, never `&mut`, and `{"op":"verb.complete","args":{line, cursor?, project, env?, labelled?}}` answers `{items}` | completion writes nothing, and a command bar asks on every key; a caller keeps one `song::Cache` (`complete_in_env`) so the compile is read again only for what changed |
| an item is `{line, label?}`, and with `labelled: false` (the default) a bare string | the command bar shows a label where there is one and the line otherwise |
| `cursor` counts UTF-16 units and cuts the line; what follows it is not read | a browser's `selectionStart` is in UTF-16 |
| `commands.json` carries each argument's `complete:` and `values:` | the dispatch needs no YAML at run time; the provider names are the palette's (`providerInfo.ts`), which colours each class |
| a provider whose read fails answers an empty list | a broken curve file or an unloadable project costs its completion, never the command bar |
| `surfaceTypes` is the four built-in types | the plugin registry of surface types is the page's, and a native caller has none |
| `curves` lists `curves/*.k7x` by file id, without parsing each curve | a curve the compile would refuse still has a file to name |
| a chain project's node is offered by uuid, labelled `<module> · <chain> · step n` | the uuid is what every command carries and the label is what a person reads |
| the playlists come off the workspace folder, not the project's | a set list belongs to the workspace |
