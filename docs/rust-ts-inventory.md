# TypeScript inventory: what moves to Rust

The plan that shrinks k7's TypeScript to what draws (`rust-first.md`). Every in-scope module of
`apps/k7-daw-frontend` has one verdict: **MOVE** (pure logic, to a Rust crate reached through the
WebAssembly module or a backend route), **GENERATE** (a type mirroring a Rust struct, to ts-rs),
**STAY** (rendering, UI state or a browser-only API) or **DELETE** (dead, or served only by code
that is gone). Lines are counted without tests, stories, `.d.ts` and generated files.

Out of this page, because another plan owns them: the song model twins (`song`, `song-schema`,
`k7x`, `song-edit`: about 24,800 lines) and the verbs' leftovers (`commands`, `song-video`,
`translator`, `src/verbs`, the verbs' Worker: about 9,400 lines, deleted by the verbs branch).
`k7-wasm` is the binding itself and stays.

## Choices

| choice | why |
|---|---|
| a gesture's arithmetic that runs on every pointer move moves behind a typed numeric wasm export (numbers in, numbers out), not a JSON op | the k7-wasm ABI is one JSON request per call; a pointer move cannot afford parsing a project, a few numbers can be passed for under a microsecond |
| per-frame animation (springs, glides, playhead clocks, meters, the compositor's per-layer lookups) stays in TypeScript; where a document drives it, Rust precomputes a table once per document | a wasm call per frame buys nothing, and a native interface brings its own clock and springs |
| Rust returns message keys and parameters; TypeScript turns them into words with paraglide | the words are the page's, the rule is Rust's, and a native interface has its own string tables |
| a moved module's TypeScript tests are the oracle: their inputs and expected values become the Rust tests, and the TS test is deleted with the TS twin | the TS tests already say what each function returns; porting them is the cheapest proof the Rust answers the same |
| a module is moved by milestone, in clusters that touch disjoint files, not one by one when touched | the clusters share a target crate; three builders can work at once without conflicts |
| a file over 500 lines that STAYs (`App.tsx`, `TimelineTab.tsx`) is split by concern while its logic leaves, not moved | it is drawing; its size is a structure problem, not a Rust one |

## Totals

Measured on `main` at 264cd285b: 937 files, 134,047 lines in scope.

| verdict | files | lines in those files | lines that leave TypeScript |
|---|---:|---:|---:|
| MOVE | 172 | 20,928 | 18,590 |
| STAY (40 of them hold logic that moves) | 711 | 105,190 | 2,395 |
| GENERATE | 24 | 3,881 | 2,512 (replaced by ts-rs output) |
| DELETE | 30 | 4,048 | 3,819 |
| **all** | **937** | **134,047** | **27,316** |

By zone, the lines that leave: the app's modules 6,700, its views and hooks 2,800, the chains,
panels and plugins 9,960, the UI kit, protocols and engine clients 7,860.

## The size at the end

About **106,700 hand-written lines** of TypeScript stay in scope, plus about 2,500 lines of ts-rs
output. What stays is the React views (`src/components` alone is 29,000 lines), the UI kit, the
chains' renderer and widget sandbox, the DJ desk's views, the WebGL compositor, CodeMirror, the
browser's engine client and audio stream, the project shell, Web MIDI and the build tooling.
Two decisions below can take a few thousand more: per-frame maths (about 2,000 lines) and the
shell's command registry (`src/ui/commands`, 2,018 lines). Of the song twins and the verbs,
whatever their own plans leave is added to that figure.

## Milestones

Three builders at once, each on its own files; a wave starts when its dependency has merged. Each
milestone ends with `bash scripts/agent/gate.sh --milestone`; the hours are agent-hours.

| milestone | content | lines out | target | depends on | hours |
|---|---|---:|---|---|---:|
| **M1a** quick deletes (done) | the DELETE rows below that need no branch: barTint, PairThisDevice (then `requestPairing`), RemoteSlotsSection, menuAccount, the EngineSyncProgress component, ModeStrip, the `aiOrganize` re-export, `audio-probe/src/waveform.ts`; WebRTC end to end: rtcSocket (keeping a 25-line socket wrapper), rtcFrames, `engine-tunnel/rtc.ts`, `node-datachannel`, the backend's `/rtc` route and `rtc` flag | 1,100 | none | nothing | 3 |
| **M1b** first ts-rs | `graph-model/types.ts` from `song/src/graph/view/web.rs` (already derives); the `ts` feature and a bindings binary for the backend crates; the app's mirrors (AuthUser, K7Prefs, LocalApp, ServiceProcess, sync history, the desktop engine snapshot, LanPeer) | 700 | k7-wasm bindings, k7-front, k7-lan, k7-sync, k7-supervisor | nothing (exports only from the song crate) | 8 |
| **M1c** (built) Rust twins | textHash, compat, quality and choose, engineWarnings and engineMissing (classified by Rust, decision 4), performance (the page's scoring deleted; the supervisor scores); paths, clipEnvelope, engineSession, engineTaken and the five rows with no Rust function (lcdSounds, libraryTree, historyMarks, surfaceRows, paneInUse) stay, see below | 310 | song-schema `engine/` and `text_hash`, k7-wasm ops, k7-file-service, k7-supervisor | nothing | 10 |
| **M2a** (built) chains build | `ui-chains` build and measure steps (buildGraph, buildChains, buildContext, assembly, subgraph, bindings, geometry, moduleRow, rowSeams, gridFit, stackLayout, nodeUIs, widgetValue, flow, the logic of cables, curvePlacement...), `chainsModel`, `moduleField`, `widgetTarget`; graphNode generated; dagre stays in TS until M5b | 3,700 | new crate `apps/k7-daw-wasm/chains`, op `chains_build` | M1b | 16 |
| **M2b** (built) gesture arithmetic | snap, rollDrag, rollFit, gridCaret, barWindow, clipPlayhead, clipWarp, videoDrag, takeCounter, timelineTools, formatAt; the arithmetic inside TimelineTab, useRollGestures, PianoRoll, useEdgeDrags, useVideoDrag, AudioClipPanel; the ruler's tick ladder and `ui-timeline` ticks and anchored zoom; what stays, see below | 1,100 (1,800 of tests) | new crate `apps/k7-daw-wasm/gesture`, typed exports | decision 1 | 13 |
| **M2c** tunnel stand-in (done) | the hub suites ported to Rust tests over the Rust supervisor; `e2e/src/standInEngine.ts` on the supervisor binary with a fake engine; `engine-tunnel` deleted with its 2,100 test lines | 2,500 | k7-supervisor, k7-hub tests | nothing | 10 |
| **M3a** verbs catalogue (built, see below) | contextMenu, menuTabs, menuTitle, palette, argOptions, rankCommands, fuzzy, commandFill, commandPrefs, emptyBar, stepSentence, exportArgs, pendingEdits, nodeInsert, nodeMenu, clipOps, playlistVocabulary, reloadScope, verbWrites, homeCommands' catalogue and parse; `commandTypes.ts` generated | 2,500 | verbs crate | verbs branch merged; decisions 2, 6 | 14 |
| **M3b** pictures and render tooling (done) | take-render's verbs-only files deleted; engine-client and the render scripts relocated to `k7-runtime/scripts/lib`; ui-eyes scene generated; its clip schedule, transitions and `.cube` reader; the grip drag of `viewportRect`; the camera name rules | 1,000 | new crate `apps/k7-daw-wasm/eyes`, op family `eyes` | verbs branch merged; decision 7 | 8 |
| **M3c** (done) git rules and media bytes | gitState, commitState, connectState, forgejoState, gitLock, gitPageTabs, mergeState, saveAct, githubState and roamState rules; serde structs for the git routes, then their types generated; webmDuration, trimGate, spectrumColour | 1,000 | git-rules (new, `apps/k7-daw-wasm`), k7-git, k7-front, waveform, k7-wasm | decision 5 | 11 |
| **M4a** (built, see below) reads on the doc | takes, rackLit, noteSpans, videoSpans, activity, clipLoop, clipInstances, clipNames, rows, timelineView, mixerStrips, gainTrim, mastering, midiIn, laneInput, rackFor, graphNotes, templateGraph, headphonesHint; the hooks' and pickers' doc reads, one mute/solo rule, tapTempo, projectName, projectKind; `engine-protocol/project.ts` and `timelineTypes.ts` generated, `build.ts` moved | 2,800 | song crate, k7-file-service | rust-song-twins merged | 16 |
| **M4b** (built) buffer text and paths | bufferReads, clipWindowText, sliceSymbols, clipFiles, timelineDrop, rackBuilder, presetOrder, videoTake, editorPaths, projectPaths, localTargets, openInEditor, sequenceEdit, moduleBehind, scalarField, curveDraft, clipboard; the editor's cursor scan, entity values and preview triggers; noteGrid's notes | 1,500 | song-edit, song-schema, k7x | rust-song-twins merged | 10 |
| **M4c** (built) panels and AI organize | layoutTwins, groupGeometry, autoLayout, gridCells, panelTabs, widgetClipboard, widgetKinds, useParamRows, moduleUi, panelWrite, panelText, checkLayouts; the AI loop to a backend route (see "M4c, as built") | 2,860 (415 of tests) | song `modules/ui`, k7-front `routes/ai` | rust-song-twins merged; decision 3 | 11 |
| **M5a** (built, see below) DJ desk | deck, metronome, beats, grid, loop, mapping, controls, mixBindings, setList, put-on; deckVocabulary generated; `mixView` and the lookups of `mixBindings` stay (see "M5a, as built") | 1,380 (1,800 of tests) | new crate `apps/k7-daw-wasm/turntables`, op family `desk` | M2a (bindings), M4a | 11 |
| **M5b** (built) graph layout | graphLayout (the only dagre importer) in the `chains` crate, `graph.layout` op; `pendingNode`, `trackPanelLayout` and the screens' shaping stay (see "M5b, as built") | 440 | chains crate + rust-sugiyama (decision 2) | M2a | 5 |
| **M5c** (done) web-protocol's rest | workspace, machine, playlists, resources and cameras types generated; the project address grammar of `projects.ts` | 1,100 | k7-file-service, k7-front | M1b | 8 |

**M1a is done.** It removed: `barTint`, `PairThisDevice` and `requestPairing`, `RemoteSlotsSection`,
`menuAccount`, the `EngineSyncProgress` component (`syncRunning` now sits in `EngineSyncFailedDialog.tsx`),
`ModeStrip` and `stripButton`, `ui-module-builder`'s `aiOrganize` re-export (`api.ts` imports `@studio/panel-rules`),
`audio-probe`'s `waveform.ts` (the desk's `Envelope` type is written in `k7-plugins/src/turntables/waveform.ts`),
the TS key detector and `scripts/key-truth.ts` (`key.ts` keeps `PITCH_CLASSES` and `camelot`), `pulledSentence`
written once (`src/git/pulledSentence.ts`), the knip-flagged exports, and WebRTC end to end: the page's `rtc:`
socket (`engine-browser/src/socket.ts` is the 5-line `openSocket` left), `rtcFrames`, `engine-tunnel/rtc.ts`, the
`rtc` flag of an engine's listing and the `rtc-offer` and `rtc-answer` frames in the hub, the supervisor and the TS
reference agent, `/api/engine/<id>/rtc` in k7-front, `node-datachannel` and `docs/rtc.md`. The generated
`INTERACTIONS.md` and `gaps.csv` lost the gap line naming `RemoteSlotsSection`; the Sheet's Gaps row still holds it.

**M3b, as built.** About 500 lines of page TypeScript went (the scene's 240 interfaces, `timeline`'s rule,
`transitions`, `cube`, `matchCamera`, the grip arithmetic, the three camera name rules the page never called),
with about 450 of their tests; 734 lines of engine tooling moved out of `packages/` to `k7-runtime/scripts/lib`
(`engine-client`, `take-render`, their two test files and two doc pages). About 1,100 lines of Rust came in the
`eyes` crate (`docs/eyes.md` there) and its `eyes.*` ops. The take-render files the row named
(`materialRender`, `renderToWav`, `wavLength`) were already gone with `@studio/commands`. The choices:

| choice | why |
|---|---|
| one new crate, `eyes`, which `verbs` (the server's scene) and `k7-wasm` (the page's ops) both depend on | the scene's types and the rules over them have one home that both a native and a browser host can link; `verbs` itself is built into no browser module |
| the scene's types are generated into `k7-wasm`'s `types/`, and `ui-eyes/src/scene.ts` re-exports them | `k7-wasm`'s bindings binary already runs with the module build; a second bindings target would need `k7-front` to link `eyes` for nothing |
| a layer carries a `schedule`, a table of spans looked up by binary search; a blend's progress is a ramp | the per-frame rule stays off the wasm boundary (decision 12), and `layerFrameAt` shrinks to a lookup plus three ramp reads |
| the scene always carries the two built-in transitions | the compositor drops its own copies; a hand-built test scene takes them from `eyes.builtins` (`ui-eyes/src/sceneFixture.ts`) |
| a LUT's floats come back as a view of the module's memory (`k7_floats_ptr`), the grip drag as a typed export (`k7_drag_rect`) | JSON would carry a million numbers once, or a rect per pointer move |
| `cleanCameraName`, `uniqueCameraName` and `matchRemoteCamera` are deleted, not moved | the page called none of them: the hub's Rust (`k7-front`'s `names.rs`) is the one copy, pinned by the committed `camera-vectors.json` |
| `gripAt`, `layersAt` and the other hit-tests of `viewportRect` stay TypeScript | they read what is on screen |

The proofs: `layerFrameAt` against the old TypeScript rule on 3,000 random layers, 606,500 times read at the
breakpoints, a hair either side and at random (frame, top shot and shape-lane visibility all equal); the new
reader against the old on forty generated `.cube` files (sizes 2 to 65, domains, CRLF, scientific notation),
byte for byte, and the same message for seven malformed ones. The repository holds no `.cube` file, so none is
a real grade. The old TypeScript tests became `eyes/tests/*.rs` with their inputs unchanged.

**Parallel groups.** Wave 1: M1a, M1b, M1c. Wave 2: M2a, M2b, M2c. Wave 3: M3a and M3b once the
verbs branch merges, M3c meanwhile (it waits on nothing but decision 5). Wave 4: M4a, M4b, M4c
once the song twins merge. Wave 5: M5a, M5b, M5c. About 155 agent-hours of building, five waves,
plus one review per milestone.

**W6c, as built.** `packages/k7-wasm/src` holds 1,205 lines of tests (it held 4,668): a facade keeps one test that proves a call reaches
the module and answers its shape, and the rules are Rust's. The k7x document editor and printer are pinned in `k7x/tests/document_*.rs` and
`read_print.rs`; the text writers (lane, sequence, cue sheet, playlist, pins, assembly, chain, short forms) in `k7-wasm/src/tests_text_*.rs`, which
call the ops by name through `tests_support.rs`; the editor's check, the events and `project.layers` in `song/tests/all/files_validate_cases.rs`;
AI organize and the param rows in `song/src/modules/ui/ai_cases_tests.rs` and `rows_tests.rs`; the order of the completions in
`song/tests/all/views_fields_order.rs`; the timecode and the curve odds and ends in `song-schema` and `k7-wasm/src/tests.rs`. The TS tests that stay
test TypeScript: the typed-call marshalling (`gesture`), the build scripts (`build`), the tree cells (`k7x/ast`), the resolvers that pick by the browser's
collator (`paths`), the handle's healing (`curve`), the facades' maps in and out (`chains`, `schemaPage`, `ui`). The facade exports with no caller but a
test are deleted (`curvePreview`, `editSequence`, `sequenceData`, `readChain`, `applySugared`, `readAiAnswer`, `gridIssues`, `placementStartOf`, the
constants of the ruler and `fuzzyScore` among them); the exports whose only caller is another package's test (`familyOf`, `isPlumbing`, `isBlend`,
`groupParams`, `validatePanel`, `toAlphas`, `verbCatalogue`) and the typed gesture calls whose Rust export would have to go with them
(`dragOffset`, `snapPosition`, `clampKey`, `keyOffset`) stay for the facade generator.

**M4c, as built.** The panel's rules are the song crate's `modules/ui` (`apps/k7-daw-wasm/song/src/modules/ui`): `geometry` (groups, cells,
the two layouts), `edit` (tabs, clipboard, retype), `auto` (the proposal from a parameter list, its pattern table matched by `rx`, a 150-line matcher
for the few JavaScript regular expressions it keeps as written), `write` (a panel read from and printed to its k7x text, the fewest edits, a widget a
line), `rows` (the union of a module's params), `check` (`lint:layouts`) and the AI organize files `ai_prompt`, `ai_answer`, `ai_check`, `ai_script`.
They are reached through the `panel.*` ops and `@studio/k7-wasm/panel`, a panel travelling as loose JSON. Gone: `groupGeometry`, `layoutTwins`,
`panelText`, `gridCells`, `panelTabs`, `autoLayout`, `panelWrite`, the panel half of `moduleUi`, the union of `useParamRows`, `checkLayouts`' rules, the
retype, bindable and ranged rules of `widgetKinds`, the cut, copy and paste of `widgetClipboard` (2,860 lines of TypeScript, 415 of them tests; 5,400
of Rust, 1,100 of them tests, 155 of them the model's prompt). The AI loop is `POST /api/ai/organize` (`routes/ai/organize.rs`, decision 3):
`aiLoop`, `aiRequest`, `aiAnswer`, `aiOrganize`, `aiPrompt`, the `/organize/turn` route and `zod` in `panel-rules` are gone, the package keeps the
reader of the run's stream. What stays TypeScript: the palette and icons (`widgetKinds`), the held clip, the file's other fields in `moduleUi` (M4a's),
the collator that orders the rows, and the browser's compile of `widgets.js`. Parity: the old TypeScript against the ops on the 250 library panels and
1,500 generated ones, 764,000 comparisons in all (every rule of geometry, tabs, clipboard, retype and auto layout; 1,975 writes of a `ui.k7x` and 1,975 of a
module's `ui(…)` block after one to three edits, byte for byte; 1,204 param-row unions; 1,500 AI proposals judged, 1,250 answers read, 400 prompts built), and the
sixteen recorded cases of `organize-vectors.json` replayed through a whole front (the same requests to Claude, progress lines and end). `lint:layouts` prints
the same on the 250 panels and on 250 mutated ones (156 with findings). Two differences on purpose: the host reads `widgets.js`' strings, comments and brackets
(it has no JavaScript engine; the page still compiles it where the panel lands), and a refused request says its own sentence, not zod's.

**M2a, first part as built.** The crate `apps/k7-daw-wasm/chains` (no dependency but serde, serde_json and
indexmap) and `@studio/k7-wasm/chains` carry `widgetValue`, `subgraph`, `bindings` (with `stepsByModule`,
`widgetFor` and `resolveTarget`, now inside the Rust join), `flow` and `computeFlow` of `cables`, and `curvePlacement`:
about 650 lines of TypeScript left (`widgetValue`, `subgraph`, `flow`, `bindings`, `curvePlacement` and the `computeFlow` block
of `cables`) and 1,000 lines of their tests, replaced by 1,770 lines of Rust (half of them tests) with 44 Rust tests and a 150-line facade test. The old tests ran
against the Rust ops first (56 cases through the facade), and 40,000 generated widgets and values over six
functions showed no difference with the old widget maths but a NaN: a typed text that was NaN is now "no number". One
behaviour differs on purpose: a track's module row puts its own chains first and keeps each group in reach order (the
TypeScript comparator was not a total order). A call costs 1 microsecond for a widget, 40 for a 64-lane seed, 0.27 ms for the
flow of 500 cables, 0.26 ms for a lane read and 1 ms for a 20-line binding join on a 1,200-step graph, the graph being sent slimmed
(`k7-wasm/docs/chains.md`).

**M2a, second part as built.** The build core moved into the same crate: `build`, `build_chains`, `node_ui`, `geom`, `grid_fit`,
`assembly`, `module_row`, `row_seams`, `stack_layout`, with the ops `graph.build` (the assemblies folded in the same call),
`graph.row`, `graph.align`, `graph.space`, `graph.seams`, `graph.inserts`, `graph.jackRef`, `graph.stack` and `graph.nodeSize`.
The TypeScript twins `buildChains`, `buildContext`, `gridFit`, `nodeUIs` and `assembly` are gone, `buildGraph`, `moduleRow`,
`rowSeams`, `stackLayout` and `graphNode` are facades of a few dozen lines each (`graphNode`'s shapes are generated), `geometry`
keeps only its drawing half, `insertFor` and `jackRef` and the node-size rule are Rust's. Parity: the old tests ran unchanged against
the ops through the facades (about 1,000 cases), and a differential over the 174 golden graphs and 220 random ones, each built under
nine option sets and also folded, rowed, stacked and seamed against the frozen twins, compared 32,308 nodes, 24,106 cables, 168 boxes,
263 per-tab widths, 3,393 clip lists, 249 rows, 590 seams and 45 side cables with no difference. The pure tests (the fold, the sockets,
the built-in panels, `insertFor`, `jackRef`, `chainExit`) were ported to 43 Rust tests, beside 12 build tests (99 in the crate, and two dispatch tests in `k7-wasm`).
Costs are in `k7-wasm/docs/chains.md`: the build is slower than its twin (0.7 ms against 0.3 on a real project, 27 ms against 10 on
1,300 steps), `stackLayout` 16 times faster. What stays in TypeScript, and why, is the last section of that page: the cables'
drawing, `widgetTarget` and `moduleField` (per widget per render), `autoTarget`, the layout. A folder dropdown's names are sorted by
the page's collator after the call. About 1,500 lines of TypeScript went with their files and 2,100 more with the facades' bodies and tests.

**M1c, as built.** Seven twins went (about 310 lines, 560 of tests): `textHash`, `compat`, `quality`,
`choose`, the classification in `engineWarnings` and `engineMissing`, and `performance`'s scoring, with
their TypeScript tests,
which first ran against the Rust ops as a differential (about 130,000 generated inputs, no
difference but the compatibility score's integer floor) and now live as `song-schema` tests. They
live in `song-schema` because it is the one crate both the page's module and `k7-file-service`
can depend on. What stays, and what each needs:

| module | why it stays | what moves it |
|---|---|---|
| `web-protocol/paths.ts` | its callers are the song twins | their deletion (rust-song-twins); then it is unused |
| `clipEnvelope.ts` | not a twin of `wfSig`: it keys what the page holds, `wfSig` what the service holds | the service sending its signature with the waveform |
| `engineSession.ts` | address parsing around a one-line slug; `session_of` takes a workspace root, not an address | M5c's project address grammar |
| `engineTaken.ts` | a three-flag decision over React state, called on render | stays |
| `lcdSounds`' WAV decode | the waveform module is embedded by the Rust front, the page does not load it | a decode route in the file service |
| `libraryTree`, `historyMarks`, `surfaceRows`, `paneInUse` | no Rust function answers them: ports, not replacements | M4a (doc reads), M3c (git), M5c |

**M3a, as built.** The bar's, the menus' and the edits' rules are the `verbs` crate's `ui` module (`apps/k7-daw-wasm/verbs/docs/verbs/ui.md`),
reached by the `ui.*`, `edit.*` and `menu.*` ops, which the browser's module carries by building with the crate's `verbs` feature
(the module grows by about 1.1 MB). Gone from TypeScript: `fuzzy`, `rankCommands`, `argOptions`, `emptyBar`, `commandFill`, `stepSentence`,
`commandPrefs`' rules, `exportArgs`, `pendingEdits`, `reloadScope`' predicates, `nodeInsert`'s rules, `playlistVocabulary`'s, `clipOps`' filter and line,
the table of `verbWrites` (a verb is a write unless listed as a read or the device's), the whole of `contextMenu`'s derivation, `menuTabs`, `menuTitle`'s
rule, `nodeMenu`, `palette`'s merge and vocabularies, `homeCommands`' catalogue and readers, and `commandTypes.ts` (generated). Each TS test first ran
against the Rust ops, then became a Rust test. What stays, and what moves it:

| module | why it stays |
|---|---|
| `commandPrefs.ts` (types), `reloadScope.ts` (type and hook), `clipOps.chainOps`, `nodeInsert.ts`, `playlistVocabulary.ts`, `menuTitle.ts`, `nodeMenu.ts`, `menuTabs.ts` | thin glue: the page's own types, the browser's collator, a namer callback, a message catalogue, or a re-export that keeps an import path |
| the shell's command registry rows (`ui/commands`) | decision 11 asked for its sentences and arguments to become catalogue data; they are still read from the registry and handed to Rust (`buildPaletteCommands`), so that move is open |
| `homeCommands.runHomeCommand`'s branches | they call the file service's acts and keep the order of their refusals |

**M5b, as built.** `ui-chains/src/graphLayout.ts` (440 lines) and its tests are gone, and `@dagrejs/dagre` with them: the
layout is `apps/k7-daw-wasm/chains/src/graph_layout.rs` (engine, nested frames) and `graph_layout/arrange.rs` (rank rows, master run, push
below inputs, frames pushed aside), reached by the op `graph.layout` and the 60-line facade `ui-chains/src/layout.ts`. The engine is
**rust-sugiyama 0.4.0** (published 2025-09-21) over **petgraph 0.8.3**, in the chains crate only; no golden vectors were pinned
(the user's change of plan: the layouts are judged in the app). Kept on top of its answer: the rank rows and collision sweep, the master
run, the pins and held places, the curve cards beside their targets, the push below inputs, the open frames laid out by a recursive pass
and pushed aside. Dropped: the cables' weights and the build's rank as a layering hint (the engine takes neither), a cable's route (it was
already replaced by the sockets, so it is the two centres), and dagre's own margin (replaced by the same 16 px). Added: the cables that
close a cycle are withheld from the engine, because rust-sugiyama never returns on some dense cyclic graphs (a 12-card, 53-cable random
graph hung it), and the separate pieces it returns are set side by side. 122 Rust tests in the crate (14 of the layout, 9 of the arranging
rules ported from the old tests, one run of 1,000 random cyclic graphs under a watchdog), 1 dispatch test in `k7-wasm`; the old layout tests
run unchanged through the facade, minus the numbers that were dagre's. Left in TypeScript: `pendingNode` (it splices the page's own laid-out
cards with the drawing's `jackPoint`, and the laid graph would cross the ABI both ways for a spliced cable) and `trackPanelLayout`
(a composition of three Rust ops), and ChainsTab's and GraphPanel's memo shaping (React state).

**M5a, as built.** The desk's rules are the crate `apps/k7-daw-wasm/turntables` (about 1,160 lines of Rust, `desk` ops in `k7-wasm`, `@studio/k7-wasm/desk`
for the page, `k7-plugins/docs/turntables/rust.md` for the choices). Gone from TypeScript: `beats`, `grid`, `loop`, `mapping`, `controls`, `metronome`,
`deck`, and the parts of `deckVocabulary`, `mixBindings` and `setList` that left (1,140 lines in all), `put-on.ts` (238, now the binary `apps/k7-daw-wasm/put-on`), and about 1,800 lines of their tests (a trimmed
`mixBindings.test.ts` and `setList.test.ts` stay for what stays). `deckVocabulary.ts` is 47 lines of re-exports and the cue colours: its numbers are printed
into `consts.generated.ts` and its types are generated, `DeckModule` extending the generated `DeckModuleDoc`. `with_decks` and `deck_doc` build over `song`'s `ProjectDoc`;
`setList` returns issue keys that the page puts in words. Not moved, and why: `mixView.ts` (file reads and the 404 rule, browser I/O), the five lookups of
`mixBindings.ts` that read the engine's live state each render (`hasPlugin`, `shownValue`, `engineAgrees`, `controlDriving`, `leavesByItsOwnPair`), `browseSongs`
(the browser's `fr` collator), `entryForDrag`; `normalise`, `denormalise` and `mixControl` were called by no page code and are deleted. `useDeckSet` kept almost no arithmetic
of its own (two lines of `deckRunning` and `worthARebuild`, which moved), so the 400 lines the row counted were not there; `scratchDelay`, asked on every pointer move, is the typed
export `k7_g_scratch_delay`. The proofs: the old TypeScript against the ops on 101,000 generated cases (3,000 beat grids and 3,000 alignments against a clustered, jittered
and random onset mix; 6,000 each of the loop rules; 1,500 whole documents, half over a compiled project with stem lanes, `Seek`, extra clips and modules, deep-equal; 3,000 each of
the platter, sync, keylock, nudge, click-deck, crossfade, scratch and rebuild rules; 1,500 random graphs and mappings resolved, stretched, published and scaled, 880 values
through linear, log, stepped and mirrored widgets; 300 playlists), all equal. One difference is accepted: the grid's `cos`, `sin`, `atan2` and `hypot` are Rust's, not V8's;
61 % of 20,000 random grids are bit-identical and the worst gap is 7e-16, so a `confident` flag could flip only on the exact edge of its threshold. 55 Rust tests in the crate
(the old tests' cases, seconds in debug), 6 in `k7-wasm`.

**M2b, as built.** Eleven modules and the rulers' ladder went (about 1,100 lines of logic, 1,800 of
tests; the page keeps `timelineTools.ts` (the tool names, their labels and the words of a note),
`videoDrag.ts` (the drag's state and `DRAG_SLOP`), `gridCaret.ts` (the shell's handle) and the
re-exports of `ui-timeline/ticks.ts` and `panZoom.ts`). The crate has no dependency but `song-schema`
(`round_beat`) and serde; every function answers what JavaScript's did, down to `Math.round`'s half-up,
the sign of zero and `toFixed`'s ties (`js.rs`). Each TS test file first ran against the Rust calls
(about 200 cases of the old suites, no failure but one assertion on object identity and one on a
constant that is Rust's now), and each function then ran against the old code on 100,000 random inputs with
no difference beyond one ulp of `exp` on a pinch's zoom factor and the sign of a zero that crosses
a JSON op. Call costs (an idle Mac, Node, per call): a typed call about 25 ns (the old TypeScript 5
to 15), a move of the clip drag (two calls) 57 ns, a picture drag move with 40 ticks 225 ns, a
300-tick `nearestTick` 1 µs; the JSON ops 2 to 18 µs (`planGroupMove` 3.7 µs, `rulerTicks` 18 µs
per scroll). What did not move:

| left in TypeScript | why |
|---|---|
| the rest of `TimelineTab`'s inline `Math.*` (small clamps on beats and rows, the caret's step, the marquee) | each is two lines on React state; moved without its state it is a call per line |
| `AudioClipPanel`'s `zoomTo`, the pinch baseline and the playhead's beat | a callback over a React updater, and per-frame drawing (decision 12) |
| `useChainsCamera`'s wheel use of `panZoom` | `ui-chains-react` belongs to M2a; it still imports `isMouseWheel` and `isZoomWheel` from the re-exports |
| `ClipActivity`, `clipLoop`, `playedZone` and the other reads on the doc | M4a |

**M2c, as built.** `engine-tunnel` is gone (about 4,300 lines: 2,500 of tests and their harness, 1,800 of source); its four doc pages
live in `apps/k7-daw-backend/docs/engine/` (`tunnel.md`, `tunnelJobs.md`, `backpressure.md`, `sockets.md`). The choices:

| choice | why |
|---|---|
| the hub's suites are Rust, in `k7-front/tests/tunnel`, over the real agent linked as a library and the testbed in a process of its own | the hub's routes and sockets are the front's; a process per test keeps its registry and its limits apart, and the agent as a library needs no binary beside it |
| `k7-front` turns `test-hooks` on for its own tests through a dev-dependency on itself | `cargo test` then builds the testbed the suite spawns, so the gate runs the suite without a feature flag |
| the TypeScript unit tests of code that is gone moved to the Rust unit that holds the rule (`k7-engine`'s backpressure, frames and spelling; `request_job`'s time limits in `k7-front`; the agent's drain mark in `k7-supervisor`) | the rule stays pinned where it lives; the agent's path map and segment cases were already `tunnel_paths.rs` |
| the conformance stand-in keeps its TypeScript fake engine (the specs script its frames) and runs the supervisor's `tunnel_harness` for the tunnel | the specs need frames they choose and a heard-list; only the machine's end is the supervisor's. The real `k7-supervisor` would add its own probes to that list |
| `e2e-macos.sh` builds `tunnel_harness` into the bed's target folder and names it in `TUNNEL_HARNESS_BIN`; a missing binary fails the stand-in's start | nothing is skipped without a word |

**M3c, as built.** About 1,525 lines of TypeScript left (805 of source, 720 of tests), against 464 of wrappers and tests and 611 of
generated types; 2,490 lines of Rust came (700 of them tests, 77 tests, every old case ported). The page's rules of git and the three
media modules are Rust, reached through `@studio/k7-wasm/git` and `@studio/k7-wasm/media`. What each became:

| module | now |
|---|---|
| `gitState`, `commitState`, `connectState`, `forgejoState`, `gitPageTabs`, `mergeState`, `saveAct`, `githubState`, `roamState` | the rules are `git-rules`' (`address.rs`, `state.rs`, `files.rs`, `merge.rs`, `github.rs`, `roam.rs`, `save.rs`); the TypeScript keeps the reducers, the menu's wording and the toasts, and calls the crate |
| `gitTypes`, `forgeState`'s and `githubState`'s wire types | generated from `git-rules`' structs by `k7-wasm`'s bindings; the front's state, forge, GitHub, Forgejo and pull routes build those structs |
| `gitLock` | the gate, its route table and its installation in `App.tsx` are gone, with `setWriteGate`, `refusedWrite` and `ReadOnlyDocument` of `project-shell`; `afterRead` (fail closed) and the sentence stay |
| `webmDuration` | `waveform`'s `webm_duration.rs` patches the head; the Blob slicing and the whole-file fallback stay |
| `trimGate` | `waveform`'s `trim.rs`; `AudioClipPanel` calls `trimWindow` of `@studio/k7-wasm/media` |
| `spectrumColour` | `waveform`'s `spectrum.rs` returns an RGBA strip per tile; `paintColumns` draws from it, and `bandsAt` moved into `spectrum.ts` |

| choice | why |
|---|---|
| the git rules live in a new crate, `git-rules`, with no git2, and `k7-git` re-exports its `Entry`, `Conflict` and `LogEntry` | the inventory named `k7-git`, but libgit2 does not build for the browser: the page's WebAssembly module needs the rules and the backend needs the shapes, so both link one pure crate |
| the git wire types are exported by `k7-wasm`'s bindings, not by `k7-front`'s (decision 8's one binary) | a generated file imports its neighbours by relative path, and a request type must land in one folder with the structs inside it; the derives are in the crate behind its `ts` feature all the same |
| a rule is one JSON call of the page's module, not a field the route adds to its answer | the page keeps its fixtures and tests, and a rule on typed input (an address, a filter) has no route to ask |
| a list's by-name order stays in the browser (`localeCompare`) | Rust has no locale collation, and the roam list is the one place a person reads names in order |
| a repository with no push date sorts last in the picker | the old comparator left it in place, which no total order can say |
| the Rust WebM patcher refuses a head whose sizes run past its end, where the old one patched it with a NaN scale | a refused head falls back to the whole-file fix; garbage in no longer yields garbage out |
| `waveform` gains a default `ffi` feature, which `k7-wasm` turns off | linking the arithmetic must not add the sidecar module's exports to the page's module |
| a duplicate of `trim_window` remains in `apps/k7-daw-wasm/verbs/src/verbs/event_render.rs` | the verbs crate is another milestone's; it takes `waveform::trim` when that merges |

**M4b, as built.** About 1,000 lines of page TypeScript went net (1,930 deleted, 930 of facades and glue written; 1,630 lines of
tests went with them), replaced by 3,900 lines of Rust and 1,140 of Rust tests. The rules live in `song-edit` (`project_paths`, `buffer_text`,
`intake`, `kit`, `clipboard`, `curve_draft`) and `k7x` (`cursor`, `scan`, `tokens`), reached by six op families (`paths.*`, `buffer.*`,
`intake.*` with `kit.*`, `clipboard.*`, `draft.*`, and the buffer's writers under `text.*`) and the facades
`@studio/k7-wasm/{paths,buffer,intake,kit,clipboard,draft}`; `song-edit/docs/buffer.md` holds the choices. Gone from TypeScript: `bufferReads`,
`clipWindowText`, `rackBuilder`, `projectPaths`, `editorPaths`, `localTargets`, `openInEditor`, `sequenceEdit`, `moduleBehind`, `scalarField`,
`curveDraft`, `clipboard`, the editor's `cursor.ts`, the scans of `entityValues`, `previewTriggers` and `sliceSymbols`, and `noteGrid/notes.ts`'s
read and write. What stays, and what moves it:

| module | why it stays |
|---|---|
| `presetOrder.ts` | an order for people is the browser's collator; the module has no locale table |
| `clipFiles.ts`' `fileClip`, `timelineDrop.ts`' `dropReport` and `videoTake.ts`' `pickVideoMime` | the upload round trips, a sentence in the page's words, and `MediaRecorder` |
| `sliceSymbols.ts`' colours and extension, `entityValues.ts` and `previewTriggers.ts`' span mapping | CodeMirror's own edits decide where a span now sits |
| the resolvers of a typed name (`resolveEntry`, `resolveEditorFile`...) in `k7-wasm/src/paths.ts` | scoring is one `fuzzyScores` call per pool; the tie between equal scores is the collator's |
| `norm` | a one-line string replace the path rules' callers loop over |
| `RackBuilder.tsx`' sentences | Rust answers `rb.*` message keys and their parameters |

Two behaviours differ on purpose from the TypeScript they replace, neither reachable from the page's own calls: a clip window written
from a non-finite `from` deletes the key (the old write failed on the printer and left the text), and `@meta/constructor` is no input
(the old lookup found the prototype's own `constructor`). A dropped file's accents are folded by the page before Rust reads the name.
The proofs: the old TypeScript tests (17 files, 196 tests) first ran unchanged against the Rust calls through shims, then became
Rust tests (55 in `song-edit/tests`, 17 in `k7x`, 11 through the ABI in `k7-wasm`). A differential then ran main's frozen twins against the
facades on 2,011,229 generated inputs (1,173,543 of them answering something) over 84 functions: path kinds with the loose dot, the case
fold of an entity file, JavaScript whitespace in `headTokens`, scalar fields with escapes, resolvers over random pools, a clip window
read and written on random grids, slice racks with quoted and astral names, the buffer's reads, drop plans over accented and
slash-ridden names, the rack builder's sentences and kit text, the clipboard, curve drafts with half-typed times, 80,000 random and
27,899 real-file prefixes of the cursor scan, tokens and ▶ lines through random CodeMirror edits, and the note grid's seam; no
difference but the two above. The bed does not drive the rack builder's pad drops, so those rest on the shims and the differential.

**Tests.** A moved function's TS tests become Rust `#[test]`s with the same inputs and expected
values; a large table becomes a JSON vector file the Rust test reads. During the milestone the TS
test runs once against the wasm call (parity), then goes with the TS twin. A typed export carries
a bench showing its call cost; a GENERATE milestone's proof is `tsc` over every caller, with the
generated file checked fresh by `npm run rebuild:generated`. M2c's proof is the four conformance
specs passing on the Rust supervisor.

## Decisions for the user

| # | decision | options | recommendation |
|---|---|---|---|
| 1 | gesture arithmetic on pointer moves | typed numeric wasm export; JSON ops; keep it in TS | a typed export for the hot functions (snap, rollDrag, videoDrag, clip edges, widget drags), JSON for the rest |
| 2 | the crate that replaces dagre (`ui-chains/src/graphLayout.ts`) | **rust-sugiyama**: petgraph-based Sugiyama like dagre, takes node sizes; young, small API, no edge routing points, multi-edges folded first. **layout-rs**: Graphviz-dot style in pure Rust, no dependency; coarser node-size control, one maintainer. **a Sugiyama of our own on petgraph** (600-900 lines): full control of rank snapping, which graphLayout already post-processes heavily; the most work, no dependency. dagre ports to Rust: sparse and unmaintained. elkjs: JS only. | **chosen (2026-11-27): rust-sugiyama**, with petgraph; no golden vectors: the user judged the layouts in the app and kept them (2026-11-27) |
| 3 | where AI organize's loop runs | in wasm with the page holding the request; on a backend route | **chosen (2026-11-27): in Rust, on the backend route** (k7-front `routes/ai`): the key stays on the host and the page keeps one request and the UI |
| 4 | engine warnings carry prose from the C++ engine | codes in the C++ engine; classify the prose in k7-engine | classify in k7-engine now, send codes to the page; codes in C++ later |
| 5 | the page's merge-lock gate (`src/git/gitLock.ts`) | keep it beside the backend's refusal; drop it | **dropped (M3c)**: the backend's refusal and its toast are the one gate, at the cost of one round trip |
| 6 | the home page's commands (`src/projects/homeCommands.ts`) | their own table in TS; a `home` scope of the verbs catalogue | the verbs catalogue, where the editor's commands already are |
| 7 | engine-client and take-render's render scripts (Node, used by k7-runtime's dev scripts) | relocate under `k7-runtime/scripts`; rewrite on the Rust engine client | relocate now, rewrite when a script changes |
| 8 | ts-rs in the backend | one shared types crate; derives in each crate exported by one bindings binary into `web-protocol` | derives in each crate, one binary in k7-front |
| 9 | the TS key detector (`audio-probe/src/key.ts`, `scripts/key-truth.ts`), called by no page | keep; delete | delete, keeping `camelot` (30 lines, BpmFader) until a Rust detector lands in k7-mediatek |
| 10 | fuzzy matching for the command bar (`src/fuzzy.ts`, 24 lines) | `nucleo` (fast, Helix's, larger than the need); `fuzzy-matcher` (skim's, simple, little maintained); a port of the 24 lines | port the 24 lines: no dependency for a subsequence score |
| 11 | the shell's command registry (`src/ui/commands`, 2,018 lines) | stays TS; its sentences and arguments become verbs-catalogue data | stays TS: its handlers act on React state |
| 12 | per-frame drawing maths (hang sway, camera glide, needle clock, playhead glide, meters) | stays TS; Rust precomputed tables | stays TS (the choice above); Rust emits a schedule only where a document drives it |

## Quick wins

- **M1a** deletes about 1,100 lines with nothing to port (add 466 with decision 9), and removes a
  dead WebRTC route from the backend.
- **M1b** (done) generates `graph-model/types.ts` (430 lines) from structs that already derive ts-rs: the types are
  `graph-model/generated/`, written by k7-front's `bindings` binary (`--features ts`) from `song`'s `web.rs` and its panel
  and curve types, run by `npm run rebuild:generated` and stamped for `lint:generated` (`apps/k7-daw-backend/docs/ts-types.md`).
  Choices: the folder lands in `graph-model`, not `web-protocol`, so drawing a graph drags nothing in, and the next
  package takes one line in the binary; the page's additions to a panel (`arg`, a tab's `cols`, `tabsAs`) are `Page*`
  types in `types.ts`; the app's mirrors (AuthUser, K7Prefs and the rest of the row) are not done and follow with M5c.
- **M5c** (done) generates `web-protocol/generated/` (63 types) from `k7-file-service/src/wire/` (workspace, machine, library
  structs), `k7-front`'s `cameras/wire.rs` and song's `Hints`, `IoPort` and `CatalogueModule`; `workspaceTypes.ts` keeps the upload
  limit and two aliases, `machineTypes.ts` is gone, `ChangedFile` is k7-wasm's. The address grammar is `song-schema/src/address.rs`
  behind the `address.*` ops (`@studio/k7-wasm/address`); `projects.ts` (355 to about 130 lines) keeps the origin join and the
  signatures callers use. Choices: the routes write JSON by hand, so the wire structs are checked by `tests/all/wire.rs` rather than
  serialised; `RepoProject.surfaces` and `active` are required, as the route always sends them.
- **M1c** replaced the TS twins that had a Rust function by calls to it (`k7-wasm/docs/engine.md`); the rest had no Rust twin to call.
- `pulledSentence` is written twice (`src/git/GitBar.tsx`, `src/shell/HeaderGit.tsx`): one copy.
- knip's unused exports in the zone (`setListenHere`, `knobWidgets`, `filterWorkspaces`,
  `slotDetail`, `scanStep`, `HomeTabs`, `GitBarOf`, `SOURCE_URL`, `BASE`) go with M1a.

## The inventory

Paths are relative to `apps/k7-daw-frontend`, a package's without its `packages/` prefix. Every
MOVE, GENERATE and DELETE row is one file; a split file reads `STAY n / MOVE m`; whole-STAY files
are grouped by folder and extension, with the reasons they stay.

### The app's modules (`src/*.ts` and its small folders)

| module | lines | verdict | target | note |
|---|---:|---|---|---|
| src/activity.ts | 80 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc | buckets per clip, computed per doc not per frame |
| src/address.ts | 69 | STAY |  | URL/router scope |
| src/androidBusy.ts | 69 | STAY |  | phone bridge + hook |
| src/api.ts | 446 | GENERATE 10 / STAY 436 | GENERATE AuthUser/K7Prefs from k7-front | route client; downloadName small |
| src/appNav.ts | 151 | STAY |  | history API navigation |
| src/appShell.ts | 31 | STAY |  | host detection |
| src/argOptions.ts | 68 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | ranking of completions; per keystroke, fine |
| src/barTint.ts | 10 | DELETE |  | imported only by its test |
| src/barWindow.ts | 90 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | zoom arithmetic; per wheel event |
| src/bufferReads.ts | 45 | MOVE | song-edit crate (text edits on the buffer) | one k7x parse of the buffer; per keystroke (debounced) |
| src/chainsLabels.ts | 89 | STAY |  | i18n words |
| src/chunkReload.ts | 33 | STAY |  | browser reload |
| src/clamp.ts | 4 | STAY |  | 4-line helper |
| src/clipEnvelope.ts | 39 | MOVE | k7-file-service media (wfSig exists) | mirror of Rust wfSig: expose it |
| src/clipFiles.ts | 100 | MOVE 60 / STAY 40 | song-edit crate (text edits on the buffer) | declaredSource/candidateIds; upload stays |
| src/clipInstances.ts | 47 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc |  |
| src/clipLoop.ts | 67 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc | shared by rackLit/takes/roll |
| src/clipNameWords.ts | 23 | STAY |  | i18n words |
| src/clipNames.ts | 69 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc | id grammar written by song-edit record_take; Rust returns parts, TS words |
| src/clipOps.ts | 23 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | line spelling |
| src/clipPlayhead.ts | 23 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per animation frame: keep TS unless typed export |
| src/clipWarp.ts | 73 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) / verbs line | per pointer move |
| src/clipWindowText.ts | 104 | MOVE | song-edit crate (text edits on the buffer) | scan+rewrite buffer; per drag move |
| src/clipboard.ts | 202 | MOVE | song-edit (verbs selection copy/paste) | offset arithmetic |
| src/commandEntityNames.ts | 30 | STAY |  | i18n words |
| src/commandFill.ts | 153 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | form slots of a line |
| src/commandPrefs.ts | 210 | MOVE 150 / STAY 60 | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) (prefs over catalogue) | labels/order rules; hook stays |
| src/contextMenu.ts | 645 | MOVE 450 / STAY 195 | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) (menu: derivation) | rows derived from catalogue; Rust answers keys, TS words |
| src/curveDraft.ts | 360 | MOVE 280 / STAY 80 | k7-wasm curve ops (preview already Rust) | draft->fields; pointer placement |
| src/disabledReasonToast.ts | 9 | STAY |  | wiring |
| src/docs/examples.ts | 41 | MOVE | song-schema Rust test (after twins) | test helper only |
| src/downloadBlob.ts | 24 | STAY |  | browser download |
| src/downloadSignal.ts | 53 | STAY |  | cookie poll |
| src/editorPaths.ts | 67 | MOVE | song-schema crate (path/file conventions) | gutter source resolution = compiler's rule |
| src/emptyBar.ts | 47 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) |  |
| src/engineMissing.ts | 66 | MOVE | k7-engine (backend) classifies warnings, sends codes | parses C++ prose |
| src/engineSession.ts | 19 | MOVE | k7-file-service sessionOf | twin of Rust sessionOf |
| src/engineWarnings.ts | 154 | MOVE 90 / STAY 64 | k7-engine warning codes | sentence/toast stays |
| src/exportArgs.ts | 138 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) (parse project export) | line grammar |
| src/exportHost.ts | 167 | STAY |  | host bridges |
| src/fileDrop.ts | 59 | STAY |  | DataTransfer |
| src/floatingRect.ts | 75 | STAY |  | window geometry + localStorage |
| src/formatAt.ts | 10 | MOVE | k7-wasm timecode_format (exists) | per frame clock: typed export or keep |
| src/freezeWatchdog.ts | 246 | STAY |  | browser perf probes |
| src/fuzzy.ts | 24 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | no Rust fuzzy yet; per keystroke over a list |
| src/gainTrim.ts | 20 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc (master meter fix) |  |
| src/graphNotes.ts | 25 | MOVE | song graph read |  |
| src/graphWarnings.ts | 67 | STAY |  | i18n of Rust warning codes |
| src/gridCaret.ts | 120 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per key/pointer |
| src/headphonesHint.ts | 29 | MOVE 10 / STAY 19 | song graph read | localStorage stays |
| src/historyMarks.ts | 74 | MOVE | k7-git/k7-sync history answer | paths->marks |
| src/historySource.ts | 19 | STAY |  | view offers |
| src/holdDrag.ts | 155 | STAY |  | pointer events |
| src/i18n.ts | 10 | STAY |  |  |
| src/keys.ts | 286 | STAY |  | key bindings table; UI host's |
| src/laneInput.ts | 24 | MOVE | song graph read (runCarriesInput exists server-side) | twin |
| src/latestAnswer.ts | 17 | STAY |  | async ordering |
| src/lcdSounds.ts | 144 | MOVE 70 / STAY 74 | waveform crate decoders (WASM) | decodeWav twin; fetch/cache stays |
| src/leaveAsk.ts | 20 | STAY |  |  |
| src/libraryTree.ts | 240 | MOVE 180 / STAY 60 | k7-libraries route answer | tree shaping |
| src/listenHere.ts | 32 | STAY |  | per-browser pref; setListenHere unused export |
| src/localApp.ts | 34 | GENERATE 6 / STAY 28 | GENERATE LocalApp |  |
| src/localCommands.ts | 42 | STAY |  | shell gate |
| src/localEngine.ts | 34 | STAY |  | context |
| src/localTargets.ts | 67 | MOVE | song-schema crate (path/file conventions) | name->path |
| src/logProblems.ts | 16 | STAY |  | log filter |
| src/longPress.ts | 171 | STAY |  | pointer events |
| src/lucideIcons.ts | 62 | STAY |  | icons |
| src/mastering/bindings.ts | 97 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc views (viewBindings) | join with ui-chains/bindings |
| src/mastering/controls.ts | 203 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc views (vocabulary) | mostly a table |
| src/mediaSeconds.ts | 55 | STAY |  | browser media metadata |
| src/menuConfirm.ts | 19 | STAY |  |  |
| src/menuTabs.ts | 57 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | with contextMenu |
| src/menuTitle.ts | 62 | MOVE 40 / STAY 22 | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) + song names |  |
| src/midiAccess.ts | 69 | STAY |  | Web MIDI |
| src/midiActivity.ts | 44 | STAY |  | Web MIDI LED |
| src/midiControl.ts | 177 | STAY |  | Web MIDI CC; map is song's |
| src/midiIn.ts | 52 | MOVE | song graph read |  |
| src/midiMessage.ts | 10 | STAY |  |  |
| src/mixerStrips.ts | 18 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc |  |
| src/modelDownload.ts | 60 | STAY |  | job poll |
| src/moduleBehind.ts | 27 | MOVE | song-schema crate (path/file conventions) (extends resolution) |  |
| src/noSwipeBack.ts | 56 | STAY |  | wheel events |
| src/nodeInsert.ts | 89 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | line spelling |
| src/nodeMenu.ts | 63 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) + song alternatives |  |
| src/noteNames.ts | 18 | STAY |  | labels; per row render |
| src/noteSpans.ts | 59 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc | once per doc; lookup per frame stays |
| src/openInEditor.ts | 23 | MOVE | song-schema crate (path/file conventions) |  |
| src/pairing/PairThisDevice.tsx | 101 | DELETE |  | unused (knip, no importer) |
| src/palette.ts | 178 | MOVE 100 / STAY 78 | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) (VerbComplete) | vocabulary per arg |
| src/paneVisible.ts | 11 | STAY |  |  |
| src/pendingEdits.ts | 152 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) (predict a line's effect) | uses song-schema timecode |
| src/pendingNode.ts | 106 | MOVE 60 / STAY 46 | ui-chains layout (with graph-layout move) | box placement |
| src/placeResource.ts | 48 | STAY |  | menu rows async |
| src/playlistVocabulary.ts | 40 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) compl. providers |  |
| src/plugins.ts | 32 | STAY |  | registry |
| src/poll.ts | 55 | STAY |  | timers |
| src/popins.ts | 62 | STAY |  |  |
| src/presetOrder.ts | 16 | MOVE | song-edit node_presets | list order rule |
| src/processes.ts | 100 | GENERATE 3 / STAY 97 | GENERATE ServiceProcess | store |
| src/projectPaths.ts | 104 | MOVE | song-schema crate (path/file conventions) | path kinds; twin of compiler conventions |
| src/rackBuilder.ts | 133 | MOVE | song-edit (kit node) |  |
| src/rackFor.ts | 48 | MOVE | song graph read |  |
| src/rackLit.ts | 206 | MOVE 140 / STAY 66 | song crate (k7-daw-wasm/song), a read on the compiled doc | hits per doc; per-frame lookup+hook stays |
| src/radioKeys.ts | 36 | STAY |  | keyboard |
| src/rankCommands.ts | 48 | MOVE | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | per keystroke |
| src/readFailure.ts | 16 | STAY |  |  |
| src/recentCommands.ts | 38 | STAY |  | localStorage |
| src/reloadScope.ts | 63 | MOVE | verbs run answer (files->routes) |  |
| src/ringBuffer.ts | 42 | STAY |  | per-frame meter buffer |
| src/rollDrag.ts | 148 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per pointer move |
| src/rollFit.ts | 104 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) |  |
| src/rollMode.ts | 37 | STAY |  | UI mode |
| src/rollPending.ts | 70 | STAY |  | optimistic paint |
| src/routes.ts | 117 | STAY |  | URL<->screen |
| src/rows.ts | 231 | MOVE 150 / STAY 81 | song crate (k7-daw-wasm/song), a read on the compiled doc (lane rows) | HATCH/style stays |
| src/saveLabel.ts | 30 | STAY |  |  |
| src/scalarField.ts | 20 | MOVE | k7x crate scan |  |
| src/scopeLeave.ts | 15 | STAY |  |  |
| src/sequenceEdit.ts | 18 | MOVE | song-schema crate (path/file conventions) |  |
| src/settings/RemoteSlotsSection.tsx | 86 | DELETE |  | unused (knip, no importer) |
| src/settings/desktopEngine.ts | 206 | GENERATE 71 / STAY 135 | GENERATE snapshot from k7-supervisor | SSE client |
| src/shellEquality.ts | 65 | STAY |  | React bail-outs |
| src/sliceSymbols.ts | 140 | MOVE 80 / STAY 60 | song-edit crate (text edits on the buffer) | symbol scan; CodeMirror extension stays |
| src/snap.ts | 65 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per pointer move; hottest |
| src/sounding.ts | 16 | STAY |  | meters, per frame |
| src/statusLine.ts | 43 | STAY |  | store |
| src/stepSentence.ts | 68 | MOVE 40 / STAY 28 | verbs crate (k7-daw-wasm/verbs: catalogue/compl.) | words stay |
| src/surfacePanes.ts | 100 | STAY |  | React registry |
| src/surfaces/lanApi.ts | 80 | GENERATE 30 / STAY 50 | GENERATE LanPeer/LanHello from k7-lan | route client |
| src/surfaces/paneInUse.ts | 61 | MOVE | k7-lan presence answer | small rules |
| src/surfaces/surfaceRows.ts | 134 | MOVE | k7-lan route answer | filter order |
| src/syncHistory.ts | 110 | GENERATE 23 / STAY 87 | GENERATE from k7-sync | route client |
| src/takeCounter.ts | 43 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per frame while recording |
| src/takeSpool.ts | 202 | STAY |  | OPFS/MediaRecorder |
| src/takes.ts | 265 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc (comp/takes) |  |
| src/templateGraph.ts | 46 | MOVE | song chains graph |  |
| src/textScale.ts | 30 | STAY |  | iOS text size |
| src/timelineDrop.ts | 164 | MOVE 120 / STAY 44 | song-edit crate (text edits on the buffer) (drop naming) |  |
| src/timelineMode.ts | 35 | STAY |  | UI mode |
| src/timelineTools.ts | 142 | MOVE 80 / STAY 62 | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) / verbs line | tool->verb |
| src/timelineView.ts | 33 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc |  |
| src/trackPanelLayout.ts | 25 | MOVE | graph layout (with ui-chains) |  |
| src/transportPanic.ts | 46 | STAY |  | timing of a 2nd stop |
| src/types.ts | 10 | STAY |  | re-exports |
| src/useIsMobile.ts | 60 | STAY |  |  |
| src/useViewRoute.ts | 88 | STAY |  |  |
| src/version.ts | 4 | STAY |  |  |
| src/videoDrag.ts | 112 | MOVE | gesture crate (new, k7-daw-wasm/gesture; typed export, no JSON) | per pointer move |
| src/videoSpans.ts | 35 | MOVE | song crate (k7-daw-wasm/song), a read on the compiled doc pictures (activeAt) | twin of compositor rule |
| src/videoTake.ts | 54 | MOVE 24 / STAY 30 | song-edit (take naming) | MediaRecorder mime; naming MOVE song-edit |
| src/visiblePoll.ts | 43 | STAY |  |  |
| src/waveMode.ts | 34 | STAY |  | UI mode |
| src/waveformsWarmed.ts | 29 | STAY |  |  |
| src/webmidi.ts | 201 | STAY |  | Web MIDI capture |
| src/about/*.tsx (3 files) | 252 | STAY |  | React view |
| src/about/noticeFilter.ts | 30 | STAY |  | search filter |
| src/camera/CameraMode.tsx | 181 | STAY |  | React view |
| src/camera/*.ts (2 files) | 245 | STAY |  | browser WebRTC |
| src/docs/mountSite.tsx | 15 | STAY |  | React view |
| src/docs/pages.ts | 54 | STAY |  | bundled docs |
| src/docs/site/*.tsx (2 files) | 265 | STAY |  | React view |
| src/docs/site/pages.ts | 116 | STAY |  | docs site routes; buildSite.mjs |
| src/mastering/latestWriter.ts | 27 | STAY |  | async |
| src/pairing/PairRequestsPrompt.tsx | 68 | STAY |  | React view |
| src/pairing/usePairing.ts | 47 | STAY |  | hook |
| src/queries/*.ts (3 files) | 189 | STAY |  | TanStack |
| src/settings/*.tsx (13 files) | 1575 | STAY |  | React view |
| src/settings/*.ts (6 files) | 362 | STAY |  | phone bridge; route client; interfaces mirror k7-front answers (GENERATE 4); route client |
| src/state/ShellContexts.tsx | 23 | STAY |  | React view |
| src/state/*.ts (4 files) | 444 | STAY |  | UI state; line spellings per gesture |
| src/surfaces/*.tsx (2 files) | 230 | STAY |  | React view |
| src/surfaces/*.ts (2 files) | 44 | STAY |  | timer |
| src/ui/commands/*.ts (12 files) | 2018 | STAY |  | shell's own commands, handlers on React state |

### The app's views, hooks, git, projects and shell

| module | lines | verdict | target | note |
|---|---:|---|---|---|
| src/App.tsx | 3768 | STAY 3688 / MOVE 80 | song (graph reads: midi-in lanes, device present) | orchestration of panes and command runs; 3768 lines is over the 500 rule, split by concern when touched |
| src/components/AssetPicker.tsx | 151 | STAY 131 / MOVE 20 | song asset tree + search | buildAssetTree/searchAssets |
| src/components/AudioClipPanel.tsx | 943 | STAY 903 / MOVE 40 | song-edit clip window (zoomTo, window maths) | waveform canvas and recorder view |
| src/components/Breadcrumb.tsx | 198 | STAY 178 / MOVE 20 | k7-file-service paths (childrenOf, dirPrefixes, underRoot) | low priority |
| src/components/ChainsTab.tsx | 838 | STAY 778 / MOVE 60 | ui-chains graph shaping (Rust layout crate) | laneNodes/laid/laidShown shape the graph before dagre; moves with the layout decision |
| src/components/ClipMiniRoll.tsx | 99 | STAY 89 / MOVE 10 | song-edit notes (placedNotes) |  |
| src/components/ClipView.tsx | 464 | STAY 434 / MOVE 30 | verbs (noteVerbs: note command lines) | clip content host |
| src/components/CommandBar.tsx | 580 | STAY 560 / MOVE 20 | verbs line parse (splitArgs) | palette UI |
| src/components/EditorTab.tsx | 1529 | STAY 1499 / MOVE 30 | song (placementsIn) | CodeMirror host; file-op callbacks call the file service |
| src/components/EngineSyncProgress.tsx | 45 | DELETE |  | the component has no importer (knip); syncRunning (3 lines) STAYs or goes beside its callers |
| src/components/FileTree.tsx | 737 | STAY 722 / MOVE 15 | k7-file-service paths (isShared, foldModules) | @studio/ui file-tree host |
| src/components/GraphPanel.tsx | 350 | STAY 320 / MOVE 30 | ui-chains graph shaping | same pipeline as ChainsTab (subgraph -> buildGraph -> dagre) |
| src/components/IoPanel.tsx | 420 | STAY 390 / MOVE 30 | song io (portDetail, portName, sameChannels) | meters poll |
| src/components/MidiTakeRoll.tsx | 208 | STAY 185 / MOVE 23 | song-edit notes (keyRange, beatWindow) |  |
| src/components/ModuleKnobs.tsx | 92 | STAY 78 / MOVE 14 | panel-rules Rust (knobWidgets) | knobWidgets export unused outside (knip) |
| src/components/ModulePicker.tsx | 334 | STAY 274 / MOVE 60 | song module catalogue: tree + pathScore search op | catalogue tree build and scored search |
| src/components/PianoRoll.tsx | 899 | STAY 819 / MOVE 80 | song-edit roll geometry (with rollDrag.ts/rollFit.ts) | note placement arithmetic left in the view; pointermove hot path |
| src/components/PresetPicker.tsx | 215 | STAY 200 / MOVE 15 | song preset tree + search | presetTree/searchPresets |
| src/components/Ruler.tsx | 379 | STAY 334 / MOVE 45 | song timeline ruler ladder (tickAt, tickBeats, rulerTicks, rulerScale) | oracle: src/rulerTicks.test.ts; recomputed on zoom/scroll, not per frame |
| src/components/TimelineTab.tsx | 3997 | STAY 3547 / MOVE 450 | song-edit timeline gesture ops via k7-wasm | inline gesture arithmetic: clip move offset, crossfade length, caret clamp/step/nudge, contentBeats, zoom ... |
| src/components/engineTaken.ts | 34 | MOVE | k7-engine claim policy (holdsClaim) | takenBanner wording STAYs |
| src/git/commitState.ts | 40 | MOVE | k7-git commit.rs too_large exists: the route answers commitView | formatSize/identityReady STAY (15) |
| src/git/connectState.ts | 87 | MOVE | k7-git remote (sshHost, isWebAddress; vet() exists) | form reducer STAYs (62) |
| src/git/forgeState.ts | 71 | GENERATE | k7-front git_forge* answers + ts-rs | MOVE 0; canPickFrom/isSignout small STAY (31) |
| src/git/forgejoState.ts | 98 | MOVE | k7-front git_forgejo (shortVersion, sshPort) | screen reducers STAY (83) |
| src/git/gitLock.ts | 89 | MOVE | k7-front git_lock: the backend refuses writes while merging and answers a code | afterRead fail-closed state STAYs (49) |
| src/git/gitPageTabs.ts | 24 | MOVE | k7-git (nameFromUrl, with cloneName) | firstTab STAYs |
| src/git/gitState.ts | 136 | MOVE | k7-front git state route answers the offers (barView, menuView) | menu entry wording STAYs (46) |
| src/git/gitTypes.ts | 75 | GENERATE | k7-front git routes: serde structs + ts-rs | the routes answer json! literals today: structs come first |
| src/git/githubState.ts | 120 | GENERATE | k7-front git_github: ts-rs types (25) + MOVE repo order/filter/date rules (40) | device-flow screen states STAY (55) |
| src/git/mergeState.ts | 90 | MOVE | k7-git merge (offered, canFinish) | choices reducer STAYs (70) |
| src/git/roamState.ts | 70 | GENERATE | k7-front git_roam: ts-rs (15) + MOVE offered/cloneName (25) | reducer STAYs (30) |
| src/git/saveAct.ts | 203 | MOVE | k7-front git_autosave (saveMessage) | toasts and the push/pull sequence STAY (188) |
| src/git/verbWrites.ts | 245 | MOVE | verbs crate catalogue: a write/read/device class per verb | coordinate with the verbs owner (translator branch); 245-line table |
| src/hooks/useEdgeDrags.ts | 174 | STAY 134 / MOVE 40 | song-edit clip_resize / event fade length | pointermove hot path; clip_resize.rs exists in song-edit |
| src/hooks/useInstrumentStatus.ts | 97 | STAY 87 / MOVE 10 | song graph issues (instrumentNeeds/Warnings) | engine ask STAYs |
| src/hooks/useMidiControl.ts | 247 | STAY 193 / MOVE 54 | song graph controller map (mappedParams, droppedBindings) | Web MIDI wiring STAYs |
| src/hooks/useMidiRecording.ts | 285 | STAY 245 / MOVE 40 | song-edit notes capture (note on/off pairing, inTake) | Web MIDI capture and beatNow STAY; called per MIDI event, cheap |
| src/hooks/useProjectDoc.ts | 136 | STAY 106 / MOVE 30 | song (track order, mixer state off the doc) |  |
| src/hooks/useRackTrigger.ts | 79 | STAY 69 / MOVE 10 | song rack (rackPreviewReq, laneOfPlugin) |  |
| src/hooks/useRollGestures.ts | 484 | STAY 364 / MOVE 120 | song-edit roll gesture ops (beside rollDrag.ts) | pointermove hot path: scalar-in, scalar-out ops |
| src/hooks/useTrackControls.ts | 159 | STAY 134 / MOVE 25 | song mixer (effective mute/solo) | same rule as TimelineTab's anySolo/silent: one Rust function |
| src/hooks/useVideoDrag.ts | 179 | STAY 149 / MOVE 30 | song-edit picture drag (beside videoDrag.ts) | pointermove hot path |
| src/modeIcons.tsx | 195 | DELETE |  | ModeStrip and stripButton have no importer (knip); the icons STAY |
| src/projects/RepoList.tsx | 359 | STAY 314 / MOVE 45 | k7-file-service workspace order/filter (pageOrder, filterWorkspaces) | low priority |
| src/projects/homeCommands.ts | 607 | MOVE | verbs crate catalogue (home scope) + line parse/resolve | acts calling the file service STAY (207); after the translator branch merges |
| src/projects/menuAccount.ts | 10 | DELETE |  | unused file (knip) |
| src/projects/projectKind.ts | 42 | MOVE | song project shapes (shapeOfSurfaces) | editedAgo STAYs (Intl) |
| src/projects/projectName.ts | 56 | MOVE | k7-file-service names (workspaces_names tests exist) | the suffix decides the app: one rule with the backend's |
| src/projects/workspaceTasks.ts | 49 | STAY 39 / MOVE 10 | song-schema tasks (anchorProject) | TODOS.md parse is song-schema's (out of scope) |
| src/shell/tapTempo.ts | 22 | MOVE | song tempo (tap_tempo) | oracle: shell/tapTempo.test.ts |
| src/*.tsx (9 files) | 836 | STAY |  | React rendering |
| src/components/*.tsx (94 files) | 15714 | STAY |  | React rendering |
| src/components/timeline/*.tsx (12 files) | 816 | STAY |  | React rendering |
| src/components/timeline/*.ts (3 files) | 218 | STAY |  | browser: hidden <video> seeks, ImageBitmap; per-frame compositor animation; a wasm call per frame buys nothing; CSS ... |
| src/git/*.tsx (24 files) | 3277 | STAY |  | React rendering; pulledSentence duplicated in HeaderGit.tsx |
| src/git/*.ts (17 files) | 871 | STAY |  | UI state / browser glue; React state/effect hook; list paging state |
| src/hooks/*.ts (40 files) | 4086 | STAY |  | React state/effect hook; browser WebSocket client wiring; requestAnimationFrame, 60 Hz: stays TS |
| src/hooks/useNodePresets.tsx | 135 | STAY |  | React rendering |
| src/projects/*.tsx (30 files) | 4382 | STAY |  | React rendering |
| src/projects/*.ts (6 files) | 589 | STAY |  | HTTP transport and route names; types are web-protocol's (zone D); UI state / browser glue; React state/effect hook |
| src/shell/*.tsx (10 files) | 2203 | STAY |  | React rendering; pulledSentence duplicates GitBar.tsx's: dedupe |
| src/shell/*.ts (2 files) | 272 | STAY |  | React state/effect hook; UI state / browser glue |

### The chains, panels and plugins packages

| module | lines | verdict | target | note |
|---|---:|---|---|---|
| k7-plugins/scripts/turntables/put-on.ts | 238 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/beats.ts | 83 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/controls.ts | 174 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/deck.ts | 363 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/deckVocabulary.ts | 111 | GENERATED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/grid.ts | 29 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/loop.ts | 145 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/mapping.ts | 42 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/metronome.ts | 107 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/mixBindings.ts | 178 | MOVED 90 / STAY 88 |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/mixView.ts | 67 | STAY |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/setList.ts | 108 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/useDeckLoops.ts | 201 | MOVED |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/turntables/useDeckSet.ts | 1004 | STAY |  | built in M5a: see "M5a, as built" |
| k7-plugins/src/video/viewportRect.ts | 162 | MOVE | song pictures (k7-wasm op) | grip arithmetic writes the rect; hit-test per pointer stays; OVERLAP worktree-agent-a073bfbdd607a7ec8 touches ... |
| panel-rules/src/aiAnswer.ts | 173 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | answer parse + JSON schema |
| panel-rules/src/aiLoop.ts | 93 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | loop runs on the host with the key |
| panel-rules/src/aiOrganize.ts | 444 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | request wire + answer check |
| panel-rules/src/aiPrompt.ts | 150 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | prompt text as Rust const |
| panel-rules/src/aiRequest.ts | 36 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | zod -> serde |
| panel-rules/src/aiTurnStream.ts | 57 | MOVE | backend k7-front routes/ai (whole loop server-side); checks in song crate modules/ui ... | NDJSON turn reader: vanishes once the loop is server-side |
| panel-rules/src/groupGeometry.ts | 56 | MOVE | song crate modules/ui (panel rules) via k7-wasm op |  |
| panel-rules/src/layoutTwins.ts | 199 | MOVE | song crate modules/ui (panel rules) via k7-wasm op |  |
| panel-rules/src/panelText.ts | 21 | MOVE | song crate modules/ui (panel rules) via k7-wasm op | needs Rust k7x printer (rust-song-twins) |
| ui-chains-react/src/chainsModel.ts | 76 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | memo per graph; part of chains_build output |
| ui-chains-react/src/moduleField.ts | 63 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | node field -> option index |
| ui-chains-react/src/useEdgeGestures.ts | 304 | STAY 250 / MOVE 54 |  | pointer hook; what a drop lands as (54) MOVE to chains |
| ui-chains-react/src/useWidgetDrag.ts | 363 | STAY 300 / MOVE 63 |  | drag hook; override reconcile rule (63) MOVE |
| ui-chains-react/src/widgetTarget.ts | 203 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | what a widget actuates and which value wins; read per render: one call per graph, not per widget |
| ui-chains/src/anchorRight.ts | 39 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/assembly.ts | 251 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/automationDepth.ts | 55 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | mirrors EditBuilder.cpp rule |
| ui-chains/src/bindings.ts | 173 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | shared by desk mixBindings and src/hooks/useMidiControl |
| ui-chains/src/buildChains.ts | 333 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/buildContext.ts | 282 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/buildGraph.ts | 267 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | runs on doc change (useMemo), not per frame |
| ui-chains/src/cables.ts | 372 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | computeFlow/insertFor/autoTarget/jackRef move; cubic/smoothPath SVG stay |
| ui-chains/src/curvePlacement.ts | 90 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | layout post-pass |
| ui-chains/src/flow.ts | 112 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | runs per state frame (20 Hz): batch all cables in one call, numeric buffer |
| ui-chains/src/geometry.ts | 420 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build); constants exported to TS ... | renderer reads the sizes: constants generated, not copied |
| ui-chains/src/graph.bench.ts | 40 | MOVE | criterion bench in chains crate |  |
| ui-chains/src/graphLayout.ts | 440 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) + graph-layout crate | dagre: whole file depends on it; blocked by layout-crate decision |
| ui-chains/src/graphNode.ts | 279 | GENERATE | ts-rs from chains crate node/edge structs | types + carryStep (29 lines MOVE) |
| ui-chains/src/gridFit.ts | 203 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/groupEdge.ts | 40 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/modulePreview.ts | 122 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/moduleRow.ts | 322 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | dagre unused here |
| ui-chains/src/multiSelect.ts | 70 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | linked travel read on pointer move: scalar |
| ui-chains/src/nodeUIs.ts | 432 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | built-in card panels |
| ui-chains/src/patchGesture.ts | 18 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/rowSeams.ts | 152 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/shownName.ts | 40 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) |  |
| ui-chains/src/stackLayout.ts | 197 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | no dagre |
| ui-chains/src/subgraph.ts | 90 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | pure WebGraph read; song graph views may cover it |
| ui-chains/src/widgetValue.ts | 74 | MOVE | new crate k7-daw-wasm/chains (k7-wasm op chains_build) | called per pointer move during a drag: scalar call, no JSON |
| ui-module-builder/scripts/checkLayouts.ts | 158 | MOVE | Rust test/bin over k7-core with layoutTwins |  |
| ui-module-builder/src/aiOrganize.ts | 5 | DELETE |  | re-export shim; one importer (src/api.ts) -> import @studio/panel-rules |
| ui-module-builder/src/autoLayout.ts | 296 | MOVE | song crate modules/ui (panel rules) via k7-wasm op | panel proposed from params |
| ui-module-builder/src/claudeCodePrompt.ts | 67 | MOVE | backend k7-front routes/ai (whole loop server-side) | prompt text |
| ui-module-builder/src/gridCells.ts | 59 | MOVE | song crate modules/ui (panel rules) via k7-wasm op | cell arithmetic, per pointer move: scalar |
| ui-module-builder/src/moduleUi.ts | 318 | MOVE | song crate modules/ui (panel rules) via k7-wasm op + k7x crate | k7x read/write/extends: overlaps rust-song-twins |
| ui-module-builder/src/panelTabs.ts | 110 | MOVE | song crate modules/ui (panel rules) via k7-wasm op |  |
| ui-module-builder/src/panelWrite.ts | 107 | MOVE | k7x crate edit (rust-song-twins) | minimal k7x edits |
| ui-module-builder/src/useAiOrganize.ts | 177 | STAY 120 / MOVE 57 |  | hook; re-check (57) goes with the AI move |
| ui-module-builder/src/useModuleFiles.ts | 389 | STAY 300 / MOVE 89 |  | buffers hook; dirty/extends rules (89) MOVE |
| ui-module-builder/src/useParamRows.ts | 230 | MOVE | song crate modules/ui (panel rules) via k7-wasm op | param-source union; hook shell stays |
| ui-module-builder/src/widgetClipboard.ts | 83 | MOVE | song crate modules/ui (panel rules) via k7-wasm op |  |
| ui-module-builder/src/widgetKinds.ts | 79 | MOVE | song crate modules/ui (panel rules) via k7-wasm op | retype rules move; palette entries stay |
| k7-plugins/src/*.tsx | 41 | STAY |  | 2 React components; small inline helpers left |
| k7-plugins/src/*.ts (2 files) | 21 | STAY |  | composition / catalogue |
| k7-plugins/src/turntables/*.tsx | 64 | STAY |  | 1 React components; small inline helpers left |
| k7-plugins/src/turntables/*.ts (11 files) | 657 | STAY |  | what a deck draws at its height: drawing; per-frame drawing maths; see per-frame decision; per-frame drawing maths; see ... |
| k7-plugins/src/turntables/views/*.tsx | 3173 | STAY |  | 13 React components; small inline helpers left |
| k7-plugins/src/turntables/views/*.ts (2 files) | 66 | STAY |  | styles; visibility |
| k7-plugins/src/video/*.tsx | 647 | STAY |  | 5 React components; small inline helpers left |
| k7-plugins/src/video/camera/*.ts (4 files) | 368 | STAY |  | WebRTC browser API (remote cameras via backend routes/cameras); live |
| k7-plugins/src/video/*.ts (4 files) | 161 | STAY |  | per-frame smoothing for shaders; OVERLAP worktree-agent-a073bfbdd607a7ec8 touches src/video; page channel; OVERLAP ... |
| ui-chains-react/src/*.tsx | 4175 | STAY |  | 17 React components; small inline helpers left |
| ui-chains-react/src/*.ts (19 files) | 3308 | STAY |  | React hooks / CSS / UI stores / drawing; camera glide per frame; per-frame drawing maths; see per-frame decision |
| ui-chains-react/src/widgets/*.tsx | 1125 | STAY |  | 7 React components; small inline helpers left |
| ui-chains/src/*.ts (8 files) | 1362 | STAY |  | iframe sandbox + postMessage; runs user JS in sandbox worker, OffscreenCanvas; Canvas2D drawing, per frame |
| ui-module-builder/src/*.tsx | 3117 | STAY |  | 13 React components; small inline helpers left |
| ui-module-builder/src/*.ts (4 files) | 308 | STAY |  | UI state / DataTransfer / store interface / probe calls |

### The UI kit, the protocols, the engine clients and the tooling

| module | lines | verdict | target | note |
|---|---:|---|---|---|
| audio-probe/scripts/key-truth.ts | 266 | DELETE |  | dev sweep of the TS key detector, which no page calls |
| audio-probe/src/key.ts | 200 | DELETE |  | chromagram/keyOf used only by scripts/key-truth.ts; camelot+PITCH_CLASSES (~30, BpmFader) MOVE to k7-mediatek ... |
| audio-probe/src/trimGate.ts | 49 | MOVE | waveform crate measure.rs (trim window over the envelope) via k7-wasm or sidecar | AudioClipPanel; once per clip, cheap |
| audio-probe/src/waveform.ts | 244 | DELETE |  | regionWaveform/beatGrid/detectTransients/warpStretch twin of k7-file-service media/region.rs, no runtime ... |
| engine-browser/src/choose.ts | 104 | MOVE | k7-engine registry: which engine (policy) as part of the list reply | pure policy, no React |
| engine-browser/src/probe.ts | 372 | STAY 357 / MOVE 15 |  | browser measurement over WebSocket; rtc candidate (~15) DELETE |
| engine-browser/src/rtcSocket.ts | 126 | DELETE |  | WebRTC dead: Rust supervisor refuses rtc-offer; keep ~25-line openSocket WebSocket wrapper |
| engine-protocol/build.ts | 170 | MOVE | song crate: ProjectDoc/TrackDoc Default + k7-wasm op `project_doc_empty` | used by page (metronome, ClipView, deck, useProjectDoc); after project.ts GENERATE |
| engine-protocol/project.ts | 237 | GENERATE | ts-rs on k7-daw-wasm/song/src/doc/types.rs (ProjectDoc, TrackDoc: serde, no TS derive yet) | C++ mirror ProjectDoc.h stays; song crate is rust-song-twins territory: coordinate |
| engine-protocol/types.ts | 109 | GENERATE | ts-rs on engine state frames; no Rust struct yet except k7-supervisor wire.rs subset | engine is C++: Rust structs must be written first (k7-engine already parses some frames) |
| engine-tunnel/agent.ts | 456 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/agentFiles.ts | 195 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/backpressure.ts | 139 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/channel.ts | 90 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/drain.ts | 16 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/frames.ts | 194 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/jobSpelling.ts | 50 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/jobsClient.ts | 139 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/pathMap.ts | 395 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/rtc.ts | 176 | DELETE |  | node-datachannel WebRTC agent; Rust supervisor dropped WebRTC |
| engine-tunnel/segments.ts | 35 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/testAgents.ts | 83 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/testBadFrame.ts | 45 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/testCore.ts | 434 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| engine-tunnel/upgrade.ts | 48 | DELETE |  | TS reference tunnel agent + harness. Importers: e2e/src/standInEngine.ts (4 conformance specs) and its own ... |
| graph-model/panel.ts | 23 | MOVE | song crate modules/panels.rs | lookup over every tab; small |
| graph-model/types.ts | 430 | GENERATE | song crate graph/view/web.rs ALREADY derives ts-rs (35 derives): export and import | ~30 lines of shared values (port separator, categories) STAY or generated consts |
| recorder/src/webmDuration.ts | 239 | MOVE | waveform crate webm.rs/webm_write.rs: patch Duration in the header bytes (wasm op over ... | binary format = Rust rule; Blob slicing stays in TS (~20) |
| take-render/src/materialRender.ts | 135 | DELETE |  | only @studio/commands (deleted on the verbs branch); Rust verbs material_render.rs does it |
| take-render/src/renderToWav.ts | 34 | DELETE |  | only materialRender/commands |
| take-render/src/wavLength.ts | 33 | DELETE |  | only commands diskBundle/bundleRuntime |
| ui-editor/src/k7x/cursor.ts | 219 | MOVE | k7x crate: tolerant cursor scan -> k7-wasm op | per keystroke completion: one wasm call fine; k7x TS twin owned by rust-song-twins |
| ui-editor/src/k7x/entityValues.ts | 77 | MOVE | k7x crate / song hints: values with their field | tree walk |
| ui-editor/src/k7x/previewTriggers.ts | 75 | MOVE | song crate: which lines audition (clip source, slice window) |  |
| ui-editor/src/noteGrid/notes.ts | 101 | MOVE | song-edit notes.rs / material_notes.rs (exist): read/write a sequence's notes | Rust edit exists; TS round-trip is a twin |
| ui-eyes/src/camera.ts | 97 | MOVE | with web-protocol cameras name rules (k7-front cameras) | matching half; getUserMedia half STAY |
| ui-eyes/src/cube.ts | 57 | MOVE | a Rust .cube reader (verbs picture or k7-mediatek) -> bytes for texImage3D | file format |
| ui-eyes/src/scene.ts | 242 | GENERATE | ts-rs on k7-daw-wasm/verbs/src/picture/doc.rs (lands with the verbs branch) | ~40 lines of helpers STAY |
| ui-eyes/src/timeline.ts | 164 | MOVE | verbs picture compile: emit per-layer clip schedule in the scene | called per frame per layer: do NOT call wasm per frame; precompute a table in Rust, look it up in TS |
| ui-eyes/src/transitions.ts | 64 | MOVE | verbs picture glsl.rs: the two builtin transitions' GLSL named in the scene | data, not drawing |
| ui-timeline/src/panZoom.ts | 243 | STAY 183 / MOVE 60 |  | React hooks; ~60 lines of anchored-zoom arithmetic MOVE with ticks (gesture arithmetic) |
| ui-timeline/src/ticks.ts | 172 | MOVE | gesture crate (new, k7-daw-wasm/gesture): ruler unit, labels, hit tick | per zoom change, not per frame; native UI needs the same rulers |
| ui-waveform/src/spectrum.ts | 189 | STAY 99 / MOVE 90 |  | Canvas drawing; ~90 lines of column maths MOVE with spectrumColour (RGBA tile from wasm) |
| ui-waveform/src/spectrumColour.ts | 51 | MOVE | waveform crate: column colour from band energies | per column; return an RGBA strip from wasm per tile, not per pixel |
| web-protocol/cameras.ts | 166 | GENERATE | ts-rs on k7-front cameras hub frames; name rules (clean/unique/match ~50) MOVE to the ... | split: 12 types GENERATE, ~50 name rules MOVE, constants STAY |
| web-protocol/commandTypes.ts | 100 | GENERATE | ts-rs on verbs catalogue (k7-daw-wasm/verbs, song views/catalogue.rs derives TS) | mirrors GET /commands; verbs agent's branch edits it: after its merge |
| web-protocol/compat.ts | 50 | MOVE | backend route /project-compat (k7-front files/service/routes.rs) already scores; page ... | score is computed in Rust already; TS twin |
| web-protocol/engineAccess.ts | 146 | STAY 116 / MOVE 30 | constants of the wire; backoff (~20) MOVE optional | header names/budgets could be generated as consts; low value |
| web-protocol/machineTypes.ts | 122 | GENERATE | ts-rs on k7-file-service listings + k7-supervisor stem jobs | 13 types |
| web-protocol/paths.ts | 56 | MOVE | k7-file-service/src/paths.rs (exists) -> k7-wasm op or drop browser use | lexical confinement; Rust side exists |
| web-protocol/performance.ts | 93 | MOVE | k7-supervisor/src/performance.rs: send the score on the heartbeat | the format/describe strings (~20) STAY (i18n) |
| web-protocol/playlists.ts | 13 | GENERATE | ts-rs on k7-file-service playlists reply |  |
| web-protocol/projects.ts | 355 | MOVE | new address module in k7-front (or k7-wasm op): launch/address grammar | contract between backend and page; ~55 lines of page-path constants STAY |
| web-protocol/quality.ts | 52 | MOVE | k7-engine registry (or k7-wasm op): qualityOf/audioLatencyMs | formatLatency STAY; measured in browser, scored anywhere |
| web-protocol/resources.ts | 17 | GENERATE | ts-rs on k7-file-service resources reply |  |
| web-protocol/rtcFrames.ts | 63 | DELETE |  | WebRTC fragments: Rust supervisor answers every rtc-offer with an error; only rtcSocket.ts and ... |
| web-protocol/textHash.ts | 20 | MOVE | k7-file-service text_hash.rs already exists -> expose as k7-wasm op | Rust twin exists (k7-file-service/src/text_hash.rs); per-write call, cheap |
| web-protocol/timelineTypes.ts | 400 | GENERATE | ts-rs on song crate views/web (clip.rs, project.rs already derive TS) + diagnostics | 30 hand types; most mirror song::views::web; overlap rust-song-twins: coordinate |
| web-protocol/types.ts | 11 | GENERATE | re-export index of generated types | becomes an index over generated/ |
| web-protocol/workspaceTypes.ts | 332 | GENERATE | ts-rs on k7-file-service / k7-front reply structs (no ts-rs in backend yet) | 29 types; 1 const STAY |
| engine-browser/src/*.ts (18 files) | 2789 | STAY |  | browser WebSocket client, external stores, registry fetches, Web Audio; rAF playhead prediction, per frame; per audio ... |
| engine-client/src/*.ts (5 files) | 489 | STAY |  | Node engine client for k7-runtime dev scripts (list-sessions, two-sessions, platter-node-probe) + ... |
| engine-protocol/*.ts (4 files) | 381 | STAY |  | reply decoding on the browser socket; types part GENERATE with types.ts; binary frame header decode, per audio frame ... |
| env/src/index.ts | 169 | STAY |  | dev env.yaml reader for e2e + dev-front.mjs (build tooling) |
| http-client/src/*.ts (3 files) | 285 | STAY |  | fetch/XHR upload progress, download list |
| i18n/src/*.ts (2 files) | 65 | STAY |  | Paraglide seam (build + React) |
| i18n/src/react.tsx | 127 | STAY |  | Paraglide seam (build + React) |
| k7-sdk/src/plugin.ts | 32 | STAY |  | plugin contract (TS interface for panes) |
| notices/*.ts (4 files) | 431 | STAY |  | build tooling: third-party notices |
| preload/vitePlugin.ts | 67 | STAY |  | build tooling |
| project-shell/src/*.tsx (6 files) | 1261 | STAY |  | fetch transport, gate state machine, toasts, React screens |
| project-shell/src/*.ts (13 files) | 1193 | STAY |  | fetch transport, gate state machine, toasts, React screens |
| recorder/src/*.ts (2 files) | 70 | STAY |  | Intl date wording, live value store |
| song-strudel/src/*.ts (2 files) | 230 | STAY |  | Strudel is JS: bundled into the engine's QuickJS; not page code |
| take-render/src/*.ts (4 files) | 240 | STAY |  | engine dev scripts (renderer); relocate with renderProject; Node tooling: 9 k7-runtime scripts import it; Node tooling ... |
| ui-editor/src/YamlPane.tsx | 112 | STAY |  | CodeMirror extensions and panes |
| ui-editor/src/*.ts (7 files) | 725 | STAY |  | CodeMirror extensions and panes; debounce + CodeMirror state; validity asked from song-schema (twins) |
| ui-editor/src/k7x/*.ts (6 files) | 505 | STAY |  | CodeMirror extensions and panes |
| ui-editor/src/noteGrid/*.ts (2 files) | 270 | STAY |  | DOM piano roll; CodeMirror extensions and panes |
| ui-editor/src/yaml/*.ts (5 files) | 310 | STAY |  | CodeMirror extensions and panes; lezer tree cursor context (CodeMirror) |
| ui-eyes/src/*.ts (23 files) | 3424 | STAY |  | WebGL2 compositor, <video>, WebCodecs/mediabunny export; Canvas measureText wrapping; per-frame keyframe eval + affine; ... |
| ui-mini-player/src/*.tsx (4 files) | 636 | STAY |  | React bar + Media Session API |
| ui-mini-player/src/*.ts (3 files) | 209 | STAY |  | React bar + Media Session API |
| ui-timeline/src/*.ts (2 files) | 121 | STAY |  | pointer drag, lane geometry constants |
| ui-waveform/src/*.tsx (2 files) | 920 | STAY |  | Canvas waveform drawing |
| ui-waveform/src/*.ts (4 files) | 162 | STAY |  | Canvas waveform drawing |
| ui/src/*.tsx (41 files) | 4596 | STAY |  | UI kit (shadcn), DataTransfer, focus, long press |
| ui/src/*.ts (6 files) | 158 | STAY |  | UI kit (shadcn), DataTransfer, focus, long press |
| ui/src/lib/*.ts (4 files) | 225 | STAY |  | UI kit (shadcn), DataTransfer, focus, long press |
| vite-config/src/*.ts (3 files) | 538 | STAY |  | build tooling |
| vite.config.ts | 101 | STAY |  | build tooling |
| vite.docs.config.ts | 32 | STAY |  | build tooling |
| vitest.config.ts | 2 | STAY |  | build tooling |
| web-protocol/*.ts (2 files) | 115 | STAY |  | origin/port resolution of the page (browser location); Intl.Collator sort for display; Rust order is code-point by ... |

**M4a, as built.** The reads moved into `apps/k7-daw-wasm/song/src/reads` (`apps/k7-daw-wasm/song/docs/reads.md`), the ops `reads.*` of k7-wasm and `@studio/k7-wasm/reads`. The page's modules keep their paths and exports as thin wrappers (`takes`, `rows`, `rackLit`, `noteSpans`, `videoSpans`, `clipLoop`, `clipInstances`, `clipNames`, `mixerStrips`, `gainTrim`, `midiIn`, `laneInput`, `rackFor`, `graphNotes`, `templateGraph`, `headphonesHint`, `timelineView`'s `followRename`, `projectName`, `projectKind`); `activity.ts` and `tapTempo.ts` are gone. The shell's projection (`useProjectDoc`) and the mute/solo rule of `TimelineTab` (`laneMix.ts`) are Rust's. `engine-protocol/project.ts` is generated (decision 301 superseded), the Web* types of `timelineTypes.ts` are generated from `views/web`, and `build.ts` reads its defaults from `reads.blanks`. Each moved function ran against the old TypeScript on the 172 golden documents and on generated inputs (about 700,000 comparisons, no difference; the run caught `countIn` of a blank document, 1 in the compiler's default and 0 in `emptyDoc`, which `blanks` keeps at 0). What stayed in TypeScript and why: the table in `reads.md`; `mastering/` (a control table of labels and per-frame agreement), `hooks/useTrackControls` (React state and a command line), `rowKey` and the row lookups (they act on the page's row objects), `timelineView` (an object spread over React state), and `templateGraph`'s caller. Not done: the other types of `timelineTypes.ts` (waveform, picture, diagnostics) have no Rust twin.

