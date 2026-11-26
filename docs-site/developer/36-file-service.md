# The file service: an account's files, in the front

`apps/k7-daw-backend/packages/k7-file-service` is the only code that touches a workspace's files. k7's
Node backend has no file operation of its own: the Rust front answers `/api/files/*` itself, one
runtime per account held in memory ([file-hub](./37-file-hub.md)). An engine's machine runs no file
service; what the server needs from that machine is a job its supervisor runs over the tunnel.

The crate is plain functions over a workspace's folder, with no HTTP and no runtime of its own. The
front's half (`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/`) is the routes, the account's state, the
live capture and the engine link. The choices are in `apps/k7-daw-backend/docs/service.md` and
`file-native.md`; this page is the map.

An account is a `Held` made on its first request: its workspaces, engine link, caches, undo journal
and capture live there, and an idle account costs memory only. No process, thread or
JavaScript runtime exists per account. A route no piece answers is refused with a sentence
(`apps/k7-daw-backend/docs/declines.md`), and nothing falls back to anything.

## Where each concern lives

| Concern | Crate module |
|---|---|
| Path model, confinement, the workspace tree and reads | `paths.rs`, `confine.rs`, `read.rs`, `tree.rs` |
| Entries: folders, moves, renames, deletes to the recycle bin | `entries/` |
| Revision token | `revision.rs`, `revisions.rs` |
| Undo journal | `journal/` |
| Workspaces, projects and their lifecycle, the bench | `workspaces/`, `projects/`, `bench/` |
| Listings (references, playlists, libraries, chains) | `listings/`, `chains/`, `libraries.rs`, `pins.rs` |
| Compile, the change set and the verb host | `songs.rs`, `changes/`, `verb_bundle.rs`, `verb_host/` (the compiler is the `song` crate) |
| The engine link and its load, sync and reconcile | `engine/` |
| Engine jobs: auto-freeze, previews, separations, plugin queries | `jobs/` |
| Waveforms, media, model downloads | `media/` |
| Shares, export, picture render | `shares/`, `apps/k7-daw-backend/packages/k7-picture` |
| Logs, background registry | `runtime/` |

## Authentication

The front authenticates the user and stamps the account; there is no second process to trust.
`x-studio-user` (the email), `x-studio-workspace` (resolved to a workspace, 404 if unknown) carry the request's identity; their names are in
`apps/k7-daw-frontend/packages/web-protocol/engineAccess.ts`. An account's workspaces are held to the one recorded owner
(`service/owner.rs`).

## HTTP API

There is no event stream: the browser polls (`GET /workspace` every couple of seconds,
`GET /revision`). The account is a WebSocket *client* of the engine and of the engine machine's jobs
socket.

### Ungated

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | `{ ok: true }`. |
| GET | `/media` | A workspace file for `<video>` (`user`, `workspace`, `path` in the query). |

### Account and workspaces (no workspace header)

| Method | Path | Purpose |
|---|---|---|
| GET, POST | `/workspaces` | List; create `{ name }` (pins the libraries). |
| POST | `/workspaces/remove`, `/workspaces/copy-project`, `/workspaces/bench` | Remove a workspace; copy a project between workspaces; open the bench. |
| GET | `/workspaces/projects`, `/workspaces/surfaces` | Every project (home page); projects a second device can open. |
| GET | `/engine/served` | The project the engine holds for this user. |
| GET | `/plugin-params`, `/plugin-native-ui` | A scanned plugin's parameters; its native window. |
| GET, PUT, POST | `/account/live-chains`, `/account/cold-chains[/:which]`, `/account/cold-chains/download`, `/account/models/:id` | Account-level chain choices and model downloads. |

### Inside a workspace

| Group | Routes |
|---|---|
| State | `GET /workspace` (the poll; also prewarms the engine), `GET /revision`, `GET /project`, `GET /project-doc` (timeline view), `GET /engine-doc` (the compiled `ProjectDoc`), `GET /graph` (`WebGraph`), `GET /surfaces`, `GET /project-errors`, `GET /hints`, `GET /tree`, `GET /workspace-tree` |
| Files | `GET /file`, `PUT /file` `{ path, content }` (a `.yaml` path is refused: every workspace document is `.k7x`), `POST /upload`, `POST /folder`, `POST /rename`, `POST /move`, `POST /entry/delete` (to the recycle bin), `POST /fmt` |
| Projects | `POST /project/create` `{ shape, name }`, `POST /open` `{ path }`, `POST /project/delete`, `POST /project/rename` |
| Modules | `POST /module`, `POST /module/clone`, `POST /node`, `POST /library/move`, `GET /module-libraries`, `GET /module-catalogue`, `GET /base-modules`, `GET /module-ui`, `GET /module-engines`, `GET /module-base`, `POST /module-base/snapshot`, `GET /presets`, `GET /consoles`, `GET /io-ports`, `GET /library-chains`, `GET /library-assemblies` |
| Verbs | `POST /verb` (a line run by the verbs crate, landed as one step), `POST /verb/complete`, `GET /verb/catalogue`, `POST /picture` and `POST /picture/scene` (the picture), `POST /import`, `POST /changes` (a change set, whole or not at all), `/verb-host/*` (what a verb asks of the machine: a render, a probe, a separation) (`docs/verbs-port.md`) |
| History | `GET /history`, `GET /history/entries`, `POST /undo`, `POST /redo` |
| Engine | `POST /engine/project` (a browser-built doc), `POST /engine/unload`, `POST /engine/sync/retry`, `POST /auto-freeze/thaw`, `GET /project-plugins`, `GET /project-compat`, `POST /models/download` |
| Audio views | `GET /waveform`, `POST /waveforms/prewarm`, `GET /waveforms/status`, `POST /waveforms/cancel`, `POST /preview` |
| Sharing, export | `GET /export` (`wav`/`mp3`), `POST`/`GET /share/link` |
| Misc | `GET /preferences`, `PUT /preferences/vim`, `GET /playlists`, `GET /references`, `GET /resources`, `GET /logs` |

## Writes, revisions and the journal

`PUT /file` and every change set run inside one change (`changes/`): live capture is held for the
window, the tree is snapshotted before and after (texts, binaries by `size:mtime`), and the
difference is pushed onto the project's undo journal (`journal/`, at most 1000 entries, kept under
`<STUDIO_HOME>/.history`, outside the project). Binary pre-images are saved before they are
overwritten; on a bound engine machine they are kept there.

The **revision token** is how every derived view knows it is stale: a count of mutations plus the
newest mtime among the active project's `.k7x` and `.vstpreset` files (and the machine's
preferences). The compiled `ProjectDoc` and the `WebGraph` are memoised on it.

## From a save to the engine

The **active project** is the one a play opened (`POST /open`), remembered per workspace under
`<root>/.active-projects/`. For it, the engine link (`engine/`, `service/native/engine/`):

1. keeps one socket per engine session, reconnecting with backoff;
2. on a trigger (open, a `PUT /file` or change set inside the project, undo/redo, the poll
   when nothing is served) compiles the project, memoised on the revision, and pushes the document;
3. copies to the engine's machine every file the document names that the machine does not hold yet,
   as one `sync` job over the jobs socket; a failed copy holds every document back until
   `POST /engine/sync/retry`;
4. sends the document, then warms it up: wait until the session is loaded, play it silently for half
   a second, restore. The phases are reported through `GET /workspace`.

Module folders are watched separately (`modwatch.rs`) to invalidate the module caches.

## The capture inside the service

The capture and the ledger are native (`apps/k7-daw-backend/packages/k7-capture`, `apps/k7-daw-backend/packages/k7-replica`'s ops). Capture watches the
workspace (100 ms quiet window, 5 s sweep) and turns changes into batches of file operations; writes
it lands itself are recorded first so they are not echoed. The choices are in `apps/k7-daw-backend/docs/file-capture.md`.

## Tests

The crate's tests sit beside its sources and in `apps/k7-daw-backend/packages/k7-file-service/tests/`; the front's in
`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/tests_*.rs`. Frozen vectors are replayed, never
regenerated. The conformance specs (`e2e/specs/conformance/files*`) drive the routes through the
front.
