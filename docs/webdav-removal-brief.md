# Work order: W10, the server's workspaces leave (branch `webdav` only, never `main`)

The decisions are the share's plan (removed with the share, G0 of `project-sync-brief.md`; "What goes with the server's workspaces"); this page splits the removal
into milestones a builder works from alone. Rows are numbered 3100 to 3199. Every milestone ends with
`bash scripts/agent/gate.sh --milestone` green and leaves `webdav` building: a route leaves together with every caller
of it (Rust, TypeScript, e2e, docs) in the same milestone, never one end before the other.

What STAYS, whatever a grep suggests: project shares (`file-shares.md`, W8's upload, the médiathèque's share store and
the server's engine that plays them); the pairing of a device with a server (device tokens, `/api/sync/devices*`,
`/api/sync/remote*`, the device door) since shares, the médiathèque's server copy, favorites, practice and the recorder
ride on it; the engine's own pairing (`apps/k7/frontend/src/pairing/`, `k7-local`'s `pairRequests`), which is another
thing; the capture (`k7-capture::capture`, `worker`, `ledger`), which feeds the log and the history room (its former `dav/` went with the share, G0 of `project-sync-brief.md`); the médiathèque's
and the recorder's doors under `/api/sync/` (`mediatek`, `library`, `moments*`, `publications`, `k7`, `favorites`, `practice`).

## The tracks

```
W10a download my workspaces ──> W10b remote engine ──> W10c libraries off the relay ──> W10d device workspaces off the relay
                                                                                         (needs W9) ──> W10e the relay ──> W10f hosted workspaces ──> W10g sweep
```

One builder, in order. W10a ships first and alone if the user wants a release between it and the rest (3103).

## Rules every builder follows

- Work in the slot named at launch; `eval "$(node engine/scripts/sccache.mjs agent --export)"`, `--target-dir ~/.cache/k7-slot-<n>`.
- Narrow checks while building (`cargo test -p <crate>`, `npx vitest run <file>`), `docs/agent-self-check.md` then `gate.sh --milestone` at the end.
- No push, no merge, no `npm install` or `npm ci` (a removed npm workspace leaves `package-lock.json` by hand).
- A new crate dependency is a stop-and-ask unless this page names it. W10 needs none.
- Keep `.claude/state/webdav.md` at every commit; the report is the short format (`agent-playbook.md`).
- Files under 500 lines, English, one-line body comments, headers of 10 lines at most.
- A deletion is proved by the compiler and `npm run lint:dead` (knip), not by a grep alone; `npm run lint:docs` refuses a page naming a deleted file.
- A page that described a removed part is deleted or rewritten AS IT IS now (AGENTS.md: no history, no "used to").
- Every new route comes with the test that fails without it, shown by reverting it.

## Decisions (APPROVED by the user on the recommended side, 3101 to 3108)

| # | decision | recommended (APPROVED) | other |
|---|---|---|---|
| 3101 | a server workspace nobody downloads | its folder stays on the server's disk untouched; W10 deletes code, never data; it stays listed and downloadable through 3110's routes with no end date; deleting is a later admin act | removed N days after the release, with a notice on the hosted home page |
| 3102 | the hosted site's view of a workspace | none: its home page lists the account's server workspaces with "Open in k7 on a computer or phone" and a per-workspace download link (3113), and opens none | a read-only project view on the server (needs the account runtime kept, which is most of W10f) |
| 3103 | how the download reaches a device | new per-file export routes on the server (3110) that read the account's folder directly and survive the relay's removal, so one release can carry both; the existing room download (`download_workspace`) goes with the relay | reuse `download_workspace` over the relay, which forces a release with W10a alone, then one without the relay once everyone downloaded |
| 3104 | the engine registry's remote engines | the server's registry lists only engines that heartbeat to it for itself (its own engines, which play shares and run the médiathèque's jobs); a device's engine is no longer listed at its paired server, `/api/server-engines` and the device door's registry routes go | keep a device's engine listed at the server with nothing to play |
| 3105 | the libraries on a device | the published versions only, fetched from the paired server's `/api/public/libraries` as an unpaired device already does (`k7-libraries/src/fetch.rs`); the `@lib/` follow and the libraries' rooms go; an admin's draft reaches no device until published | a read-only library follow kept on a reduced relay |
| 3106 | the account's own library (frozen chains, own chains) | local to each device, never synced; it travels inside a WebDAV share only if a later decision puts it there | a `.k7/account-library/` beside a shared workspace now |
| 3107 | history | W9 (local history) is merged into `webdav` before W10d; W10d stops if it is not | W10d first, the History panel showing the undo journal only until W9 |
| 3108 | `packages/live-sync` and `k7-sync`'s leftovers | delete the package, its generators under `apps/k7/backend-rs/fixtures/` (`settle*.ts`, `diff3Vectors.ts`) and its e2e users; the committed vectors stay as frozen referees; `k7-sync` keeps only what a crate still calls (`fsx`, `json`, `path`, `bases`, `settle`, and `log`/`history` if W9 uses them), not renamed | keep the package as the referee generator |

## W10a — download my workspaces (track E, rows 3110)

| # | row |
|---|---|
| 3110 | the server's routes, in a new `k7-front/src/routes/files/export.rs`, behind the session or the device door's token: `GET /api/account/server-workspaces` (name, files, bytes), `GET …/{name}/manifest` (path, size, SHA-256; `.sync*`, dot-named and the bench left out, as `dav/tree.rs` leaves them), `GET …/{name}/file?path=` streamed; they read `$STUDIO_FILES_ROOT/<account>/workspace/` directly, no account runtime, confined as `k7_file_service::confine` confines |
| 3111 | the device's route `POST /api/files/workspaces/import { name }` and `GET …/import` (each workspace's progress): through `routes/remote/forward.rs` with the device token, the download loop of `k7-capture/src/dav/tree.rs` behind a source trait with a WebDAV and a server implementation, into a dot-named sibling renamed into place; the new folder is marked `local-only` (`records::mark_local_only`) and never joins a room |
| 3112 | a name already taken here gets `local_name_for`'s suffix; a failed import leaves no folder and says which file |
| 3113 | the frontend: `ServerOnlyList.tsx` takes its rows from 3110 and gains "Download all"; each Download calls 3111; the hosted site's home page shows the same list with a per-file-tree download link (3102) |
| 3114 | docs: a new `export.md`; `apps/k7/frontend/docs/projects.md`; the user docs in `apps/k7/docs/` (one paragraph: how to bring a workspace down) |
| 3115 | gate: a front test serves a fixture account folder and a device front imports it byte for byte (audio included); a path with `..`, a link out of the folder and another account's name are refused; one e2e spec in `e2e/specs/conformance/` |

## W10b — the remote engine leaves (rows 3120)

| # | row |
|---|---|
| 3120 | the device stops listing its engine at its paired server: `k7-front/src/routes/files/pairing/follow.rs` and its calls in `pairing/mod.rs`, `remote.rs`, `direct.rs` |
| 3121 | `/api/server-engines` goes: `k7-front/src/routes/remote/server_engines.rs`, its line in `remote/mod.rs` and `lib.rs`; `k7-local/tests/all/front.rs`' case |
| 3122 | the device door's registry routes go: `k7-front/src/routes/engine/door.rs`, `ALLOWED_EXACT` in `door.ts` and `door.test.ts`; `engine/devices.rs` stays if a heartbeat's bearer still asks it, else goes (the compiler says) |
| 3123 | the page: the engine-browser `serverEngines` client and its test (done, deleted), `via` in `access.ts` and `packages/web-protocol/engineAccess.ts`, `apps/k7/frontend/src/api.ts`, `components/EngineGate.tsx`; `packages/engine-browser/package.json`'s export |
| 3124 | docs: delete the backend's and the frontend's `serverEngines` pages (done); edit `localEngine.md`, `engine.md` (the door's row), `packages/engine-browser/docs/access.md`, `engine/ownership.md`, `files/serverClient.md` |
| 3125 | `EngineSyncProgress.tsx`, `EngineSyncFailedDialog.tsx` and the tunnel's file copy stay while a server engine still plays a share through them; if the builder finds no such caller left, stop and ask |
| 3126 | gate: `e2e/specs/conformance/localEngine.spec.ts` and `engines.spec.ts` lose their remote cases; a heartbeat by a device token for a device engine is refused, by a test |

## W10c — the libraries without the relay (rows 3130)

| # | row |
|---|---|
| 3130 | the origin's libraries relay nothing: `k7-front/src/routes/files/service/libraries.rs` drops `STUDIO_SYNC_URL`, the `LibraryRelay` and the `link` it builds; `hub/spawn.rs` the libraries' `STUDIO_SYNC_URL` |
| 3131 | a paired device brings libraries as an unpaired one does (3105): `k7-libraries/src/follow.rs` and `remote.rs` go or fold into `fetch.rs`, origin = the active server's URL from `k7_pairing::remotes` |
| 3132 | `k7-capture/src/library.rs` goes; `FOLLOW_PREFIX` and `.sync-follow/`, `.sync-replica/<name>` are no longer read; `k7-libraries/tests/all/relay.rs`, `followed.rs` go, `hostile.rs` gains the paired-origin case |
| 3133 | the account's own library is no room any more (3106): its row in `libraries.md` and `files.md` |
| 3134 | docs: `apps/k7/backend-rs/docs/libraries.md` "The relay" section rewritten; `files.md` "The libraries" |
| 3135 | gate: a paired device front with a fake origin gets a published version; a draft does not arrive; `e2e/specs/conformance/filesLibraries.spec.ts` green |

## W10d — the device's workspaces leave the relay (rows 3140, after W9)

| # | row |
|---|---|
| 3140 | the capture links to no relay: `k7-capture/src/link/` whole, `account/rooms.rs`, `account/ended.rs` (the room's end; a local deletion keeps what `dav/` needs), `account/hidden.rs`, `account/inert.rs` if unused, `servers.rs`; `account/out.rs` keeps the log and the WebDAV queue; `account/shared.rs` loses "hands its edits back to the relay"; `account/attach.rs`, `folders.rs` lose join and room names |
| 3141 | the environment: `STUDIO_SYNC_URL`, `STUDIO_SYNC_TOKEN`, `STUDIO_LIVE_SYNC` out of `hub/spawn.rs`, `k7-hub/src/env.rs` (and `fixtures/hub-vectors.json`' sync-target case), `k7-front/src/config.rs`; `k7_pairing::remotes::sync_target` and `home_links` go |
| 3142 | the six routes of `k7-front/src/routes/files/service/native/capture.rs`: log, project, conflicts, resolve, status and download go; `open_prepare.rs`' sync state and `engine/flow.rs` `sync_state` go; `k7-replica`'s `Replica`, `store.rs` and converge tests go, `ops.rs`, `blob.rs`, `Confirmed` stay (the capture's) |
| 3143 | the page: `ConflictsPanel.tsx`, `hooks/useConflicts.ts`, `syncConflicts.ts` and test, `projects/WorkspaceSyncIcon.tsx`, the sync columns of `ProjectsTable.tsx`, `usePolls.ts`, `StartupBar.tsx`, `ShellFooter.tsx`, `ShellOverlays.tsx`, `App.tsx`, `EditorTab.tsx`; project-shell's `syncWait.tsx` and its use in `ProjectGate.tsx`, `useGatePhase.ts`, `gateTypes.ts`, `EngineLoadScreen.tsx`, `files.ts`; `packages/web-protocol/workspaceTypes.ts`' sync types; `ServerOnlyList.tsx` keeps only W10a's rows |
| 3144 | docs: delete the frontend's `conflicts.md`; edit `projects.md`, `shell.md`, `packages/project-shell/docs/launch.md`, `fileApi.md`, `apps/k7/backend-rs/docs/file-capture.md`, `files/replica.md` (what is left of the crate) |
| 3145 | the k7-capture tests that go: `link*.rs`, `relay_*.rs`, `rooms.rs`, `servers.rs`, `account_attach.rs`' relay cases, `capture_sync.rs` if it is only the room's; `k7-front` `tests/all/files_sync.rs`' capture half, `native/tests_capture.rs`, `tests_hidden.rs`, `runtime/tests_hidden_wire.rs` |
| 3146 | gate: the WebDAV suite (`dav_*.rs`, `tests_dav*.rs`) unchanged and green; a workspace edited on a device with no server leaves no `.sync/` link record; `e2e/specs/conformance/serviceLive.spec.ts`, `serviceWorkspaces.spec.ts`, `syncConflicts.spec.ts` deleted or cut to what stays |

## W10e — the relay leaves (rows 3150)

| # | row |
|---|---|
| 3150 | `k7-front/src/routes/files/sync/`: `upgrade.rs`, `accounts.rs`, `blobs.rs`, `pages.rs` (`rooms`, `events`; `history*` moved by W9), `mod.rs`' `routes()` and `bare()` and their mounting in `files/mod.rs` and `lib.rs`; `/api/sync` itself becomes the backend's 404, its children above stay |
| 3151 | `k7-sync`: `relay/`, `room.rs`, `protocol.rs`, `events.rs`, `blobs.rs`, `conflicts.rs`, `diff3.rs` and `examples/diff3_measure.rs` unless a remaining module calls them; its tests `relay.rs`, `diff3.rs`; `lib.rs`' header says what it is now (3108) |
| 3152 | the page: the frontend's `syncFeed` module and its test; `useRevision.ts` keeps the `/revision` poll alone |
| 3153 | 3108's deletion: `packages/live-sync`, its line in `e2e/package.json`, `e2e/src/sync.ts`, the fixtures' generators; `package-lock.json` by hand |
| 3154 | e2e: `syncRelay.spec.ts`, `syncRelayLog.spec.ts`, `syncBlobs.spec.ts`, `syncEvents.spec.ts`, `syncHistory.spec.ts` (if W9 did not move it), `serviceHistory.spec.ts`' room half; `e2e/docs/conformance.md` |
| 3155 | docs: delete `files/syncRelay.md` after moving its pairing table to a new `files/pairing.md` (it stays true), `files/blobs.md`, `docs/live-sync-plan.md`; edit `files.md` (the family's paths), `files/history.md` (W9's), `routes.md`, `docs/architecture.md` (the plans list, line on `live-sync-plan.md`), `docs/audit-moteur.md` if it names the relay |
| 3156 | gate: `cargo test --workspace`, `npm run lint:dead` names no orphan of these, `lint:docs` green |
| 3157 | a one-time pass at a server's start, beside the export and the runtime and skipped on a device (`k7-front/src/routes/files/rooms_out.rs`): each account room (`.sync-relay/<ws>`, blobs in `.sync-blobs`) is replayed with `k7_sync::relay::load_room` and its head written into `<account>/workspace/<ws>`; a folder file that is absent is written, an equal one left, a differing one keeps its content and the room's copy lands beside it as `<name> (server room).<ext>` (`server room 2`… when taken); no time is compared, since the server has no reliable per-file time; the log and blobs are never deleted; `folder.done` in the room's folder holds the head written; a room with a missing blob is retried at the next start |
| 3158 | tests in `rooms_out/tests.rs`: a room with no folder gets one, a differing folder file keeps its content whatever its time, also for a legacy room whose load writes its log, a taken beside name is never replaced, a file named like a temp is untouched, a second run writes nothing and the log and blobs stay, a blob ref is written as its bytes, a missing blob leaves no marker and the next run lands it, a link is not followed; each rule shown failing by reverting it alone |
| 3159 | docs: one row and one hazard in `files/export.md` |

## W10f — the hosted site's workspace services leave (rows 3160)

| # | row |
|---|---|
| 3160 | a front that is not a device (`!settings.local_user`) makes no account runtime: `hub/spawn.rs`, `hub/proxy.rs`, `hub/answers.rs`; `/api/files` there answers 410 with one sentence naming the tray and the phone apps, except W10a's export routes and the libraries |
| 3161 | the device door's files prefixes go: `k7-front/src/routes/files/door.rs` and its use in `hub/proxy.rs`, `favorites.rs`, `pairing/mod.rs` kept to what favorites and pairing need; `door.ts` `ALLOWED` |
| 3162 | an engine bound to a server account's workspace: `k7-hub/src/binding.rs` and `seam.rs` keep only the device's own engine; `k7-engine/src/tunnel/files/` keeps what a share's play needs (stop and ask if that is everything) |
| 3163 | the hosted page: `browserAccount.ts`, `desktop.ts` and test, the home page on a server shows 3102's list |
| 3164 | docs: `files/hub.md` rewritten (the hub serves a device's local user and the libraries); `files.md` "The hub"; `docs/architecture.md` lines on the disk and `apps/k7/backend-rs`; `apps/k7/docs/40-technical-shape.md`; `engine-tunnel/docs/tunnel.md` |
| 3165 | gate: `tests/files_hub.rs`, `files_hub_refusal.rs` cut to the device and the 410; `e2e/specs/conformance/filesHub.spec.ts`, `deviceDoorPaths.spec.ts`, `e2e/specs/app/workspace-ui.spec.ts` against a device bed; a share made on a tray still plays on the server's engine (W8's gate rerun) |
| 3166 | decision (user, W10f): on a server the médiathèque's stem cut (`cold-run`) resolves its chain in the libraries' context (`Services::cut`, `$STUDIO_FILES_ROOT/_cut`), not an account's: no `stemsChain`/`instrumentalChain` setting, no draft overlay; roots = médiathèque, libraries and the model's folder; `slot` (the engine slots' compile) takes the same context, roots = libraries and médiathèque read-only (own chains are local per 3106); `serviceChains.spec.ts` stays the device's cut and `serverFiles.spec.ts` holds the server's |
| 3167 | a device minted on a server records the account's `owner.json` (`pairing/devices.rs`), since the device door names an account by it and no runtime is there to write it |
| 3168 | decision (user, W10f): the hosted home lists each server workspace as a lazily loaded file tree, each file a download link on 3110's `file?path=` route (the manifest asked on opening a workspace, 200 entries per folder at a time); no zip, no new dependency: `HostedFiles.tsx`, `hostedTree.ts` |
| 3169 | decision (user, W10f): the hosted home also lists the account's project shares with their revoke and comments, through the existing `/api/shares` routes and `ShareComments`: `HostedShares.tsx` |

## W10g — the sweep (rows 3170)

| # | row |
|---|---|
| 3170 | `docs/inventaire.md` and `docs/architecture.md`: every crate, package and page named above checked against the tree |
| 3171 | `npm run lint:dead`, `lint:docs`, `lint:comments` on the touched files; `Cargo.toml` workspace members and `[workspace.dependencies]` without a removed crate or an unused dependency (`tokio-tungstenite` only if the engine still uses it) |
| 3172 | Android and iOS (`SyncPairing.kt`, `SyncPairing.swift`) and the tray (`backend.rs`) unchanged but for a removed variable they set; `android-check.sh` at the merge gate |
| 3173 | gate: `bash scripts/agent/gate.sh` (the merge gate) green on `webdav` |

## Stop and ask

- Any decision above not yet taken by the user, or W9 not merged when W10d starts (3107).
- A share, the médiathèque, the recorder, favorites or practice that turns out to need a removed route, room or runtime.
- A removal that would touch `main`, the C++ supervisor or engine.
- A user's data deleted by any code path (3101: W10 deletes code, never a workspace folder).
- The tunnel's file serving or the engine copy turning out to be used only by the remote engine (3125, 3162).
