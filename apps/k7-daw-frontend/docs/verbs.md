# The verbs in the browser

The page runs the command bar's verbs in a Worker, over the files the server hands it (`apps/k7-daw-frontend/packages/commands/docs/runtime.md`
is the runtime, `apps/k7-daw-backend/docs/file-changes.md` the server's half). This page holds the choices of the page's side.

## Choices

| choice | why |
|---|---|
| the Worker is `apps/k7-daw-frontend/packages/commands/scripts/bundleRuntime.mjs`'s esbuild output, served by `verbs/vitePlugin.ts`: from memory in dev, an emitted hashed asset under `assets/verbs/` in a build | the finder, `runner.test.ts` and the measures prove THAT artefact; Vite's own worker pipeline would ship a second, unproven one |
| the three `node:` builtins song still imports (`crypto`, `util`, `worker_threads`) are rewritten to stand-ins in `verbs/shims/`, built beside the Worker | a browser cannot resolve a `node:` specifier, and one static import fails the whole module; the digests are checked against node's at every padding boundary |
| `process` is installed by `prelude.ts`, the Worker's first import | song reads `process.env` while its modules load, before any `init` |
| the asset is left out of the service worker's precache | 3.3 MB gzipped, fetched on first use and kept by the HTTP cache, its names being hashes |
| one Worker per page, one runtime per `at` inside it, sharing the library cache; the calls take turns | the shims hold one current Vfs: two runtimes running at once would read each other's files |
| a call before the Worker is ready waits for it; the page is told `loading` past 300 ms | a click is never lost and never runs against nothing |
| the calls made while a project is opening share that one open | each open fetches and compiles the whole project, about a second on a phone; the page's first calls (the picture, a pin, a knob's writes) each opening it queued eight seconds of opens ahead of a knob's write |
| a Worker that dies fails its pending calls with one sentence, and the next call starts a new one and opens the projects again | a crashed runtime must not leave the bar dead for the session |
| a change of session (workspace, ticket, the engine named in it) ends the Worker; a picture read it failed in flight is asked once more of the new one, a run never is | the page asks the picture once per reload, and picking the engine changes the ticket while that read waits: a phone opened a video project with no picture lanes; a run may have written |
| `refused` and `unsupported` are fields of the answer | a structured clone is not guaranteed to keep an array's own properties |
| a missing host member is a refusal naming the member | it is a known gap, not a failure |
| `applyProject` and `sendProject` are the reload; the separation chains are read from the bundle's preferences | the verb has already written the files, and the server compiles them and pushes the engine after the landing; the preferences travel with the bundle |
| a completion or a run may ask again as many times as the compile finds a new folder to list | a compile meets one unlisted library folder per round, and the bundle cannot name them all |
| a stat outside the workspace is answered "absent" to the compiler as well as to the Vfs | or the compiler asks for the same path every round |
| the second edit of a session may run twice, once | the engine's first load moves the revision once after the first commit; every later run is one run and faster than the server's |
| the front always answers `/changes` and `/verb-bundle`: every native piece answers on every host, with no switch | without them a page runs no verb at all |
| the server runs no verb: `POST /command`, `GET /commands` and `GET /complete` are gone from the front and from node | one runtime, the Worker's |
