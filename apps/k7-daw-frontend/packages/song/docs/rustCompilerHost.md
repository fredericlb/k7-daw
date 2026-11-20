# The compile's host: the cache, the roots, the bundle and the compare

The decisions of how a host calls the compiler: [rustCompiler.md](rustCompiler.md) keeps the output rules and the stage gate; the earlier decisions are in
[rustCompilerClips.md](rustCompilerClips.md), [rustCompilerCurves.md](rustCompilerCurves.md) and [rustCompilerLanes.md](rustCompilerLanes.md). The native file
service is the host: one cache per workspace behind it, roots and a rig it computes, a bundle the browser replays, a compare of two compiles, and the seal on the shared version.
A row marked DEPARTS differs from the spec's row or from the TypeScript's old answer. Paths are relative to `packages/song`; `crates/` is `apps/k7/backend-rs/crates/`.

## What the host decides (2l)

Numbered from 700; 790 on are host mechanics, which change nothing the compile answers and so excuse nothing; 798 on are the builder's own. Each is named by its number in the test that holds it.

### The host function and the cache

| # | decision | why |
|---|---|---|
| 701 | One host function, `__host.song(text) -> [status, text]`, in `crates/k7-compile/src/host/native.rs` (a module named `song` would hide the crate), its call `timed("song", ...)` so a compile moves the step's deadline | QuickJS copies one string in and one out |
| 702 | Five ops: `song.compile`, `song.forget`, `song.shed`, `song.bundle`, `song.version`. An unknown op, a request that does not decode, one past 64 MiB or a workspace named by more than 8,192 bytes is status 1 with a sentence, the actor alive | one door, and no input of any size crashes it |
| 703 | `Session` (`session.rs`) lives in the crate, not the host: a `Cache` plus the answers, over any `&dyn Source`. The host holds one per workspace in `Shared` behind a `RefCell` | step 1's native file service shares it (`file-service-rust.md`), and it is testable with no QuickJS |
| 704 | A workspace is keyed by `spell(ws, rule)`; `workspaceDirOf(dir)` is what the TypeScript sends. At most `MAX_WORKSPACES = 4` sessions per runtime, the least recently asked dropped | an actor lives long and an account has many workspaces |
| 705 | `Views` are made in groups. `Core` is `Engine`, `Web` and `Errors` from one `answer` call and always made together; `Hints`, `Surfaces`, `IoPorts`, `Catalogue` and `BaseModules` one call each; `Graph` asks `Core`, then makes `load_compiled_graph` over that compile. Each group has its own `ReadSet`, and the graph is held only over the very compile it was drawn over. Serialized once into `Arc<str>`, answered by concatenation | the frontend fetches four views per revision and three need the same compile; a graph drawn over a compile that has been made again answers the old freezes (a render lands, the engine says ready and the graph says waiting); `Handed` needs the `GraphDoc` the graph call builds, so the host never makes one |
| 706 | An answer is reused when `Ask::rev` equals the one it was made with (no stat at all), else when its read set is `settled` and `valid`; a compile that lists jsdsp builds is never reused by the token alone | two views of one revision must not compile twice, and a just-edited file is unsettled for two seconds exactly when the views are fetched; the host builds the graph a compile listed and asks again at the same token, which must not answer the held compile (decision 732) |
| 707 | The key of a held answer is a hash of `roots`, `rig`, `vsts`, `thawed` and the graph's folder; `rev` never keys. At most `MAX_ANSWERS = 4` per session, the least recently asked dropped | the spec's bound; an answer must not change with the order things were asked in |
| 708 | Bytes: a session weighs its `Cache::bytes` plus its held strings and the compile it keeps, which weighs three times the document's text; past the runtime's budget, whole sessions are dropped, least recent first, and the one just asked last lets go of its answers. The budget is `min(heap / 2, 256 MiB)`, a quarter of the heap for a runtime of 192 MB or less. `song.shed` sheds every session but the one it names, spelled as the sessions are keyed | the crate's bound is per workspace and the phones have 192 MB in all; a session of the capacity project measures about three times its text, and a sole session that stays whatever it weighs is a budget that holds nothing |
| 709 | The TypeScript's revision memos stay in front (`docCache`, `webDocCache`, `graphCache`, `hintsCache`, `derived`). Spec correction | one parse per revision; step 1o retires them with the route |
| 710 | A panic inside `Session` is caught (`catch_unwind`, the backend does not abort on panic), that workspace's session is dropped, status 1 | the crate denies panics; one account's bad file must not take its actor |
| 711 | One `DiskSource` per call, built from `Roots::mounts`. A `Missing` from a disk is a bug and answers status 1 | the spec's one instance per compile |


### What `on` answers

| # | decision | why |
|---|---|---|
| 724 | DEPARTS in the wording, as the spec says. A `failure` becomes a JavaScript throw: `<file>:<line>: <message>` joined by newlines, and a `FormatTooNewError` with its two levels when `formatTooNew` is set, so `compile_error` and `compileErrorBody` answer the same 400 shape. A host that fails (status 1, no answer) is a `HostFailure`, which the views that fall back on an empty answer throw on instead | the routes stay as they are; a failed host read as a project that will not read answers `{errors: [], diagnostics: []}`, a clean project |
| 725 | `/project-errors` is the native `errors` view as it stands, never a throw; `projectErrorsIn`'s TypeScript is not run | one compile answers it |
| 726 | `/graph` of the open project is `Graph` made by `load_compiled_graph(..., Some(&compiled), Folder::Open)` over the compile `Core` holds; a project that does not compile is the same call with `None`, which still draws its manual plans | `ifCompiles`; a `Handed::default()` would leave the plans out |
| 727 | `/graph?at=` is `Graph` with `Roots::project` set to that folder, `rev` none and `Folder::Other`, over the compile of that folder or `None` when it does not compile (433); `songRoots` works on any folder | the graph of a library chain |
| 728 | `moduleCatalogue`'s absolute `path` is relativized by `toRel` in `Workspace.moduleCatalogue`, as today; `hints`' `pluginNames` and `projectRoot` are the TypeScript's, the native `hints` being the crate's `Hints` (633) | 620 and `Workspace.moduleCatalogue` |
| 729 | `AutoFreeze`'s `plan.lanes` and `plan.modules` are arrays on the wire (359) and turned back into `Set`s in `compileNative`, so `autoFreezeJobs` and `activeFreezePlans`' users see what they saw | a `Set` does not cross JSON |

### jsdsp and the renders

| # | decision | why |
|---|---|---|
| 730 | the compile lists no source to build: a DSP graph is decoded or read, never built | nothing in the compile runs a module's code (docs/dsp-literal-plan.md) |
| 731 | a module whose graph cannot play gets one ERROR on its `own_label`, `module "<name>": its DSP does not build (<reason>) — it passes the sound through untouched`, the TypeScript's own sentence; the errors list says it once | the message and severity are `jsDspBuildIssues`' |
| 732 | the host asks once per call: there is no build to wait for | no refresh step stands between an ask and its answer |
| 733 | every host decodes a `dsp.k7x` with the song crate; the TypeScript in node, which has no decoder, reads the `jsdsp.json` beside it, which a test keeps equal | one decoder, and the committed graph is the node host's copy of it |
| 735 | `renders_used` is touched by the host after a `song.compile` that carries `touch`, which the TypeScript sets in `on` for every view one compile makes (`engine`, `web`, `errors`, `graph`), the graph's answer carrying what its compile used: a render whose `max(atime, mtime)` is under an hour old is left, else its atime alone is set to now; every failure is silent | `markUsed`; a stat is one per render, and an atime moves no stamp |

### Roots and the rig

| # | decision | why |
|---|---|---|
| 736 | `songRoots(dir)` (`src/songRoots.ts`) spells every path with `/`, a drive letter kept; the set of its mounts equals `projectRoots(dir)`, none less and none more; `also_readable` is every folder `libraryReadRoots()` names, which admits a library's other version folders as the TypeScript admits them | a folder the compile may read must be a field, since the crate finds no root; the test holds the union against `projectRoots` both ways |
| 737 | DEPARTS. `also_readable` also holds the real path of a library's family folder when it is a link: for each library, each of the eight families, one `lstat` and one `realpath` when it is a link | a dev and an e2e library link `samples/` into the repository, and the crate refuses a read through a link out of the mounts; the TypeScript's `confineTo` refuses that target (`compile/clips.ts`, `compile/inputs.ts`, `moduleAssetFiles.ts`), so the native compile reads a path the TypeScript returns as `""` or undefined; the shadow excuses a rack or a clip source it finds there, only when the roots mount such a path (`shadow/allow.rs`) |
| 738 | `songRig()` answers `{channels, cueOutput}` of the preferences, already normalized; a preferences file that does not read is the empty rig | the rig is the file service's fact |
| 739 | `PathRule` (`Posix` or `Windows`) is the rule of the machine that spelled the roots: `Roots::paths`, which the host sets to its own machine's rule on every ask (the actor's `process.platform` is always `linux`, whatever the machine), keys the vocabulary and is held by `Inputs`, `Places`, the `Cache` and the freeze fingerprinter. `spell(path, rule)` and `is_absolute(path, rule)` take it, and `cfg!(windows)` is asked in two places only, `PathRule::HOST` and `source/disk.rs`, which is the machine; `path_rule.rs` reads `src/` for a third | a browser replaying a Windows desktop app's bundle must read `C:\x.wav` as absolute and `sounds\a.wav` as a relative path with a separator; the rule travels with the roots, and the Windows rule is tested on any machine |
| 740 | `vsts` is `scan` but in `shareSnapshot`'s `none`; `thawed` is `[...thawedIn(dir)]` | 303; a share's engine has no VST |
| 741 | `songRoots` has no memo: it reads the libraries' home and the pins on every call | a memo would need the pins file's stamp |

### The bundle

| # | decision | why |
|---|---|---|
| 742 | `format` stays 1; `SourceBundle` gains `refused`, `failedStats`, `failedLists`, `absent`, `digests`, `jsdspStale`, `lengths`, `request` and `overflow`, all optional, in camelCase; a bundle with none of them reads; an unknown key is refused | nothing was ever stored, so nothing is versioned; `Roots` keeps its snake_case keys |
| 743 | `refused` holds every path that answered `Outside` to a `stat`, a `list` or a `read`; `MemSource` answers `Outside` for it and for anything below it, looking up only as many leading names as the deepest entry has and as many bytes as its longest; a refusal outside every mount is dropped when the bundle is made, and one with a mount below it answers for itself alone, since the mount below is not refused by it | the lookup of a path a million names deep costs its shallow end; a link out whose folder holds a mount must still be refused when asked |
| 744 | `failedStats` and `failedLists` keep the message of a `stat` or a `list` that answered `Io`; a listing that fails makes its folder a folder all the same | a name below a file (`clips/a.wav/sub.wav`, where Windows answers `NotFound` for the name and `Io` for the file as a folder) and a folder that will not list |
| 745 | `{len, binary: true}` is a file read whole that was not UTF-8: `read(Span::All)` answers one byte of `0xFF`, so each caller takes its own "not UTF-8" branch; a range read answers `Missing`, as a bare length does; a `binary` of length 0 is refused | the browser holds no bytes, and a replay must not end in `Missing` where the disk gave a finding |
| 746 | `{len, io}` is a read that answered `Io` after a good stat: the stat answers the length, the read answers `Io` with the message; a read of a folder answers `Io` | a family file that costs that file and no other |
| 747 | `digests`: `Cache::digest` tells the Source through `note_digest`, on a hit too; a Recorder keeps those of files it does not hold as text; `Cache::digest` asks `known_digest` first. A `binary` file with no digest hashes as one `0xFF`, which is at worst a render reading as not ready, the same outcome as `#<len>` | the browser's fingerprint of a preset or a model must equal the server's |
| 748 | `jsdsp_stale`, set by the Recorder from `Compiled`; `known_stale` answers from it, so a replay gives the same WARNING and list | 352: stamps in memory are hashes |
| 749 | `lengths` is `WebAnswer::lengths`; a replay measures through `ByTable`, and `answer` over a bundle answers `Missing` by design | 611 |
| 750 | `request` holds what a replay must ask (`roots`, the rig's JSON, `vsts`, `thawed`) | the spec's two shapes reconciled |
| 751 | `song.bundle` makes one by running the `Core` group through a Recorder, off the same `Cache`, and fails with the Recorder's sentence when it outgrew a limit; no HTTP route at 2l | the route's first client is the browser's wiring; the op is what the gates use |
| 752 | `MemSource` holds the set of every proper ancestor of a held file or listing, built once, so "something lies below" is one lookup; drafts stay a linear scan | a scan of everything held inside a loop costs 7 s for 40,000 modules |
| 753 | `MAX_BUNDLE` is 128 MiB of text, names and listing entries, `MAX_ENTRIES` is 200,000 paths across `refused`, `failedStats`, `failedLists`, `absent` and `digests`; a `Recorder` stops adding past either and `overflow()` says which, and `MemSource` weighs the whole bundle and refuses one past either | a limit weighs the output; a bundle past it is sent nowhere |
| 754 | no gate exempts a bundle replay: every project is read through its bundle, the one that links a folder out included | the point of the seal |

### The shared version

| # | decision | why |
|---|---|---|
| 755 | `CRATES` gains `packages/song/rust` and `packages/k7x/rust`, after the facade and `song-schema`, and `rustSourceHash` adds `packages/song/rust/Cargo.lock` to the facade's lock and every file a crate compiles in with `include_str!` (`packages/song-schema/src/builtinParams.generated.json`) | the spec's one hash over the compiler and its path dependencies, and over what it embeds: a table that changes alone changes what the module answers |
| 756 | `buildWasm.mjs` computes the hash first and builds with `K7_WASM_SOURCES` in the environment; the facade's `version` op answers `option_env!("K7_WASM_SOURCES")`, else `"dev"`. `build.test.ts` holds `version` equal to `SOURCES`, and a test on a copied tree that one changed byte in `song` changes the hash | not circular, and the module proves what it was built from |
| 757 | `crates/k7-compile/build.rs` reads the `SOURCES` line of `packages/k7-wasm/src/k7Wasm.generated.ts` (the first ten lines only), `rerun-if-changed`, and writes `$OUT_DIR/sources.rs`; `SONG_SOURCES` is that constant. A missing file or line fails the build with a sentence. The `song.version` op and a test compare it with the file again | "read that one value and compute no other"; `OUT_DIR` rather than an env var so sccache sees the content |
| 758 | DEPARTS, additive. With `on` and only then, the compile routes the actor answers carry the header `x-k7-compiler: <SONG_SOURCES>`: `/project-doc`, `/project-errors`, `/engine-doc`, `/graph`, `/surfaces`, `/io-ports`, `/module-catalogue`, `/base-modules` and `/hints`. `/workspace`, which the live service answers, does not (decision 807) | a key in the bodies would change a contract; the frontend reads the routes' shapes |
| 759 | No frontend reader at 2l. Spec correction | nothing in the browser compiles yet; it lands with the first browser operation, on these same three values |
| 760 | `packages/k7-wasm/src/k7Wasm.generated.ts merge=rebuilt` in `.gitattributes`, `rebuild:generated` also runs `npm run build:wasm -w @studio/k7-wasm`, one row in `docs/agents.md` | every branch touching the compiler now changes that file's `SOURCES` line |

### Measures and the suites

| # | decision | why |
|---|---|---|
| 761 | `tests/disk_cost.rs`, not ignored: a tree of 2,000 folders, the Source's stat of each against a plain `lstat`, printed with the second pass, and a generous bound (the musl rule). The memo that outlives a compile is built only if the Windows run shows the checks above 5 % of a real compile, read by the designer from `rust checks windows`' log | the Risk row; Windows is checked here only by `ci/` |
| 762 | `session_capacity.rs` builds 2h's capacity project on a disk, answers every view through a `Session` and again from memory, takes its bundle, and prints the times, each view's size, the session's bytes and the bundle's; each is held under a third of its limit, but two that are not (decision 808) | a real-size project must fit, measured |
| 763 | The `on` gate holds out no spec by name; a spec that pins a compiler sentence is loosened to its status, file and shape in the same commit and listed here | the spec rewrites the messages |

### The builder's own (798 on)

| # | decision | why |
|---|---|---|
| 798 | DEPARTS from the work order's fixed `Ask`. `Ask` gains `folder` (`Folder::Open` or `Folder::Other`), which keys an answer and which `song.compile`'s `at` sets | `Folder::Other` draws a library chain's folder over the compile of that folder, and the session cannot tell it from the open project otherwise |
| 800 | `Cache::set_paths` makes a cache forget what it decoded under another rule, and `load_inputs` sets the rule of its roots; `Cache::bytes` is public and counts the vocabularies | one session serves roots of one rule, and a test or a host that changes it must not read a chain decoded under the other |
| 801 | A native answer of another kind than the TypeScript's (a null, a number, a list for an object) is one difference at the root, whatever the view reads | an answer that is no object has no pointer to compare |
| 802 | `compare_view_in` takes what the host knows: the native read set holds a refusal (65 and its rows), the host cannot build jsdsp (350), a library's family folder is a link whose real path the roots mount (737), the project's time signature has a zero half (454), the compile cut a nest of timelines (518), and the folders the roots mount (20); `compare_view` is the strict reading with none | an entry that depends on a fact of the host must be handed it, never guess it |
| 803 | The native views are answered as the routes answer them: `hints` is the `Hints`, `ioPorts` the ports, `catalogue` and `baseModules` their rows; the WARNING of a view that hit its limit has no route to reach and is dropped from the wire | the routes' shapes are a contract, and the TypeScript has no such warning |
| 806 | A `song.compile` with `touch` touches the renders the compile used; the TypeScript sets it in `compileNative` and nowhere else | 735: only the native compile that answers marks a render used |
| 807 | `/workspace` carries no `x-k7-compiler` header: it is the live service's route, which the actor does not answer | 758 reaches the routes of the actor; the connection's header lands with the browser's reader (759) |
| 808 | CLOSED by 916. The capacity project's engine document is 35.1 MB (the web project 18.6 MB). It lives at about 101 MB of QuickJS heap and needs between 128 and 192 MB to parse, so the server's runtime (384 MB) holds it and an embedded one (192 MB) is marginal. Two parsed documents of it took a process from 274 MB to 1,338 MB, and a session that said 86 MB measured +263 MB. So: no comparison of an answer past 16 MiB (723), a held compile weighs three times its text (708), and an embedded runtime keeps a quarter of its heap, 48 MiB, for its sessions, which a sole session over it lets go of after each answer | the work order says to stop and ask when a real project reaches a third of a limit; `session_capacity.rs` pins the two numbers it knows are past it, so a change shows |
| 811 | A project root that is not there, a file other than a `.k7x`, or a folder with no `project.k7x` whatever else it holds, is one ERROR naming no file, `unsupported song '<name>' — use a .k7x file or a folder project`, the TypeScript's sentence; `errors` lists it as its sentence alone. A folder with a `project.k7x` and no graph is still 305's | the TypeScript's `isFolderProject` is the presence of `project.k7x`: a folder being copied, seeded or removed has its graph, nodes and clips without it, and the native compiler answered or refused it at `project.k7x` with nothing moving |
| 812 | A placed clip, sequence or timeline that failed to load is named by its own file, with the cause: `<holder> — sequence '<id>' failed to load — <file>: <cause>` | the TypeScript's sentence says both, and a refusal that names the lane which placed it loses the file the author must edit |
| 813 | DEPARTS. A project that places a clip or sequence that is not there, or whose module extends one that is not, is refused at the node file of the lane or module that holds the reference (`nodes/<name>.k7x`); the TypeScript's `unresolved reference(s):` throw names no file. | the native answer says which file to edit |

### Host mechanics (790 to 797)

Rows 790 to 797 are the host's own mechanics and change nothing the compile answers, so none says DEPARTS: that is deliberate.

| # | decision | why |
|---|---|---|
| 790 | `Roots::also_readable` is mounted like a library root and carries no namespace; a folder inside a mount adds no mount | the mounts stay the minimal set |
| 791 | `Recorder::overflow` stays for a caller that wants to stop early; the check that cannot be skipped is the bundle's own `overflow` (796). `into_bundle` returns a bundle, not a `Result` | `into_bundle` has callers that cannot fail |
| 792 | the lookup of a path beyond the deepest held one starts at that depth (`MemSource::absent`), and a stat of a path a million names deep is one hash of it | a walk up from the leaf copies the whole path at every step |
| 793 | a vocabulary reused and a decoded file served from the cache read a non-UTF-8 file again, through the Source they were handed, so a Recorder learns it is `binary` | the Source has the three calls and `note_text`; a fourth provided method is not added for a rare file |
| 794 | the bundle's `digests`, `jsdspStale` and `lengths` are read back by `MemSource` (`digests()`, `jsdsp_stale()`, `lengths()`) and normalized; a `request`'s `vsts` is `scan` or `none` | a key that does not normalize refuses the bundle |
| 795 | `absent` holds every path that answered `NotFound` to a `stat`, a `list` or a `read` and that no listing the bundle holds proves absent; `MemSource` answers `NotFound` for it. A folder mount or a file mount that is not there is one such entry, never an empty listing | a listing proves a name absent only if the disk could list the folder: a closed folder (mode 0311), a name below a file on Windows and a dangling link are not provable that way, and a real `NotFound` replayed as `Missing` asks for a larger bundle that cannot exist |
| 796 | `overflow` is the sentence of a `Recorder` that outgrew a limit; `into_bundle` sets it and `MemSource::from_bundle` refuses a bundle that carries it | a cut bundle replays a real file as `Missing` and a dangling link out as `NotFound`; the check travels with the bundle, so no caller can ship one by forgetting to ask |
| 797 | A bundle is refused whole past: 4,096 mounts; a key of more than 8,192 bytes or 1,024 names; `MAX_BUNDLE` bytes of names (each weighs its length), text and listing entries (each weighs its name and 8); and `MAX_BUNDLE` bytes of the folders above its paths | the index of folders copies every ancestor of every key, so one key of 20,000 names builds 381 MB and 65,000 traps a browser at 4 GB; `mounts` is scanned on every call; 8,192 bytes is twice Linux's `PATH_MAX` and 1,024 names is far deeper than any project k7 reads: the staged corpus's bundles hold at most 19 names and 215 bytes in a key, 41 `absent` paths and 2.4 MB of the 128 MiB |

## What the step's end decides (2m)

Numbered from 900; a row marked DEPARTS differs from the spec's row or from the TypeScript, and says why. A decision is named by its number in the test or line that holds it.

### The smoke

| # | decision | why |
|---|---|---|
| 900 | `serviceGolden.spec.ts` keeps its name and stays in the conformance `service` lane; it runs on a running service in whichever mode the session is in, and compiles (`/engine-doc` 200, `/project-errors` and `/graph` 200), renders through the engine, and judges the sound. It reads no committed fixture. It is green with `off`, `shadow` and `on` | the gate survives step 5; a check that only holds with one setting tests the setting |
| 901 | DEPARTS. A render is the whole edit: `k7-runtime --render <doc.json> --out <dir>`, a 60 s bound per render. An archetype, demo or fixture renders in about a second on the Mac, so the bound has room | the engine has no range option, and cutting the document would test a document nobody plays |
| 902 | DEPARTS. "Not silent" is a table per project, asserted both ways: the seven in the table's first rows must sound (`arrangement`, `video`, `crossfade-demo`, `rack-demo`, `synth-demo`, `video-demo`, `node-demo`), the three below must not (`mix` exits 0 and reads silent; `live` and `live-video` exit 1 and say `nothing audible`). A project that begins to sound fails as one that stops, and a demo with no row fails | three of the ten hold nothing to hear under either compiler; only a table can both pass today and fail tomorrow |
| 903 | "Sounds" is a peak at or above 0.001 (-60 dBFS) over every frame of `mix.wav`, read by `--decode` (one JSON line, then float32) so no WAV parser lands in the suite; a render that exits non-zero, or leaves no `mix.wav`, is not silent but a failure, except the table's two `live` cases | one threshold, one reader, the engine's own |
| 904 | "The engine loads each" is `--render` exiting 0 for every e2e fixture that compiles. `clip-loop.k7` renders like the rest, its own WAVs being placeholder silence: it is loaded, not sounding | a load is one offline process each |
| 905 | The e2e fixtures carry no table. They must exit 0, and at least 12 of them must sound. The last test of the serial file holds the counts it saw: `compiled` at least 41, `rendered` at least 39, `sounding` at least 12 of the fixtures and exactly 7 of the archetypes and demos | a floor counts staged work and the effect; a fixture's own silence is its own README's business |
| 906 | A negative in the same file: `synth-demo`'s engine document with every track's clips and plugins emptied is rendered and reported as not sounding, and an empty or absent `engine binary` path fails with a sentence. The render helper is judged before the corpus is. DEPARTS in the document emptied: clips alone leave the pattern generators of the `kit` and `pad` tracks sounding (peak 0.16) | a check must be able to fail, and silence must be visible to the function that says "sounds" |
| 907 | The two broken fixtures: `/engine-doc` is 400 with an `error` naming a file, `/project-errors` is 200 with an error listed. Neither is rendered, and the two are exactly the set that fails: a third fails the run, one that starts compiling fails it too. The first names `sequences/melody.k7x` in every mode; `CONFORMANCE_SONG_NATIVE` says the second's name: `nodes/ghost.k7x` under `on`, `ghost` with `off` or `shadow` (the TypeScript's message names no file for it) | the failures are the output; a sequence that fails to load is named by its own file by both compilers (812), and the native compile locates an unresolved module at the node that extends it (813) |
| 908 | `CONFORMANCE_ENGINE_BIN` names the binary; else `<repo>/dist/k7-runtime/k7-runtime` (`.exe` on Windows). A binary that is not there FAILS the render tests with that sentence; nothing is skipped silently. `e2e-macos.sh` passes both variables | a check that can skip silently is a check that cannot fail |
| 909 | k7-compile's `the_staged_corpus_compiles_natively_and_the_two_broken_name_their_files` runs over a staged corpus: `open`, then `engineDoc`, then `projectErrors`; each document is decoded strictly into `song::doc::ProjectDoc` with no `null` in it, the two broken (`e2e/broken-sequence.k7`, `e2e/broken-unresolved.k7`) are exactly those that throw, each naming its file as 907, and only they list an error. Floors: 175 documents decoded, 2 refused. Not ignored: `a_broken_project_fails_naming_its_file_and_its_repair_compiles` over `probe_workspace` (a `nodes/ghost.k7x` extending nothing, then corrected). `scripts/stage-gates.sh`, part of `npm run ci:local`, stages and runs the corpus test | it runs on any staging of the corpus and needs no engine; the small negative runs in every `cargo test`. |
| 910 | `goldenCorpus.ts`, `goldenCorpus.test.ts`, `makeGoldenCorpus.ts` and the committed fixtures are unchanged. `goldenCorpus.test.ts` is the TypeScript's own pin while `off` is the rollback, and the files are what `doc_golden.rs` and the song crate's `dsp_literal` tests decode. A golden change is still regenerated and its diff read | step 5 deletes the TypeScript; until then the rollback must not drift unseen |

### Measures and the merge

| # | decision | why |
|---|---|---|
| 917 | A branch is checked on the Mac (`gate.sh`, `bed.sh`); no CI job runs a branch | the checks are the developer's, and Windows is set aside |
| 918 | Decision 761's Windows figure is read from a Windows run's `disk_cost` output. A first pass under 5 % of a compile on Windows closes it with no memo; above, the memo is built in a follow-up. The phone's figure is in the Risks (about 19 ms per 2,000 folders) | the Risk row |
| 919 | No phone bed for 2m: 914 means no phone runs the native compile yet, and the figure of 918's phone half is in hand | a switch inert on the phone has nothing to measure there |
| 920 | The merge: `main` into the branch (generated files by `npm run rebuild:generated`), `npm run ci:local` and the Rust checks, then a `--no-ff` merge in a detached worktree of `main` | the way every step merged |

### The builder's own (941 on)

| # | decision | why |
|---|---|---|
| 941 | `e2e/broken-sequence.k7` is named by `sequences/melody.k7x` under every mode: 812 makes the native error name the sequence's own file, as the TypeScript does | 907 asserts the name whichever compiler answers |
| 942 | `projectTree.ts` is `golden.ts` renamed: `REPO`, `repoFiles` and `putTree`, which six specs import; `engineRender.ts` holds `engineBinary`, `renderDoc`, `peakOf` and `sounds` | the corpus' normaliser has no reader left |

## The seams

| neighbour | what 2l gives it | what 2l takes |
|---|---|---|
| 2b, 2c | `refused`, and mounts equal to the TypeScript's | `Roots`, `read_family`'s findings |
| 2e | | `Rig::from_json` (250), `Inputs` |
| 2f | the bundle says `Outside` and `Io` | the replay gaps of the Clips page's risks |
| 2h | `Request::jsdsp_failed`, `Compiled::jsdsp_builds`, `known_digest`, `known_stale`, the digests and the stale list in a bundle | `compile`, `Compiled`, `Cache::digest`, `AutoFreeze`'s arrays (359) |
| 2i | | `Pictures` is filled inside `compile`: the host passes nothing for it |
| 2j | `lengths` in the bundle (611) | `answer`, `Want`, `Views`, `ByTable`; `path` of a catalogue row is absolute (620) |
| 2k | | `load_compiled_graph`, `GraphError`, 433's `Folder::Other` |
| step 1 | `Session`, `songRoots`' rules to port | |

## The gate

`rust/tests/all/bundle_shape.rs` holds the JSON (every field round-trips, an unknown key is refused, an old bundle reads). `rust/tests/all/bundle_replay.rs` reads a
temporary tree through a `Recorder` on a `DiskSource` and again through the bundle it made, each answer equal, none `Missing`, each case asserting a pointer into
the bundle that shows the path was asked: a link out as a file, as a folder, as a family's entry, as a whole family folder; `clips/a.wav/sub.wav` on every platform (Io on Unix, `absent` on Windows); a pattern, a
`widgets.js` and an assembly template that are not UTF-8, the script also through a vocabulary hit and a family file through a cache hit; a family file that
answers `Io` (mode 000, skipped as root); a folder that will not list; a listing that names a link out. `rust/tests/all/bundle_absent.rs` replays Windows' answers from a fake Source (a name below a file), a file in a folder of mode 0311, a folder mount that is not there, a refusal with a mount below it and a file seen at length 0 and read as binary. `rust/tests/all/bundle_bounds.rs` holds the bounds of 797.
The stage gates (`stages`, `stages_clips`, `stages_lanes`, `curves_stage`) run unchanged through the new `Recorder`, and `stages` also asks each library family
folder that is a link out of its mounts: 4 over the staged corpus, each refused and replayed as refused.

`rust/tests/all/path_rule.rs` reads a Windows desktop app's roots and bundle on any machine (`C:\x.wav` absolute, `sounds\a.wav` a relative path with a separator, a `\` in a lane name refused), and holds `src/` free of any third `cfg!(windows)`; each other site of the rule has its case under both rules in the test of its own file. `known.rs` holds the digests and the stale verdict of a bundle, `jsdsp_host.rs` the builds and the failed build, `session.rs` the hold, the key, the bound and the bundle.

The stage gates (`stages`, `curves_stage`, `stages_clips`, `stages_lanes`, `stages_pictures`, `stages_views`, `stages_graph`) and both twin checks run with every project read through the bundle it made, `e2e/library-rack.k7` included: the views and graph dumps carry the roots' `also_readable` that `songRoots` gives (736, 737), and the document gate's roots leave it out, as the TypeScript refuses the link there (389).

### Hostile input

Each is answered in under five seconds, thirty on musl.

| pass | worst input and answer |
|---|---|
| the bundle | 100,000 `refused` paths and a path a million names deep below none: `Missing`; a refused entry of depth 2 and a stat at depth 10,000: `Outside`; 200,000 absent stats over 100,000 held paths; `binary` on a length of `u64::MAX`: one byte; a file with text and a length, or text and an `io`: refused; a `failedStats` key that does not normalize: refused; more than `MAX_ENTRIES` named paths: refused; a bundle that says `overflow`: refused |
| the keys | a key of 20,000 names: refused before anything is built; 200 distinct keys of 1,000 names: refused by the weight of their folders; 40,000 keys of 3.6 KB and one listing of 140,000 entries of 1 KB: refused by the weight; 200,000 mounts: refused |
| the Recorder | 10,000 folders that are each a link out: 10,000 refusals; past `MAX_BUNDLE` and past `MAX_ENTRIES`: it stops and `overflow` says so |
| the session | a fifth answer drops the oldest; a hundred requests of different rigs are held to four; `rev` equal and `roots` different is not a hit; a read set with an unsettled file is not reused without `rev`; a session past the byte budget is dropped and the next answer is right |
| the host | a request of 100 MB of garbage, a `ws` of 1 MB, a project that does not exist, a `roots` with no `project`: status 1 with a sentence, the runtime alive |

### Capacity

2h's capacity project (128 lanes of 200 events of 16 notes, 16 plugins of 60 params a lane, 16 buses, 64 VST freeze points, 3,000 automations), on a disk, through a `Session` on this Mac: every view cold in 0.73 s and held in 6 µs; the engine document 35.1 MB, the web project 18.6 MB, the graph 3.3 MB, the hints 233 KB; the session holding them 105 MB; its bundle 2.6 MB, made in 0.26 s. The engine document and the session are past a third of the heap and of the budget (decision 808).

## Risks

| risk | answer |
|---|---|
| a native-only WARNING that matches an entry is not logged | `noted` is reported per workspace; the designer reads the count |
| the disk Source's real-path checks cost on Windows (a handle per directory) | measured over 2,000 folders in a release build: on the A069P a plain `lstat` 3.80 ms, a first pass of the Source 22.95 ms, the same Source again 9.03 ms; on this Mac 2.35, 19.03 and 3.96 ms (`disk_cost.rs`); about 19 ms per 2,000 folders on both, so the per-compile memo is enough on the phone, and a memo that outlives a compile waits for the Windows figure, which only `rust checks windows` gives |
| a `binary` file with no digest hashes as one byte (747) | reads as not ready, as `#<len>` does |
| a refusal kept for a folder with a mount below it answers for that folder alone | every path below it that was asked is an entry of its own, and the mount below is allowed by the disk too |
| the engine document of a full-size project is 35 MB, the session over 175 MB (808) | measured: the runtime's heap is 192 MB (96 MiB is the `Config` default); accepted by 916 |

