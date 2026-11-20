# The file service in native Rust

The account's file logic in Rust, piece by piece, on the I/O the front has (`service.md`). The compiler is the `song` crate
(`apps/k7-daw-frontend/packages/song/docs/rustCompiler.md`), which the native file service calls directly. 

The crate is `packages/k7-file-service`: no HTTP, no tokio runtime of its own; plain functions over a
workspace's folder, called by the front's routes (`packages/k7-front/src/routes/files/service/`).

## What stays a contract

| contract | why |
|---|---|
| every route's method, path, status, body shape and sentence, as the clients read it; a difference is in "Where it answers differently" below, or it is a bug | the frontend and the specs read them; `@studio/web-protocol`'s types are the answers' shapes |
| the answers stay `@studio/web-protocol`'s hand-written types, and Rust serializes them in the TypeScript literal's key order | no route changes shape, so the frontend imports nothing new |
| the engine's `ProjectDoc`, and every compiled view, comes from the `song` crate | one compiler answers every route |
| the relay's wire (`SYNC_PROTO` and its frames) is unchanged by the native capture | the relay (`k7-sync`) and every replica keep syncing |

## The rules every piece follows

| choice | why |
|---|---|
| a native read takes no lock; native writes of one workspace take its write lock, in order | the files are written atomically (a temp, then a rename), so a read never sees half a file |
| a native route runs on the front's runtime (`spawn_blocking` for the file calls) | blocking file calls must not hold the async workers |
| the active project is read from its marker (`.active-projects/<workspace>`) | one reader per piece, never a second memory |
| a piece that cannot answer declines with a sentence (`declines.md`) | a decline costs one call where a guess would be a wrong answer |

## The inventory

Every piece answers on every host. Each has its own page for the choices.

| piece | routes | page |
|---|---|---|
| reads | `GET /file`, `POST /fmt`, `GET /tree`, `/workspace-tree`, `/media`'s path | `file-native.md` |
| entries | `/folder`, `/move`, `/entry/delete`, `/rename`, `/upload`'s halves | `file-entries.md` |
| revision | `GET /revision` | `file-native.md` |
| journal | `PUT /file`, `/undo`, `/redo`, `/history`, `/history/entries` | `file-journal.md` |
| listings | `/references`, `/playlists`, `/module-libraries`, `/account/frozen-chains`, `/account/cold-chains`, `/account/live-chains` | `file-listings.md` |
| workspaces | `/workspaces` and its acts, `/workspaces/bench` | `file-workspaces.md` |
| projects | `/open`, `/project/create`, `/project/delete`, `/project/rename`, `/project` | `file-projects.md` |
| capture | no route: a device's workspaces join no relay | `file-capture.md`, `files/replica.md` |
| engine | `GET /workspace`, `/engine/project`, `/engine/unload`, `/engine/sync/retry`, `/engine/served` | `file-engine.md` |
| engine jobs | `/preview`, `/plugin-params`, `/plugin-native-ui`, `/module-base/snapshot`, `/auto-freeze/thaw`, `/account/slot` | `file-engine-jobs.md` |
| media | `/waveform`, `/waveforms/*`, `/models/download`, `/account/models/:id` | `file-media.md` |
| exports | `/export`, `/export/tracks` | `file-shares.md` |
| runtime | `/logs` | `file-runtime.md` |

## Where it answers differently, on purpose

| difference | why |
|---|---|
| the compile's answers are the `song` crate's | `apps/k7-daw-frontend/packages/song/docs/rustCompiler.md`, "Where it answers differently" |
| the revision token is spelled differently | no client reads it but for equality |
