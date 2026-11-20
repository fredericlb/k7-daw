# The capture in native Rust

A device's capture, its ledger, its blobs and the account's set of captured workspaces, native Rust, beside what is left of the
replica ([files/replica.md](files/replica.md)). No workspace on a device joins a relay room: it is local, and git moves it between devices ([files/git.md](files/git.md)). This page holds the choices: the threads, the front's host, the disk, the proofs.

## The crate

The capture itself (`capture/`) is synchronous and reads a `Clock`: the
timers, the watcher and the window's epoch belong to the worker, which calls its `on_quiet`, `on_sweep` and `on_young`
handlers, so every rule is tested with the clock moved by hand and the watcher replaced by `touch`.

| choice | why |
|---|---|
| the ledger takes a file written for another replica after it landed, and `pending.json` is saved before the ledger marks a file pending (2133, 2134) | memory is never ahead of the disk; a restart reads a pending file absent from `pending.json` as removed |
| the walk is depth first in name order over an explicit stack, a dirent's own type deciding | a link is neither a file nor a folder and is never entered; a deep nest does not use the blocking pool's stack |
| a scan returns its ops and a ledger delta and changes nothing; the worker commits both only if no window opened meanwhile, else it gives the dirty paths back and evicts the snapshots the scan kept (2121) | a walk on another thread runs while a verb writes; a dropped scan whose ledger had moved would count the verb's edits known and never send them |
| past 256 dirty paths a flush is one full sweep | each dirty scope costs a pass over the ledger, so a thousand touched paths in a large workspace would cost more than the sweep |
| a scope that is itself a link is not walked, a scope that climbs is refused, and no write goes through a linked parent folder (DEPARTS: the TypeScript follows a link scope and checks only lexically) | a link to the home folder would send it, and a remote op on `dir-link/x.md` would write outside the workspace |
| a file placed for another replica is copied beside its target and renamed over it (DEPARTS: the TypeScript removes, then copies) | a failed copy must leave the old file |
| a start marks the files waiting for bytes pending in memory without saving, and a failed prepare leaves no ledger | `pending.json` already holds them; a save per file shrinks it to a prefix, and a half-built ledger reads what it missed as removed |
| a write that clears a pending file records the ledger, then saves `pending.json`; a failed save re-marks it pending at the next start, and the old blob may then overwrite newer text (as the TypeScript does) | the one order that never loses a pending mark; the window is a disk that fails twice |
| a path below a file is absent in the confirmed mirror on every host | `ENOTDIR` here, `NotFound` on Windows; a room holding `a` and `a/b` must apply on both |
| a `.sync` file that is there and makes no sense is "not now" (2109, pending the user's ruling) | a replica id read as none would make a second device of this one |
| the `C:/`-spelled rows and UNC roots have no vector from node | only a node service of the last release would differ |
| the account keys its captures by `resolve_all` of the root (`root_key`), on insert, lookup and removal | a handler holds `C:\ws` where the capture attached `C:/ws`; a window opened on one spelling would miss the capture of the other, and the watcher would send the save as a second, unlabelled `edit` batch |

## The threads

| choice | why |
|---|---|
| one serial worker per captured workspace, a tokio task fed by a channel; the capture's state is owned by the task, moved into `spawn_blocking` for each file step and handed back | the TypeScript's capture is single-threaded and its rules assume it (a write recorded before the watcher hears it, a local edit captured before an entry is applied); a worker keeps that order without a lock held across an await |
| no `Mutex` around a ledger; what other threads read is published by the worker: the change generation, `watching`, the text state, the pending counts, the head | a reader never waits on a worker in the middle of a sweep, and a worker never waits on a reader |
| a mutation window is an atomic count and an epoch per workspace, moved synchronously by whoever opens it (a native write of the front, directly) | a hold must take effect before the verb's first write, which a message to the worker could not promise |
| a sweep or flush notes the epoch when it starts, and commits its ledger and its batch only when the count is zero and the epoch has not moved; otherwise its result is dropped and the release sweeps again | in node a sweep could not run while a verb wrote; here the walk runs on another thread while a verb writes, and a half window emitted as `edit` would split one verb into two batches |
| a window's release sets a `closed` flag the next scan reads, so a flush that wins the race against the worker's release message still sweeps whole | a release is synchronous and its message to the worker is not |
| the window's labels wait in the window, and the commit that emits a batch takes them | a release is synchronous and its message to the worker is not, so a flush between the two would otherwise emit the window's batch without its name |
| the labels of a window whose flush finds nothing wait for the next non-empty batch, even an unrelated one | the TypeScript's `flushNow` returns before `emit`; kept |
| `place_file`'s temp is a dot name, hidden from the walk only because the default `skip` hides dot names; a crash leaves a stray `.name.<uuid>.tmp` in the workspace, as `write_atomic` does | an `Options.skip` that shows dot names would capture the temp |
| pinned roots are never evicted: on Linux many accounts' workspaces can exhaust `max_user_watches`, and a down root's re-pin after its retry re-adds its whole tree and moves every root's generation (FSEvents) | the price of a watcher that never goes quiet |
| no test unpins a pinned root and renames its folder on Windows, which 2126's "unpinned before a rename" promises; the `Process` source and the spellings use `is_within`, which ignores case there | `win-clippy` only compiles the Windows case; a Windows machine's run is the judge, and none is wired |
| a start without a baseline learns a verb's write during the start as known | only a start with a baseline holds its diff for a window; the account attaches with the window free |
| the account awaits a worker's `stop` before it spawns another for the same root | `unpin` drops by sink, but a stop that is still running holds the folder open on Windows |
| an entry's compute takes `&LiveCapture` and only reads; every write is its write phase, which first checks the count and the epoch read before the compute and answers `None` to compute again | a compute runs while a verb's window may be open, and what it changed would land inside it; a verb that wrote and released meanwhile leaves the count at zero and moves only the epoch |
| the files an entry or a state reads are captured by an ordinary scan of the worker (`Handle::capture`, a whole sweep for a long list) BEFORE the entry is queued; the write phase only compares the stats of the files it will write with the ledger (`uncaptured`, a young file counting as a difference) and a difference goes back to a scan (`Recapture`), a pause growing between two tries | a scan in the write phase calls the Settler for a k7x diff and hashes a changed binary under the `applying` lock, which `hold` waits for (2127) |
| a text read within the settle time keeps no stamp, so the stat check reads and hashes it (a text of 128 KiB at most, 5 MiB for a k7x document); a binary is never read there | a same-size rewrite in one timestamp tick would pass a stamp check |
| a state or a seed needs the sweep of a closed window before it is computed (`Tried::Scan`, 2129); an ordinary entry waits for an open window only | trying at every pause copied every welcome file into a scratch replica each time |
| a worker's entry is `Done`, `Window`, `Again` or `Scan`; `Again` comes only from an epoch that moved during a cheap compute | an entry never spins |
| a write phase never calls the Settler: the compute warms a memo of its calls, and a write whose merge would miss it requeues instead | `hold` waits for the write phase, so it must last milliseconds, never the Settler's 10 s timeout (2127) |
| `ConfirmedDir` keeps an in-memory hash of every file it holds, built once when the workspace opens and updated only after a write landed; a `set` of the text it already holds writes nothing, and a state is settled from the texts it has just stored and from the ledger (`Host::same`), not from reads (DEPARTS: the TypeScript rewrites and re-reads every file, 2131a) | a resync that changed nothing held the window's lock for 8 s on 12,540 files |
| a write phase never opens a window of its own, and no closure given to `run_when_free` holds a `Handle` | the `applying` lock is not reentrant; a queued entry that holds the mailbox's sender keeps the worker alive |
| a worker that panicked stays stopped until the account starts another (2128), and the worker holds no sender: only the pinned sink holds one, weakly | until C5's account restarts it, a panic stops that workspace's capture |
| the worker's sweep asks the source whether the watcher is up, and pins the root again when it is not | a watcher that went down is a workspace swept at its 5 s net, not one that never hears again |
| the shared `Settler` (2106) took four accounts changing 300 k7x files each, 1,200 diffs, in 160 ms through the workers, every one a k7x op and none a whole text | no cap on the count of k7x changes per scan is needed at this size |

## The account

`account/` is the workspaces one account captures, by root. What it holds of each is an `Attached` (the worker's `Handle`
and its log folder). The host (the front's `Bundle`) tells it what to attach; it owns no `Workspace`.

| choice | why |
|---|---|
| a workspace is attached from its folder alone: no server, no room and no link record, whatever an older k7 left in its log folder | a device's workspace is local; a record read once relinked a workspace to a server that is no longer asked |
| `attach` reads the replica id before it builds anything, one at a time per account; an unreadable one is an error naming the file, and nothing is started or written (2102) | two attaches of one root would be two workers over one folder |
| a capture's batches go to its log and its history room through one `Out` | the first scan's batch is emitted before anything else exists |
| `end_workspace` stops the capture, lets go of the history and removes the log folder; the files are untouched | a deleted workspace has nothing to tell a server, and a later workspace of that name starts afresh |
| a rename is the account's, with the host's `let_go_under` awaited first and the host's lock held across the move; the log folder and the marker move with the folder, and the capture attaches again at the new root | the front holds watches and the served project under a folder; the move must not be captured as every file removed and added |
| `attach_named` attaches a folder that is there and makes none | a name no folder holds is nothing to capture |
| a start attaches every folder and the account's library | the library is local to the device (a capture with no history), the folders are the workspaces |

## The registry and the host

| choice | why |
|---|---|
| `Captures` gates a home's start and stop one after the other, and a stop names the account it means to stop | a stop spawned before a restart stopped the account that replaced it |
| the host's attach and detach tells run in the order they were told, one task | a detach that ran first left a removed folder captured |
| `Bundle` carries `lock()` and `folders_changed()` (APPROVED by the user, 2026-11-11) | the rename runs under the account's lock, and the home page's listing hears of it |
| the account's captures stop with its `Held` (dropped or stopped), each ledger saved | a capture that outlives its account is memory held for an idle account |
| the native writes (the upload, `PUT /file`) open and close the capture's window on the registry directly; a window is released where it was opened | the window must be the capture's that emits the batch, and no write waits on another thread's turn |
| the capture reaches the front through its `Bundle` (`native/capture_bundle.rs`): the listings memo, the module watch, the engine link and the account's write lock; `letGoUnder` lets go of the module watches and the served project | the capture owns no `Workspace`; the front does |
| the captures belong to the process: an attach of a root already captured returns it | a `Held` made again attaches nothing twice |
| no route answers for the capture: the log, a project's sync state and the conflicts are not asked of a device | nothing orders a workspace but its own disk |

## The k7x half's size

The k7x half is a stateless native unit shared by the relay and every account's capture; it has no pool to size. `settler_scale.rs` (an ignored release measure) has N accounts each settle the 1,207 k7x files of `k7-core` at once, as N library joins do.

## The watcher

The process's `notify` watcher of the revision (`packages/k7-file-service/src/watch.rs`) gains pinned roots: a captured
workspace is watched for as long as it is captured, never evicted by the revision's limit of 64 roots nor its
15-minute idle rule, and its events are delivered as paths to its worker besides moving the root's generation. A
pinned root is unwatched before its folder is renamed or removed, by `unpin`, which drops only the pin made with the
sink it names (a root pinned again by a newer worker stays): `unwatch`, which the revision's eviction calls, leaves a
pinned root. The revision's recovery from a missed change is `rewatch`, which re-adds notify's watch and keeps the root
and its sink. One watcher keeps one inotify instance per process, and on Windows one handle per workspace root.

A worker holds its mailbox weakly, so it ends when every `Handle` is dropped, and a step that panics ends it as stopped
with its root unpinned (2128). Every call that walks a tree or writes a file (`pin`, `unpin`, a commit or an abort) runs
on the blocking pool; the baseline's diff at a start is the worker's first scan, so a window held before the start counts.

## What the capture and its watches decide

| choice | why |
|---|---|
| capture keeps a ledger of the workspace and diffs a change against it, whoever made the change | the service writes through about a hundred call sites and most bypass the journal; a take or a tunnel upload writes with no verb at all |
| the ledger holds a k7x file's text, any other text file's hash and an opaque file's blob ref (sha256 and length) | only a k7x file is diffed into path ops; holding every text instead was 37 MB against 1.4 MB on the library checkout |
| a file with a NUL byte in its first 8,000 bytes is binary whatever its name, and a text that is not k7x is opaque past 128 KiB | `.fxp` presets and amp models no extension list covers would arrive corrupted as text; a 1 MB model held as text sits in the files, the merge bases and the log, and a model never has a concurrent line edit |
| one recursive watcher per workspace, with a full sweep a second after the start, every five seconds while the workspace is active (changed in the last two minutes), every five minutes once idle, and every sixty seconds when no watcher could start | a watcher loses events, and a sweep per five seconds for every idle workspace on a server is disk work proportional to everything stored; an edit the watcher hears makes the workspace active again |
| a sweep never skips a folder whose own mtime has not moved | a rewrite in place leaves its folder's mtime alone, and it is the event Linux's watcher loses |
| the change feed reports every path the watcher reports, every file a sweep finds moved and every file written for another replica, and reports more rather than less | the strudel watch and the module watch subscribe instead of opening watchers: one handle per workspace, none inside a project to release before a rename on Windows |
| a file that cannot be read now (`EBUSY`, `EPERM`, `EACCES`) is skipped for the round, said once and left in the ledger as known; a folder that cannot be listed is not an emptied one | a lock lasts a second and a timer nobody awaits must not throw; nothing under an unlistable folder is removed that round |
| a mutation window is one batch, emitted by a full sweep after a quiet period of 100 ms rather than from the watcher's events; writes outside any window close a batch after the same quiet period, labelled `edit` | the watcher reports a window's writes late and under load splits one window in two; a periodic sweep never emits half a window |
| the log lives in the account's state, rotated once past 20 MB | inside the workspace it would be captured and synced with the songs |
| a file's hash is kept by path and stamp (`blob-index.json`), texts under their own key | a start would otherwise read every take again, 0.4 s per gigabyte from the page cache; a reader of only `files` must never take a text's hash for a blob's |
| a rename is a removal plus a creation | a renamed binary costs no upload because the store holds its bytes |
| the module-stamp walk is skipped only while a watch vouches for it, and ten seconds is the ceiling after which it walks anyway | a watcher drops events under load, across shares and in rename storms, so a dropped event is a delay of ten seconds, never a vocabulary stale for good |
| a watcher is not enough alone: the process that writes says so too, on every write, on a command that moved the revision and on a project made or copied in | a watcher reports the filesystem's events and the writer is ahead of them; one extra walk after a write is what a write costs without any of this |
| the module watch is not told about the plugin scan or the models folder, whose mtimes are checked when a held stamp is handed back; a root inside the captured workspace is heard, a root outside it is watched | the hub rewrites the scan from its own process; a root created after the watch began is covered |
| every path that moves a project or a workspace closes the watches under it first, and a session leaving a project closes its watch unless another session serves it | a handle inside a directory makes its parent refuse a rename on Windows; two people can have one shared project open |

## The disk

The capture's state is kept under the home's `.sync/`. A `head`, a `confirmed/`, a `server.json` or a `displaced/` an older k7
left in a log folder is neither read nor written.

| file | kept as |
|---|---|
| `<home>/.sync/replica-id` | the replica's id, made once; one replica per home |
| `<home>/.sync/<name>-<12 hex>/` | the log folder: `<name>` the folder's base with `[^\w.-]+` as `_`, the hex the sha256 of the root's `windowsSpelling` (1d's `windows_spelling`) |
| `batches.jsonl`, `batches.1.jsonl` | one batch per line, rotated once past 20 MiB |
| `blobs/<sha256>`, `part-*`, `tmp-*`, `pending.json` | the bytes in flight, a resumed download, a copy not yet hashed, the files waiting for bytes |
| `blob-index.json` | `{files, texts}` by path and stamp, the stamp `<size>:<mtimeMs>` with `mtimeMs` printed as JavaScript prints it |

A file of this table that cannot be read is not a file that is absent: only `NotFound` reads as absent; any other
error leaves that workspace unattached for the round and says so once.

## The referees

| referee | what it judges |
|---|---|
| vectors from the TypeScript's own functions (`fixtures/capture-vectors.json`, frozen) | the pure rules still used: the log folder's name, the stamp, binary by content, the ceilings, the confined path, the local name of a download, the files written |
| the capture's own tests (`tests/all/capture*.rs`, `worker*.rs`, `account*.rs`) | the capture's cases, as tables |
