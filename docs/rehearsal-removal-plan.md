# Removing the rehearsal recorder

The user's decision: the rehearsal recorder is no longer part of k7. Its code goes; the data it stored stays
on disk, unread: no code path is added or kept that deletes a recording, a moment, a release or a file.
A route that goes answers the front's ordinary 404. Rows 33xx are this plan's.

## What goes

| row | zone | what |
|---|---|---|
| 3300 | screens | `packages/ui-rehearsal` (127 files) and its docs, but for row 3320 |
| 3301 | types | `packages/rehearsal-shared`, but for row 3321 |
| 3302 | Rust | `crates/k7-rehearsal` and `k7-front/src/routes/rehearsal/` (22 files), the `rehearsal` field of `AppState`, its claim branch in `lib.rs`, `K7_REHEARSAL`, the PocketBase import, the publish tokens (`/api/token/publish`), `stop_tests`' rehearsal case |
| 3303 | Rust paths | the recorder's 12 prefixes (`/api/config`, `/api/sessions`, `/api/moments`, `/api/releases`, `/api/pages`, `/api/public/pages`, `/api/singing`, `/api/token/publish`, `/api/publications`, `/api/sync/k7`, `/api/sync/moments`, `/api/sync/moments-share`) and its device-door share; `files/mod.rs`' list of them; the seam's `GET /api/internal/files/rehearsal` |
| 3304 | Express | `door.ts`' recorder prefixes, `internalFiles.ts`' `rehearsal` family, the `k7.` → `/rehearsal` alias (`server.ts`, `last/legacy.rs`, `web-protocol/services.ts`), `hardening.ts`' comment, `dev-front.mjs`' switch |
| 3305 | frontend | the `/rehearsal/*` route, `RehearsalPage`, `RehearsalHost`, `RehearsalRecording` above the home routes, the public page scope (`address.ts`, `appMain.tsx`), the sidebar's rehearsal entries, `NewSessionButton` in `HomeFrame`, `RehearsalSettings` in Settings, `MomentUploads` in `SongSyncDownloads`, `configureRehearsal`, the catalogue's build in `vite.config.ts` and `pretypecheck`, `index.css`' `@source`, `.storybook`'s wrapper |
| 3306 | web-protocol | `REHEARSAL_PAGE` and its reserved prefix; a PUBLISHED package, so its version is raised (`docs/publishing.md`) |
| 3307 | recorder | `@studio/recorder`'s take, detection, moment player, tag input, live waveform, backup and hold-to-stop, used by the screens alone |
| 3308 | e2e | `src/rehearsal.ts`, `specs/conformance/rehearsal*.spec.ts` (8), `specs/app/rehearsal.spec.ts`, `rehearsal-take.spec.ts`, the rehearsal cases of `routes.spec.ts`, `serverCopy.spec.ts`, `lastRoutes.spec.ts`, `CONFORMANCE_REHEARSAL_DB` in the three bed scripts and `e2e/docs/conformance.md` |
| 3309 | phones, tray | `K7_REHEARSAL` in `BackendHost.kt`, `BackendHost.swift`, the tray's `backend.rs`; the comments naming the recorder (`MainActivity.kt`, `AppViewsPlugin.kt`, `native.ts`); the iPhone's microphone sentence loses "take a rehearsal" |
| 3310 | image | `/rehearsal-data` and `K7_DATA_DIR` in the Dockerfile and `start.sh`; the compose file's PocketBase service, its three variables and `K7_API_TOKEN_SECRET`; `.env.example`, `env.example.yaml`. The `k7-data` volume's mount is dropped, never the volume: its bytes stay where they are |
| 3311 | docs | the backend's rehearsal pages (10 files), the phase 4c plan, the docs site's eight recorder pages (four developer, four user); every other page rewritten as it is (about 90 pages name the recorder in passing); `AGENTS.md`'s zone row and the two map mentions |

## What stays, and why

| row | what | why |
|---|---|---|
| 3320 | the song attachments (`SongAttachment`, `SheetPanel`, `ScoreSheet`, `ChordDiagram`, `useLyricsLead`, the chord tables, `song.css`) MOVE into `@studio/ui-library`, their words given by the host's labels | the médiathèque's song page draws lyrics, chords, tablature, sheets and diagrams with them |
| 3321 | `sheetKind` moves into `@studio/mediatek` | the médiathèque's song menu and its attachments take a sheet by that rule |
| 3322 | `@studio/recorder`'s `format`, `liveValue`, `metronome`, `webmDuration` | the editor (the home engine, the roll's hover, the camera take, the slots page) and the médiathèque (dates, the song's metronome) use them |
| 3323 | `@studio/score`, `lyrics`, `tablature` | the attachments of row 3320 |
| 3324 | `/api/favorites`, `/api/practice` and their doors `/api/sync/favorites|practice` | the médiathèque's lists, answered by `files/favorites.rs`, not the recorder |
| 3325 | `k7_rehearsal::ids::iso`, `iso_now` move into `k7-js` | the editor's take push and the shares' timestamps |
| 3326 | `drain::unread` moves to `k7-front/src/drain.rs` | the médiathèque's upload routes are layered with it |
| 3327 | the library's `songs/K7/` kept out of the songs listing, and the stored rows of `k7.db` left where they are | the releases filed there stay unread rather than appearing as songs |
| 3328 | `ui-chains-react`'s `rehearsal` prop | another meaning: a graph previewed without writing, not the recorder |
| 3329 | `apps/k7-rehearsal-ios` | on its own branches, never in `main` |

## What other features lose, as the user decided

| row | feature | decision |
|---|---|---|
| 3340 | the médiathèque's Recordings deletes a capture | through the library's own `DELETE /api/mediatek/file`, whose gate lets an admin or the machine's user remove a capture; `manage.rs`' recorder rule (`RECORDER_SUBJECT`, `recorder_files`) goes. A person who is neither deletes no capture |
| 3341 | the Recordings' **Record** button, the recorder's take | goes; the zone keeps **Add recordings** |
| 3342 | the Recordings' moments under each capture, their shared-copy icons, the microphone on one's own takes, a take's duration from its session, a row's link to the page that cut moments | go; every capture's duration is the médiathèque's own measure, and the rows open nothing |
| 3343 | the editor's **Share to k7** (`share k7`, `POST /share/k7`), which published a project as a release, with the token minted for each file service (`K7_TOKEN`) | goes with the releases: the command, `/share/k7`, `shares/release.rs`, the file-service model, the token's minting in `hub/spawn.rs` |
| 3344 | saved links on the `k7.` host | the alias goes; such a link opens k7's home |

## Order of the cut

1. The moves of rows 3320 to 3326, each with its callers, nothing removed yet.
2. The frontend, web-protocol and the e2e app specs (rows 3305, 3306, 3308's app half).
3. The Rust routes, the crate, the Express doors and the conformance specs (rows 3302 to 3304, 3308's other half).
4. The packages (rows 3300, 3301, 3307), `package-lock.json` by hand.
5. Phones, tray, image, docs (rows 3309 to 3311). `lint:docs` and knip prove it.
