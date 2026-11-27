# k7x — the plan

Every file of a workspace is a `.k7x` file: data written in a syntax that reads like Strudel and is
never evaluated. The language and why it is shaped that way:
[apps/k7-daw-frontend/packages/k7x/docs/language.md](../apps/k7-daw-frontend/packages/k7x/docs/language.md).

## What "every text file" covers

The files a person writes and the UI edits: project, graph, modules, nodes, sequences, curves,
surfaces, chains, assemblies, playlists, ui panels, and a library's pins and manifest. Out of scope,
each for its own reason:

| files | why they stay |
|---|---|
| `.strudel` | code the engine runs; k7x borrows its look precisely so these need not change |
| `jsdsp.ts`, `.glsl` | code, in the language its compiler reads |
| `.json` | amp-model weights and generated `jsdsp.json` — written by tools, not by hand |
| `.md` | prose |
| the media library's store (`song.yaml`, its playlists, `mediatek.yaml`) | not a workspace: a volume of its own |
| `env.yaml`, `commands.yaml`, `latest.yml`, a machine's `preferences.yaml` | the repository's and the machine's configuration, read by name, never edited as a workspace file |

## Measured scope

- No C++ or Kotlin reads a workspace file: the engine receives a compiled `ProjectDoc`. The change
  is TypeScript-only.
- 1,272 `.k7x` files in `demos/` and `k7-core/`, the corpus every reader and writer is held to.

## Phases

1. **The core** — `@studio/k7x`: grammar, span-carrying tree, canonical printer, a document whose
   edits keep every other byte. Proven on the whole corpus against the Rust reader, a print/parse
   round trip and the comment lines every edit must keep (`apps/k7-daw-frontend/packages/k7x/src/corpus.test.ts`). Done.
2. **One door for reading and writing a file** — `apps/k7-daw-wasm/song-edit/src/workspace_doc.rs`: a file is
   read as k7x text or data, edited as a k7x document by key path, or written whole from data. A
   writer takes a document scope and changes values in it, so a knob drag is a one-line diff and an
   author's comments stay (`apps/k7-daw-frontend/packages/song/docs/k7xFiles.md`). Done.
3. **Reading `.k7x` only** — wherever a kind is looked up by name, the name is `.k7x`; a folder
   listing, the revision token and the browser's path rules know no other. The file service refuses
   to save a YAML name into a workspace. Every demo
   compiles and takes a run of commands (`apps/k7-daw-wasm/verbs/tests/project.rs`). Done.
4. **history over k7x** — a change is a `k7x` op: the same key paths and natural list keys
   (`apps/k7-daw-backend/packages/k7-sync/src/k7x/locate.rs`), diffed on the k7x tree, replayed byte for byte, checked
   and set aside by the room when it does not replay. A replica
   on an older protocol is refused at hello and told to update. Done.
5. **The editor** — a `.k7x` file has its own colours (`apps/k7-daw-frontend/packages/ui-editor/src/k7x/language.ts`),
   completion from the zod-derived vocabulary (`apps/k7-daw-frontend/packages/ui-editor/docs/completion.md`), entity links
   and the preview gutter on the k7x tree (`apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md`), and the same check
   before autosave, its findings placed on the k7x text's own lines. Done.
6. **Every stored workspace is k7x** — `demos/` and `k7-core/` are `.k7x`, and so is every workspace
   the server and the devices hold: a build reads no other format, so none is left to convert at
   run time. The one exception is a library's pins and manifest, which a folder older than the
   format may still hold as `.yaml`: their I/O owner upgrades one once, removing the `.yaml` only
   after the `.k7x` reads back equal. Done.
7. **YAML gone from workspaces** — no workspace code imports `yaml`. A migration whose only input
   was a pre-k7x file has no input left and does not exist; the tests write their fixtures as k7x
   and assert on data or k7x text. Done.
8. **Sugar, kind by kind** — `notes("c4 d4 ~ e4")` for a sequence's notes, `runs([[…]])` for a
   graph's segments (a chain cannot repeat `.to(…)`, a key being unique in a map). Expanded before
   any reader sees the file; kept through an edit that leaves its expansion alone, written as the full
   data by one that changes it (`apps/k7-daw-frontend/packages/song-schema/docs/sugar.md`). Done.

## The hazards

- **A device on an older build** reads YAML and cannot open a k7x workspace, and its live-sync is
  refused at hello. The answer is the update it is told to take, not a second format.
- **A stray `.yaml` in a workspace folder** — copied in by hand — is never read as a workspace
  file. It is the user's file, so nothing deletes it either.
- **A writer that rewrites a whole file** loses its author's layout and comments. The demo commands
  and the corpus test are the guard: a new writer edits values in the existing document.
