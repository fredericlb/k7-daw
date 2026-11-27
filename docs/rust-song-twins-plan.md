# The song model's TypeScript twins leave for the Rust crates

The page reaches k7's song model through TypeScript packages that each have a Rust half
(`docs/rust-first.md`). This plan replaces them with the Rust crates the backend already links,
reached from the page through the one WebAssembly module (`apps/k7-daw-wasm/k7-wasm`). Direct
replacement: no TypeScript stands behind a Rust call, and a minor bug is fixed forward.

## Where it stands

| package | TypeScript (src, tests) | state |
|---|---|---|
| `@studio/song` | 0 | M1 deleted its code; `docs/`, `meta/` and `samples/` stay where the backend and the crates read them |
| `@studio/song-edit` | 0 | M1 deleted its code; `docs/` stays |
| `@studio/song-schema` | 6.1k + 4.4k lines | 212 names imported by the page and its packages (below) less the timecode's, the 24 M3a moved and the validation's; its module-type tables and name constants come from Rust since M2 and M3a, its check and completions since M3b |
| `@studio/k7x` | 0 | M3c deleted it: the page reads and edits k7x through `@studio/k7-wasm/k7x` over the `k7x.*` ops; its vectors are frozen in the crate |

M1 found that nothing in production reached `@studio/song` or `@studio/song-edit` once the verbs
moved to Rust: their only importers were take-render's material cut, wav length and freeze bounce
(no caller), engine-client's plugin cache (no caller), the bench scripts' group fold (their docs
have no groups) and `lint:panels`. M1 removed 33k lines.

M2 built the seam. The browser module is one file, `apps/k7-daw-frontend/packages/k7-wasm/src/k7Wasm.wasm`,
carrying `song-schema`, the `song` crate (without `disk`) and `k7x`, served as a hashed asset; its
dispatch is split per family (`apps/k7-daw-wasm/k7-wasm/src/ops/`); the page's timecode is
`@studio/k7-wasm/timecode` over the Rust ops; the constants the page reads from the crates are
`consts.generated.ts`; and `edit.call`, `host_fs`, the verbs' base64 module and `test:edit-replay` are gone.

| M2 measure | value |
|---|---|
| module size | 3.38 MB raw, 1.06 MB gzip, 0.74 MB brotli (the base64 curves module it replaces was 0.41 MB of TypeScript, the verbs' 6.7 MB) |
| module ready, desktop (M-series Mac under a load of 19, V8 in node, the engine Chrome runs) | 2-3 ms to instantiate with V8's default lazy compilation, 1.2 ms for the first call; 70 ms when every function is compiled eagerly on one thread, the worst case |
| module ready, phone-class estimate (a mid-range phone counted 4-6× slower than the desktop) | 15-20 ms lazily, under 0.5 s eagerly; the download is a loopback read or the service worker's precache, and the streaming compile overlaps it |
| one sync call | 0.5 µs inside the module for `timecode.parse` (desktop), the JSON both ways on top |

M3b added `file.validate`, `fields.kind`, `fields.at` and `fields.values` (families `ops/file.rs` and
`ops/fields.rs`) over `song::files::validate_file` and `song::views::fields`. The module grew to 3.46 MB raw,
1.07 MB gzip, 0.76 MB brotli. The editor's check and completions run in Rust; what keeps `zod` in
`@studio/song-schema` is below.

A user does not notice the mount delay, on the phone either, so the module stays whole (decision 1).

M3c moved k7x over. The editor's tree is held to the TypeScript parse it replaced, measured on the
largest library file before the switch (`apps/k7-daw-wasm/k7x/docs/outline.md`); the module grew to
3.63 MB raw, the k7x editor and printer now linked.

| M3c measure | `surge-xt/ui.k7x` (302 KB) | the corpus (1,356 files) |
|---|---|---|
| the TypeScript `parseTree` (the budget) | 3.6–3.8 ms | 16.4–17.7 ms |
| `k7x.outline` and its nodes | 3.3–3.5 ms | 17.5–18.1 ms |
| the same outline as JSON numbers, not kept | 4.9–7.7 ms | 25–37 ms |
The browser pane refused the measurement page on a loopback port, so no figure comes from a
browser with its CPU throttled: the phone line is an estimate from V8's own figures.

M3a moved 24 names (`moduleTypes` predicates but `splitPort`, `docExt`, `audioFiles`, `builtinParams`,
`instanceName`, `keyframes`, `placementId`, `playlist` reads and path, `takes`, `tasks`, `timelineLane`,
`warpPoints`) to the 23 `schema.*` ops of `apps/k7-daw-wasm/k7-wasm/src/ops/schema.rs`, reached as
`@studio/k7-wasm/schema`, and generated the name constants of the module vocabulary (`consts.generated.ts`,
from `song::modules::names`). The module is 3.55 MB raw, 1.09 MB gzip, 0.77 MB brotli. A call costs
about a microsecond (`isDocFile` 0.8 µs, `valueAt` 1.2 µs, a thousand `beatsAtSources` over 40 markers 0.08 ms,
desktop under load). Group (b) still holds `renderSettings`, `trackKind`, `fields` and `sugar`
(M3b, M4b), and the `k7x` names (M3c).

## Inventory

Every name the page (`apps/k7-daw-frontend/src`) and the other packages import from
`@studio/song-schema` and `@studio/k7x`, counted on this branch: 212 names, 423 import sites
(356 outside tests). A † marks a name imported by tests only. The target column names what
replaces each group.

### (a) a Rust twin the browser module already answers: 4

- `song-schema/timecode`: `parsePosition`, `parseDuration`, `beatsToPosition`, `beatsToDuration` (ops `timecode.parse`, `timecode.format`)

Target: `@studio/k7-wasm/timecode`, sync functions over the existing ops (M2, built: `roundBeat` is the hot ABI call `k7_round_beat`, `TIMECODE` and `TICKS_PER_QUARTER` are generated constants; the event-start helpers had no importer and left with `timecode.ts`).

### (b) a Rust twin not yet exposed: 37

- `k7x/document`: `K7Document`, `parse`; `k7x/parse`: `parseTree`, `parseValue`; `k7x/ast`: `walk`; `k7x/print`: `stringify` — crate `k7x` (`edit::Document`, `parse_tree`, `edit::print::stringify`)
- `song-schema/timecode`: `roundBeat` — `song_schema::timecode::round_beat`
- `song-schema/moduleTypes`: `engineModuleOf`†, `isEngineRef`, `isLibraryFileOf`, `isLibraryName`, `isPicturePass`, `moduleEmits`, `moduleStatusOf`, `ownLevelSide`, `ownMixSide`, `splitPort` — `song::modules::{types, files}`, `song::doc::own_mix`, `song::routing`
- `song-schema/yaml/fields`: `fieldsAt`, `fileKindFor`, `valuesAt` — `song::views::fields` (M3b, built: `@studio/k7-wasm/fields`)
- `song-schema/yaml/sugar`: `sugarFor`† — `song_edit::sugar`
- `song-schema/docExt`: `isDocFile`, `stripDocExt`; `audioFiles`: `isAudio`; `builtinParams`: `builtinPluginForRef`; `instanceName`: `instanceName`; `keyframes`: `valueAt`; `placementId`: `materialRefOf`; `playlist`: `parsePlaylist`, `playlistPathOf`; `renderSettings`: `resolveVideoSettings`; `takes`: `setCompSpan`; `tasks`: `parseTasks`; `timelineLane`: `timelineLaneOf`; `trackKind`: `orderPosition`†; `warpPoints`: `beatAtSource`, `sourceAtBeat` — in `song`, `song-edit`, `verbs` or the backend's `k7-file-service`; a twin in the backend or in `verbs` moves down into `song-schema` or `song` so the module can link it

Target: one op family per crate area (`k7x.*`, `schema.*`, `fields.*`), M2 to M3c. M3c built the k7x group: `K7Document`, `parse`, `parseTree`, `walk` and `stringify` are `@studio/k7-wasm/k7x` over `k7x.edit`, `k7x.data`, `k7x.outline` and `k7x.print`.

### (c) TypeScript only, no Rust twin: 57

- `song-schema/yaml/validate`: `validateFile`, `firstIssue` — the router over the per-file schemas, zod inside (M3b, built: `@studio/k7-wasm/validate`)
- writers of a file's text: `yaml/videoTrackFile` (`listVideoPlacements`, `setVideoPlacementField`, `setVideoPlacementFieldAt`, `shiftVideoAnimationAt`), `yaml/assemblyFile` (`dropAssemblyParam`, `parseAssemblyTemplate`, `setAssemblyParam`), `yaml/sequenceFile` (`editSequence`, `sequenceData`), `yaml/mixFile` (`cueMarkers`, `parseMix`, `withMarkers`), `yaml/mixViewFile` (`parseMixView`), `yaml/chainFile` (`parseChain`), `yaml/chainSchema` (`isEndStep`, `itemsOf`), `yaml/moduleSchema` (`rackAddBind`, `rackAddBindAt`, `rackBindSymbol`), `playlist` (`appendPlaylistItem`, `movePlaylistItem`, `playlistStub`, `removePlaylistItem`), `libraryVersions` (`parseLibraryPins`, `pinnedVersion`, `stringifyLibraryPins`)
- readers and rules: `records` (`audioFilesOf`, `recordFolder`, `recordMatches`, `recordName`, `recordPathOf`, `resolveRecords`), `stems` (`deckPlatter`, `stemControl`, `stemFile`, `stemLane`, `stemLaneOf`, `stemPlatter`), `seqWindow` (`seqWindowFromText`, `windowSpan`), `trackKind` (`trackKind`, `trackLabel`, `reorderTracks`†), `moduleTypes` (`assetsForParam`, `libraryRef`, `moduleKeyFromPath`, `shaderUniformFor`), `aspect` (`aspectOf`), `builtinParams` (`builtinPluginFor`), `placementId` (`placementStartOf`), `projectTags` (`projectTagsOf`), `renderSettings` (`parseVideoSettings`), `surfaceRegistry` (`allSurfaceTypes`, `surfaceTypeOf`†), `docExt` (`docPattern`)

Target: written in Rust (`song-schema` for rules, `song-edit` for text-in, text-out writers), then
reached as wasm ops (M3b, M4a, M4b). M4a built the writers: `@studio/k7-wasm/text` carries the video track, sequence, cue sheet, playlist, pins, assembly and chain reads, and `sugared.ts` and `sugar.ts` left with `sugarFor` (`text.sugar_expand`, `text.sugar_apply`); `rackAddBind*` stay in `moduleSchema.ts` with the widget schema, and `pinnedVersion` stays a lookup in `libraryVersions.ts` (`apps/k7-daw-frontend/packages/k7-wasm/docs/text.md`).

### (d) types and constants, generated from Rust: 105

- types (31): `K7MapNode`, `K7Path`, `K7Change`, `K7Scope`, `Key`, `LibraryPins`, `LibraryFileFamily`, `ModuleStatus`, `PlaylistItem`, `NodePresets`, `PresetEntry`, `ProjectShape` (both), `ResolvedRecord`, `ReferenceTrack`, `ExportFormat`, `ExportQuality`, `VideoFormat`, `VideoSettings`, `WindowSpan`, `Stem`, `Task`, `FileKind`, `MixView`, `ModuleUI`, `Widget`, `WidgetType`, `ProjectSurface`, `ProjectSurfaces`, `ModuleIssue`, `Validity` — ts-rs from the Rust struct the value is decoded into
- zod schemas (17): `ClipSchema`†, `CurveSchema`†, `SequenceSchema`†, `GraphFileSchema`†, `MixViewSchema`†, `ModuleConfigSchema`†, `ModuleSchema`†, `UISchema`, `UiFileSchema`, `PresetFileSchema`†, `ProjectFileSchema`†, `SettingsSchema`†, `SurfaceSchema`†, `TimelineSchema`†, … — M3b removed the clip, sequence, curve, timeline, preset, project, settings, controller and config schemas, which only `validateFile` and the docs test read; the module, ui, graph, surface and mix-view schemas stay while their readers (`parseSurface`) and the zod of the widget and placement types do (M4a removed `parseChain`, `parseMixView`, `parseMix` and `parseAssembly` with the chain, mix and assembly schemas) and the `Widget` and `ModuleUI` types do, until M3a and M4a/M4b move them; what a page reads of one is its ts-rs type
- constants (57): the `moduleTypes` tables (`MODULE_TYPES`, `IO_MODULES`, `FILE_PARAMS`, `MIX_PARAM`, `INPUT_VOLUME_PARAM`, `OUTPUT_VOLUME_PARAM`, `SHADER_PARAMS`, `META_NS`, the folder names…), the file names (`TASKS_FILE`, `PLAYLISTS_DIR`, `STEMS_DIR`, `GRAPH_FILE`, `MIX_BASE`…), the vocabularies (`ASPECTS`, `STEMS`, `EXPORT_FORMATS`, `VIDEO_FORMATS`, `LFO_SHAPES`, `CURVE_CUTS`, `WIDGET_TYPES`…), the bounds (`MASTER_MAX_DB`, `VOLUME_RANGE`, `STEM_RANGE`, `TAB_NAME_MAX`…), `TIMECODE`

Target: `@studio/k7-wasm/types` and a generated `@studio/k7-wasm/consts` (M2 built the door with `TIMECODE`, `TICKS_PER_QUARTER`, `MODULE_TYPES`, `IO_MODULES` and `IO_SOURCES`; M3a adds the rest).

### (e) UI-only, stays TypeScript: 9

- `seriesColors` (`SERIES_COLORS`, `seriesColor`), `trackColor` (`categoryShade`): colors on screen
- `records` (`RECORD_DRAG_TYPE`, `RecordDrag`, `packRecordDrag`, `parseRecordDrag`): the browser's drag-and-drop payload
- `reference` (`REFERENCE_ACCEPT`): a file input's `accept` string
- `textOrder` (`byCodePoint`): the order a list is drawn in

They move into the page's own packages with M5, since no Rust side shares them.

## Choices

| choice | why |
|---|---|
| ONE browser module carries `song-schema`, `k7x`, and the `song` crate's file decoders and views; `song-edit`'s pure text writers join it in M4a | a page loads one memory and one allocator (`apps/k7-daw-frontend/packages/song-schema/docs/wasmBoundary.md`); a second module would ship serde twice |
| the page's module is a `.wasm` asset that Vite hashes, streamed with `WebAssembly.instantiateStreaming`; node hosts (tests, scripts) read the same file from disk | a 2-3 MB module as base64 inside a JS chunk is a third larger and is parsed as JavaScript before it compiles |
| `appMain.tsx` keeps awaiting `wasmReady()` before it mounts, and every call after it is sync | React reads a value during render; a call made before the module is ready throws `unavailable` rather than waiting, so a top-level call at import time is a bug a test catches |
| the calls stay coarse: one call per gesture, per validation, per completion request | each call copies JSON both ways; a per-keystroke or per-frame call is designed as a hot ABI call (the curve playhead) or not made |
| validation answers the shape the editor already draws, `Validity { ok, depth, issues: ModuleIssue[] }`, generated by ts-rs, with offsets in UTF-16 units | the editor's diagnostics and autosave change nothing but the import; Rust counts bytes, so the crossing converts |
| zod leaves with `validateFile`: a schema is the Rust decoder that reads the file for the compile | the editor asks a buffer exactly what the loader asks a file on disk, from one implementation |
| validation answers the compile's own decoder's FIRST finding, placed on the key it is about, its dotted path derived from the tree | the decoder stops where it cannot read; zod listed every issue, and a union left `Invalid input` with no line, which the Rust decoders answer with a sentence and a place |
| a kind the compile never reads (the cue sheet, the desk's bindings, a preset, a module folder's config) has a small decoder in `song::files` | the editor's router must not keep a TypeScript schema for them; they cost 200 lines of Rust and a test each |
| `k7x` edits cross as one batch per gesture: `k7x.edit {text, ops}` answers the new text and its `K7Change`s in UTF-16 units | `K7Document` is held across a gesture's edits; the crate's `Document` is a byte-for-byte port held to the same vectors (`apps/k7-daw-wasm/k7x/docs/rustEdit.md`) |
| the editor's tree is a `k7x.outline` op: what CodeMirror reads (paths, kinds, UTF-16 spans of keys and values), never the full tree | the full tree of the corpus is 13.9 MB of JSON and parses slower than the TypeScript parse (`apps/k7-daw-wasm/k7x/docs/rust.md`) |
| when `@studio/k7x` goes, its vectors freeze, as song-edit's are | the TypeScript generator is what is being deleted; the Rust tests replay the frozen vectors |
| the constants are Rust's: the `bindings` binary writes `consts.generated.ts`, and `genModuleTypes.mts` stops writing Rust from TypeScript | one source per table; ts-rs emits types, not values |
| a TypeScript test ported to Rust where it proves arithmetic or a format; kept as a TypeScript test over the module where it proves what the page sees | the existing tests are the oracle for what each function answers |
| the op dispatch is split per family (`ops/timecode.rs`, `ops/k7x.rs`, `ops/schema.rs`, …) before parallel work starts: each file declares its family's request enum and `dispatch`, `ops/mod.rs` routes on the op's prefix | three builders adding variants to one `Request` enum would conflict on every merge |
| a value the page asks per pointer move or per row drawn (`roundBeat`) is a scalar export (`k7_round_beat`), not a JSON op | a JSON call per note key would cost more than the arithmetic it replaces |
| a timecode op takes the meter as floats, as the page's project states it | the page read any meter before; a curve still refuses one that is not two integers of at least 1 |
| the module imports nothing from its host | no TypeScript host answered `edit.call` since M1; a writer the module carries is text in, text out (decision 5) |
| the module-type tables live in `apps/k7-daw-wasm/song/src/modules/type_table.rs`, and `moduleTypes.ts` reads them from `consts.generated.ts` | no TypeScript writes Rust; the tables' comments moved with them |
| a rule asked per row or per frame is a JSON op, and a loop that asks it of many inputs has a batch op (`schema.beats_at_sources`) | a call is about a microsecond, as much as the arithmetic of a scalar export; a waveform's column edges cross as one list, not one call each |
| a predicate argument crosses as the list it would read (`instanceName`'s `taken`) | a closure cannot cross, and each caller holds a Set already |
| `splitPort` stays in `@studio/graph-model` | graph-model owns it and calls it itself, below `song-schema`; its Rust twin serves the compiler |
| `song-edit` is linked into the module whole (`verbs` stays optional); a rule that lived in `verbs` (`parse_tasks`, `parse_playlist`) moved down into it | M3a's rules (document extension, instance name, comp span, tasks, playlists) are its functions, and `verbs` calls the same ones |
| a TypeScript function only its own test imported left with its file (`renamedInstance`, `compPieces`, `seamCrossfades`, `setWarpPoint`, `transformAt`…) | each has a Rust twin in `verbs`, `song` or the compile; keeping the TypeScript would be a second copy nobody calls |
| the PWA precaches `.wasm` with the scripts | the page mounts only once the module is compiled, so a module left to the network would put its download before every opening |

## Milestones

| | content | depends on | parallel group | agent-hours |
|---|---|---|---|---|
| M1 | the compiler's TypeScript leaves: `@studio/song`'s and `@studio/song-edit`'s code, take-render's dead half, engine-client's plugin cache, song-schema's dead writers; `lint:panels` moves to ui-module-builder | — | done | 3 |
| M2 | the seam: dispatch split per family; the module built with `song` (decoders, views) and `k7x`, as a hashed asset; `consts.generated.ts`; `@studio/k7-wasm/timecode` replaces `song-schema/timecode` (group a, `roundBeat`, `TIMECODE`); `edit.call`, `host_fs` and the `song`-feature TS module removed (decision 4); size measured | M1 | built | 5 |
| M3a | the schema rules of group (b): `moduleTypes` predicates, `docExt`, `playlist` reads, `tasks`, `warpPoints`, `keyframes`, `placementId`, `timelineLane`, `instanceName`, `audioFiles`, `builtinParams`, `takes`; `moduleTypes.ts`'s tables generated from Rust | M2 | built | 4 |
| M3b | validation and completion: `file.validate`, `fields.at`, `fields.values`; the router ported to Rust over `song::files`; `validate.ts`, `fields.ts` and the schemas only they read leave song-schema; `zod` stays there until M4a/M4b move the readers that still use it | M2 | built | 6 |
| M3c | k7x: `k7x.edit`, `k7x.data`, `k7x.print`, `k7x.outline`; ui-editor, ui-module-builder, panel-rules and the page move over; `@studio/k7x` deleted, its vectors frozen | M2 | built | 6 |
| M4a | the text writers of group (c) in `song-edit` (video track, assembly, sequence, mix markers, playlist edits, library pins, chain reads), reached as `text.*` ops | M3a, M3c | built | 6 |
| M4b | the readers and rules of group (c): `records`, `stems`, `seqWindow`, `trackKind`, `aspect`, `renderSettings`, `projectTags`, `surfaceRegistry`, the rest of `moduleTypes` | M3a | B | 4 |
| M5 | `@studio/song-schema` deleted (group e moves into the page's packages); `song/docs`, `song/meta`, `song/samples`, `song-edit/docs`, `song-schema/docs` move into their crates if decision 3 says so | M4a, M4b | alone | 3 |

Group A runs three builders on disjoint files once M2 has split the dispatch: M3a owns
`ops/schema.rs` and the song-schema rule files, M3b `ops/fields.rs` and `yaml/validate.ts` with its
schemas, M3c `ops/k7x.rs` and `@studio/k7x`. Group B runs two. Total about 37 agent-hours, 27 left
after M1; with the parallel groups, about 20 hours end to end.

Each milestone's tests: the Rust tests of the ops it adds (`apps/k7-daw-wasm/k7-wasm/src/tests.rs`
and the crate's own), the TypeScript tests of the functions it replaces kept and pointed at the
module, the k7-wasm build test (sources and types hashed), `tsc --noEmit` and vitest in
`apps/k7-daw-frontend`, and `bash scripts/agent/gate.sh --milestone`.

## Decisions

The user accepted the five recommendations on 2026-11-26.

1. **The module's size and loading.** One module with `song`'s decoders and views and `k7x`, a hashed
   `.wasm` asset awaited before mount; it is split only if a measure shows a mount delay a phone user
   would notice. M2 measured 3.38 MB and a few milliseconds to be ready (above): it stays one module.
2. **The editor's tree from Rust.** `k7x.outline` replaces `parseTree`, held to a budget measured on
   the largest library file (`surge-xt/ui.k7x`, 300 KB) before the switch, so `@studio/k7x` is
   deleted whole (M3c).
3. **Where the song model's docs and data live.** M5 moves `song/docs`, `song-edit/docs` and
   `song-schema/docs` into `apps/k7-daw-wasm/<crate>/docs`, and `song/meta` and `song/samples` into
   `apps/k7-daw-wasm/song/`, one mechanical commit with `lint:docs`.
4. **The `edit.call` door and its replays.** Removed in M2 with `host_fs`, the verbs' base64 module
   and `test:edit-replay`; song-edit's native replays stay, run by `scripts/check-vectors.sh` and the
   crate's tests.
5. **Writers in the module or behind a route.** The group (c) writers run in the module as pure text
   in, text out (M4a); a writer that must read other files stays a verb behind the backend.
