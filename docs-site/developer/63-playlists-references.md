# Playlists and reference tracks: code

Both are **workspace** folders at the root, beside `_modules`: `_playlists/` and `_references/`. Neither belongs to a project, so both are reachable whatever project is open, and the playlist verbs work with no project open.

## The playlist format

`packages/song-schema/src/playlist.ts` (exported as `@studio/song-schema/playlist`) is the single definition of a workspace playlist, shared by k7 and the desk. It has no `node:` import, so the file service and the browser apply identical rules.

| Export | Role |
|---|---|
| `PLAYLISTS_DIR`, `PLAYLIST_EXT` | `_playlists`, `.k7x`. |
| `playlistPathOf(name)` | A name to its workspace path; null for one that would escape the folder. Accepts an explicit `.k7x`. |
| `parsePlaylist(text)` | `{ title?, items }` from k7x text, or **null** when the text is not a playlist. An unreadable file is not an empty one. Items are a bare string or `{ song }`, the record's workspace-relative path; anything else is skipped. |
| `playlistStub(name)` | What `playlist new` writes: a comment, `title`, `items([])`. It parses, unlike an empty file. |
| `appendPlaylistItem`, `removePlaylistItem`, `movePlaylistItem` | Edits on a `K7Scope`, preserving comments and layout. Each returns false when `items` is in a shape it cannot safely edit, and the caller refuses with "change the line by hand". |

Resolving a line against the workspace's audio files is `packages/song-schema/src/records.ts`: `recordPathOf` (a line to its workspace path), `resolveRecords` (keeps unresolved lines, since a dropped row hides a typo; a record's stems are `stems/` beside it), `recordName`, `recordFolder`, and the drag payload (`RECORD_DRAG_TYPE`, `packRecordDrag`). What travels in a drag is the **line**, so a deck resolves it exactly as its own set list does.

## Commands

`packages/commands/commands.yaml` declares `playlist new`, `playlist stems`, `playlist song add|remove|move`; the handlers are `packages/commands/src/verbs/playlist.ts` (`PLAYLIST_VERBS`), with `playlistFile` in `verbs/shared.ts`. They act on the workspace root (`repoDir`), not `songPath`. Every write snapshots first (`h.snapshot`) so the undo journal has the previous text. Positions are 1-based and by position, never by song, since a list may name a song twice. The runtime treats `playlist …` as a global command (`GLOBAL_COMMAND`) that needs no open project. Tests: `packages/commands/src/playlist.test.ts`, `playlistStems.test.ts`.

## File-service routes

`apps/k7/backend-rs/crates/k7-front/src/routes/files/service/routes.rs`:

| Route | Answer |
|---|---|
| `GET /playlists` | `playlists` (`apps/k7/backend-rs/crates/k7-file-service/src/workspaces/refs.rs`): every file under `_playlists/` at any depth, with `path`, `name`, `title?`, `count`, `ok` (did it parse). |
| `GET /references` | `references` (same module): every reference file under `_references/` at any depth, with `path`, `name`, `bytes`. |

There is no route that creates a playlist: that is `playlist new`. Reading and writing one go through the ordinary `/file` and `/write`.

## The k7 panels

| Path | Role |
|---|---|
| `apps/k7/frontend/src/components/PlaylistsPanel.tsx` | Right-docked panel: a searchable dropdown (`searchPlaylists`, the shell's `fuzzyScore`), New (dispatches `playlist new`), the full editor toggle. |
| `apps/k7/frontend/src/components/PlaylistEditor.tsx` | The k7x text on the left (`YamlPane`, `useAutosave` with `parsePlaylist` as validity), resolved rows on the right; in `panel` mode the text half is left out. |
| `apps/k7/frontend/src/components/PlaylistPane.tsx` | Resolved rows: browser playback through one `<audio>`, search-to-add over the workspace's audio files, move/remove, draggable rows, and the "Separate the stems" run (see [Stems and transcription](./64-stems-transcription.md)). |
| `apps/k7/frontend/src/playlistVocabulary.ts` | Completion for a playlist file: fields and the workspace's audio files. |
| `apps/k7/frontend/src/localCommands.ts`, `palette.ts` | `ui playlists` and `ui references` toggle the right panel; the footer shows the playlists tab only for a project with a `turntables` surface, the references tab always. |

Playlist and reference panels are right panels (`rightPanel` in `apps/k7/frontend/src/App.tsx`), not panes.

## The desk's set list

`resolveSet` (in `packages/k7-plugins/src/turntables/setList.ts`) turns the playlist text into `SetEntry` rows against the audio files **of the workspace**: duplicates by record path dropped, `file` set to the record's workspace path, null when the workspace does not hold the file; a row shows the file's name and its folder. `NO_PLAYLIST` (surface names none), a missing file and an unparseable file are three distinct states. `browseSongs` folds accents and case, requires every term, and sorts by folder then name with a French collator.

## Pre-fetching to the engine's machine

`apps/k7/backend-rs/crates/k7-front/src/routes/files/service/native/media/prefetch.rs`: every 60 s (first after 5 s), for every live workspace's active project, `playlistRecords` (`packages/song/rust/src/send_refs.rs`) reads the `turntables` surface's `playlist`, parses it, and keeps the lines naming an audio file. Each record and the files of its `stems/` folder are listed and sent as one `prefetch` job (`@studio/engine-tunnel/jobs-client`) to the bound machine's jobs URL. The machine side copies what is missing or changed. The signature of `(jobsUrl, files)` is remembered after a success so an unchanged list is not re-sent; errors are logged once per distinct message.

A line that names no workspace audio file is not pre-fetched.

## Reference tracks

`packages/song-schema/src/reference.ts`: `REFERENCES_DIR`, `REFERENCE_EXTS` (what a browser `<audio>` may decode) and `REFERENCE_ACCEPT` for the file picker, `isReferenceFile` (the extension is the whole rule; no sidecar), `referenceName`, `referencePathOf`, `ReferenceTrack`. Tested by `packages/song-schema/src/reference.test.ts`.

| Path | Role |
|---|---|
| `apps/k7/frontend/src/components/ReferencesPanel.tsx` | Lists `/references`, uploads with `api.uploadWithProgress` into `_references/`, deletes with `api.deleteFile` (recycled). Plays nothing itself. |
| `apps/k7/frontend/src/components/ReferenceBar.tsx` | Owns the `<audio>` element and draws `@studio/ui-mini-player`'s `MiniPlayer`. Mounted in `App.tsx` below every pane so it survives pane and panel changes. |

The choice that shapes this: the reference is played by the **browser**, never the engine, so it can sound over a live rig or a running arrangement without asking either. The cost is that it leaves by the browser's output device, which the bar says.
