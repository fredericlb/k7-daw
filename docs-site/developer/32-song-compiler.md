# The compiler: packages/song

`packages/song` reads a project folder: the files, the module vocabulary, the lanes and the graph the pages and the verbs read. The compile into the `ProjectDoc` the engine plays is the Rust `song` crate (`packages/song/rust`), run natively by the front and as WebAssembly in the verbs' Worker. The package also owns everything that walks a disk around a project: the shared `_modules/` folder, the
account's libraries, surfaces, assemblies, the archetype scaffolds, and the migrations between file
layouts. It reads; changing a project is `@studio/song-edit`'s job ([song-edit](./33-song-edit.md)).

## Entry points

| Function | File | What it does |
|---|---|---|
| `loadGraph(dir)`, `parseChains`, `checkChainInputs` | `packages/song/src/graph.ts` | The derived chain graph, its validation, and the port type checks. |
| `loadModules`, `loadModuleRefs`, `loadChains`, `loadLanes`, `loadVideoTracks`, `loadProjectRoot`, `loadSettings`, `exportSub`, `readYamlEntities` | `packages/song/src/loader.ts` | Reading each file kind off disk, validated or not. |
| `resolveModule`, `resolveModuleMap` | `packages/song/src/moduleResolve.ts` | Inheritance. |
| `projectSurfaces`, `projectAssemblies` | `packages/song/src/surfaces.ts`, `assemblies.ts` | The optional `surfaces/` and `assemblies/` folders. |
| `scaffold`, `setProjectFiles`, the live scaffolds | `packages/song/src/scaffold.ts`, `scaffoldSet.ts`, `scaffoldLive.ts` | New projects per archetype. |

The output types are the wire's, in `packages/engine-protocol/project.ts`: `ProjectDoc` (tempo,
metre, `masterVolume`, `masterPlugins`, `recordDir`, `tracks`, `groups`, `markers`, `settings`), with
`TrackDoc`, `ClipDoc`, `NoteDoc`, `PluginDoc`, `SendDoc`, `AutomationDoc`. `project.ts` re-exports
them so a producer imports one module. The graph the browser's Chains tab draws is a different
document, a `WebGraph`, built by the `song` crate's graph, answered by `GET /graph` (`apps/k7/backend-rs/crates/k7-front/src/routes/files/service/native/docs.rs`); it never
reaches the engine.

## Two layouts, one compile

A project's routing is written either as segments (`graph.k7x` + `nodes/<name>.k7x`) or as chain
files (`chains/*.k7x` + `tracks/` + `vstpresets/` + `layout.k7x`). `isChainProject(dir)` in
`packages/song/src/chainProject.ts` decides by the presence of `chains/`.

The compiler reads only segments. `loadChains` turns a chain project into segments
(`readChainProject` → `toSegments`), so everything downstream — chain derivation, port checks,
plugin ordering — has one input shape. In memory a chain-project node is known by its lane name or
its `id`. Link levels, which segments cannot carry, travel apart (`loadLinkLevels`, `applyLinkLevels`).
Writes go the other way: a graph edit is a transform over segments, written back as chain files by
`writeChainProject`; the two directions must round-trip.

New `arrangement`, `video` and `live` projects are scaffolded as chain files directly
(`packages/song-edit/rust/src/scaffold_files.rs`). `assemblies/` and `views/` name nodes, and a
surface with a `mapping` binds node params — which is why a `mix` set stays in the graph layout.

## The compile

The compile is the Rust song crate's, and the one compiler: `packages/song/rust/src/doc/` builds the `ProjectDoc`,
`views/` the pages' answers. Its pages are `packages/song/docs/rustCompiler.md` and the sub-step pages beside it. What
the TypeScript here keeps is the read side the verbs and the pages call: the module vocabulary and its inheritance
(below), the lanes, the derived chain graph and the timelines' lengths.

## Inheritance

`resolveModule` in `packages/song/src/moduleResolve.ts` folds `extends` deepest-first (cycle-guarded by
ref) with `mergeAuthored`:

- the child's defined keys win over the parent's;
- `params` and `assets` merge per key; `rack` merges per `symbols` entry, `ui` per `widgets` entry;
  `engines` per engine then per param;
- lists are scalars to the merge and replace whole (`automations: []` drops the inherited ones);
- `OWN_ONLY` keys (`kind`, `inputs`, `category`, `name`, `file`, `node`, `home`, `presetRel`,
  `chain`, `nodeId`) are never inherited.

After the fold, a chosen **alternative** is applied once (`alternatives.ts`); an alternative with its
own `extends` swaps the module, except on a lane or a mixer strip (`keepsItsModule`). Then a module
with **engines** runs as the one that is on (`engines.ts`, `asEngine`), its per-engine params applied.
File-valued params and a `base:` named by the child are re-resolved against the whole ancestry's
folders, nearest first (`withInheritedFiles`, `basePreset.ts`, `libraryFiles.ts`).

Panels come from the `ui.k7x` beside each module, attached by ref in `applyUiLibrary`
(`packages/song/src/loader.ts`); `packages/song/src/uiLibrary.ts` holds the panel lookup helpers.

## Lanes

A lane is a node of one of three built-in modules (`packages/song/src/trackModule.ts`), none with a
file: `@meta/track`, `@meta/strudel-track` (generated), `@meta/group-track` (no outputs). A lane has
no input and two outputs, `audio` and `midi`, and is not typed by its signal; `laneOf` is the one
projection every surface reads. Strudel lanes are compiled by the song crate
into a `studio_pattern` plugin carrying the pattern's source, the lane's label, the labels its file's
other lanes claim, whether it is the catch-all (the file's first lane by name), and the sample banks
(`samples/`, `$DIRT_SAMPLES`, `$STRUDEL_SAMPLES`). See [song-strudel](./35-song-strudel.md).

## Diagnostics

Two kinds of failure, by design:

- **Thrown errors** from the compile (`invalid module(s)`, `invalid chains`, `unresolved reference(s)`,
  `invalid video track(s)`): the project does not load. The message is the user interface in a
  text-first tool, so each names the file, the node and the reason.
- **`Diagnostic`** records `{ file, line, col, message, severity }` (`loader.ts`) for what can be
  reported without stopping: library modules that would not read (`libraryModuleIssues`), dead
  unreadable surfaces and assemblies. The file service serves them on `GET /project-errors`, and the editor
  draws them inline.

A missing module in a chain is not fatal: it compiles to nothing, like a bypassed step, and the graph
draws a `?`. A missing `extends` target is.

## Caching

Parsed files are cached against `(mtime, size)` once the mtime is old enough that a later write could
not reproduce it (`packages/song/src/yaml/yamlCache.ts`, `settledParse`); module trees likewise
(`moduleCache.ts`). A file is read and edited as k7x through `packages/song/src/yaml/workspaceDoc.ts`
(`wsText`, `wsData`, `wsEdit`…), plain bytes through `workspaceFile.ts` — see [k7x](./34-k7x.md).

## Tests

- `packages/song/src/testSupport.ts` holds the helpers that write a temporary project.
- `scaffold.test.ts`, `scaffoldLive.test.ts` scaffold each archetype and read it back.
- The compile's own tests are the song crate's (`packages/song/rust/tests/all/`); the verbs' tests compile a project
  through it with `packages/commands/src/runtime/testing/compileDisk.ts`.
