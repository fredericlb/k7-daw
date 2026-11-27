# song-schema: the file format

Every file a project is made of is read by a Rust decoder: `apps/k7-daw-wasm/song` holds the readers, the module
vocabulary and the views, `apps/k7-daw-wasm/song-schema` the small value rules (timecodes, curves, stems, aspects),
and `apps/k7-daw-wasm/song-edit` the writers. The page reaches them through the one WebAssembly module
(`apps/k7-daw-wasm/k7-wasm`), as `@studio/k7-wasm/*`; nothing of the file format is written twice. Their pages
are the `docs/` of each crate.

## Layout

| Path | Contents |
|---|---|
| `apps/k7-daw-wasm/song/src/files/` | The per-kind readers: `project.rs` (the root, its two tiers, the archetypes), `placement.rs` (one event, a sound or a shot), `entity.rs` (clips, sequences, curves), `desk.rs` (the cue sheet and `views/mix.k7x`), `preset_file.rs`, `timeline.rs`. |
| `apps/k7-daw-wasm/song/src/modules/` | `schema.rs` (every field a module, node or lane may carry), `ui.rs` and `panels.rs` (the panel and the controller beside it), `type_table.rs` and `types.rs` (the module vocabulary: type folders and their default ports, built-in ends, namespaces, refs). |
| `apps/k7-daw-wasm/song/src/routing/graph_file.rs` | `graph.k7x`: `modules`, `bypassed`, `bus_only`, `tap_feeds`, `positions`. |
| `apps/k7-daw-wasm/song/src/views/surfaces.rs` | `surfaces/<name>.k7x`: the type and layout pair, the playlist, the mapping. |
| `apps/k7-daw-wasm/song/src/wire.rs` | The small shapes a route answers that the page reads (`ProjectShape`, `NodePresets`, `ReferenceTrack`), and the words they are spelled from. |
| `apps/k7-daw-wasm/song-edit/src/` writers (`sequence_file.rs`, `picture_file.rs`, `playlist_edit.rs`, `mix_cues.rs`, `library_pins.rs`, `assembly_param.rs`, `format_chain.rs`) | Text editors per kind, reached as `text.*` ops (`apps/k7-daw-frontend/packages/k7-wasm/src/text.ts`). |
| `apps/k7-daw-wasm/song/src/files/validate.rs`, `route.rs` | The editor's check: path to the compile's own reader, reached as `file.validate`. |
| `apps/k7-daw-wasm/song/src/views/fields.rs` | The editor's completion vocabulary, derived from the readers' own decoders, reached as `fields.*`. |
| `apps/k7-daw-wasm/song-edit/src/sugar.rs` | The short forms, reached as `text.sugar_expand` and `text.sugar_apply`. |
| `apps/k7-daw-wasm/song-schema/src/keyframes.rs` | Shot animation keys and their evaluation. |
| `apps/k7-daw-frontend/packages/k7-wasm/src/consts.generated.ts`, `src/types/` | What the page reads of them: the constants printed from the crates, and the types ts-rs emits. |

## Choices

**Strict objects.** Almost every reader refuses an unknown key: an unknown key is a typo, and silently
ignoring `paramz:` costs more than refusing it. The exceptions are deliberate: `settings.k7x` is
loose; `plugin:` is a loose object because the plugin's shape is the engine's business. Fields that are
dead but still found on disk (`modules_from`, `video`, `type`) are accepted and ignored rather than
removed, because a strict refusal would make an old project unopenable.

**Refinements, not unions, where the editor is mid-edit.** A widget's per-type fields are checked after the
widget is read as one flat struct: a union would reject the half second where the user changed `type` but not
yet the fields, and would report a typo as a bare "Invalid input" with no line.

**Cross-file references are not checked here.** `extends`, a placement's `ref`, a rack's `clip`
need the rest of the project; they are checked by the compile.

**Every reader takes k7x text.** A string in k7x is always quoted, so `5:1:1` and `on` stay
strings with no rule of their own, and a finding is placed on the text's own lines by key path.

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
columns in UTF-16 units; only the first finding is reported. `firstIssue` phrases one for a status line;
`validatePanel` asks the same of a panel's text.
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

`cargo test` in each crate holds the readers and the writers: the writers on literal texts (an edit must keep
comments and every other byte), `validate` beside the loader in `apps/k7-daw-wasm/song/tests/all/`. What the
page sees through the module is `npm test -w @studio/k7-wasm`, where `validateModule.test.ts` judges a module,
its panel and a lane's events with the readers the compile uses.
