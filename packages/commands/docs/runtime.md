# The verbs' runtime

The verbs are TypeScript and run where the files are: in a browser Worker, or in node for the CLI and the conformance specs. The
code is `src/runtime/`; the server half is `apps/k7/backend-rs/docs/file-changes.md`.

## Choices

| choice | why |
|---|---|
| the verbs run unchanged over a `Vfs`, with `node:fs`, `path`, `os`, `url`, `fs/promises` and `child_process` aliased to `src/runtime/shims/` by the bundler | 160 verb imports and the song read side keep compiling; one source of verbs for the server's tests and the browser |
| a path under a folder nobody listed is unknown, never absent: the read throws `NeedsFiles` and the runtime fetches and runs the verb again | the bundle holds the project, the shared texts and the libraries; anything else a verb reads must not be read as "missing" |
| the runtime asks `vfs.takeNeeded()` after every run, whether the verb threw or swallowed the error | a verb that catches `ENOENT` around a stat would otherwise turn an unknown file into a wrong answer |
| a path outside the workspace and the libraries reads as absent, with no fetch | the browser cannot reach the machine's own files (the plugin scan, the models folder); a run is not repeated for a file no fetch can give |
| the writes are staged in the Vfs and posted as ONE change set; a verb that throws or refuses posts nothing | the server lands a set whole (file-changes.md), and a verb that fails writes nothing |
| a text is posted with the hash it replaced (`was`), a file read outside the project with a `read` op, the run with the project's token | a stale `base` or `was` is a 409; a stale module the verb only read is a 409 too |
| on a 409 the runtime refreshes what moved and runs the verb again once, by itself, when it called no host member; otherwise it says "the project moved — run it again" | before any server call a verb is a pure function of the Vfs; a rerun repeats only the edit |
| a 409 that says another project is open is an error, never a rerun | it is a wrong `at`, not a race |
| a picture view asked at a token the runtime has held since it opened answers from what it holds; only a token it never held reopens the project | the page reads its token before the view crosses the Worker's queue, so a token from before the runtime's own last edit is a late read, not a peer's change; a reopen is seconds on a phone and holds up every call queued behind it |
| the compile is the WebAssembly song module (`@studio/k7-wasm/song`), opened over the bundle with the libraries merged in; `h.project()` lays the run's staged texts over it first | song's read side stays TypeScript over the Vfs; the compile is Rust, one source with the server's |
| the song module is a second build of the same crate (`song` feature) | the curves load on the main thread and must not pay 3 MB for a compiler they never call |
| a binary is its length and, for a WAV, its seconds, which the compile reads from the bundle | a walk bundle holds no header bytes; the song crate's `Source` answers `known_seconds` |
| a library is fetched by its hash once per page and mounted at its root | 1.8 MB per k7-core version is the largest thing a project open moves, and it is shared by every project |
| commands.yaml is handed in (`provideCommandsYaml`) | a bundled Worker has no file beside it to read |
| a host member (a render, a probe, a picture, the engine, the plugin list…) is a request to `/verb-host/*` in `src/runtime/members/` and marks the run `hostCalled`; a long one is a job polled each second, its files adopted into the Vfs | nothing in the browser pretends to succeed, a 409 after a server call is said, never rerun, and the files a job made reach the project in the one change set (`apps/k7/backend-rs/docs/verb-host.md`) |
| an import (`importer.ts`) is not a verb: it opens the workspace alone (`at` empty) in a Vfs of its own and runs the translator's emitter over it | a foreign project makes a new folder and shared modules, which no project's verb may write, and it has bytes (samples) a verb never holds |
| the emitter's binaries go to a sink and are uploaded one by one through `/upload` into the folder the set then creates; its texts land as one `/changes` set with `create` | a body cannot carry audio, and a set that fails after the uploads takes the folder (and so the samples) away: apps/k7/backend-rs/docs/file-changes.md |
| the folder's name is checked against the workspace listing before the first upload | a name that is taken must cost no byte of audio; the server refuses a folder that holds a project all the same |
| a `.als` from the tree is parsed twice: once to learn which samples it names, then with those samples fetched | a parse reads its samples synchronously and the Worker's fetch is not; a path that climbs out of the workspace is counted and never fetched |
| the plugin list comes from the `plugins` member, and an engine-less server's 503 reads as "unknown" | the emitter checks plugin names against a scan only when there is one; the engine's parameter dump (a process of the machine) is not asked for, so a plugin's state stays a `.vstpreset` |
| a zip or gzip inflates to at most 512 MiB and names at most 20,000 files; XML that declares an entity is refused whole | the page holds the bytes, and a stranger's archive or entity must not take the tab down |
| the tests that run verbs through the shims use a second Vfs as the server's disk | the real `node:fs` is the shim under the alias, so fixtures and the fake server cannot use it |

## Known differences from a run on the server

| difference | effect |
|---|---|
| the machine's plugin scan reads as absent; the models folder is the bundle's listing of `/s/models` | a module vocabulary lists the plugins the `plugins` route returns, an engine-less front returning none |
| `engine scan` with no engine, or an engine silent for 60 s, prints `scanned 0 plugin(s)` | the route answers an empty list, and the verb words it |
| the compile's reads of workspace files are not checked by `read` ops | a module edited by a peer during a verb's compile is seen at the next run |
| a binary's bytes are not in the bundle | an automatic freeze fingerprint over audio differs from the server's until the bundle carries digests |

## The proof

| proof | what it holds |
|---|---|
| `vitest.vfs.config.ts` runs 67 of the 84 existing verb test files (521 tests) unchanged, with `node:fs` and friends the shims | the verbs run over the Vfs; the other 17 write audio bytes or read the repo's own files from the disk, and are listed in the config |
| `runtime.vfs.test.ts`, over a second Vfs as the server's disk | the change set's shape, the stale rerun, the unknown read, the swallowed read, the wrong project, completion |
| `compile.vfs.test.ts` | the WebAssembly compile over a bundle made from a project, a draft laid over it, a read outside it named |
| `runner.test.ts` | the BUILT node runner and the BUILT Worker (in a worker_threads thread, over HTTP to a fake server): the aliases, the wasm chunk and `?raw` are what is proven |
| `cap128.measure.test.ts` (`K7_MEASURE=1`) | the figures below |

## Measures on `cap128`

One run on the Mac, the Worker in a node thread, the project's files on disk, libraries empty (k7-core is 1.82 MB, about 100 KB gzipped, fetched once by hash).

| figure | value |
|---|---|
| `/verb-bundle` of the project (2,203 files) | 2.55 MB, 50 KB gzipped |
| the Worker bundle | 3.3 MB gzipped: the runtime 1.65 MB, the song module chunk 1.64 MB |
| open: bundle in the Vfs and the compiler's bundle | 240 ms |
| `track rename l0 lane0`, first run (one fetch of a missing file) / the next | 4.9 s (two runs) / 4.9 s |

| choice | why |
|---|---|
| no lazy load by folder | the bundle is 50 KB gzipped against the plan's 8 MB line, and the open is 240 ms against its 2 s |
| the figures of a verb are a TypeScript read side's, not the bundle's | the verb reads the whole project through song's loader over the Vfs; the WebAssembly compile is a fraction of it |

## What the Worker bundle still holds that a browser cannot run

The build lists the `node:*` imports it leaves external and the server-only modules it holds. A server-only module in the Worker
fails the build (`scripts/bundleRuntime.mjs` exits 1, and `finder.test.ts` pins the list empty); the one builtin left
(`node:crypto`) is the song package's own and is pinned there until it stops importing it.
