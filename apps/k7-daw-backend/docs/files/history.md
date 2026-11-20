# A workspace's history

`k7_sync::log` keeps a room's whole log on disk; `k7_sync::history` serves it to the device's own routes
(`packages/k7-capture/src/account/history_ask.rs`, "History on the device", below).

## The room's log is the history

Every edit the capture sees becomes a batch, and the room gives it a place in one order.
That ordered log is therefore the complete account of what happened to a workspace between two commits
(`git.md`), and it is kept whole rather than a second record being built beside it. It is this repository's
own code on every platform, which a git binary is not.

History lives on the device that captures the workspace ("History on the device", below). Undo stays
local and works offline.

## Segments, never truncated

The room is saved whole every 200 entries and starts its working log again. The entries are
not dropped at that point: the working log is ROLLED into `log/<firstSeq>.jsonl`, a segment that is
never rewritten or deleted. A segment is named by its first `seq`.

`log/index.json` knows each segment's first and last `seq` and its entry count, so a page of the
listing and a rebuild read only the segments that can hold them, and the working log only when the
range reaches past the last segment. It is a cache: a segment it lacks, or whose length differs
from the one it recorded, is read again and the index rewritten, and a missing index is rebuilt.

Loading a room on restart reads the saved state and the working log only, as it would without any
history: the segments are for reading the past, never for catching up.

Every entry line and the index carry `v`, the log version that wrote them, and a reader refuses a
version newer than its own rather than rebuild a point from entries it may misread
(`LOG_V` in `k7_sync::protocol`). An index of another version is a cache miss.

An op of a RETIRED kind (`RETIRED_OP_KINDS`: `yaml`, from before every workspace file was k7x) is
the other way round: no update reads it, so refusing it would lock the room for good. It is
skipped wherever a log or a saved room state is read, in an entry's ops and among the conflicts set
aside; the entry keeps its `seq`, so the order has no hole. The
files those ops changed were converted to k7x on disk, and the room's next texts of them are k7x.

A point those skipped ops reach is answered `410`, like any point the log cannot rebuild: one
where a file a skipped op changed has not since been written whole or removed, and one whose files
still hold a `.yaml` name. Rebuilt anyway, it would be the text before the skipped edit passed off
as the file at that point, and a restore would write a YAML file back into a k7x workspace.

## Snapshots, so a point is rebuilt from nearby

Every 1,000 entries the files are written whole to `snapshots/<seq>.json`. The files at any point
are the nearest snapshot at or before it with the entries after it replayed, never a replay from
seq 1. With no snapshot before a point, the empty room at seq 0 is the base, which holds only when
the log reaches back to 1. A point the kept entries cannot reach is answered `410`, not guessed.

A room with no segments yet gets its saved catch-up log as its first segment and its saved files as
its first snapshot when it is next loaded.

## Checkpoints

A checkpoint is a named `seq` in `checkpoints.json`: nothing is copied, since the log already holds
the point. Named by a person, it records their email. The room also takes one, named `auto`, when a
room has been edited and then left alone for ten minutes, at most one per quiet period, so a session
of work has a point to come back to without anyone thinking of naming it.

## A restore is an ordinary batch

Restoring rebuilds the files at a point and submits ONE batch to the live room the device holds:
the whole text of every file that differs from now, and a removal of every file the point did not
have. It is ordered and written exactly as the capture's own batch, and it lands in the history itself — a
restore can be undone by restoring to the point before it.

Every op names the text it replaces (`was`), so a value that moved between the rebuild and the
order is set aside as a conflict rather than overwritten.

The batch also settles every open conflict on a file it rewrites, by naming it in `resolves`: an
alternative to a value the restore just replaced would otherwise be offered over the restored
one. A conflict on a file the restore leaves alone stays open. Settling loses nothing, since the
alternative stays in the log.

## What is not kept here

A binary file travels as a blob ref: a text starting with a NUL byte that names a content hash. The
log keeps the ref; the bytes live in the capture's blob cache (`../file-capture.md`). A diff never compares refs as
text and only says a binary file changed.

Diffs are whole texts before and after, per file. Comparing them is the browser's affair.

## History on the device

A workspace's device runs
a room (`k7_sync`'s `relay`, `log`, `history`) over its own capture's batches, so its history needs no
server.

| choice | why |
|---|---|
| the device's rooms are a `k7_sync` relay (the room's loader and idle timer) of the capture, each room in its workspace's log folder, `<home>/.sync/<name>-<hex>/.sync-relay/history/` (so a rename moves it with the log), with no socket and its own idle timer | the log, the segments, the snapshots and the restore are already written and tested there; a second implementation would drift |
| every batch of a workspace is ordered in its room after it is in the capture's log, and a room failing to write is said and never stops the capture | the capture's log is this machine's record; the history is a view of it, and an edit must never be lost to a full history disk |
| the room's files at start are the capture's baseline, so what changed while k7 was down is one ordinary entry, and a workspace with no room yet gets its first entry made of every file | the history then holds every edit and the numbering never skips; the baseline is what the capture already diffs against for a relay |
| a restore writes the files through the capture's landing (its ledger learns them first), then orders one batch `restore to <point>` in the room, so it is an entry like any other and can be restored from | the capture must not see the restore as a second edit, and the history must say what happened |
| a room that cannot be loaded (a `room.json` that does not parse, a `v` newer than this build's) is said in the log and in the page's `error`, and the workspace is captured with no history and the full rescan of a restart | the history is a view of the capture: it never stops it, and an unreadable room is never taken for an empty one |
| the working log rolls into a segment every 20 entries and a snapshot is kept every 1,000  | each entry carries the whole text of each file it changes, so a log rolled only every 200 entries makes the folder's upload grow with the square of the edits; at 20 the past travels as segments, put once |

### The routes

The file hub's own, beside `/api/files/git/` (`git.md`): JSON, the account's ticket or session, the account's
workspaces only (another account's is 403), the workspace in the query or the body. The answers are
`k7_sync::history`'s, the shapes below.

| route | body or query | answers |
|---|---|---|
| `GET /history/page?workspace=&before=&limit=` | `before` an exclusive seq, `limit` 1 to 500 (50) | `{ source: "device", head, checkpoints: [{ seq, name, ts, by }], entries: [{ seq, ts, replica, label, files: [string], parked }] }`, entries newest first |
| `POST /history/checkpoint` | `{ workspace, name, seq? }` (`seq` defaults to the head) | the checkpoint `{ seq, name, ts, by }` |
| `GET /history/diff?workspace=&from=&to=` | two points of the log | `{ from, to, files: [{ file, before, after, binary? }] }`, a text or `null` each side |
| `POST /history/restore` | `{ workspace, seq }` | `{ entry: <summary>, kept: [path] }`, or `{ entry: null, kept }` when the texts already are the point's; `kept` names each binary file left as it is |

A refusal is `{ error: <a sentence>, code: <a word> }`:

| status | code | when |
|---|---|---|
| 400 | `bad-request` | a field missing or malformed, a point past the head, a name empty or over 120 characters |
| 403 | `forbidden` | the workspace is another account's |
| 404 | `unknown-workspace` | the workspace is not here; or `no-history`: it has no entry yet |
| 500 | `history-unavailable` | the room could not be loaded when the workspace started: every route but the page (which answers an empty one with `error`) is refused with the reason |
| 410 | `gone` | the log cannot rebuild the point (a segment is missing) |

| choice | why |
|---|---|
| **PENDING the user's word** — a restore brings back the TEXTS of a point and leaves each audio or other binary file as it is now, naming them in `kept`, the panel saying so | the device keeps a binary's blob ref, not its bytes (`../file-capture.md`), so a restore cannot bring one back; refusing the whole point would put every point before the last recording out of reach. The other side is a `409 binary` refusal with those rows greyed out |
| a restore is a write: it takes the same refusals as a file write | it changes the workspace's files |

### Size

No retention policy yet (**PENDING the user's word**): nothing is deleted, a segment is kept for ever, and each entry carries the whole text of the files it changes.
