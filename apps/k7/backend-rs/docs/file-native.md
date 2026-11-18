# The file service's native reads

The file service's reads in Rust. The crate is `crates/k7-file-service` (no HTTP, no runtime of its own); the front's half is
`crates/k7-front/src/routes/files/service/native/`. `reads` is `GET /file`, `POST /fmt`, `GET /tree` and
`/workspace-tree`, and the path of `GET /media`. Nothing here writes. The compile's nine routes (`docs`) are
[file-docs.md](file-docs.md). The home page's listings and the acts on a whole
workspace are the piece `workspaces`: [file-workspaces.md](file-workspaces.md).

## Answering

Every piece answers on every host (`service/default_pieces.txt`, one file for the front and `bedSummary.mjs`); no variable switches one off.

| # | decision | why |
|---|---|---|
| 1010 | A piece that cannot answer returns `Declined(why)`: the request is refused with a sentence (`declines.md`). Said once per workspace and reason, the memory of what was said capped at 1,024 entries then cleared | a decline costs one call where a guess would be a wrong answer |

## The path model

| # | decision | why |
|---|---|---|
| 1020 DEPARTS | The lexical model is a port of node's path shim with `web-protocol/paths.ts`: `\` read as `/` on every host, a `C:/` root on every host, Windows-ness from the spelling of the root (`is_within` folds case under a drive or UNC root only), no `cfg!`; a test greps `src/` so `cfg!(windows)` exists in `filter::code_of` alone. `C:/x` on a Mac escapes, as node answers it | `process.platform` is always `linux` in the shim; a rule per host would answer what no branch of the referee answers, and Windows is then ordinary data in a test |
| 1021 | `safe_path` is `safePath` gate by gate: split on `[\\/]+`, `""` and `.` dropped; a first segment `_libraries` goes to the library branch before any resolve; the second a library name or `account`, confined to that library's root (`path escapes the libraries: <rel>`); anything else below `_libraries/` is `names no library: <rel>`; bare `_libraries` is the libraries' home. Otherwise `.` is the root, else `resolve(root, rel)`, confined to the root (`path escapes the workspace: <rel>`) | the sentences are the specs' |
| 1022 | `confine_to` is `confineTo` over a `Probe`: a NUL is refused; lexical `is_within`; the probe climbs to the nearest present ancestor by `lstat`, a dangling link present and refused; no ancestor present is allowed; the real path against the root's real path, read by the `canonicalize` the shim's `realpath` calls, so a link the platform cannot open (on Windows, a folder link to a file) is refused by both. Each call looks afresh. The climb stops where a parent is its own and absent, which the TypeScript would loop on | a fake probe exercises Windows spellings and links on any host |
| 1023 | `tests/vectors/paths.json` is frozen: the shims were run under node once, 3,351 cases of every function of the model and of the name rules; the Rust test replays them | the harvested shim answers are the referee; a hand-written expectation would repeat a misreading |
| 1024 | A `rel` past `Limits::max_rel` (16 KiB) is `Declined(LongPath)` before any `lstat` | the probe's climb costs an `lstat` per segment |
| 1025 | `Home::of`: `STUDIO_HOME` of the account's environment, else `<HOME or USERPROFILE or />/.k7`, through `resolve`; `STUDIO_LIBRARIES_HOME`, else `<studio>/libraries`; `admin` is the request's stamp | the account's environment decides |

## The libraries' roots

| # | decision | why |
|---|---|---|
| 1030 | `is_library_name`, `version_of_dir` and the reserved names are held to `moduleTypes.ts` and `libraryVersions.ts` by a test that reads their text | one rule in two languages, kept honest |
| 1031 | `workspace_pins` walks from the workspace up to the root of the disk for the nearest `_library-pins.k7x`; a read or parse failure is no pins and ends the walk; `_library-pins.yaml` found first is `Declined(LegacyPins)`; a text with a lone surrogate is `Declined(LossyText)`; a file past 16 MiB is declined. A value is `"draft"` or an integer of at least 1, read from the tree's scalars (one past `u32` is clamped, naming a folder that does not exist either way) | a native read never writes, and the TypeScript's walk leaves the workspace too |
| 1032 DEPARTS | `name in pins` reads own keys only. The TypeScript's plain object answers `true` for a library named `constructor`, and joins the source of a function to the library's path | a prototype's key is no pin |
| 1033 | `library_root` in the TypeScript's order: `account` is `<studio>/library`; standing inside a version of this library, that version; a pin, through the version folder; an admin's overlay; version 1. A library's folder holds versions when one `readdir` finds a version folder in it | `libraryRoot` and `libraryVersionRoot` |
| 1034 | Overlays are derived at each request from `admin` and the disk, never held. A library added since can differ from the TypeScript's memory until its flag flips; that difference is logged and read by hand | no second memory |
| 1035 | `library_of` keeps no linear search: the path is cut below the libraries' home, its first segment is the name, the account's folder is one prefix test, an overlay lives under its library's home | the module catalogue asks it per row |

## Reads

| # | decision | why |
|---|---|---|
| 1040 | `Space::active`: a root that is a project is itself; else the marker `<parent>/.active-projects/<name>`, trimmed as JavaScript trims, resolved, inside the root and a project; else none. Read only when asked: a path outside `_libraries/` asks nothing | one reader of the marker, and `/file` stays one `lstat` chain |
| 1041 | A failed marker read with `<root>/.studio-active` present is `Declined(LegacyMarker)`; a marker naming `_libraries/...` is `Declined(LibraryActive)` | a library's project is held in memory, which the disk cannot say |
| 1042 | `read_text` in the TypeScript's order: the gate; a binary name answers `{binary: true, content: ""}` before any stat, a missing `.wav` included; `stat` follows links; absent is not found, a folder is refused; the text whole, invalid UTF-8 lossy and a BOM kept. `path` in the answer is the caller's spelling | the specs pin `{path, binary, content}` |
| 1043 | `is_binary` takes the shim's `extname`: the last dot not at the start of the name, lower-cased, against the table | `.wav` is no extension, `a.WAV` is |
| 1044 | Failures as `entryFailure` words them: not found is a 404 `no such file or folder: <rel>`; a folder a 400 `'<rel>' is a folder, not a file`; a gate a 400 with its sentence; an I/O code `ENOENT` after a good stat a 404 `no such file or folder`, any other a 400 `the file could not be changed (<CODE>)`. `ENOTDIR` is that last case: the shim's `throwIfNoEntry` covers `ENOENT` alone. Windows names a path below a file `ERROR_PATH_NOT_FOUND`, which both sides read as `ENOENT`: a 404 there | what `serviceFiles` reads, and the referee is the shim |
| 1045 | A file that is not regular is `Declined(NotRegular)`; one past `max_text` (64 MiB) is `Declined(TooLarge)`, held by a read that stops one byte past the limit | a FIFO would block a thread for good, and native memory must have a limit |
| 1050 | `tree` as `fileTree`: `skip` and the order in the folder loop, folders first then the bytes of the UTF-8 name, stable; a link is not followed, so it is `dir: false`; `project` and `app` by name, with one `stat` only for a name ending in `.k7`; keys in the literal's order | the spec's contract |
| 1051 | An explicit stack, two path buffers pushed and truncated, one arena of names per folder, the JSON written once into one buffer with `serde_json::to_writer` for each name and path; no `Value`, no per-item path | a name copied per item is a defect, and a deep tree must not recurse |
| 1052 | `Limits`: every entry a `readdir` yields counts against `max_seen` (2,000,000), the written text against `max_bytes` (64 MiB), a folder's depth against `max_depth` (256); past any, `Declined` | a limit weighs output and every walk, hidden entries included |
| 1053 | A name holding `\` is spelt with `/` in `path`, as the shim's `join` spells it, and the walk goes on that spelling; a spelling that climbs, is not a plain child, or leads out of the workspace through a link (`confine_to` on the spelt path) is not entered and costs that folder (1054). On POSIX such a folder cannot be listed | what the referee does, without leaving the workspace |
| 1054 DEPARTS | A folder that cannot be listed (unreadable, vanished, or 1053's) costs that folder: `children: []`, counted in `unlistable`, one `[files]` line per tree. The TypeScript throws and the whole tree is a 500 | an unreadable file costs that file only |
| 1055 | The native tree is sent as its text. | sending bytes needs no `Value` |
| 1056 | `/media` takes the same `safe_path` from the query's `workspace`: a name failing `workspace_name_ok` is a 400 `invalid workspace name: <name>`, a root that is not a folder a 404 `unknown workspace: <name>`. An account with hidden servers is declined, and so is a hidden workspace of one the native capture runs for (2651) | `getWorkspace`'s order; the bytes are the front's already |
| 1057 | Answering holds weight: a process total of 512 MiB and a share of 128 MiB an account, taken before the blocking read and waited for up to 2 s, else a 503 with `Retry-After` (`busy`, counted `declined`). A tree weighs 72 MiB; a file or a media path 4 MiB, grown to three times the file's size once it is known, without waiting (none free is `busy`). The tree's names are weighed against `max_bytes` inside the walk, as each is kept | a thread per request with no bound would hold up to 512 blocking threads and about three copies of a 64 MiB file each |
| 1058 | An I/O failure answered natively is logged as the route logs it: `[files] <CODE>: could not read '<rel>'`, to the log and the Logs panel, the client hearing only `entryFailure`'s words | answering must not lose the line an operator reads |
| 1059 | A file is opened on its resolved path with `O_NONBLOCK` and `O_NOFOLLOW` where the platform has them, and the opened handle must be a regular file | the stat that said it was regular can be stale: a FIFO swapped in after it would block a thread for good |

## Known gaps

What the TypeScript itself gets wrong, which a native read copies or leaves open:

| gap | state |
|---|---|
| a case fold of Unicode names against NTFS's own | `is_within` folds ASCII under a drive root, as the shim does |
| a UNC home | read as a path, with no share-aware gate |
| a path swapped for a link between the gate and the open | closed at the leaf (1059), open in the folders above it |
| a hard link that leads out of a workspace | no gate sees it |
| a draft pin | a pin of `"draft"` is read with its overlay |

## Limits and capacity

A limit weighs what a read writes and every entry a walk meets, and a real workspace must pass through with room to spare.

| limit | value | what reaches it |
|---|---|---|
| `max_seen` | 2,000,000 entries yielded by any listing | a tree of 14 times the capacity workspace below |
| `max_bytes` | 64 MiB of tree text | 4.4 times the capacity workspace's 14.8 MB |
| `max_depth` | 256 folders | a nest of 300 is declined and the stack holds (the walk is iterative) |
| `max_text` | 64 MiB of file text | a file past it is declined, not read |
| `max_rel` | 16 KiB of path asked | a body can carry 8 MiB of `a/`; the gate's climb costs an `lstat` a segment |
| answering weight | 512 MiB for the process, 128 MiB an account, 72 MiB a tree, 4 MiB a file plus three times its size | see below |

`tests/all/capacity.rs` (ignored, run by hand) writes 64 copies of the shape of step 0's `cap128` (2,200 files each: a project of 128
lanes, 16 modules a lane, 16 buses) into one workspace and walks it.

| measure | value | against |
|---|---|---|
| entries written | 149,248 | 7.5% of `max_seen` |
| tree text | 14.76 MB | 22% of `max_bytes`, under a third of it |
| depth | 3 | 256 |
| time, best of three | 440 ms for the 64 projects, about 7 ms a project | — |

The memory an answer holds was measured in release on this Mac: the capacity tree's walk peaks at 51 MB resident (14.8 MB of text) against a tree weight of 72 MiB, and a 60 MiB file read, written into a JSON value and stringified peaks at 193 MB resident against a weight of three times its size (180 MiB). The process total admits two such files at once and seven capacity trees; an account's share admits one tree and files up to 42 MiB (a file whose three times its size passes the share is `busy` and it is refused). The 4 MiB base bounds the count of small reads at 128 at once, a quarter of the blocking pool.

Hostile inputs are answered inside five seconds (thirty on musl): a folder of 20,000 entries, a nest 300 deep, names at the file
system's limit, hidden entries alone passing `max_seen`, a path of 16 KiB, 8,000 segments of `..`, a link out at the leaf
and in the middle, a FIFO, a file one byte past `max_text`, a 100,000-byte pins file, a library with 10,000 version folders.

## The revision

Sub-step 1c of `file-service-rust.md`: `GET /revision`, the change token, in Rust. The walk and the token are `crates/k7-file-service`'s
`revision.rs`, the memo and its epoch `revisions.rs`, the one `notify` watcher `watch.rs`; the front's half is `native/revision.rs`.

### The token

| # | decision | why |
|---|---|---|
| 1310 | DEPARTS, the user's choice. The spelling is `<boot>.<epoch>`, or `none:<boot>.<epoch>` with no active project. `boot` is the front's start in milliseconds, base 36; `epoch` is ONE counter for the process, and a workspace takes its next value when its inputs differ from the last ones it answered: the active folder, the mutations, the summary, the outside files' newest. Inputs that did not change answer the same token. `serviceProjects` line 156 becomes `toMatch(/^none:/)`, line 173 `toMatch(/^\S+$/)`; both hold against node | the TypeScript's token is not monotonic: deleting the newest file lowers it, a restart resets its counter, and a value can come back for another state. This one never repeats within a front's life or across lives, is a few bytes, and a poll answers it with no walk while the memo stands |
| 1311 | DEPARTS, the user's choice. The summary carries `names`: the wrapping sum of FNV-1a 64 over each source's project-relative path, `/`-spelled. A rename or a swap of two sources by another process moves the token; the TypeScript's count and newest mtime do not move for either. An edit that keeps both mtime and count is still unseen, as today | a renamed `.k7x` keeps its mtime, so another editor's rename left every view stale until an unrelated edit; a sum is order-free, and the walk already holds each name |
| 1312 | `walk_sources` is `walkProject(dir, true)`: `filter::skip` on each name; a source is a name matching `\.(k7x\|vstpreset)$` without case; a folder is entered only when the entry's type is a folder and not a link (a Windows junction is a link, as `host/fs.rs`' `kind` says); a source is stat'ed by its path, following links, never through the listing's metadata (NTFS updates it lazily: `host/walk.rs`); paths are spelled by `paths::join`, so a name holding `\` is the shim's spelling; an unreadable folder is empty and a failed stat skips the file. `newest` is `host/fs.rs`' `ms` formula, which `tests_revision.rs` holds equal on the same files | what node's walk answers through its shims, on every host |
| 1313 | `outside_newest` stats, at every ask and never kept: `<studio>/exts.json`, the preferences' file, the models' folder (`STUDIO_MODELS` else `<studio>/models`), from the `Home` of the account's environment. A test reads `engineCatalogue.ts`, `preferences.ts` and `modelCache.ts` and holds the three names equal, so the preferences' move to k7x (1100) moves this line or fails it | three stats are 1.5 µs; a plugin rescan or a channel label must move the token as today |
| 1314 | A walk that reaches `Limits::max_seen` or `max_depth` makes the inputs unknowable: that workspace takes a new epoch at every ask, said once per workspace | a token that says "unchanged" when the walk stopped early would hide an edit; a real project is 0.1% of the limit |
| 1315 | One walk per root at a time: an ask meeting a walk in flight for that root waits for it and answers its token. A walk records the feed's generation BEFORE it starts, and a result is kept only when it started after the kept one | two polls must not walk twice, and a slow walk finishing last must not put back an older state; reading the generation after the walk would absorb an event the walk had already passed |

### Decided with the route

| # | decision | why |
|---|---|---|
| 1335 | `GET /revision` is `{"rev": "<token>"}`, 200, from `Revisions::token` on a blocking thread under 1057's gate at a fixed weight of 1 MiB | the poll never waits behind a verb, and the walk is bounded like every native read |
| 1306 | A bump comes after the write has landed, never before; a token is always taken before the reads it keys; a native write calls `Revisions::bump` after its rename and before it answers | a token taken after a bump and before the write would key old files under the new token, for good; the other order costs one compile at most |

### The memo and its watcher

| # | decision | why |
|---|---|---|
| 1320 | The kept summary answers while the root is watched and up, its generation unmoved since the walk began, and its age under 10 s (`REVISION_WALK_ANYWAY_MS`, the TypeScript's). Otherwise the ask walks. With the watcher down or a root not watched, every ask walks, as the TypeScript does without a capture | a stale token hides an edit from every page; a lost event may only delay it |
| 1321 | A ceiling walk (the age ran out, nothing heard) that finds the inputs changed counts `missed`, re-watches the root, and logs once an hour per workspace: `[files] native: the watcher missed a change in <ws>, watching again` | Windows loses an overflowed batch, or the watch, without a word; inotify misses what a new folder held before its watch |
| 1322 | One `notify` watcher for the process, watching many roots; an event is routed to its root by `paths::is_within` over the `paths::slashed` spelling of its path, against the root as spelled and as canonicalized (`\\?\` stripped). A root holds a generation counter, an `AtomicU64` | one thread and one inotify instance whatever the number of workspaces; FSEvents names `/private/var` for `/var`; Windows joins its names with `\` to the root as given |
| 1323 | The watch is the workspace's root, recursive, as the capture's. Its watch is dropped before `/workspaces/remove` runs, in `mod.rs`'s branch for that route | the active project changes without a new watch, and on Windows a folder above a watched one cannot be renamed or removed: a project renamed or recycled stays below the root |
| 1324 | Events: an `Access` kind is ignored; an event whose every path is under a skipped name or is a file of an extension other than a source's is ignored; anything else moves the root's generation (a folder, a rescan, an event naming no path). An error is the root down: it walks at every ask, and is watched again at its next ceiling, at most once a minute | inotify reports the walk's own `readdir` as an open, so without the first rule the memo would never stand on Linux; a take recording into `clips/` must not make every poll walk |
| 1325 | At most 64 roots are watched; past it the root asked least recently is unwatched, and a root not asked for 15 minutes too. Its counter of mutations and its last inputs stay; the epoch is the process's, so a root watched again never repeats a token | inotify's watches are one per folder (132 for a `cap128` project) and shared with the capture's watcher until 1i |
| 1326 | The watcher is `notify` 8.2.1, already the workspace's (the capture's, `k7-hub`'s dev watch), with its default features: no new library. Its debouncer crates are not taken | the memo needs a generation, not a debounced list; a debounce would only widen the window in which a token is stale |

### Decided while building

| # | decision | why |
|---|---|---|
| 1340 | DEPARTS when an account's environment sets `STUDIO_MODELS` differently from the front's. `outside_newest(home)` reads `STUDIO_MODELS` from the PROCESS environment (the front's), because `Home` has no models field; the TypeScript reads the account's environment. Where the two differ, the native token watches another models folder than the page's views read | a field on `Home` is reversible: add it and read it in `outside_newest` |
| 1342 | `/workspaces/remove` forgets every root below the account's `workspace` folder (`Revisions::forget_under`), because the route is node's and its body is not parsed in the front; the roots are watched again by their next ask | 1323 needs the removed workspace's watch dropped, and the name is in a body the front does not read |
| 1344 | A rename, or a removal whose kind the platform does not name (Windows' removals), of a path that is gone moves the generation whatever its extension | nothing is left to stat, and a folder named `parts.v1` moved out of the root may have held sources: every platform's rename names no folder; the cost is one walk per renamed or removed file |
| 1345 | Every root's generation moves after a watch or an unwatch of any root | FSEvents runs one stream for every root and restarts it on each change, from "now": what fell between is lost for every root, and the walk that follows the move sees it |
| 1346 | An ask joins a walk in flight only when that walk began at the same count of mutations; a later walk takes the flight's place | a walk begun before a bump would answer the state before the write to an ask made after it, the old token included |

### What the revision measured

| measure | value |
|---|---|
| the walk of one `cap128` project (2,200 files, 132 folders) | median 17 ms, best 11 ms |
| a memo hit (three stats and a lock) | median 7.8 microseconds |
| another process's write to a moved token | under 1 ms in the test, 7 ms from the spawn |

### Risks of the revision

| risk | answer |
|---|---|
| a watcher that dies without a word (Windows) | the ceiling: a change is seen within 10 s, `missed` is counted and the root watched again (1321) |
| inotify's limits on a server's container | one watcher for the process, 64 roots at most; a failed watch is the root down, which walks as the TypeScript does without a capture |
| a removal refused on Windows by a watched handle | the workspace's watch is dropped before `/workspaces/remove` (1323) |
| a token that moves more often than the TypeScript's (1311, a rename; 1310, a front restart) | one reload of the page's views each time; never a view left stale |
| a phone's slow storage | the memo is what spares it the walk |

The engine link (`engine`) has its own page: `file-engine.md`.

## Portability

| choice | why |
|---|---|
| the lexical model has no `cfg!`; the vectors and the fake `Probe` spell `C:/` roots on any host | Windows is data in a test, and the Windows CI compares nothing the Mac did not |
| `Disk::real` strips `\\?\` and turns `\` into `/` unconditionally, and `code_of` carries the host's `cfg!(windows)` for 32 and 33 | the one place Windows differs in fact is `host/fs.rs`, which both read |
| a link test skips where links cannot be made, a mode-`000` test is Unix-only and guarded on root, a name that is not UTF-8 is skipped where the file system refuses it (APFS) | each is a property of the host, not of the code |

## Risks

| risk | answer |
|---|---|
| a tree past a limit | `Declined`, and the capacity table shows how far away that is |
| a legacy marker or pins file | declined and left where it is; nothing is lost |
| the later pieces (entries, revision, journal) | they stand on `Space`, `safe_path` and the decline; nothing here writes |

The undo journal, the last of these pieces, is `file-journal.md`: one journal in the TypeScript's folder and format, read and written by both.
The project's life (open, rename, recycle, the poll) is the piece `projects`: [file-projects.md](file-projects.md).

The account's chains (`chains`, step 1p): [file-chains.md](file-chains.md).

The engine's jobs (plugin queries, audition, thaw, freezer, renders, separations) are the piece `engine-jobs`: [file-engine-jobs.md](file-engine-jobs.md).

The account's waveforms, downloads and takes, the piece `media`, are in [file-media.md](file-media.md).
