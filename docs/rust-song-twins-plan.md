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
| `@studio/song-schema` | 7.2k + 5.1k lines, 63 files | 212 names imported by the page and its packages (below) |
| `@studio/k7x` | 1.9k + 1.5k lines, 11 files | the editor's tree and value-by-value edits; generator of the k7x crate's vectors |

M1 found that nothing in production reached `@studio/song` or `@studio/song-edit` once the verbs
moved to Rust: their only importers were take-render's material cut, wav length and freeze bounce
(no caller), engine-client's plugin cache (no caller), the bench scripts' group fold (their docs
have no groups) and `lint:panels`. M1 removed 33k lines.

## Inventory

Every name the page (`apps/k7-daw-frontend/src`) and the other packages import from
`@studio/song-schema` and `@studio/k7x`, counted on this branch: 212 names, 423 import sites
(356 outside tests). A † marks a name imported by tests only. The target column names what
replaces each group.

### (a) a Rust twin the browser module already answers: 4

- `song-schema/timecode`: `parsePosition`, `parseDuration`, `beatsToPosition`, `beatsToDuration` (ops `timecode.parse`, `timecode.format`)

Target: `@studio/k7-wasm/timecode`, sync functions over the existing ops (M2).

### (b) a Rust twin not yet exposed: 37

- `k7x/document`: `K7Document`, `parse`; `k7x/parse`: `parseTree`, `parseValue`; `k7x/ast`: `walk`; `k7x/print`: `stringify` — crate `k7x` (`edit::Document`, `parse_tree`, `edit::print::stringify`)
- `song-schema/timecode`: `roundBeat` — `song_schema::timecode::round_beat`
- `song-schema/moduleTypes`: `engineModuleOf`†, `isEngineRef`, `isLibraryFileOf`, `isLibraryName`, `isPicturePass`, `moduleEmits`, `moduleStatusOf`, `ownLevelSide`, `ownMixSide`, `splitPort` — `song::modules::{types, files}`, `song::doc::own_mix`, `song::routing`
- `song-schema/yaml/fields`: `fieldsAt`, `fileKindFor`, `valuesAt` — `song::views::fields`
- `song-schema/yaml/sugar`: `sugarFor`† — `song_edit::sugar`
- `song-schema/docExt`: `isDocFile`, `stripDocExt`; `audioFiles`: `isAudio`; `builtinParams`: `builtinPluginForRef`; `instanceName`: `instanceName`; `keyframes`: `valueAt`; `placementId`: `materialRefOf`; `playlist`: `parsePlaylist`, `playlistPathOf`; `renderSettings`: `resolveVideoSettings`; `takes`: `setCompSpan`; `tasks`: `parseTasks`; `timelineLane`: `timelineLaneOf`; `trackKind`: `orderPosition`†; `warpPoints`: `beatAtSource`, `sourceAtBeat` — in `song`, `song-edit`, `verbs` or the backend's `k7-file-service`; a twin in the backend or in `verbs` moves down into `song-schema` or `song` so the module can link it

Target: one op family per crate area (`k7x.*`, `schema.*`, `fields.*`), M2 to M3c.

### (c) TypeScript only, no Rust twin: 57

- `song-schema/yaml/validate`: `validateFile`, `firstIssue` — the router over the per-file schemas, zod inside
- writers of a file's text: `yaml/videoTrackFile` (`listVideoPlacements`, `setVideoPlacementField`, `setVideoPlacementFieldAt`, `shiftVideoAnimationAt`), `yaml/assemblyFile` (`dropAssemblyParam`, `parseAssemblyTemplate`, `setAssemblyParam`), `yaml/sequenceFile` (`editSequence`, `sequenceData`), `yaml/mixFile` (`cueMarkers`, `parseMix`, `withMarkers`), `yaml/mixViewFile` (`parseMixView`), `yaml/chainFile` (`parseChain`), `yaml/chainSchema` (`isEndStep`, `itemsOf`), `yaml/moduleSchema` (`rackAddBind`, `rackAddBindAt`, `rackBindSymbol`), `playlist` (`appendPlaylistItem`, `movePlaylistItem`, `playlistStub`, `removePlaylistItem`), `libraryVersions` (`parseLibraryPins`, `pinnedVersion`, `stringifyLibraryPins`)
- readers and rules: `records` (`audioFilesOf`, `recordFolder`, `recordMatches`, `recordName`, `recordPathOf`, `resolveRecords`), `stems` (`deckPlatter`, `stemControl`, `stemFile`, `stemLane`, `stemLaneOf`, `stemPlatter`), `seqWindow` (`seqWindowFromText`, `windowSpan`), `trackKind` (`trackKind`, `trackLabel`, `reorderTracks`†), `moduleTypes` (`assetsForParam`, `libraryRef`, `moduleKeyFromPath`, `shaderUniformFor`), `aspect` (`aspectOf`), `builtinParams` (`builtinPluginFor`), `placementId` (`placementStartOf`), `projectTags` (`projectTagsOf`), `renderSettings` (`parseVideoSettings`), `surfaceRegistry` (`allSurfaceTypes`, `surfaceTypeOf`†), `docExt` (`docPattern`)

Target: written in Rust (`song-schema` for rules, `song-edit` for text-in, text-out writers), then
reached as wasm ops (M3b, M4a, M4b).

### (d) types and constants, generated from Rust: 105

- types (31): `K7MapNode`, `K7Path`, `K7Change`, `K7Scope`, `Key`, `LibraryPins`, `LibraryFileFamily`, `ModuleStatus`, `PlaylistItem`, `NodePresets`, `PresetEntry`, `ProjectShape` (both), `ResolvedRecord`, `ReferenceTrack`, `ExportFormat`, `ExportQuality`, `VideoFormat`, `VideoSettings`, `WindowSpan`, `Stem`, `Task`, `FileKind`, `MixView`, `ModuleUI`, `Widget`, `WidgetType`, `ProjectSurface`, `ProjectSurfaces`, `ModuleIssue`, `Validity` — ts-rs from the Rust struct the value is decoded into
- zod schemas (17): `ClipSchema`†, `CurveSchema`†, `SequenceSchema`†, `GraphFileSchema`†, `MixViewSchema`†, `ModuleConfigSchema`†, `ModuleSchema`†, `UISchema`, `UiFileSchema`, `PresetFileSchema`†, `ProjectFileSchema`†, `SettingsSchema`†, `SurfaceSchema`†, `TimelineSchema`†, … — gone with `validateFile`; what a page reads of one is its ts-rs type
- constants (57): the `moduleTypes` tables (`MODULE_TYPES`, `IO_MODULES`, `FILE_PARAMS`, `MIX_PARAM`, `INPUT_VOLUME_PARAM`, `OUTPUT_VOLUME_PARAM`, `SHADER_PARAMS`, `META_NS`, the folder names…), the file names (`TASKS_FILE`, `PLAYLISTS_DIR`, `STEMS_DIR`, `GRAPH_FILE`, `MIX_BASE`…), the vocabularies (`ASPECTS`, `STEMS`, `EXPORT_FORMATS`, `VIDEO_FORMATS`, `LFO_SHAPES`, `CURVE_CUTS`, `WIDGET_TYPES`…), the bounds (`MASTER_MAX_DB`, `VOLUME_RANGE`, `STEM_RANGE`, `TAB_NAME_MAX`…), `TIMECODE`

Target: `@studio/k7-wasm/types` and a generated `@studio/k7-wasm/consts` (M2).

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
| `k7x` edits cross as one batch per gesture: `k7x.edit {text, ops}` answers the new text and its `K7Change`s in UTF-16 units | `K7Document` is held across a gesture's edits; the crate's `Document` is a byte-for-byte port held to the same vectors (`apps/k7-daw-frontend/packages/k7x/docs/rustEdit.md`) |
| the editor's tree is a `k7x.outline` op: what CodeMirror reads (paths, kinds, UTF-16 spans of keys and values), never the full tree | the full tree of the corpus is 13.9 MB of JSON and parses slower than the TypeScript parse (`apps/k7-daw-frontend/packages/k7x/docs/rust.md`) |
| when `@studio/k7x` goes, its vectors freeze, as song-edit's are | the TypeScript generator is what is being deleted; the Rust tests replay the frozen vectors |
| the constants are Rust's: the `bindings` binary writes `consts.generated.ts`, and `genModuleTypes.mts` stops writing Rust from TypeScript | one source per table; ts-rs emits types, not values |
| a TypeScript test ported to Rust where it proves arithmetic or a format; kept as a TypeScript test over the module where it proves what the page sees | the existing tests are the oracle for what each function answers |
| the op dispatch is split per family (`ops/timecode.rs`, `ops/k7x.rs`, `ops/schema.rs`, …) before parallel work starts | three builders adding variants to one `Request` enum would conflict on every merge |

## Milestones

| | content | depends on | parallel group | agent-hours |
|---|---|---|---|---|
| M1 | the compiler's TypeScript leaves: `@studio/song`'s and `@studio/song-edit`'s code, take-render's dead half, engine-client's plugin cache, song-schema's dead writers; `lint:panels` moves to ui-module-builder | — | done | 3 |
| M2 | the seam: dispatch split per family; the module built with `song` (decoders, views) and `k7x`, as a hashed asset; `consts.generated.ts`; `@studio/k7-wasm/timecode` replaces `song-schema/timecode` (group a, `roundBeat`, `TIMECODE`); `edit.call`, `host_fs` and the `song`-feature TS module removed if no host remains (decision 4); size measured | M1 | alone | 5 |
| M3a | the schema rules of group (b): `moduleTypes` predicates, `docExt`, `playlist` reads, `tasks`, `warpPoints`, `keyframes`, `placementId`, `timelineLane`, `instanceName`, `audioFiles`, `builtinParams`, `takes`; `moduleTypes.ts`'s tables generated from Rust | M2 | A | 4 |
| M3b | validation and completion: `file.validate`, `fields.at`, `fields.values`; the router ported to Rust over `song::files`; `validate.ts`, the zod schemas and `zod` leave song-schema | M2 | A | 6 |
| M3c | k7x: `k7x.edit`, `k7x.data`, `k7x.print`, `k7x.outline`; ui-editor, ui-module-builder, panel-rules and the page move over; `@studio/k7x` deleted, its vectors frozen | M2 | A | 6 |
| M4a | the text writers of group (c) in `song-edit` (video track, assembly, sequence, mix markers, playlist edits, library pins, chain reads), reached as `text.*` ops | M3a, M3c | B | 6 |
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

## Decisions for the user

1. **The module's size and loading.** One module with `song`'s decoders and views and `k7x`
   (estimate 2-3 MB, measured in M2), a hashed `.wasm` asset awaited before mount. The alternative is a
   small module at boot plus a second one fetched when the editor or the module builder first
   opens. Recommendation: one module; split only if M2 measures a mount delay a user would notice on
   the phone.
2. **The editor's tree from Rust.** Recommendation: `k7x.outline` replaces `parseTree`, held to a
   budget measured on the largest library file (`surge-xt/ui.k7x`, 300 KB) before the switch, so
   `@studio/k7x` is deleted whole. The alternative keeps `parse.ts` as the editor's tree only, a
   twin the vectors keep honest.
3. **Where the song model's docs and data live.** Recommendation: M5 moves `song/docs`,
   `song-edit/docs` and `song-schema/docs` into `apps/k7-daw-wasm/<crate>/docs`, and `song/meta` and
   `song/samples` into `apps/k7-daw-wasm/song/`, one mechanical commit with `lint:docs`. Some 900
   references name `packages/song/`, the backend's code root and its frozen goldens among them, so
   the other answer is to leave them where they are.
4. **The `edit.call` door and its replays.** Since M1, no TypeScript host calls `edit.call`: the
   6.7 MB `k7WasmSong.generated.ts`, `host_fs` and `test:edit-replay` prove a module nothing loads.
   Recommendation: remove them in M2 and keep song-edit's native replays; M4a's writers are pure
   text in, text out, and need no file system.
5. **Writers in the module or behind a route.** Recommendation: the group (c) writers run in the
   module (M4a), since the page edits the open buffer and autosave writes it; a writer that must
   read other files stays a verb behind the backend.
