# The project's Worker

A browser Worker holds the open project's files in a virtual file system for what the page still reads in TypeScript:
the picture views (`videoDoc`, `videoErrors`), the scene a `picture render` posts, and an import of a foreign project.
No verb runs here: a line is the backend's `POST /verb` (`docs/verbs-port.md` at the root). The code is `src/runtime/`;
the server half is `apps/k7-daw-backend/docs/file-changes.md`.

## Choices

| choice | why |
|---|---|
| the song read side runs unchanged over a `Vfs`, with `node:fs`, `path`, `os`, `url`, `fs/promises` and `child_process` aliased to `src/runtime/shims/` by the bundler | the picture loader and the translator keep compiling; one source for the tests in node and the browser |
| the Worker opens a project only when the page first reads its picture | a verb needs no bundle, so a project with no picture never costs the Worker an open |
| after a line lands, the page hands the Worker the files its step changed (`refresh`), with the token after it | the Worker holds the files of the open project, and a view read after an edit must be the project as it is now |
| a path under a folder nobody listed is unknown, never absent: the read throws `NeedsFiles` and the runtime fetches and reads again | the bundle holds the project, the shared texts and the libraries; anything else a view reads must not be read as "missing" |
| a path outside the workspace and the libraries reads as absent, with no fetch | the browser cannot reach the machine's own files; a read is not repeated for a file no fetch can give |
| a picture view asked at a token the runtime has held since it opened answers from what it holds; only a token it never held reopens the project | the page reads its token before the view crosses the Worker's queue, so a token from before the runtime's own refresh is a late read, not a peer's change; a reopen is seconds on a phone |
| `picture render`'s scene is built here, as the browser's export builds it, every media URL naming its file by its virtual path; the page posts it to `/verb-host/picture` | the compositor's scene is TypeScript, and the render page reaches only what the person could read |
| a library is fetched by its hash once per page and mounted at its root | 1.8 MB per k7-core version is the largest thing a project open moves, and it is shared by every project |
| an import (`importer.ts`) opens the workspace alone (`at` empty) in a Vfs of its own and runs the translator's emitter over it | a foreign project makes a new folder and shared modules, and it has bytes (samples) a change set never holds |
| the emitter's binaries go to a sink and are uploaded one by one through `/upload` into the folder the set then creates; its texts land as one `/changes` set with `create` | a body cannot carry audio, and a set that fails after the uploads takes the folder (and so the samples) away: apps/k7-daw-backend/docs/file-changes.md |
| the folder's name is checked against the workspace listing before the first upload | a name that is taken must cost no byte of audio; the server refuses a folder that holds a project all the same |
| a `.als` from the tree is parsed twice: once to learn which samples it names, then with those samples fetched | a parse reads its samples synchronously and the Worker's fetch is not; a path that climbs out of the workspace is counted and never fetched |
| a zip or gzip inflates to at most 512 MiB and names at most 20,000 files; XML that declares an entity is refused whole | the page holds the bytes, and a stranger's archive or entity must not take the tab down |
| the tests that read through the shims use a second Vfs as the server's disk | the real `node:fs` is the shim under the alias, so fixtures and the fake server cannot use it |

## The proof

| proof | what it holds |
|---|---|
| `vitest.vfs.config.ts` runs the runtime's tests and the translator's emitter tests with `node:fs` and friends the shims | the reads run over the Vfs |
| `video.vfs.test.ts` | the picture views over the corpus' picture projects, and after a refresh |
| `runner.test.ts` | the BUILT Worker (in a worker_threads thread, over HTTP to a fake server): the aliases, the wasm chunk and `?raw` are what is proven |
| `apps/k7-daw-frontend/verbs/workerLoad.test.ts`, `workerFlow.test.ts` | the built Worker in a real Chromium |

## What the Worker bundle holds that a browser cannot run

The build lists the `node:*` imports it leaves external and the server-only modules it holds. A server-only module in the
Worker fails the build (`scripts/bundleRuntime.mjs` exits 1, and `finder.test.ts` pins the list empty); the one builtin
left (`node:crypto`) is the song package's own and is pinned there until it stops importing it.
