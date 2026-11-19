# The compiler in Rust

The compiler (and the `@studio/song-schema` readers it stands on) is one Rust crate, linked natively by the backend and
compiled to WebAssembly for the browser. The engine's `ProjectDoc`, the file service's
routes and answers, and the frontend's calls and types are its contract; the legacy syntaxes are not.

Paths below without a top folder are relative to `packages/song` (`rust/src/source.rs` is the crate's).

## What stays a contract

| contract | why |
|---|---|
| the `ProjectDoc` of `packages/engine-protocol/project.ts`, field for field | the C++ engine plays it; the Rust type is its twin and a test holds them equal |
| every compile route's answer (`/project-doc`, `/graph`, `/hints`, `/project-errors`, `/surfaces`, `/io-ports`, `/module-catalogue`, `/base-modules`, `/engine-doc`) in the shapes of `@studio/web-protocol` and `@studio/graph-model` | the frontend reads them; a difference is listed under "Where it answers differently", or it is a bug |
| the frontend's types stay the hand-written TypeScript while the routes answer them; ts-rs derives the Rust side's twins behind the `ts` feature, and a type test holds each assignable | the browser wiring lands later; until then no frontend import changes |
| the native file service calls the crate directly, with no host function between | one compiler, one cache per workspace, no copy of a request in a runtime's string |

## The crate

| choice | why |
|---|---|
| ONE crate, `song`, in `packages/song/rust`, its own Cargo workspace and lock, `rlib` only | the backend links it natively and the browser facade `@studio/k7-wasm` links it into the one module (`packages/song-schema/docs/wasmBoundary.md`); one source is the point of the port |
| it holds the readers of the file formats too (`rust/src/files/`), though their TypeScript is song-schema's | every change to a crate the facade links forces the checked-in module to be rebuilt; song-schema's crate stays the value vocabulary the facade already serves (the timecode, the curves), and `song` depends on it |
| no FFI: no `extern "C"`, no `#[unsafe(no_mangle)]`, no `thread_local!` buffer | the wasmBoundary rule: the facade owns the ABI, the native file service calls the crate as a library |
| features: `disk` (default) holds the disk `Source`; `ts` derives the TypeScript twins | the browser build takes `default-features = false` and so compiles no file system code; the native backend never compiles ts-rs |
| dependencies: `song-schema` and `k7x` (path), `serde`, `serde_json` with `preserve_order` and `float_roundtrip`, `indexmap`, `sha2`; `ts-rs` optional; `insta` (feature `json`) for tests only | all approved (`serde`, `serde_json`, `ts-rs` for the browser crates; `indexmap`, `sha2` for the backend; `insta` for this crate's tests); the two features are the backend's own, so a `Value` keeps the authored key order in the browser as in the backend |
| `schemars` is approved for the editor's field vocabulary (2j) and not a dependency before it: it derives a JSON Schema from the serde structs the decoder reads, where `fields.ts` introspects zod | one declaration of a file's shape then serves the decoder and the editor; the derive goes behind a feature that is off by default and must build clean for wasm32 |
| no regex, no YAML, no hashing crate beyond `sha2`: a timecode or a file extension is matched by hand, a stamp's FNV-1a is ten lines | none is on the approved list, and none earns one: the compile reads k7x only (below, "What the compile reads") |
| `k7x` is linked as it is, its `ffi` module and `bumpalo` with it | the reader is the one the backend already links; whether the facade gates `k7x`'s four `k7x_*` exports behind a feature is decided when the facade first links `song` |
| `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]`, and `cargo clippy --target wasm32-unknown-unknown --no-default-features` clean | a panic in WebAssembly traps the module; in the backend it would take the account's session down |
| the compiler's types are `Send` (no `Rc`, no `RefCell` across calls) | the native file service holds them behind a lock |

## The Source

Everything the compile reads goes through one trait; nothing in the crate names `std::fs` outside
`rust/src/source/disk.rs`.

```rust
pub trait Source {
    fn stat(&self, path: &str) -> Result<Stat, SourceError>;        // follows links
    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError>;    // sorted by name, bytewise
    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError>;  // Span::All or a byte range
}
pub struct Stat { pub kind: Kind, pub len: u64, pub stamp: Stamp }   // Kind: File | Dir
pub struct Entry { pub name: String, pub kind: Kind }
pub enum SourceError { NotFound, Outside, Missing, Io(String) }
```

| choice | why |
|---|---|
| three calls, bytes out; text is a helper that refuses invalid UTF-8 | what the compile asks of a file system is small; a range read is what a WAV header needs, and nothing else reads a binary |
| a path is absolute and spelled with `/`, a drive letter kept (`C:/Users/…`); `..` and `.` are resolved lexically before any call | one spelling on every host, the memory bundle's keys included; the TypeScript's `path` port already reads `\` as `/` |
| a listing is sorted by name, bytewise, on every OS | a compile's answer then does not depend on the file system's order |
| `Outside`: a path that lies outside every mount, lexically or through a link, is refused by the Source itself | confinement (`docs/confine.md`) becomes a property of every read, not a check a caller may forget; the compile cannot reach another account's files whatever a song file names |
| `Missing` (memory only): the bundle does not hold this path, which is not the same as the path not existing | the browser then fetches a larger bundle instead of compiling a wrong answer |
| a `Stamp` is the disk's `(mtime in ns, len)`, and is `settled` only when that mtime is older than two seconds; the memory Source's is a generation number, always settled | an unsettled stamp is never a cache key: a write of the same size in the same tick would reproduce it (`docs/moduleCache.md`) |
| the disk Source checks the real path of each DIRECTORY once per instance, and one instance serves one compile | links are rare and directories few; a real-path check per file would open a handle per file on Windows |
| the memory Source (`MemSource`) is a map of paths to texts or to a length alone, plus the listings | the browser holds the files the server's compile read and nothing else (below, "The bundle") |
| a mount may name ONE file, which admits that path alone, lexically and by its real path (the engine's scan) | mounting the folder that holds `exts.json` would let any confined read reach the renders and the history |
| a Source may be handed a text the caller already holds (`note_text`, a provided method that does nothing by default), and a Recorder keeps it as if it had been read | a cache hit reads nothing, and a bundle made after one would hold lengths alone, which the browser can never decode |

## What the compile reads, and who gives it

Every input the TypeScript compile takes, read off its code (`src/compile/inputs.ts`, `src/sharedFolders.ts`,
`src/confine.ts`, `src/autoFreeze.ts`, `src/compile/jsDspGraph.ts`, `src/preferences.ts`, `src/engineCatalogue.ts`,
`src/enginePattern.ts`, `src/modelCache.ts`), and what it becomes:

| input | the TypeScript reads it | the crate takes it |
|---|---|---|
| the project's files: `project.k7x`, `settings.k7x`, `graph.k7x`, `chains/`, `modules/`, `nodes/`, `tracks/`, `clips/`, `sequences/`, `timelines/`, `curves/`, `surfaces/`, `assemblies/`, `mix.k7x`, the layout and link levels, `.strudel` patterns, `vstpresets/` | `node:fs`, through `yamlCache` and `parseStore` | through the Source, under the `project` root |
| the workspace's `_modules/` beside the project | `moduleLibraryDirs` | through the Source, under the `workspace` root |
| the libraries: which ones, at which version, and which an admin overlays | `librariesHome`, `workspacePins`, `setLibraryOverlays`, `libraryRoot` (pins, manifests, the legacy YAML ones upgraded) | `Roots::libraries`, a list of `{name, root, editable}` the HOST resolves; the crate never reads a pin or a manifest |
| each library's families (`modules`, `chains`, `assemblies`, `console`, `samples`, `irs`, `wavetables`, `models`) | `libraryFamilyRoots` | through the Source, under each library's root |
| the account's own library, `@account` | `accountLibraryHome` | the first entry of `Roots::libraries` |
| k7's own modules, `@meta` | `META_MODULES_ROOT` (`packages/song/meta`, by `import.meta.url`) | `Roots::meta`, a path the host gives (the code root on a phone) |
| the engine's plugin scan | `<studioHome>/exts.json`, `builtinScan()` | `Roots::scan`, a file mount read through the Source; the builtin list is the crate's, read from song-schema's generated table (`include_str!`) until the facade wires the compiler |
| the rig: channel labels, stereo pairs, the cue pair | `loadPreferences()`, `preferences.k7x` | `Request::rig`, JSON the host gives |
| the automatic freezes' renders | `<studioHome>/.renders`, a stat per render, an atime touched | `Roots::renders`, stats through the Source; the renders used are in the answer, and the HOST touches them |
| a small file the doc names, for a freeze's fingerprint | read and SHA-256'd when under 1 MB, memoised by mtime | read through the Source, hashed with `sha2`, cached by stamp |
| the models cache | `$STUDIO_MODELS` or `<state>/models` | `Roots::models`, stats through the Source |
| the pattern lanes' sample banks | `samples/`, `$DIRT_SAMPLES`, `$STRUDEL_SAMPLES` | `Roots::sample_banks`; the engine reads them, the compile only names them |
| a jsdsp module's graph | `jsdsp.json` when no older than `jsdsp.ts`, else a build in the jail, which writes it | `jsdsp.json` through the Source, always; a stale or missing one is a diagnostic, never a build (below) |
| an audio file's length, for the browser's lanes | `wavSeconds`, the WAV header, memoised | a range read of the header through the Source |
| the environment: `STUDIO_HOME`, `STUDIO_LIBRARIES_HOME`, `STUDIO_MODELS`, `DIRT_SAMPLES`, `STRUDEL_SAMPLES`, `STUDIO_PARSE_CACHE` | `process.env` | none: the host turns each into a root |

| choice | why |
|---|---|
| the host resolves every root (`Roots`), the crate only reads under them | where a library lives, which version a workspace pins and what an admin overlays are the file service's facts; in the browser they are the bundle's |
| the compile writes NOTHING: no `jsdsp.json`, no atime, no upgraded pins file, no parse cache | a pure compile is safe to run twice, in the browser, and on a read-only tree; each write the TypeScript made is the host's, named in the answer |
| a jsdsp module is never built by the compile: a `jsdsp.json` older than its `jsdsp.ts` plays and says `needs rebuilding`; a missing one passes through and says so | the jsdsp decision (the editor builds, the server validates the JSON graph) |
| a `jsdsp.json` is validated against the graph schema before it plays | the server never runs user code, and a graph the engine cannot read must fail in the compile with its file named |

## The caches

The crate's `Cache` is one value per workspace, owned by the host, passed to every compile.

| what | keyed by | replaces |
|---|---|---|
| a file's decoded data and its issues | its path and settled stamp | `yamlCache.ts`, `parseStore.ts` |
| the module vocabulary (the modules, refs and module folders of every root but the project, 2b; panels and MIDI maps join it at 2c) | its read set: the stamp of every file, the entries of every listing, every absent path, re-checked before reuse (2b decisions 25 to 27) | `moduleCache.ts`'s stamp walk, `stampWalk.ts`'s worker, k7-compile's `host/walk.rs` memo |
| a compile's whole answer | its request and the stamps of everything it read (its read set), re-stat'ed before reuse | a per-view memo kept beside it |
| a fingerprinted file's digest | its path and stamp | `autoFreeze.ts`' `digests` |

| choice | why |
|---|---|
| a compile records every stat, listing and read it made; reuse re-stats that set and nothing else | the read set is exactly what the answer depends on, where a revision token walks a fixed list of folders and the module stamp another |
| the cache holds owned, immutable values handed out as `Arc` | the two rules of `docs/moduleCache.md` (a cached read never leaves the loader mutable) hold by the type |
| bounded: four answers per workspace, the least recently used dropped; the parse cache dropped whole when it passes 64 MB | a long-lived process would otherwise grow with every project opened |
| no disk cache | the native parse is fast enough that a restart re-reads (`packages/k7x/docs/rust.md`'s measurements), and a disk cache is a write |

## The API

```rust
let mut cache = Cache::new();
let answer = compile(&source, &mut cache, &Request { roots, rig, vsts, want });
// answer: Compiled { engine: Result<ProjectDoc, Failure>, diagnostics, freezes, renders_used,
//                    jsdsp_stale, web, graph, hints, surfaces, io_ports, catalogue, base_modules, reads }
```

| choice | why |
|---|---|
| one call compiles a project and answers every view the request's `want` names | the views share the read and the vocabulary; the frontend fetches four of them after every command |
| a failure is `Failure { diagnostics, format_too_new }`, each diagnostic `{file, line, col, message, severity}`, `col` in UTF-16 units | the TypeScript threw a string and `workspaceDocs.ts` parsed it back with a regular expression; the editor's squiggles want a place, and CodeMirror counts UTF-16 |
| a decoding error lands on the deepest node being decoded: a custom serde Deserializer over k7x's tree, whose nodes carry spans, and each layer places its error | every finding is located, where zod's issues were located only for some files; serde reports the first finding of a file, not all of them |
| a union the TypeScript left to zod (`ClipSchema`) is dispatched by hand on what decides it (the source's extension) | an untagged serde enum would answer "data did not match any variant" |
| `vsts: scan \| none` | the automatic freezes need the engine's VSTs; a share's engine has none (`docs/autoFreeze.md`) |

| output | the frontend calls it today | will call it |
|---|---|---|
| `engine` (`ProjectDoc`) | never: the file service sends it to the engine (`/engine-doc` for the host) | never |
| `web` (`WebProject`), `diagnostics` (`ProjectErrors`) | `/project-doc`, `/project-errors` after every revision | in WebAssembly, over a draft, before it is saved (`song.compile`) |
| `graph` (`WebGraph`) | `/graph` | in WebAssembly, the graph of a draft |
| `hints`, the field vocabulary | `/hints`; `@studio/song-schema/yaml/fields` imported by the editor | in WebAssembly (`song.hints`) |
| a buffer's issues | `@studio/song-schema/yaml/validate` imported by autosave, per keystroke | in WebAssembly, `song.validate {path, text}`, debounced |
| `surfaces`, `io_ports`, `catalogue`, `base_modules` | their routes | their routes |

### The host

| choice | why |
|---|---|
| the file service holds the `Cache` per workspace (`SongSessions`, `apps/k7/backend-rs/crates/k7-file-service/src/song_ask.rs`) and calls the crate in-process | one cache per workspace, dropped with it |
| the file service computes `roots` and `rig` | they are its facts: where a library lives, what a workspace pins, which rig the preferences name |

### The bundle, for the browser

A `SourceBundle` is JSON: `{format, roots, files: {path: {text} | {len}}, lists: {dir: [entry]}, rig}`, made
by the server from the read set of its last compile of the project. The crate's `SourceBundle` is `{format, mounts, files, lists}` and a
`Recorder` makes it; 2l reconciles the two shapes (`roots`, `rig`) when the host function lands. The server's recorder adds the
listing of the parent of every `NotFound` probe, or the browser cannot tell an absent file from a missing one.

| choice | why |
|---|---|
| exactly the files the server's compile read, binaries as their length alone | the browser compiles what the server compiled, and downloads no audio |
| a draft is a path and a text laid over the bundle; a draft that makes the compile read beyond it answers `needs_files` | a new module ref is the one edit that widens the read set, and it is rare |
| `song.open {bundle}` gives a handle, `song.compile {handle, drafts, want}`, `song.close`; handles as the curves' (`packages/song-schema/docs/wasmBoundary.md`) | the bundle crosses once; a keystroke sends its draft |

## The compare

The crate's `compare.rs` judges two compiles of one project; the departures are the DEPARTS rows of these pages.

**Equal enough**, as `rust/src/compare.rs` decides it for two JSON values:

| rule | why |
|---|---|
| an object's keys compared as a set; an array in order | key order is not a contract; the order of tracks, plugins and clips is what the engine plays |
| a number equal within `1e-9` relative, or `1e-9` absolute near zero | the arithmetic is the same; only the last bits may differ |
| an automation lane compared as a function: both sampled at the union of their breakpoints, values within `1e-6` | a bake may place its breakpoints differently and play the same curve |
| a render's file name compared with its fingerprint masked (`<module>-#.wav`) | the fingerprint is the native JSON's hash, not the TypeScript's |
| a failure compared by the files it names, never by its sentences | the messages are rewritten |
| diagnostics, hints' lists and a graph's nodes and edges compared as sets, by their identity | their order was never decided |
| the departures below are an allow list in the same file, each named | a known difference that logs is noise that hides the next one |

## The version the browser and the backend share

| choice | why |
|---|---|
| the compiler's version is ONE hash over the `song` crate (manifest and `src/`), its path dependencies' (the same) and their `Cargo.lock` entries, computed by `packages/k7-wasm/scripts/rustSources.mjs` and checked in as `SOURCES` in `src/k7Wasm.generated.ts`; `build.test.ts`, the facade and the backend read that one value and compute no other | the checked-in module is proven to match its sources by the same number a server reports; a second hash would drift from it |
| the facade answers it through a `version` operation, and the backend reports the same value with every compile answer | a tab or an installed app can keep an older frontend, and its older module, while the server updates |
| the frontend compares the two when it connects and on any answer that carries a different hash; a mismatch reloads the app or asks the person to, and the browser's module compiles nothing meanwhile | two compilers that disagree would show different errors and graphs for one file |
| the module is requested at a URL keyed by the hash (`k7_<hash>.wasm`) | an installed app keeps the module it was built with, and a new one never replaces it under the same name |

## Where it answers differently, on purpose

| difference | why |
|---|---|
| a folder with no `graph.k7x`, `modules/` or `chains/` (the flat `tracks/` layout) and a single-file `.k7x` song are not projects: "not a k7 project" | the plan drops the legacy syntaxes; `folder.ts` and `readSongFile` exist for them alone |
| `project.k7x`'s `type` and `project_type` are unknown fields; a v1 wall-clock marker key is not told from a timecode (2a decisions 5 and 6) | the same |
| the legacy YAML pins and manifests are not read | the host resolves the roots; the TypeScript host keeps upgrading those files while it exists |
| messages are rewritten, and a file reports its first finding, located | serde's decoding (above) |
| every automatic freeze's fingerprint changes once, so every render is made again | the fingerprint is a hash of the native JSON |
| a stale `jsdsp.json` says so | the compile builds nothing |
| a curve file that does not decode is a located warning and costs that curve alone (2g decision 204) | the TypeScript loses it silently, and one syntax error loses every curve |

2e's to 2k's decisions are numbered on from this page's and kept in [rustCompilerLanes.md](rustCompilerLanes.md),
[rustCompilerClips.md](rustCompilerClips.md), [rustCompilerCurves.md](rustCompilerCurves.md), [rustCompilerDoc.md](rustCompilerDoc.md),
[rustCompilerPictures.md](rustCompilerPictures.md), [rustCompilerViews.md](rustCompilerViews.md) and [rustCompilerGraph.md](rustCompilerGraph.md).

## The order

Each sub-step is one Sonnet work package with its own gate, on the step's one branch. No sub-step before 2l is
wired to a route: what lands before it changes no answer. Sizes are the Rust and its tests, rough.

| sub-step | what moves | TypeScript | Rust | agent-days |
|---|---|---|---|---|
| 2a | the crate, the Source (disk and memory), the cache's file layer, k7x decoded into serde structs with located issues, the `notes` short form; clips, sequences, timelines, `project.k7x` (format level, markers, soloed, order), `settings.k7x`; `compare.rs`; the wasm32 build in CI | `src/entities.ts`' families, `src/yaml/`, song-schema's `entitySchemas.ts`, `placementSchema.ts`, `timelineSchema.ts`, `projectSchema.ts`, `formatLevel.ts`, `sugar.ts`' notes, `k7xIssues.ts`, `k7xPosition.ts`; `src/project.ts`' markers and settings | 2,000 | 1.5 |
| 2b | module files and the module tree: each root's `modules/`, a project's `nodes/` (a chain project's are 2d's), `engines/`, the library nodes merged under their module, the libraries by `Roots`, the namespaces, the engine scan; the vocabulary cache on a read set, which a reused vocabulary replays into the bundle | `src/moduleTree.ts`, `src/moduleRefs.ts`, `src/moduleSiblings.ts`, `src/engines.ts`, `src/engineCatalogue.ts`, `src/moduleCache.ts`' rules, `src/moduleResolve.ts`' `mergeAuthored`, song-schema's `moduleSchema.ts`, `moduleFile.ts`, `moduleTypes.ts`, `builtinParams.ts` | 2,300 | 1.5 |
| 2c | a module resolved: `extends`, the alternative, the engine it runs as, panels and their scripts, MIDI maps, rack audio, file params and models, presets, the per-compile `@project/` files, the not-supported and unknown-plugin findings | `src/moduleResolve.ts`, `src/alternatives.ts`, `src/engines.ts`' `asEngine`, `src/uiLibrary.ts`, `src/controller.ts`, `src/moduleAssetFiles.ts`, `src/assetPath.ts`, `src/libraryFiles.ts`' resolution, `src/basePreset.ts`' resolution, `src/moduleTree.ts`' decorations, `src/diagnostics.ts`' plugin findings; song-schema's `alternatives.ts`, `engines.ts`, `fileParams.ts`, the panel and MIDI-map schemas | 4,500 | 1.5 |
| 2d | the routing: `graph.k7x` and its `runs` short form, chain files and a chain project's nodes (its items, its lanes' `tracks/` files), segments read, subchains, runs, bypass, bus-only, link levels, taps | `src/chainsGraph.ts`, `src/chainSegments.ts` (`toSegments`), `src/chainScope.ts`, `src/chainProject.ts`, `src/graph.ts`' `parseChains`, `src/linkLevels.ts`, `src/tapOuts.ts`, `src/moduleTree.ts`' `loadChainNodes`, `src/moduleRefs.ts`' `withChainModuleRefs`, `src/entities.ts`' `loadChains`; song-schema's `yaml/graphFile.ts`, `chainFile.ts`, `chainSchema.ts`, `graphSchema.ts`, `sugar.ts`' `expandGraph` | 4,500 | 1.5 |
| 2e | lanes and who owns each chain; live inputs against the rig; assemblies; the console; the reference check | `src/trackModule.ts`, `src/compile/ownership.ts`, `src/compile/inputs.ts`, `src/assemblies.ts`, `src/console.ts`, `graphValidate.ts`' `checkChainInputs`, `graphTypes.ts`' `sourceType`, `inputLandingTrack`, `normalizeInputs`, `monitorMode.ts`, `loadVideoTracks`, the channel half of `preferences.ts`, song-schema's assembly files | 4,500 | 1 |
| 2f | placements to clips (windows, loops, fades, takes, warp, comps), timelines expanded, pattern lanes | `src/compile/clips.ts`, `src/compClips.ts`, `src/compile/timelines.ts`, `src/enginePattern.ts`, song-schema's `placementId.ts`, `timelineLane.ts`, and of `warpPoints.ts` and `takes.ts` `readClipWarp`, `compPieces`, `seamCrossfades` | 6,000 | 1.5 |
| 2g | curve files (the pilot's `CurveSpec`, on-off, cut and duration included, plus conditions), baked into automation | `src/curves.ts`, `src/curveBake.ts`, `src/curveConditions.ts`, `src/compile/plugins.ts`' `bakeAutomations`, song-schema's `CurveSchema`, `CurveConditionSchema`; `curveSample.ts` is the pilot's | 2,500 | 1 |
| 2h | the `ProjectDoc`: plugins, racks, sends, levels, freezes and automatic freezes, the highlight, the output recorder, jsdsp graphs validated; the whole compile | `src/nodeProject.ts`, `src/compile/plugins.ts`, `src/compile/freeze.ts`, `src/freeze.ts`, `src/autoFreeze.ts`, `src/compile/jsDspGraph.ts`, `src/highlight.ts`, `src/outputRecorder.ts`, `project.ts`, `ownMix`, `faustFiles`, `withConsole`, `trackKind`, `trackColor`, `surfaces`' head | 9,500 | 4 |
| 2i | a picture lane's sound, and a timeline's pictures | `src/videoSound.ts`, `src/timelinePictures.ts`' `timelineShots`, `nodeProject.ts`' picture lines; `loadTimelinePictures` is `song-video`'s, step 4 | 2,200 | 0.5 |
| 2j | the views: the errors, `WebProject` with the WAV lengths, surfaces, I/O ports, the catalogue, the base modules, hints and the field vocabulary | `apps/k7/backend-rs/crates/k7-file-service/src/engine/docs.rs`, `webProject.ts`, `webTracks.ts`, `webComp.ts`, `webNest.ts`; `src/surfaces.ts`, `src/ioPorts.ts`, `src/moduleCatalogue.ts`, `src/hints.ts`, `src/diagnostics.ts`, `src/deadAutomations.ts`, `compileError.ts`, `wavLength`, song-schema's `fields.ts`, the surface schema and view-binding files; the picture half of `workspaceDocs.ts` and `webTracks.ts` is step 4's | 6,000 | 2 |
| 2k | the module graph the Chains tab draws, its validation and warnings, a library chain's graph | `src/graph.ts`, `src/graphTypes.ts`, `src/graphValidate.ts`, `src/graphWarnings.ts`, `apps/k7/backend-rs/crates/k7-file-service/src/engine/docs.rs`, `graphChoices.ts`, `src/laneStrip.ts`' strip finder, `nodePlace`, `moduleAssetChoices`, `listFiles`, `libraryFiles`, `ampModel`, `missingModel`, `stageOf`, `panelWidgets` | 7,000 | 2 |
| 2l | the host function, the cache per workspace, the jsdsp refresh before a native compile, the renders touched, the shared version stamp | `apps/k7/backend-rs/crates/k7-file-service/src/workspaces/mod.rs`' compile half, `src/sharedFolders.ts`' roots | 1,200 | 1.5 |
| 2m | the golden corpus made a smoke test, the conformance and web suites, the step's merge | `e2e/specs/conformance/serviceGolden.spec.ts` | 600 | 1 |

| choice | why |
|---|---|
| bottom up, by the compile's own stages, each stage's output compared with the TypeScript's over the staged corpus | a stage's difference is found where it is made, not as a wrong plugin three stages later |
| 2a reads files and compiles nothing | it fixes the Source, the cache's layer, the decoding and the crate's shape that every later stage stands on, and its readers are the best-tested half of song-schema |
| the switch only in 2l, with the views all ported | one setting answers every view (above) |
| each sub-step's TypeScript stays until step 5 | the switch's off position runs it |

## The gates

| when | gate |
|---|---|
| every sub-step | `cargo fmt --check`, clippy `-D warnings` native and `--target wasm32-unknown-unknown --no-default-features`, the crate's tests; the TypeScript tests of the area ported as Rust tests, the behaviour cases (a case that pins a message's wording or a legacy form is dropped and listed) |
| every sub-step from 2a | the stage's dump: `rust/tests/all/stages.rs` compiles the staged golden corpus (`packages/song/scripts/stageCorpus.ts`) to the stage and compares, with `compare.rs`, against what a node script writes of the TypeScript's same stage; every difference is fixed or added to the allow list with its reason |
| 2h | every golden `engine` document decodes into the Rust `ProjectDoc` with unknown fields denied, and the corpus compiles equal enough to it |
| 2l | the backend's version stamp equals the facade's `version` answer, and a test that a changed `song` source changes both; the 5d conformance set and the web suite on a bed; `e2e macos` |
| 2m, the step's end | the corpus as a smoke test: every archetype, demo and e2e fixture compiles (the two broken ones fail naming their file), the engine loads each, and `--render` of four bars of each archetype and demo is not silent; `mac-checks.sh` |

The golden corpus is a smoke test: `serviceGolden` and the file service's `golden_smoke` test.

## The wasm32 build

| choice | why |
|---|---|
| from 2a, `mac-checks.sh` adds `wasm32-unknown-unknown` and runs `cargo build --locked --target wasm32-unknown-unknown --no-default-features` and its clippy in `packages/song/rust`, whenever the song crate changes | the browser's build must be clean before the browser uses it; macOS's present wasm build runs only where the target happens to be installed |
| the facade links `song` only when the first browser operation lands | every change to a linked crate forces the checked-in module's rebuild (`SOURCES`); an rlib built for wasm32 proves the same cleanness |
| `scripts/check-cargo-locks.mjs` and `docs/lints.md` hold the crate's lock | as the pilot's two crates |

## What the readers decide (2a)

Each row is numbered; a number is how a report and a test name it. Rows 1 to 4, 7 to 11, 13 and 14 depart from the order the
builder was given or from the spec's first draft, and say why.

| # | choice | why |
|---|---|---|
| 1 | a path is `/`-separated and a `\` is a character of a name, not a separator; a Windows real path is turned into the same spelling (verbatim prefix stripped, drive letter upper-cased); on Windows the disk Source refuses a path whose name holds a `\` or a `:` past the drive, as `Outside` | one spelling on every host; Linux allows `\` in a name, and Windows would read `..\..` or `name:stream` as more than a lexical check saw |
| 2 | the disk Source also checks a LEAF that is a link: its real path must lie in a mount, a dangling one is `Outside` | per-directory checks alone would read a file link that points out of every mount; one `lstat` per file is the price, a handle per file is not paid |
| 3 | the memory Source answers `NotFound` for a path when the NEAREST listing it holds above the path omits that branch (or a draft deleted it); every other path it does not hold is `Missing`; a draft in a folder known to be absent is listed, in a folder nothing is known about it is `Missing` | the compile probes files that are often absent (`settings.k7x`) and a bundle has no shape for "absent": without this the browser would ask for a larger bundle forever; the `Recorder` records that listing |
| 4 | a family folder is read in the Source's bytewise order; a repeated base name keeps the first position and the last file, and only that file is read; a `.k7x` extension is matched in any case, a bare `.k7x` is no document; a file that vanishes between listing and read is skipped; only regular files and folders are listed | `docFilesIn`'s rules; the values are an object, so the order is only the findings' |
| 5 | `project.k7x`'s `type` and `project_type` are unknown fields | the plan drops the legacy syntaxes; the TypeScript accepted them and the stage gate allows that difference by name |
| 6 | a v1 wall-clock marker key cannot be told from a timecode: every wall-clock shape is timecode-shaped, so only a key that is no timecode is refused | the schema accepted the superset by shape, and the position decides which markers play (`parseMarkers` skips what does not parse) |
| 7 | a clip is decoded as the kind its source's extension names, and a picture or caption source whose fields do not fit that kind is tried as audio | the TypeScript union takes the first member that parses, so `{source: x.mp4, from: 1}` is an audio clip there |
| 8 | `settings.k7x` is decoded by a hand visitor that keeps unknown keys in a `serde_json::Map` | `flatten` buffers the file and loses the node an error belongs to |
| 9 | a decoding error lands on the value being decoded, an unknown field on its key, a refinement (`to` after `from`, a lane named twice, an activation carrying a source key) on the key it is about; each layer of the Deserializer places what it raises, and serde's own constructors are overridden so a message names a k7x thing, never a Rust type | every finding is placed without a second walk of the tree, and no path crate is needed; the column counts UTF-16 units |
| 10 | the notes short form is decoded as a list, a string or `seq`, then expanded; nesting past 64 is refused, and what is laid (notes and steps visited) is counted as it is made and refused past 100,000; a note's octave is checked arithmetic | `!100` shares one step a hundred times, so a limit on what was written is bypassed three levels deep; a hostile string would otherwise exhaust memory or overflow, and either traps a module |
| 11 | a time signature that does not make a bar is a `compile_finding` of the head, placed at `project.timeSignature` (recorded on the `RootFile`), apart from the findings of the files' own reads | the TypeScript raises it in the compile, not in the root's read |
| 12 | the stage gate snapshots (insta) only the findings it saw and its allow list, not each project | 177 projects restate what the TypeScript's dumps already hold, and a snapshot of counts would break whenever a fixture is added |
| 13 | a repeated key in a file is a finding: the parser refuses it in an object, and a derived struct refuses it at the root; no decoder keeps "the last wins" | the TypeScript's last-wins was never a feature a file used on purpose |
| 14 | a file that is not UTF-8 is a finding for that file; a bundle holds it by its length, so the memory Source cannot decode it (`Missing`) | one bad file must not fail its family; the browser has no way to hold bytes in a JSON bundle |
| 15 | the memory Source's stamp is the FNV-1a of the file's text mixed with the path's generation, a folder's the hash of its listing mixed with the generation `set` bumps on it | a stamp that is only a counter would be equal across two bundles, and a cache that outlives one bundle would serve the other's text |
| 16 | `Recorder` (`rust/src/record.rs`) wraps any Source, remembers every stat, listing and read, and makes the bundle; it adds the nearest listing above every `NotFound` probe | the stage gate re-reads each project through the bundle a read made, and 2l ships that bundle |
| 17 | the Rust types of a file serialize as the TypeScript's output does: keys as the schema names them, a number as it stands, an unset field absent | the stage gate compares them as JSON |

### Output rules

What a Rust type that is serialized (a `ProjectDoc` from 2h on, a view's answer) must hold, so the engine and the frontend read it:

| rule | why |
|---|---|
| a field the TypeScript holds as an integer is an integer type, never `f64` | the engine's `DocJson.cpp` drops a channel written as a float |
| no `null` in a `ProjectDoc`: every `Option` is `skip_serializing_if = "Option::is_none"`, and a non-finite number is a compile error, not a value | serde_json writes a non-finite `f64` as `null`, which the engine and TypeScript then read as something else |
| no `HashMap` in a serialized type: an `IndexMap` or a `Vec` of pairs | the order of a file's keys is what a diff and a test see |
| a 64-bit integer carries `#[ts(type = "number")]` and a skipped `Option` carries `#[ts(optional)]` under `ts` | ts-rs writes a bigint for `u64` and a required key for an `Option` |
| serde_json prints `1.0`, `1e16` and `-0.0` where JavaScript prints `1`, `1e16` and `0`, which is accepted | every comparison is numeric (`compare.rs`), and the engine's reader is a JSON parser |
| a union the TypeScript left to zod is a hand-written Deserialize (a visitor on the shape), never a derived `#[serde(untagged)]` | an untagged enum buffers and answers "did not match any variant" |
| a zod `.nullable()` field is a `Nullable<T>` wrapper planned for 2e | `null` is a value there, and `Option` would read it as absent |
| the 2h gate also checks that the `ProjectDoc` the Rust emits has no `null` and integers in every integer field | the type system cannot see either |

### Hostile input

Every decoder takes files a user, or another user, wrote.

| rule | why |
|---|---|
| every number parsed is checked arithmetic, or capped before it is used | a debug overflow panics and a release one wraps; a panic traps the module |
| a limit counts the OUTPUT (what is laid, visited, allocated), not the input | an input of 30 bytes can name a million of what it produces |
| each decoder has a table of hostile inputs in its tests, each refused fast | `rust/tests/all/negative.rs` and `rust/tests/all/negative.json` hold the readers' table: notes, numbers, depth, a non-UTF-8 file |
| "fast" is five seconds of wall clock, and thirty on musl | the bound exists to catch a runaway walk, and the Linux CI's musl allocator is many times slower than any machine k7 runs on |
| the module file's numbers are integer types or ranges, and a nest past the parser's 256 is a syntax error | `modules_decode.rs` refuses `col: 1e300`, `colSpan: 0`, `note: 128`, `channel: [0]`, `alternative: 1.5` and a nest of 300 |
| the scan counts the plugins it PRODUCES (20,000), is read once whole under 32 MiB, and a nest past serde_json's limit reads as "cannot say" | `scan.rs` reads 30,000 plugins and a nest of 300, each answering with the built-ins |
| the tree walk's depth is its shape (type, category, module, engines, engine), never a listing followed deeper; the vocabulary holds at most 100,000 modules | `modules_tree.rs` walks a link loop, a category three deep, a type folder that is a file, a name with `\` and a tree past a cap |

The stage gate (`rust/tests/all/stages.rs`) runs over the staged golden corpus and over the negative one (`rust/tests/all/negative.json`,
staged by `scripts/stageDump.ts`): each project is read through a `Recorder`, then again through the bundle it made, and the two
must agree; the TypeScript's dumps are compared with `compare.rs`; and a minimum of what it must have seen (projects, clips,
sequences, markers, findings, negative cases) fails the run if it is not met. Over 177 projects (135 clips, 64 sequences, no timeline,
2 markers) and 12 negative cases it finds one difference, the dead `type` key (decision 5), which it allows by name.
`rust/tests/all/negative.rs` is the same corpus without the TypeScript, and runs wherever `cargo test` does.

From 2b the gate also reads each project's module half: its nodes, the vocabulary its roots make, the library findings, `baseModuleRefs` and
the module folders (by the panels and MIDI maps they hold), the roots being the ones the TypeScript computed for that project. One `Cache` is shared across
every project, each is read through a Recorder and again through the bundle it made (with a cache of its own), and both must agree;
nothing is stripped (decision 71). Over 177 projects and 43 negative cases (31 of them module cases, 14 of those resolution cases) it reads 764 nodes, 91 library modules,
89 category modules, 173 engines, 3 library nodes, 5 library findings and 1 scan plugin, every project through a bundle and 360 vocabulary reuses, and finds the differences it allows by name:
the dot folders (decision 20), the library node file and the plugin rule that the Rust names beyond the TypeScript (decisions 23 and 38), a refused panel, map or script that is a warning here (decisions 61 and 62),
and the `@project/` sample the vocabulary keeps unresolved (decision 66). Its minimums are set below those counts: 700 nodes, 60 library modules, 70 category modules, 140 engines, 1 library node,
4 library findings, 1 scan plugin, 100 vocabulary reuses and 15 module negative cases.

From 2c the gate also resolves: the nodes against the vocabulary and the vocabulary against itself, compared with `resolveModuleMap`'s answers key by key, the panels and maps stamped, and the two plugin findings by file and severity.
It sees 260 panels, 270 modules with a script, 2 MIDI maps (both in the negative corpus: the golden one holds no `controller.k7x`), 8 presets, 5 modules with a rack clip resolved, 2 with a file param,
4 with an `@project/` sample, 951 modules whose `extends` resolves, 2 alternatives picked past the first, 264 engines run, 1 not-supported and 1 unknown-plugin finding, and fails below 200 panels, 200 scripts, 1 map, 5 presets,
3 rack clips, 1 file param, 1 `@project/` sample, 800 folds, 1 alternative, 200 engines, 1 of each finding and 12 resolution negative cases. The staged libraries link their `samples/`, `irs/` and `models/` into the repository, and the gate runs under the real confinement (decision 73): the corpus shows no `Outside`, which the unit tests and the negative corpus hold. The most any one resolving call spent was 6,626 of the 1,000,000 budget, and the gate fails past a tenth of it (decision 81).
The run is about four minutes in a debug build.

From 2d the gate also reads each project's routing and a chain project's nodes, compared with the TypeScript's `loadChains`, `deriveChains` (with the lane set each side computes from its own resolved nodes, link levels applied), tap pairs, lanes, bypass and bus-only sets, a chain project's chains, hidden chains and nodes, and `resolveModuleMap(withChainModuleRefs(...))`; the strip list is empty and no routing difference is allowed by name. A TypeScript throw, `deriveChains`' included, needs an error finding here; the Rust names `layout.k7x` and the budget's stop beyond the TypeScript (decisions 103, 105). Over 177 projects and 65 routing negative cases (60 the TypeScript reads) it sees 54 chain projects with 65 chain files and 79 nodes, 5 of them staged with 27 chain files and 29 nodes, 214 graph projects, 6 subchain nodes, 9 picture lanes, 2 link levels (outs whose gain a level changed), 14 bypassed and 3 bus-only modules, 5 port keys and 6 port destinations, and in the negative corpus 2 tap feeds, 1 position, 2 hidden chains, 2 call steps, 1 parallel step and 2 `runs`; it fails below 5 chain projects, 20 chain files and 25 nodes of the staged corpus alone (the negative one cannot make them up), 170 graph projects, 1 subchain node, 4 picture lanes, 1 level, 8 bypassed, 1 of each other kind, and 40 routing negative cases (30 the TypeScript reads). The most any routing call spent was 76 of the 1,000,000 budget, and the gate fails past a tenth of it.

## What the module tree decides (2b)

Numbered on from the readers' table, so a report and a test name each one. 18 to 33 are the work order's, 34 on are the builder's;
a row that departs from the TypeScript says why. Each is named by its number in the test that holds it.

| # | choice | why |
|---|---|---|
| 18 | a project with `chains/` has its nodes from the chain items and the lanes' `tracks/` files (2d, decision 99); `NodesRead::chain_project` only says which format was read | they are chain items, and chain files are the routing's |
| 19 | 2b's `LoadedModule` is the AUTHORED tree plus `kind`, ports (own when declared, else the type folder's), `category`, `file`, `node`, `name`, `home`, `engineRefs`, `engineOf`, `jsdsp` and `faust`, `folderAbs`; a library node is merged under its module (`mergeAuthored`, `ui` taken whole) | the merge is part of reading the tree; every decoration that reads more files is 2c's |
| 20 | DEPARTS. one walk serves the tree and the module folders 2c finds panels in, and it skips dot entries everywhere | `moduleSiblings.ts` and `readModuleTree` walk apart and disagree on dot folders; two walks that disagree hand a module another one's panel. The TypeScript's tree reads a dot folder, so the gate allows it by name |
| 21 | a library or engine `module.k7x` that does not decode is a WARNING named `<ref>/module.k7x` at its real line and column; the module is absent, and its engines are not read | as `readModuleTree`, which placed it at 1:1 |
| 22 | a library node file that does not read is a warning, as 38 states | a broken file must not vanish with no word |
| 23 | a project node that does not decode is an ERROR finding, one per file, located, the node absent; a known plugin format with no `plugin.name` is an error at the `plugin` key, the node kept, reported beside decode findings | `validate` threw both, unlocated, the second only when the first was empty; the compile (2h) fails on any, the views ignore them |
| 24 | under a root, an entry the Source refuses as `Outside` is skipped with a warning naming it; a folder that cannot be listed (`Io`) or a dangling link is skipped silently; a name Windows reads as more than a name is refused like an `Outside` one | the TypeScript read through links unconfined and threw a raw error on the other two, failing the compile with no file |
| 25 | the vocabulary is keyed by the module roots with their namespaces and the scan file, and not by the project; four are held, the least recently used dropped | it reads no project file, so projects whose `_modules/` is the same folder share it, and a project in another sub-folder has its own; a tap's `@project/` sample (2c) is resolved per compile, outside it |
| 26 | its read set is re-checked before reuse (every file stat'ed again and compared by stamp, every listing listed again and compared by its entries, every absent path probed again) and is held only when every file stamp is settled | a listing compared by content cannot be fooled by a same-tick rename; the spec's "re-stats" is "re-stats and re-lists" |
| 27 | a reused vocabulary REPLAYS its texts into the caller's Recorder, and the per-file cache does the same on its own hits | otherwise a bundle made after a hit lacks the module files, and the browser answers `Missing` forever |
| 28 | `MemSource::set` moves the generation of every folder up to its mount, and a draft in a folder no listing names adds that folder to its parent's listing | a new module folder is a draft two levels down, and a vocabulary keyed on the library's listings must see it |
| 29 | `Roots::scan` is a mount that names one FILE (`Mounts::with_files`, `DiskSource::with_files`): it admits that path alone, and by its real path it must be its folder's real path plus its name, so a link as the file is `Outside` | mounting `<studioHome>` for `exts.json` would let any confined read reach the renders and the history |
| 30 | the scan is the builtin table when `Roots::scan` is none, or its file is absent, not JSON, or names no plugin; past 20,000 plugins it reads as "cannot say" too | `scannedPlugins`' rule; the cap counts plugins produced, and a hostile engine reply must not be held |
| 31 | the vocabulary is an `IndexMap` in `readModuleRefs`' insertion order; library node files come in bytewise order of their base | the order is not a contract and the gate compares it as a set; the TypeScript sorted `"base,name"` strings |
| 32 | a namespace comes from the root's place in `Roots` (`workspace`, a library's name, `meta`), never from a path | the host resolved the libraries; `libraryOf` is its function |
| 33 | a fixed name (`module.k7x`, `engines/`) is probed by `stat`, so it is case-insensitive where the file system is; names are compared case-sensitively (`MODULE_OWN_BASES`); `slugify` lowercases by Unicode as JavaScript does | as the TypeScript on each OS |
| 34 | a folder only drafts lie in is wholly known to `MemSource`: another path in it is `NotFound` | a new category or module folder made by drafts must be walkable, and decision 3's rule (the nearest listing above omits the branch) says `NotFound` |
| 35 | the Recorder lists a mount that answered `NotFound` and holds nothing seen as empty in its bundle | no listing above a mount can prove it absent, and the browser would ask for a larger bundle forever |
| 36 | `Source::note_text`, a fourth provided method (the three calls and the four `SourceError` cases are unchanged), is how a cache hit gives a Recorder its text; the per-file cache keeps the text beside the decode | decision 27's file layer: a hit reads nothing |
| 37 | `Roots::workspace` is the project's PARENT folder and `_modules/` is read under it, as the TypeScript's `../_modules`; a library's `root` is its root and `modules/` is read under it; `meta` is the folder of type folders itself; `libraries` are in the host's precedence order and a folder listed twice is read once, under the first name; `mounts()` serves the project, `_modules/`, the libraries, `@meta` and the confinement root of decision 73 | `moduleLibraryDirs` resolves `../_modules` from the project, not `confine.ts`'s workspace; `modules/` is what `libraryFamilyRoots("modules")` appends |
| 38 | a library node file that does not read WARNS, named `<ref>/<file name>` as the file is spelled | a broken file must not vanish with no word |
| 39 | the walk's output is weighed: a module, a folder, a finding, an engine ref copied into a module or a node, and a map entry merged into a node each weigh one; past 100,000 the walk stops with one warning named `@<namespace>` | a limit counts the output, and one module with 50,000 engines and 50,000 nodes, or a folder of broken files, makes far more output than modules |
| 40 | a module folder's `label` is its namespace; the folders recorded are the type folder's sub-folders, a category's leaves and every engine, and not the sub-folders of a module | the TypeScript's `label` was the root folder's name (`modules` for every library) and its walk also took a module's own sub-folders as leaves |
| 41 | a rack is one struct with its `rack_type`, and the flavour's rules (a slice's source and windows, a samples rack's no source) are raised by the visitor on the rack | one type, one place for the rule |
| 42 | a port's empty `name` is found by the module's check, on the `name` key | the derive that checks it at decode breaks the ts-rs derive on `Port` |
| 43 | the module file's types have no ts-rs twin but `Port`, `PortType` and `ModuleKind` | `IndexMap` and `serde_json::Value` need ts-rs features nobody approved; the frontend's types stay hand-written until the facade wires the compiler |
| 44 | a dot name is dropped from the listings the read set keeps, so a dot entry moves nothing | the walk skips dot names, and the stamp walk did |
| 45 | a scan file over 32 MiB reads as "cannot say" | one `Span::All` read must not hold what a hostile file names; the only limit on an input's size here |
| 46 | the gate compares paths spelled with `/` and not through the manifest's tokens, compares the module folders by the `ui.k7x` and `controller.k7x` each holds, and compares each library finding's severity with its file | both sides read one staged tree; the TypeScript has no function that lists every folder it walked |
| 47 | the read set is recorded by `Watch`, a Source wrapper of its own, and the Recorder keeps no read set: it takes texts through `note_text` | the vocabulary needs a read set whether or not a bundle is being made, and a Recorder needs only the texts |
| 48 | a module's text is held once: the per-file cache's `Arc<str>` is the read set's, and the texts of the held vocabularies count against the same 64 MB limit as the decoded files, the oldest vocabulary dropped first | one read must not cost twice, and four vocabularies must not sit outside the limit |
| 49 | `ReadSet::valid` answers false when the source cannot answer (`Missing`) and the rebuild decides | a re-check that fails with an error would hide a rebuild that can say `needs_files` |
| 50 | a file mount that is a folder is `Outside`, and a scan that grew past its cap between the stat and the read is "cannot say" | a mount names a file, and a stat is not a promise about a read |
| 51 | a samples rack's `source` is found by the module check, on that key | the visitor cannot place an error on a key it has already read |
| 52 | the stage gates run on the Mac in `npm run ci:local` (`scripts/stage-gates.sh`), and in no CI job; they are skipped when nothing they read changed since the merge-base with `main`, and `--all` runs them anyway | they hold on any platform, so a CI job only repeated them; in the Linux container they cost 20 of its 48 minutes, compiled cold each run |

## What resolution decides (2c)

Numbered on from the module tree's table. 53 to 72 are the work order's, 73 on are the builder's; a row marked DEPARTS differs
from the TypeScript (or from the order) and says why. The stage gate allows a difference by its number, never by a wider pattern.

| # | choice | why |
|---|---|---|
| 53 | the vocabulary holds modules DECORATED (files resolved where declared, panels and maps stamped), never resolved; resolution is a pass per call over a vocabulary and a project's nodes, and its output, `Resolved`, is owned and has no public constructor (the TypeScript's brand) | the vocabulary is shared across projects as an `Arc`; the TypeScript resolves at every use, and a consumer must not take a raw module by accident |
| 54 | DEPARTS. `extends` folds deepest first; a ref already on the path stops the fold there, and the chain past 32 levels stops at the 32nd; each is a WARNING at the module's own file (decision 60) naming the refs, and the module resolves with the ancestry folded so far | the TypeScript stops silently at a cycle and has no depth limit; a hostile library of 1,000 levels would fold 1,000 merges per use |
| 55 | an `extends` naming no module leaves the module unmerged; `Vocabulary::absent(ref)` answers `Broken { file }` when a file of that ref did not decode, else `Unknown`; the reference check (2e) words it | `checkReferences`' rule: "on disk but unreadable" is a different fault from "absent", and only the file's own finding says which line |
| 56 | DEPARTS. resolution weighs its output as decision 39 does (a resolved module one, each entry a merge copies one: params, widgets, symbols, an engine's params, assets, bindings); past 1,000,000 per call it stops with one ERROR at the file of the module it was resolving, and that module and every later one are absent | a limit counts the output: a fan-out of 10,000 nodes over one parent of 10,000 params is 10^8 copies from 2 MB of text; one weight function (decision 81) serves the walk and the resolver |
| 57 | `OWN_ONLY` is ported by KEY PRESENCE: `name`, `file`, `node`, `home` are never inherited (a library module's absent `node` is the TypeScript's `node: undefined`), `kind`, `category`, `inputs` are the child's when it has them; every other field takes the child's defined value over the parent's, a decorated path included | `k in child` is true for a key decorate wrote as undefined; an `Option` cannot say that, so the identity keys are the child's whatever they hold, and the three others are `Option`s |
| 58 | the alternative is applied ONCE, after the fold: a set naming another module swaps the node into it (identity, `alternatives` and `alternative` kept, folded afresh) unless the module is a lane or the mixer's fader (`keepsItsModule`), else the set is a patch with its `extends` ignored; a name wins over a number, a miss is the first | `docs/alternatives.md`; one place for the pick |
| 59 | a module with engines runs as the current one (the named engine if it has it, else `tracktion`, else the first by name): the engine folded, `KEPT` taken from the module, the rack kept on a rack engine, params the engine's then `engines.<e>.params`, the files those set resolved against the engine's folder then the module's | `docs/engines.md`; an engine is folded with its own `extends` and never has engines (2b) |
| 60 | DEPARTS. a 2c finding is named by REF where the file is outside the project: `<ref>/module.k7x` or `<ref>/<node file>` (decision 21), `<ref>/ui.k7x`, `<ref>/controller.k7x`, `<ref>/widgets.js`; a project node's is `nodes/<file>.k7x` | decision 40's label is the namespace, and the ref carries it; the TypeScript's `modules/<type>/<name>/ui.k7x` names no library |
| 61 | DEPARTS. a `ui.k7x` or `controller.k7x` that does not decode is a WARNING at its real line and column, among the vocabulary's findings; that panel or map is absent, so a FARTHER library's answer for the ref still wins, and the module keeps its inline `ui:` | the TypeScript collects these into an `issues` list no caller reads; a broken file must not vanish with no word (decision 38) |
| 62 | a panel is decoded strict (unknown key refused, `script` included); `widgets.js` beside it rides on it as `ui.script`, stat'ed first and read only when it is at most 1 MiB, past which it is absent with a warning; a non-UTF-8 script is absent with a warning; a panel's widgets and a map's bindings weigh one each in the vocabulary's budget (decision 39); `script` is never read from an inline `ui:` (the key is stripped) | the script is output that ships to the browser; a limit counts the output |
| 63 | a panel or map is looked up over `Vocabulary.folders` nearest first, the first readable hit for a ref winning; a vocabulary entry by its key minus the node, a project node by `moduleOwnRefs`; it replaces `ui` (or sets `cc`) WHOLE, on the built-ins and the catalogue as on the trees; `module_folder_for_ref` is the first folder of the ref holding a `module.k7x` | `applyUiLibrary`, `applyByRef`, `moduleFolderForRef`; one walk (decision 20) serves all three |
| 64 | a MIDI map is a map of non-empty param names to `{ch, cc}`, strict, `ch` a `u8` in 1..=16, `cc` a `u8` in 0..=127, decoded by checked conversion; an empty file is an empty map, stamped as `cc: {}`, and `resolved_cc` answers none for it | `ControllerFileSchema`; the output rules' integers |
| 65 | DEPARTS. a file ref that escapes its folder LEXICALLY resolves to nothing with no Source call, silently, as the TypeScript; a path the Source refuses as `Outside` (a link out, an absolute path outside every mount) resolves to nothing and is a WARNING at the module's file (decision 24's rule); the authored name stays in `assets` | confinement is the Source's, so a link out of the mounts must say so; the mounts are as wide as the TypeScript's roots (decision 73) |
| 66 | DEPARTS. every `@project/<path>` (a tap's `sample`, a file param's) is resolved per call against `Roots::project`, outside the vocabulary and its key (decision 25), and names `<project>/<family>/<path>`; the vocabulary keeps such a ref authored and unresolved; its stats join the CALLER's read set | the vocabulary is shared by the projects of a workspace; the TypeScript looks a file param's `@project/` up under the DECLARING folder, which no picker offers (ruled a bug) |
| 67 | a `@<library>/<path>` file names a library of `Roots::libraries` alone (the account's included), under `<root>/<family>`, the family folder probed, `..` refused lexically; the host lists EVERY library the project sees, with or without `modules/` | `libraryFamilyRoot`; the crate never reads a pin (decision 32) |
| 68 | `Roots::models` is a folder mount, in the vocabulary key, STAT'ED only (a test proves no read under it); no root means no fallback | `resolveModuleAssetIn`'s one fallback; a model is a binary the compile names and never opens |
| 69 | a preset is probed by stat as `presetFor` probes it (the chosen `base` trimmed, `.vstpreset` then `.fxp`, lexically below `base/`; then `<node or leaf>.vstpreset`, then `<leaf>.vstpreset`), a folder answering as absent, the PROBED spelling answering (decision 33); it is never read, so a bundle holds it by its length; a base that escapes, or that the Source refuses, falls back to the defaults | `basePresetIn`; a `.fxp` or `.vstpreset` is a binary for the engine |
| 70 | `Roots::workspace_namespace` (optional, absent means `workspace`) is the level of the project's `_modules/`, which the host gives as `libraryOf` answers it: a project standing inside a library sees its `_modules/` as that library's | panels, maps and `@<library>/` files are keyed by namespace, so a ref named after the wrong level loses its panel; the host computes it (decision 32) |
| 71 | the stage gate's strip list is EMPTY for the module half from 2c on: `rack.source`, a symbol's `clip`, `preset`, `assets`, the file params, `ui` and `cc` are compared, spelled with `/` by both dumps; an entry a later sub-step needs carries that sub-step (`{key, step}`), and the gate refuses an entry whose sub-step is not after the crate's `STAGE` | a stripped key is an untested key, and a list that only grows goes stale unseen |
| 72 | the not-supported (WARNING) and unknown-plugin (ERROR) findings judge each node of a node project RESOLVED (an inherited plugin counts), at `nodes/<file>.k7x`, placed at the node's own `plugin` key when its file has one, else 1:1; unknown is a `plugin` kind in a category that hosts a plugin whose lower-cased type is neither `not_supported`, `offline` nor one `reachesEngine` takes; a chain project's nodes join at 2d | `pluginFindings`; the node's decoded `plugin_at` places it, so no text is parsed again |
| 73 | `Roots::confine` is the whole workspace the project stands in (the TypeScript's `workspaceDirOf`), given by the host and mounted whole; `renders` and `models` are mounted too, and only stat'ed; the `_modules/` mount of decision 37 lies inside it | a file a module names under its workspace is readable, as the TypeScript's confinement roots make it; the Source then confines every read by the same set |
| 74 | a fold is kept for the rest of the call when its chain ended on its own, as an `Arc` with the folders and the refs of its ancestry; it is reused only when its refs fit in the depth left and none is on the caller's path (decision 84) | a parent folded once serves every node over it: what makes decision 56's budget count the work a fan-out really does |
| 75 | the vocabulary key adds the libraries (name and root) and the models root | a file ref is resolved in the vocabulary against them, so two hosts that list them differently must not share it |
| 76 | a panel weighs once when it is read and once more at each module it is stamped on; the walk stops with one warning named `@<namespace>` and the rest keep their own `ui:` | one panel of 90,000 widgets stamped on 10,000 nodes is a billion widgets from one file (decision 39's rule) |
| 77 | `NodesRead.warnings` holds the warnings of the files a node names, apart from `findings` (the errors a compile refuses on) | a link out of the mounts must not make a project fail to compile |
| 78 | `widgets.js` is read whole (`Span::All`) once its stat is at most 1 MiB, refused if the bytes read exceed it | a recording Source keeps a text only for a whole read, and the bundle must hold the script by its text |
| 79 | the memory Source answers a folder with a listing of its own as present first, then an absent path from the listing above it by binary search, before it scans what it holds (no other change of answer) | a vocabulary of 100,000 modules probes 200,000 absent presets; a held listing is a fact whatever the parent spells |
| 80 | a library file and an `@project/` file are NAMED and not stat'ed, as the TypeScript's `libraryFile`; only the library's family folder is probed | a link out of the mounts under a library's `samples/` is the engine's to refuse; the bundle can say a path is absent, not that a link leaves the mounts |
| 81 | the weight of a module is ONE function, `modules/weight.rs`, used by the walk and the resolver: itself, each entry of its params, panel, rack, engines (and their params), automations, events, tracks, kit, tap outs and ports, each alternative with its own, its assets, map and engine refs; the maximum a call spent in the first green stage run is asserted ten times below the budget (see the gate) | a limit that skips a list is bypassed through that list: 2,000 nodes over a parent of 5,000 alternatives were 10 million copies and not stopped |
| 82 | a library's family folder (`samples/`, `irs/`, `models/`) is there when the listing of the library's ROOT names it as a folder (a link is listed as what it points at, a dangling one is not), memoised for the call; the folder is never confined | the TypeScript's `libraryFamilyRoot` is `existsSync`; a linked family folder is a family, and the root's listing goes into the bundle |
| 83 | an absolute tail of a library ref or an absolute `base` is refused; on Windows a library tail holding `\` or `:` is refused | `resolve(root, "/x")` leaves the root in the TypeScript, and `..\..` is a climb on Windows |
| 84 | a kept fold (decision 74) is reused only when `chain.len() + its refs <= 32` and none of its refs is in the caller's `seen` | the answer must not depend on the order a map is resolved in |
| 85 | a finding about a limit is named by the file where the walk stopped: `<ref>/ui.k7x`, `<ref>/controller.k7x`, `nodes/<file>.k7x` | decision 60: a finding names a file |
| 86 | DEPARTS from the order's layout: the resolved half of the gate lives in `tests/stages/modules.rs`, not in a `stages/resolve.rs` | it shares the module half's totals and helpers |
| 87 | a pick is read as JavaScript's `Number` reads it (space, decimal, exponent, `0x`, `0o`, `0b`) | `pickAlternative` calls `Number(s)` |

Hostile input of 2c, each refused fast (`rust/tests/all/resolve_hostile.rs`, `panels.rs`, `presets.rs`):

| decoder or pass | the worst input and its answer |
|---|---|
| a panel (`ui.k7x`) | a nest of 300 is the parser's syntax error (256); a `script` key is an unknown key; 100,100 widgets are past the vocabulary's weight and dropped with a warning; a 2 MiB `widgets.js` is absent with a warning |
| a MIDI map | `ch: 0`, `17`, `1e300`, `cc: -1`, `127.5`, `128`, an empty param name, a binding without `ch`, an extra key are refused at their place; 200,000 bindings are dropped with a warning |
| `extends` | a chain of 1,000 levels stops at the 32nd, a self-extends and a cycle through a library node stop with a warning; a fan-out of 10,000 nodes over a parent of 1,000 params and a blow-up of 32 levels adding 3,000 widgets each stop at the budget |
| files | `model: /etc/passwd`, `ir: "@k7-core/../../x"`, `base: "../../../x"`, a 10,000-character base, a NUL, a `\` (a character on Unix, `Outside` on Windows), 10,000 alternatives each with a rack: no file, no panic, under five seconds |

## What the routing decides (2d)

Numbered on from resolution's table. 88 to 113 are the work order's, 114 on are the builder's; a row marked DEPARTS differs from
the TypeScript and says why. The stage gate allows a difference by its number, never by a pattern wider than the case.

| # | choice | why |
|---|---|---|
| 88 | `Segments` is the ONE routing value: a graph project decodes into it, a chain project's `to_segments` builds it, everything downstream reads it | the TypeScript's `unknown` existed only for an inlined `root.chains`, which `project.k7x`'s strict schema refuses |
| 89 | a project is a chain project when `chains` is a folder, and is then read only from `chains/`, a `graph.k7x` beside it ignored; with neither, routing is empty and no finding | `isChainProject`, `loadChains` |
| 90 | DEPARTS. `graph.k7x` is decoded by hand: `runs` first, before the file's other keys (lists of two or more strings, its error on `runs`), each run added after `modules`' own list and deduplicated against it; a run's first name is a key and only its other names are port refs; `modules` that is no object beside `runs` is refused | `flatten` buffers and loses the node; `expandGraph` drops it and takes any string, and `modules` takes a `#x` key |
| 91 | DEPARTS. a step is dispatched on its SHAPE: a list is parallel, a map holding `to` an end step, `sub` without `id` a call, keys within `{id, track, bypass}` with a string `track` a lane, likewise `video_track`, else a node; a wrong step fails on its own key | the union's first-member rule answers "invalid input"; an untagged enum is forbidden |
| 92 | a node item's module is the map minus `id`, `bypass`, `vstpreset`, `mapping`, decoded strict on a filtered `MapNode`; `Module::check` runs at the item's path; `plugin_at` is the item's own `plugin` key; the `type: plugin` default is NOT applied | `parseChain` validates with `ModuleSchema` but keeps the raw item |
| 93 | DEPARTS. an `id` or lane name that is empty, starts or ends with `#` is refused at decode; a lane name holding `/` or a NUL, or `.`, `..`, and on Windows a `\` or a `:`, is refused on `track`; a link `port` ending in `#` is refused | the first hides in `deriveChains`, which throws on a port ref with an empty half; the second leaves `tracks/` |
| 94 | every refinement of `ChainFileSchema` and `stepIssues` is a check, on the key path the TypeScript names | a file reports its first finding |
| 95 | DEPARTS. chains are read with `read_family` sorted by base name bytewise; the file read is the one listed (`a.K7X` reads); a hidden chain's findings count, its nodes do not | decision 4; `polysynth-copy` sorts before `polysynth` by file, not by chain name |
| 96 | a node name used twice is an ERROR on the later item, named `chains/<chain>.k7x`; the later value wins at the first's position | the TypeScript's behaviour; the compile fails on any finding |
| 97 | DEPARTS. a lane is a picture lane when `tracks/<lane>.k7x` decodes and extends `@meta/picture-track`; a file that does not decode is not picture | one read through the per-file cache |
| 98 | a lane with no `tracks/<lane>.k7x` is an ERROR named `chains/<chain>.k7x` placed on `track`, the node absent; a `video_track` item is a name in the segments and no module | `loadChainNodes` |
| 99 | a chain node is stamped as `loadChainNodes` stamps it, in one file with the graph's `nodes/` | the two cannot drift |
| 100 | DEPARTS. an item's preset is `base` by `base_preset_in` under `<project>/vstpresets/`, else `vstpresets/<vstpreset or id>.vstpreset`, stat'ed only; a name that escapes lexically or is absolute, and on Windows one holding a `\` or a `:`, gives no preset AND no `presetRel` | `presetRel` is where the engine WRITES |
| 101 | `LoadedModule` gains `chain`, `node_id`, `preset_rel`: never inherited, in `OWN_ONLY` and `KEPT`, kept by `swapped`; `own_label()` is the one spelling of a node's file | decisions 57, 59 |
| 102 | DEPARTS. plugin findings over a chain project are named by `own_label`, at the item's `plugin` key | `modules/undefined/...` names nothing; `stageDump.ts` respells the TypeScript's |
| 103 | DEPARTS in the name only. `layout.k7x` is read through the cache: only the root key `positions`, ignored when it is no object; an entry or syntax error is an ERROR named `layout.k7x` with no positions, a file the Source refuses (`Outside`) a warning of that name with none | the TypeScript throws it as `graph.k7x` |
| 104 | `to_segments` answers `Segments` and the levels in one pass, `targetsOf` an explicit stack in the same order with the same `seen` | a ladder of 100,000 link-only chains must not overflow a stack; a call finds its subchain by an index and a file's picture lane is looked for once, so 100,000 calls or unlinked subchains stay linear |
| 105 | one budget, `routing::BUDGET = 1,000,000` per `load_routing`, weighs the OUTPUT: each key and destination written, each `targetsOf` visit, each bypass; past it one ERROR named the `chains/<chain>.k7x` it stopped in, `capped`, empty `Segments` | a limit that skips a list is bypassed through it |
| 106 | `derive_chains` builds its indexes once (destinations by source, feeders, named ports); key order is `IndexMap`'s and the gate compares keys as sets | 100,000 keys would be 10^10 steps |
| 107 | 2d gives `Segments`, `derive_chains`, `Chain`, `UseStep`, `Out`, `lane_names`; 2e owns lanes and ownership, 2k `buildGraph` | the compile needs a `Chain` long before the graph view |
| 108 | `with_chain_module_refs` adds a vocabulary module as it stands by a step's `module` and an out's `to` when no node has the name; `resolve_map` then resolves once | idempotent in the TypeScript |
| 109 | `tap_pairs` numbers from 1 per node ordered by code point; `tap_outs_for` answers `Result<Vec<TapOut>, String>`, `note: u8`, `pair: u32` | the output rules' integers |
| 110 | `Segments` from a `graph.k7x` that does not read is default with one ERROR | `bypassedOf` is total |
| 111 | every routing file is read per compile under the caller's Source and outside the vocabulary's read set | decision 25 |
| 112 | the gate's strip list stays empty; each side computes its own lane set from its own resolved nodes | stripping it would hide the derivation's one input |
| 113 | `input` and `mapping` are decoded and read by nothing | the file's shape is checked |
| 114 | a `runs` past the budget is a decode ERROR on `graph.k7x`, not `capped`; `capped` belongs to the load of a chain project | the decode sits in the per-file cache, which holds a `Finding` |
| 115 | a node item's module DECODE errors come before the file's refinements; its `check()` errors after them | `from_map` decodes the item as it is met |
| 116 | `Outside` on the `chains` folder itself skips the whole family with one warning named `chains` (89); on one file of a family it skips that file alone, a warning naming it (24), and the rest still read. A finding names a chain file as `chains/<base>.k7x` whatever the case of its extension | one file, one spelling; a silent empty song is worse than a missing chain |
| 117 | `unplayed_levels` answers `Unplayed {chain, to, gain}`, not sentences | wording is the graph view's (2k) |
| 118 | `Item::Node` holds a `Box<NodeItem>` | clippy's `large_enum_variant` |
| 119 | `load_chain_nodes` answers nodes undecorated; `load_nodes` stamps them with the vocabulary's panels and maps | one stamp site |
| 120 | a graph project's `weight` is its keys plus destinations and is never capped | its decode is bounded by its input |

Hostile input of 2d, each refused fast (`rust/tests/all/routing_hostile.rs`):

| decoder or pass | the worst input and its answer |
|---|---|
| `graph.k7x` | a nest of 300, `runs` of a string or holding a number, `modules` a number beside `runs`, a destination `#x` or `x#`, an empty key, an extra key in a position: refused; 750,000 runs of two (1,500,000 names): one ERROR on `graph.k7x`; 300,000 keys feeding the next decode and derive in time |
| a chain file | a nest of 300, `level` -1 or 1e400, `start: false`, `to: []`, links without levels, a list of one, a list in a list, a string step, `{to, id}`, `{sub, extends}`, a first call, an empty or `#` id, a `track` of `../../x`, `a/b`, a NUL (and on Windows a `\`), a `port` ending in `#`, a call twice, a subchain named `OUT`: refused; 100,000 items or subchains decode in time, and 100,000 calls over 100,000 subchains expand in time |
| `layout.k7x` | an extra key, `x` a string, `x: 1e400`, an empty key: refused; `positions` a list: ignored; 200,000 entries in time |
| `to_segments` | two parallel steps of 5,000, an end of 5,000 nodes and 5,000 links, a ladder of 100,000 link-only chains, 1,000 chains each linking all the others: one ERROR naming the file, empty; a link-only cycle ends; 100,000 subchains in one file, with or without a picture lane to look for, in time |
| `derive_chains` | `a: [a]`, a ring of 100,000, a fan-in of 100,000, a lane keyed on 1,000 ports, a run of 300,000: in time |

## Risks

| risk | answer |
|---|---|
| serde's first-finding-only decoding makes the editor show one finding at a time | accepted; a file is rarely wrong in two places at once |
| the disk Source's real-path checks cost on Windows (a handle per directory) | measured at 2l on the A069P and Windows; the memo then outlives a compile, keyed by the directory's stamp |
| `k7x`'s exports reach the facade's module | decided when the facade links `song` |
| the bundle cannot say that a path leaves the mounts through a link: replayed in the browser it answers `Missing` for ever (2b's linked folders, 2c's `assets/`, `sources/`, presets, panels and scripts) | 2l adds a `refused` paths list to the bundle, which `MemSource` answers `Outside` for them and below; a 2l test proves a bundle made over a link out replays to the same answer |
| a file that is not UTF-8 is held by its length, so `widgets.js` replays as `Missing` (decision 14) | 2l decides how a bundle says "not text" |
| `is_absolute`, `spell` and `windows_hazard` follow the machine the crate runs on: the browser replaying a Windows tray's bundle reads `C:\x.wav` as relative and asks for more files for ever | 2l: `Roots` declares the host's path rule, and the crate reads that, not `cfg!(windows)` |
| the Source confines reads the TypeScript never confines (presets, panels, scripts, module folders), so a link out that worked there is refused here | accepted and warned; a library that links a folder out is mounted by the host |
| `Roots::libraries` lists the libraries a project sees, not every installed one (`libraryReadRoots`) | the host lists EVERY installed library at 2l, with or without `modules/` |
| the estimate (below) is above the plan's share for this step | the plan's figure is corrected with it |

## The estimate

About 17 agent-days for the builder (the table above), plus two to three days of review and the
bed runs: three and a half to four weeks with one builder.
