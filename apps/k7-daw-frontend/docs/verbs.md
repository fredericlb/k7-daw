# The verbs from the page

The page runs every command line and editing gesture through the backend's `POST /verb`, and completes the command bar
through `POST /verb/complete` (`docs/verbs-port.md` at the root, `apps/k7-daw-backend/docs/file-changes.md` the route). A
Worker holds the open project for its picture views and runs an import
(`apps/k7-daw-frontend/packages/commands/docs/runtime.md`). This page holds the choices of the page's side; the code is
`src/verbs/`.

## Choices

| choice | why |
|---|---|
| one seam, `verbsHost()` of `@studio/project-shell`: the command bar, a gesture, a menu and the chain builder all send a line through `projectApi.command` | a button never calls an edit of its own, so every edit is a line the history names |
| the answer's lines are its log, then its refusal, then an escaped throw as `error: …` | a refusal reads after what the verb said before it |
| a run past a second is listed in the process list | a render or a separation takes as long as it takes, and the request stays open until it lands |
| an ask the backend hands back (`page renderPicture`) is performed by the page after the run: the scene from its Worker, posted to `/verb-host/picture` | the compositor's scene is TypeScript; the film's job outlives the page |
| the Worker is opened by the first picture view, never by a verb; after a line lands, the page hands it the files the step changed | a project with no picture never pays an open, and a picture read after an edit is the project as it is now |
| the catalogue is read from `commands.yaml` by the page itself, loaded with the first palette | it is the catalogue the crate's `commands.json` is written from, and it is no route's to serve |
| a completion that fails answers no items | the palette keeps asking, and a command bar never shows an error for a keystroke |
| the Worker is `apps/k7-daw-frontend/packages/commands/scripts/bundleRuntime.mjs`'s esbuild output, served by `verbs/vitePlugin.ts`: from memory in dev, an emitted hashed asset under `assets/verbs/` in a build | the finder and `runner.test.ts` prove THAT artefact; Vite's own worker pipeline would ship a second, unproven one |
| the three `node:` builtins song still imports (`crypto`, `util`, `worker_threads`) are rewritten to stand-ins in `verbs/shims/`, built beside the Worker | a browser cannot resolve a `node:` specifier, and one static import fails the whole module |
| `process` is installed by `prelude.ts`, the Worker's first import | song reads `process.env` while its modules load, before any `init` |
| the asset is left out of the service worker's precache | fetched on first use and kept by the HTTP cache, its names being hashes |
| one Worker per page, one runtime per `at` inside it, sharing the library cache; the calls take turns | the shims hold one current Vfs: two runtimes reading at once would read each other's files |
| the calls made while a project is opening share that one open | each open fetches the whole project, about a second on a phone |
| a Worker that dies fails its pending calls with one sentence, and the next call starts a new one and opens the projects again | a crashed runtime must not leave the picture dead for the session |
| a change of session (workspace, ticket, the engine named in it) ends the Worker; a picture read it failed in flight is asked once more of the new one | picking the engine changes the ticket while that read waits |
