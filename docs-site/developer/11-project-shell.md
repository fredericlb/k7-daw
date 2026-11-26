# project-shell: what an app needs before it can draw

`apps/k7-daw-frontend/packages/project-shell` (`@studio/project-shell`) is what any app that opens a project needs before drawing anything: the launch address, the file-service transport, the gate that opens the project, idle handling, undo, and the app's own confirm and toast. k7 mounts it.

## Exports

| Subpath | File | Holds |
|---|---|---|
| `./launch` | `apps/k7-daw-frontend/packages/project-shell/src/launch.ts` | Re-exports of the address parsing in `apps/k7-daw-frontend/packages/web-protocol/projects.ts` (`parseLaunch`, `parseExternal`, `externalUrl`, `isExternalIndex`, `projectBasename`, `launchUrl`…) plus `projectsUrl`. The parsing lives in web-protocol because k7's home page writes these addresses and both editors read them. |
| `./fileApi` | `apps/k7-daw-frontend/packages/project-shell/src/fileApi.ts` | The transport to the file service: `configureFileApi`, `fget`, `fsend`, `fileHeaders`, `mediaUrl`. |
| `./files` | `apps/k7-daw-frontend/packages/project-shell/src/files.ts` | Routes that need no compiled view: open, bench, tree, read, write, fmt, folder, rename, move, delete, sync state, engine unload. |
| `./project` | `apps/k7-daw-frontend/packages/project-shell/src/project.ts` | `projectApi`: the shared routes plus the compiled-project ones (commands, completion, undo/redo, preferences, vim). k7's `api.ts` spreads it. |
| `./gate` | `apps/k7-daw-frontend/packages/project-shell/src/ProjectGate.tsx` | The gate component. |
| `./undo` | `apps/k7-daw-frontend/packages/project-shell/src/undo.ts` | `runUndo` / `runRedo`. |
| `./ask`, `./toast` | `src/ask.tsx`, `src/toast.tsx` | The app's own confirm/prompt (replacing the OS box, which ignored the Escape ladder) and toasts. |
| `./profiling` | `src/profiling.ts` | `PROFILING`, on with `?profile=1` or `studio.profile` in localStorage. |

## The file transport

`configureFileApi` is called **once**, by the gate, with the file hub's base URL, the workspace, and a ticket `<ticket>~<engineId>`. Every request carries the user, workspace and token headers; the cookie is never used, because the file service only trusts the hub.

`fget` shares identical GETs in flight (keyed by path and a write epoch) but is not a cache: every route answers from disk now, with `cache: "no-store"`. A read has a 120 s timeout, is retried once when its body breaks off (the service was restarting), and renews an expired hub ticket once on a 401 through the `renewTicket` callback. JSON parsing is timed through an injectable `Tally` (`setFileApiTally`) so k7's freeze watchdog can see it.

## The gate

`ProjectGate` resolves the address into a project and walks through phases:

| Phase | Shown |
|---|---|
| `resolving` | spinner |
| `cloning` | the bench being built |
| `loading` | the project on its way to the engine: copied, applied, warmed (`src/EngineLoadScreen.tsx`) |
| `open` | the app (`children`) |
| `missing` | the project is not in the workspace; no Retry |
| `no-libraries` | the origin has no library; the host renders its own card (`renderNoLibraries`) |
| `empty` | the workspace holds no project; the host renders its wizard (`renderEmpty`) |
| `error` | with a fault classification (refused origin vs. dead service) |

Only `@local/<workspace>/<project>` and `@banc` addresses resolve; an older two-segment address is refused rather than guessed. The gate takes all its wording through `strings`, since each app owns its language.

## Undo

`runUndo` / `runRedo` call the file service's journal, ask `confirmProcess` before a step that restores a bounced audio file, open the first file the step changed and notify. Each app supplies those effects through `HistoryDeps`.

## Tests

`npm run test` in `apps/k7-daw-frontend/packages/project-shell`: launch parsing, the transport's retry and sharing, command and completion routes.
