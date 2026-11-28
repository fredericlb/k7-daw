# The golden corpus

What the compiler made of 174 projects, committed and frozen under `apps/k7-daw-backend/fixtures/golden/`.
The files are recorded behaviour: `doc_golden.rs` decodes every `engine` document into the Rust `ProjectDoc`, and
`serviceGolden.spec.ts` compiles the same projects on a running service, renders them and
judges what they sound like. The native compiler is not held to the files byte for byte.

| source | projects | what they are |
|---|---|---|
| `demos/` | 5 | the bundled demos the file service mirrors into every account's Examples |
| `e2e/` | 32 | every `e2e/fixtures/*.k7`, the two broken ones included: their errors are the output |
| `k7-core/` | 132 | one project per k7-core module, with one lane per engine it offers (301 module refs in all) |
| `shapes/` | 5 | each archetype (`arrangement`, `mix`, `video`, `live`, `live-video`) as "New project" writes it |

k7-core itself holds no project: it is modules, chains, an assembly and their files. A module is
compiled by a project that uses it, so the corpus holds one per module.

## What each file holds

`<source>/<project>.json`, one per project, in this key order:

| key | what | from |
|---|---|---|
| `repairs` | what the open-time repairs logged, and which files they rewrote | the open-time repairs |
| `engine` | the `ProjectDoc` the engine receives, or `null` when the compile throws | `/project-doc`, with the group mix folded in as the engine's client folds it |
| `errors` | the compile's error, or a clean compile's non-blocking diagnostics | `/project-errors` |
| `freezes` | the automatic freezes the compile found | the compile's freezes |
| `graph` | the routing graph the Chains tab draws | `/graph` |
| `video` | for a project with a picture: the `VideoProjectDoc` and its warnings | the video document and its errors |

## The choices

| choice | why |
|---|---|
| the outputs are what the service sends the engine for a project opened as the file service opens one | pinning the compilers directly would record what they do, not what the service sends |
| the waveform prewarm is left out | it runs in the background, decodes audio and writes caches; it changes no document |
| the key order is the compiler's own, never sorted | the engine receives the document as written, so the order is part of the bytes |
| one file per project, one key per line, a leaf container on one line | a drift then reads as a diff naming the project. A container of at most 160 characters, a flat object of at most 240 and an array of scalars (wrapped at 160) stay on one line, which cuts the tree from 290k lines to under 90k; `scripts/json-fixtures-format.mjs` writes that shape for every file in `apps/k7-daw-backend/fixtures/` (`--check` reports a file that is not in it) and keeps every literal as written. The tree is 10 MB, mostly the graph's module panels; git stores it compressed |
| the files sit in `apps/k7-daw-backend/fixtures/` | beside the other vectors the Rust side reads, so a crate test needs nothing outside `apps/k7-daw-backend` |
| k7-core is staged as a real folder: its data families (`samples`, `models`, `irs`, `wavetables`) joined by junction, the rest copied | the library walk skips a top-level symlink; a compile writes caches beside a module, which must never land in the repo; 50 MB of audio and models is not worth copying. A junction needs no Developer Mode on Windows |
| every `jsdsp.json` stays in the staged tree | no host builds a `jsdsp.ts`, so that file IS the graph played for its module |
| a module's lanes play a clip (`kick.wav` from `clip-lane.k7`), notes only where the module needs notes | a synth before every effect would put its panel in nearly every graph |
| the staging points `$STUDIO_HOME`, `$STUDIO_LIBRARIES_HOME` and `$STUDIO_MEDIATEK` inside its tree | nothing of the machine leaks in: no engine scan (`exts.json`), no instance data folder, no library pins |

## Normalisations

The recorded files carry placeholders, so they read the same on every machine:

| what | is written as |
|---|---|
| the staged workspaces | `<ws>` (`<ws>/e2e/clip-lane.k7/clips/kick.wav`) |
| the library, and the repo's `k7-core` it resolves to | `<libs>/k7-core` |
| `$STUDIO_HOME`, the absent instance data folder, the temporary folder, the repo | `<home>`, `<mediatek>`, `<tmp>`, `<repo>` |
| `\` in a string that held one of those paths | `/`, so Windows reads the same file |
| an id a scaffold or an open repair minted at random | `00000000-0000-4000-8000-<n>`, numbered in order of appearance, files sorted |

Nothing else is touched: no timestamp, no hash, no random value reaches an output.

## DSP modules

22 projects compile DSP modules (the k7-core effects, the demos' `crossfade`). Their graphs
are in the documents: the recorded documents play the `jsdsp.json`
beside each, and the song crate's `dsp_literal` test holds every `dsp.k7x` to the graph the builder
prints from its `jsdsp.ts`. A module that cannot play would compile to a pass-through graph and an error
diagnostic in `errors` (`graphModel.md`); none of the 22 does.

## Frozen

The files are never regenerated: the TypeScript that wrote them is gone, and a generator over deleted code cannot run.
A change of the compiler that moves an output is judged by the song crate's tests and the conformance suite, and an expected change
is edited in the file by hand.

## What it does not cover

- **Freezes with a render**: no render is on disk and no plugin is scanned, so every `freezes` is
  empty and every VST plays live. A scanned machine compiles differently.
- **Pixels**: the `VideoProjectDoc` is pinned, not what `@studio/ui-eyes` draws from it.
- **What a session adds on the way out**: a take's hole, an auditioned timeline, a document a
  browser built (`clientDocs`), the `session` field of the frame.
- **The browser's views** of the same compile (`webProjectOf`, `WebVideoProject`), the hints, the
  plugin needs.
- **k7-core's chains and its assembly**, which compile through `resolveChain` and an assembly's
  inlining, not as projects; a library pinned at a version.
- **The `jsdsp.json` cache**: no host builds it any more; a corrupt one is an error on its module.
- **Imports** (`.dawproject`, `.als`) and the single-file `.k7x` song.
