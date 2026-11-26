# song-schema: the file format

`apps/k7-daw-frontend/packages/song-schema` defines every file a project is made of: one zod schema per file kind, a
reader/writer for the kinds k7 edits by hand-shaped text transforms, and the small pure modules the
rest of the codebase shares (timecodes, module vocabulary, playlists, stems, takes…). It has no
`node:` import: the browser imports it directly.

## Layout

| Path | Contents |
|---|---|
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/projectSchema.ts` | `PROJECT_SHAPES` (the five archetypes), `MASTER_MIN_DB`/`MASTER_MAX_DB`. The file's own shape is the Rust reader's. |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/placementSchema.ts` | `PlacementSchema` (one event), `TIMECODE` (the regex), `TIMECODE_KEYS` (keys whose value must be written quoted). |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/entitySchemas.ts` | The source extensions that tell a picture clip from an audio one, and `NoteSchema`. The clip, sequence and curve files are the Rust readers'. |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/moduleSchema.ts` | `ModuleSchema` — every field a module, node or lane may carry — plus `AlternativeSchema`, `UISchema`/`UiFileSchema`, `WidgetSchema`, `RackSchema`. |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/chainSchema.ts` | `ChainFileSchema` with its structural refinements (first step, last step, lanes only in `start: true` chains, subchain rules), and the step predicates (`isEndStep`, `isCallStep`, `isTrackItem`, `itemsOf`, `expandCalls`). |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/graphSchema.ts` | `GraphFileSchema` (`modules`, `bypassed`, `bus_only`, `tap_feeds`, `positions`). |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/surfaceSchema.ts` | `SurfaceSchema`, the type ↔ layout table and the mapping skeletons; the surface types themselves are Rust's `SURFACE_TYPES`. |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/assemblySchema.ts` | `AssemblyTemplateSchema` (library) and `AssemblyFileSchema` (project). |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/mixSchema.ts`, `mixViewSchema.ts` | A set's cue sheet and the control → param binding a surface's `mapping` reuses. |
| the `*File.ts` modules of `apps/k7-daw-frontend/packages/song-schema/src/yaml/` | Readers and text editors per kind: `trackFile`, `sequenceFile`, `curveFile`, `videoTrackFile`, `takeFile`, `chainFile`, `graphFile`, `moduleFile`, `surfaceFile`, `assemblyFile`, `mixFile`, `alternativeFile`. |
| `apps/k7-daw-wasm/song/src/files/validate.rs`, `route.rs` | The editor's check: path → the compile's own reader, reached as `file.validate`. |
| `apps/k7-daw-wasm/song/src/views/fields.rs` | The editor's completion vocabulary, derived from the readers' own decoders, reached as `fields.*`. |
| `apps/k7-daw-frontend/packages/song-schema/src/yaml/sugar.ts`, `k7xIssues.ts` | k7x short forms and k7x-located findings. |
| `apps/k7-daw-frontend/packages/song-schema/src/moduleTypes.ts` | The module vocabulary: type folders and their default ports, built-in ends, namespaces, refs. |
| `apps/k7-daw-frontend/packages/song-schema/src/keyframes.ts` | Shot animation keys and their evaluation. |
| other `src/*.ts` | `playlist`, `reference`, `songRef`, `presets`, `tasks`, `libraryVersions`, `alternatives`, `engines`, `takes`, `warpPoints`, `trackColor`, `placementId`, `curveVocab`, `builtinParams` (generated). |

## Choices

**Strict objects.** Almost every schema is `.strict()`: an unknown key is a typo, and silently
ignoring `paramz:` costs more than refusing it. The exceptions are deliberate: `settings.k7x` is
loose; a chain's node item is `.passthrough()` because its module fields are vetted by `parseModule`
after `NODE_ITEM_KEYS` are stripped; `plugin:` is a loose object because the plugin's shape is the
engine's business. Fields that are dead but still found on disk (`modules_from`, `video`, `type`) are
accepted and ignored rather than removed, because a strict refusal would make an old project
unopenable.

**Refinements, not unions, where the editor is mid-edit.** A widget's per-type fields are checked in a
`superRefine` over one flat object: a union would reject the half second where the user changed
`type` but not yet the fields, and would report a typo as a bare "Invalid input" with no line.
Transitions are widened with `z.preprocess` (`1` → `{ over: 1 }`) for the same reason.

**Cross-file references are not checked here.** `extends`, a placement's `ref`, a rack's `clip`
need the rest of the project; they are checked by `@studio/song` at compile time.

**Every parser takes k7x text.** A string in k7x is always quoted, so `5:1:1` and `on` stay
strings with no rule of their own, and a finding is placed on the text's own lines by key path
(`k7xPosition.ts`).

**Defaults applied on the way out, never into the file.** The module reader sets `type: "plugin"` on the
value it returns only when the module says neither `type`, `io` nor `extends`, so an inherited type is
never overridden and the file stays as terse as written.

## Validation: one router, in Rust

`validateFile(path, text)` from `@studio/k7-wasm/validate` calls the `file.validate` op, which runs
`apps/k7-daw-wasm/song/src/files/validate.rs`. It answers whether the compiler could read `text` at `path`:

1. A path that is not `.k7x` is not this router's business: `depth: "none"`.
2. k7x syntax first; a syntax error makes the result `malformed`.
3. `family_of(path)` (`apps/k7-daw-wasm/song/src/files/route.rs`) chooses the reader by folder and basename:
   presets, then `modules`/`_modules` (`ui`, `controller`, `config`, else a module), `nodes/`, `tracks/`,
   `chains/`, `clips/`, `sequences/`, `curves/`, `timelines/`, `views/mix`, then the root `chains`,
   `project`, `settings`, `mix`. Unknown paths get `depth: "syntax"`.

The result is `{ ok, issues, depth, malformed }`, each issue `{ path, message, line?, col? }` with the
columns in UTF-16 units; only the first finding is reported. `firstIssue` phrases one for a status line.
The editor's autosave (`apps/k7-daw-frontend/packages/ui-editor/src/autosave.ts`) holds a malformed buffer,
and holds a schema-failing one when the file on disk was valid.

## The field vocabulary is derived

`apps/k7-daw-wasm/song/src/views/fields.rs` reads the keys the decoders' own derives name, per file kind;
`fields.kind(path)` names the kind, `fields.at` and `fields.values` answer the keys and the closed value
sets. Adding a field to a decoder makes the editor offer it; there is no second list to keep in step.
The exceptions are the hand-dispatched decoders (the widget, a chain's items, the curves), whose keys are
named in that file and held against the decoders by its tests.

## Timecodes

`@studio/k7-wasm/timecode`, over the Rust `apps/k7-daw-wasm/song-schema/src/timecode.rs`: `parsePosition` (1-based) and `parseDuration` (0-based) turn
`bar:beat:half:quarter.ticks` into quarter-note beats for a given metre; `beatsToPosition` and
`beatsToDuration` go back, rounding once in ticks (`TICKS_PER_QUARTER = 960`) and dropping trailing
fields at their origin. Chains longer than four fields fall back to the deep rule (each level a
quarter of the previous).

## Tests

`npm test` in the package runs vitest over the `*.test.ts` beside each module. The k7x writers are
tested on literal texts (an edit must keep comments and every other byte); `validate` behaviour is covered
in `apps/k7-daw-wasm/song/tests/all/` beside the loader.
