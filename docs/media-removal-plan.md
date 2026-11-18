# Removing the médiathèque

The user's decision: k7 is a DAW, and its media part goes. The code goes; the data it stored stays on disk,
unread: no code path is added or kept that deletes a song, a recording, a playlist, a hunt or a file of the
library volume. A route that goes answers the front's ordinary 404. Rows 35xx are this plan's; the method is
`rehearsal-removal-plan.md`'s.

A DAW keeps: the editor, tracks, clips, plugins, chains, rendering, recording into a project, importing an
audio file into a project, the engine and its registry, projects and workspaces, the module libraries
(`k7-core/`, `@lib/`) and the project shares. The word "médiathèque" also names the module libraries in some
pages and in the `@k7-core` ref prefix (`MEDIATEK_REF`): that meaning stays, only the wording is fixed.

## What goes

| row | zone | what |
|---|---|---|
| 3500 | screens | `packages/ui-library` (150 files) and its docs, but for rows 3520 to 3522 |
| 3501 | library model | `packages/mediatek` (69 files) and its docs, but for row 3523 |
| 3502 | sheets | `packages/score`, `lyrics`, `tablature` (32 files): only the song attachments use them |
| 3503 | Rust routes | `k7-front/src/routes/mediatek/` but for row 3524: the catalogue (`core`, `read`, `write`, `upload`, `zones`, `scanner`, `sweep(s)`, `analysis`, `decodes`, `stream`, `song_tags`), the songbook and lyrics queue (`songbook*`, `lyrics*`, `lrclib`), the hunt (`hunt/`: YouTube, slskd, SFTP, identify), the machine's copies of its server's songs (`server_songs/`), the library's stem cut (`stems*`, `stem_jobs`), the sync door (`sync.rs`); `routes/hunt_settings/`; `files/favorites.rs` |
| 3504 | Rust paths | `/api/mediatek`, `/api/hunt`, `/api/stems`, `/api/settings/slskd`, `/api/sync/mediatek`, `/api/sync/library`, `/api/favorites`, `/api/practice`, `/api/sync/favorites`, `/api/sync/practice`, `/api/internal/mediatek`; their device-door share; the seam's `GET /api/internal/files/mediatek` and `K7_MEDIATEK` |
| 3505 | Rust crates | `k7-mediatek`'s library modules (`catalogue`, `scan`, `search`, `songbook*`, `tagbook`, `manifest`, `recordings`, `playlists`, `zones`, `synced`, `listed`, `marks`, `revision`, `auto_lyrics`, `hunt/`, `collate/`, `text/`, `song_*`, `tag_patch`, `readers`) and `k7-tags` once `waveform.rs`' Wasmtime tuning moves (row 3525) |
| 3506 | file service | `files/service/mediatek.rs` (the account's read of the library), `native/media/prefetch.rs` STAYS (it puts a set list's records on the bound machine); the push of a take into the library's recordings (`native/media/take.rs`' push, row 3546), the library half of the `playlist-stems` verb host (`verb_host/stems.rs` reads workspace files instead, row 3541) |
| 3507 | Express | `media/mount.ts`' library bearer and `verify-token`, `internalFiles.ts`' `mediatek` answer, `door.ts`' library prefixes; `cors.ts` stays (row 3531) |
| 3508 | frontend | `/media/*` and `MediaPage`, `MediaHost`, `mediaLabels`, `configureLibrary` (`pagesConfig.ts`), the sidebar's library sections, `SongSyncDownloads` and `songSyncRows`, the `mediatek` client of `api.ts`, `SlskdSettings` on the server page, the build's `@source` and `.storybook`'s wrapper of the screens |
| 3509 | web-protocol | `hunt.ts`, `settings.ts` (slskd); a PUBLISHED package, so its version is raised (`docs/publishing.md`) |
| 3510 | engine side | the supervisor's jobs that serve the library alone: a set list brought down, a file for the library's player, a song's cut into `stems/` beside its master. The separation itself stays (row 3527) |
| 3511 | e2e | `src/mediatek.ts`, `src/serverSongs.ts`; `specs/conformance/mediatek*.spec.ts` (6), `hunt`, `favorites`, `serverCopy`, `serverSongs`, `serverSongsJobs`, `serviceMediaPrefetch`; `specs/app/media.spec.ts`, `stems.spec.ts`; the library cases of `auth`, `deviceDoorPaths`, `settings`, `lastRoutes`, `serviceMedia`, `serviceMediaVerbs`; `CONFORMANCE_MEDIATEK_DIR` in the three bed scripts |
| 3512 | phones, tray | `K7_MEDIATEK` in `BackendHost.kt`, `BackendHost.swift`, the tray's `backend.rs`; `STUDIO_MEDIATEK` in Android's `ENGINE_ROOT_DIRS` once nothing the engine plays lives there |
| 3513 | docs | `backend-rs/docs/mediatek.md`, `mediatekWrite.md`, `mediatekZones.md`, `hunt.md`, `serverSongs.md`, `tags.md`, `mediatek/` but for row 3524's pages; the docs site's `developer/71-73`, `77`, `user/70-73`; every other page rewritten as it is; `AGENTS.md`'s médiathèque row and the two map mentions; `docs/architecture.md`'s médiathèque row |

## What stays, and why

| row | what | why |
|---|---|---|
| 3520 | `BandTable` moves into the frontend | the home page's projects table and the server-only list draw with it |
| 3521 | `EnginesSettings`, `EngineOwnersSettings`, `EngineShareEditor`, `engineShares` move into the frontend's `settings/` | the engine registry's cards: who owns an engine, who it is shared with |
| 3522 | `poll` moves into the frontend (`visiblePoll.ts` wraps it) | the home page's polls |
| 3523 | `isAudio` and `mime` (`@studio/mediatek/media`) move into `song-schema` | a file dropped on the timeline and the verbs' media members judge audio by it |
| 3524 | the shares (`shares.rs`, `share_store.rs`, `share_rules.rs`), the share player (`player.rs`, `allowlist.rs`), the engine reach (`engines.rs`, `engine_jobs.rs`, `job.rs`), `/api/engine-slots` move out of `routes/mediatek/` under their own modules (`routes/shares/`, `routes/engine_reach/`; `/api/engine-local` goes, it served the library's player alone), same paths, `routes/public_share/` and `/api/settings/admin` (`lib.rs`) untouched, with `mediatek/shares.md`, `sharePlayer.md`, `comments.md`, `engineSlots.md`, `engine.md` | a project share is a link to a project; the engine slots and owners are the registry's |
| 3525 | `k7-mediatek`'s media core: `analyse`, `decode`, `envelope*`, `sidecar`, `waveform`, `media`, `ffmpeg`, `programs`, `transcode`, `stream_copy`, `video_*`, `whisper`, `fingerprint`, `json`, `ecma`, `write::part_of`, `layout`'s root | the clip waveform, the envelopes, the picture's copies, transcription, the share export  |
| 3526 | the instance's data folder (`STUDIO_MEDIATEK`, `/library` in the image, `mediatekDir()`) | `.auth` (tokens, revocations) and `.shares` live there: the volume stays mounted, its library folders unread |
| 3527 | `@studio/stem-separation`, the supervisor's `Separation*`, ONNX, `system onnx` | a clip processed by a chain that separates (`clip` verbs' cold step) is a DAW tool |
| 3528 | `@studio/ui-waveform`, `ui-mini-player`, `http-client`, `recorder` | the clip waveform and live waveform, the reference tracks' bar, the editor's calls and downloads, the take's format |
| 3529 | `_references/` and their bar | reference tracks are workspace files the browser plays |
| 3530 | `audio-analysis`, `audio-probe`, `take-render`, `video-prep`, `ffmpeg` | renders, takes, the picture |
| 3531 | `apps/k7/backend/src/cors.ts` and `apps/k7/backend/docs/banc.md`, `projects.md`, `config.md`, `cors.md` sit one level above the library's folders | the whole process's CORS, the bench, the home page, the configuration: not the library |

## What other features lose, as the user decided

| row | feature | decision |
|---|---|---|
| 3540 | the trimmed crate's name | `k7-mediatek` is kept |
| 3541 | the DJ desk, the `mix` archetype, the `turntables` surface, the playlist pane, `StemsBar`, `playlist *`, `system stems`, `system instrumental`, `system cue` | KEPT, reshaped: a record is an audio FILE of the workspace, named by its workspace-relative path in a `_playlists/` line; its stems are `stems/` beside it, as a clip's; its waveform and tempo come from the account's file service; a record the playlist names is copied to the engine's machine with the project. What only read the library goes: the catalogue's titles and artists (a row shows the file's name and its folder), variants, a tempo or key correction written back to the record (it holds for the session), the desk's own separation of an unprepared record and the surface's `stems:` (a set is prepared beforehand, `playlist stems`), the library's cache on the engine's machine |
| 3542 | importing a library song into a project | goes; importing an audio file stays |
| 3543 | the library's stem cut (`/api/stems`) | goes; separating a clip, and a set list's records (`playlist stems`), stay |
| 3544 | favorites and practice lists | go |
| 3545 | song downloads (LRCLIB, YouTube, slskd), song tags, the songbook, sheets, chords, tablature | go |
| 3546 | `project record save` | the take stays in the project's record directory; the push into the library goes |
| 3547 | project shares, their comments, the hosted home's share list | kept, moved out of the library's code (row 3524), same paths, same files under `.shares` |
| 3548 | the tracks export and the render | kept |
| 3549 | `apps/k7-rehearsal-ios`, on its own branches | loses the library's songbook; nothing in `main` |

## Order of the cut

1. The moves of rows 3520 to 3525 and 3531, each with its callers, nothing removed yet.
2. The desk's reshape (row 3541), its tests first; then the frontend, web-protocol and the e2e app specs (rows 3508, 3509, 3511's app half).
3. The Rust routes and file-service halves, the Express doors, the conformance specs (rows 3503, 3504, 3506, 3507, 3511's other half).
4. The packages and the crate's modules (rows 3500 to 3502, 3505), `package-lock.json` and `Cargo.lock` by hand.
5. Engine jobs, phones, tray, image, docs (rows 3510, 3512, 3513). `lint:docs` and knip prove it.
