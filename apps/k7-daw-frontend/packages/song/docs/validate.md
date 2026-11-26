# Is this text a file the compiler can read?

`file.validate` answers that about a **buffer**, for any file a project is made of. It exists
because the editors autosave
([apps/k7-daw-frontend/packages/ui-editor/docs/autosave.md](../../ui-editor/docs/autosave.md)) and something has to
decide whether the text in the pane is worth writing. The router is `apps/k7-daw-wasm/song/src/files/route.rs` and
`apps/k7-daw-wasm/song/src/files/validate.rs`; the page reaches it as `validateFile` from `@studio/k7-wasm/validate`.

## Choices

| choice | why |
|---|---|
| the judge is the decoder the compile reads the file with, one reader per kind | an editor and a compile cannot disagree about whether a file is readable; a second schema written beside the editor would |
| a kind with no decoder of the compile's own gets a small one in the same crate (the cue sheet, the desk's bindings, a preset, a module folder's config) | the router must not fall back to a TypeScript schema for the three files the compile never reads |
| the answer is the first finding only | the decoder stops where it cannot read; a list of every problem would need a second, recovering reader |
| a path is read by FOLDER and basename, as the editor's completions read it (`fields.kind`) | one rule decides both what to offer and what to check, so a file is never completed as one kind and judged as another |
| `views/` is tested before the root files | `views/mix.k7x` and the root `mix.k7x` share a base and have nothing to do with each other |
| `_modules` matches as `modules` does | a shared library holds the same files, and editing one is what forks it |
| a file this does not recognise is `ok` | silence is not a verdict, and an autosave that refused what it had no opinion about would refuse a library's own files |
| `malformed` is separate from `ok` | "does not parse" and "parses but says something the reader refuses" are different answers to a caller deciding whether to write, and autosave treats them differently |
| a project root above this build's format level is refused from its text before any parse | the message says "update k7" even when the rest of the file is a syntax this build cannot read |

## What a path is read as

| path | read as |
|---|---|
| `modules/**/ui.k7x` | the panel |
| `_presets/**/*.k7x`, `<library>/presets/**/*.k7x` | a preset, checked before `modules` whatever the module path holds ([userPresets.md](userPresets.md)) |
| `modules/**/presets.k7x`, `modules/**/dsp.k7x` | nothing: a reserved name no editor reader judges |
| `modules/**/controller.k7x` | the MIDI map |
| `modules/**/config.k7x` | a library's namespace |
| `modules/**/*.k7x`, anything else | a module, generic or node |
| `nodes/*.k7x`, `tracks/*.k7x` | a module: the project's flat nodes folder, LANES included ([trackModules.md](trackModules.md)) |
| `chains/<name>.k7x`, `chains/**/graph.k7x`, `chains.k7x` | a chain file, a segments file, a segments file |
| `clips/`, `sequences/`, `curves/`, `timelines/` | that kind's file |
| `project.k7x`, `settings.k7x`, `mix.k7x`, `views/mix.k7x` | the root files |
| any other `.k7x` | parsed, not judged |
| anything not k7x | not this check's business |

## Three depths

`Validity` says how far the check could go: `schema` (parsed and read by the kind's decoder: `ok` means
the compile would accept it), `syntax` (parsed; no reader claims this path), `none` (not k7x at all).

## Where the line and column come from

The decoder's error carries a byte offset; the check places it on the key it is about, at that
key's line and its column in UTF-16 units, the way CodeMirror counts, and names the dotted key
path. A key a decoder finds missing is named at the object that lacks it. That is what lets an
editor underline the reason it is not saving, instead of only naming it in a toolbar.
