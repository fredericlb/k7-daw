# The file service's native entries (piece `entries`)

The five routes that change a workspace's entries, in Rust: `POST /folder`, `/rename`, `/move`, `/entry/delete`, and the two
halves of `POST /upload`, whose bytes the front already streams. The crate is `crates/k7-file-service` (`rel`, `entries/`);
the front's half is `crates/k7-front/src/routes/files/service/native/` (`entries.rs`, `locks.rs`). The pieces it stands on are
the path model's guards and the workspace's write lock. `/library/move` is answered natively too (1213); `/project/*` and
`/open` are `file-projects.md`. These routes are not journalled and stay so.

## The lock and the landing

| # | decision | why |
|---|---|---|
| 1210 DEPARTS | `locks.rs`: one `tokio::sync::Mutex<()>` per account, keyed by its studio home, in a map of `Weak`s pruned when one is dropped. Every native write takes it, in arrival order, before its plan and until its answer; an upload takes it for each half, not across its bytes. The spec's lock is the workspace's; this one is wider, and narrowing it back is one key | the account's library and an admin's draft are one folder for all its workspaces, and writes to it must be serialized: two workspaces each finding a name free in that folder would have the second rename replace the first's file. holding it across a write cannot deadlock |
| 1216 | A write that has passed its plan runs to its end in a task of its own, the lock moved into it; the request awaits that task | hyper drops a request's future when its client hangs up, and a rename landed without its `repoint` strands every ref to the module, without its `after` leaves the compiled doc stale |
| 1213 | A library module's refs are repointed natively (`edits::modules::repoint`, song-edit's `repoint_refs` over a staged set landed whole), after the folder moved; `/library/move` is answered natively too, under the journal's gate then the lock | a ref IS a path, so the walk and the edit are song-edit's; the write lands whole or not at all, and the folder is never renamed back |
| 1215 | Only `/upload` holds the capture, around its bytes and its second half; the four others hold nothing | the capture's watcher hears a rename as it hears any other |

## Paths and guards

| # | decision | why |
|---|---|---|
| 1220 | `to_rel` is lexical: a library path answers to `_libraries/<name>/<rel>` (the version folder is not in it), the root is `.`, always `/`-spelled; it spells an answer's `path` and every guard's sentence | a canonical path (`\\?\`, `/private/var`) never reaches an answer |
| 1221 | The guards run in the TypeScript's order, function for function: `guard_shared`, `library_writable` (the account's library, or a root equal to its admin's overlay, `overlay_root` public), `guard_mutable`, `is_recycled` (the workspace's own bin, by `is_within`, folded under a drive root) | the sentences are the specs' and the vitest's |
| 1222 | An entry exists when `stat` that follows links succeeds (any error is absent); a dangling link is refused by the gate (`path escapes the workspace`) before. The disk is a `Probe` the plan asks, so `C:/` workspaces are data in a test | the shim's `existsSync` |
| 1223 | `entries.json` is frozen, harvested once from the TypeScript's own functions, called with a host that recorded the rel it was asked and threw: the cleaned name of folder, rename and upload for 300+ names, and `refuseYamlName`, `isInSharedModules`, `isPlainSegment`, `isDotSegment` | the name rules are inline expressions; the code that runs them is the referee |
| 1224 | `ref_at` and `not_engine` port `refAt` and `assertNotEngine` over `relative(<library root>/modules, abs)`; `refs` is `Some` only for a library path under `modules/` | the ref IS the path |
| 1225 | A `rel` past 16 KiB is `Declined(LongPath)`; a library's active project is declined | a decline costs one call where a guess would be a wrong write |
| 1226 | The upload's target: `name` split on `[\\/]+`, each segment trimmed as JavaScript trims, blanks dropped; none left or a dot segment is `invalid filename: <name>`; `dir` untrimmed, `.` or empty is the root, its trailing `/` dropped; the YAML refusal on the joined rel; the gate; `guard_shared`; an existing target `Exists` (409) | measured on the TypeScript |

## Writing

| # | decision | why |
|---|---|---|
| 1230 | The rename retries up to 25 times, 100 ms apart, while the code is transient (`EPERM`, `EBUSY`, `EACCES`, `ENOTEMPTY`). Each attempt is a blocking call and each wait a timer; the crate's blocking `rename_with_retry` is for callers with no runtime. `EPERM` is never produced by `code_of` and stays for the record | 2.5 s of sleeping threads, at 512 blocking threads, is a pool starved by one stuck folder |
| 1231 | The recycle bin is the library root's `_recycle_bin` for a library entry, else the workspace's; made whole; the first free of `<base>`, `<base>-2`, `-3`... with no bound | the TypeScript's names, which the specs pin; 300 deletes of one name took 153 ms under node |
| 1232 | No clobber is a check then a rename (`std::fs::rename`, which replaces a file it finds on every host); the window is the TypeScript's | parity; closing it is a platform call per host, a later decision |
| 1233 | Failures as each route answers them: an I/O failure is worded `no such file or folder` (404) or `the file could not be changed (<CODE>)` (400) and logged `[files] <CODE>: <description>, <syscall> '<path>'`, the shim's sentence, held to `DESCRIPTIONS` by a test. This holds for all five routes (1291) | the specs and the vitest read these bodies |
| 1234 | Every code goes through `filter::code_of`; no new `cfg!` in `src/`, and the grep for it covers `entries/` | the one place Windows differs in fact stays one place |
| 1235 | The temp of an upload is `.<basename>.<pid>.<8 hex>.tmp` beside the target, held to `TEMP_NAME` by a test; the crate draws the eight hex digits from std's own per-process random keys, so it gains no dependency; the second half re-checks the target and renames, and the front's `Temp` guard removes the temp whatever happens | every `skip` hides a name of that shape |
| 1236 | A name the disk changes (Windows drops a trailing dot or space) is answered as the TypeScript answers it: the name asked, not the name made | parity |
| 1237 | A rename that changes the case alone is `already exists: <name>` where the disk folds case (APFS, NTFS) and done where it does not | `exists` asks the disk |

## What the write routes decide

| choice | why |
|---|---|
| `PUT /file` with a `base` (the text hash of what the buffer was edited from) is refused with `409 { stale: true }` when the file no longer hashes to it, the check running inside the mutation in the step of the write | an autosave a second after typing must not overwrite a timeline drag, an undo of this file or a synced edit; no base is no check, and a missing file is written |
| the base is a plain 53-bit hash, not SHA-256 | `crypto.subtle` does not exist on a plain-http LAN page |
| a write that touched the open song pushes the project to the engine, behind two gates (the token moved and the path is the song's) and without dropping the browser's document | a hand-edited clip window moves the file and the revision while the engine plays the old window; the token also moves for a playlist or a library module and the path alone fires for a `.wav` dropped beside the clips; a save during a set must not empty the decks |
| `forked` answers true when an `@engine` ref materialised into its first folder | the editor must follow a save that landed on a freshly made module |
| an upload streams to a hidden temp beside its target and is renamed, every guard runs before the first byte, existence is checked again before the rename, and a body past the ceiling is a 413 in words | a 512 MB drop held as a buffer is that much heap per upload and a phone's service is killed for less; another upload of the name may land meanwhile |
| a write to a `.yaml` or `.yml` name is refused by the one check a save, a rename and an upload share | every workspace document is `.k7x` and nothing reads a YAML one |
| a deletion is a move into a recycle bin, a workspace's at its root and a library's at its own root | a library may sit on another disk than the workspace |
| `_modules` is edited in place and never renamed, moved or deleted through the routes that do that elsewhere | a module's identity is its folder and a rename silently repoints every ref naming it; the routes are reachable without the UI |

## What holds it

| check | what it holds |
|---|---|
| `entries_guards`, `entries_plan`, `entries_plan_libs`, `entries_plan_disks` | each guard's sentence and order, every vitest case of the piece, a library as an admin and not, links, a dangling link, a `C:/` workspace through a fake disk |
| `entries_vectors` | 3,200 cases of the name rules, made by the TypeScript's own functions (1223) |
| `entries_apply`, `entries_hostile` | the retry, the bin's names, the upload's halves, a link moved as itself, the race the TypeScript leaves open, the shim's tables held equal, names and paths at the limits timed on the best of three |
| `tests_entries`, `tests_entries_end` | the lock and its account-wide key, a write that outlives its client, a retry that sleeps on no blocking thread |

## The spec, and the two rulings

| # | decision | why |
|---|---|---|
| 1250 | `serviceEntries.spec.ts` pins what `serviceFiles` does not: library renames and their refs, an engine refused, a library entry recycled into the library's own bin, the entries nothing may touch, a case-only rename, a YAML name refused, a move into a library refused for an account that is not its admin. The admin's cases run in a workspace an admin made, which pins each library's draft, the only version written | a spec pins both implementations |
| 1290 | An upload into a library the account writes lands: `uploadEnd` takes a target inside the workspace or inside a library it may write. Approved by the user, 2026-11-10 | the first half's guard says an admin may add a file to a library, and the second half contradicted it with an answer that leaked a server path |
| 1291 | `/folder` and `/upload` answer an I/O failure through `entryFailure` like the other three, in the TypeScript and in `stream.rs`. Approved by the user, 2026-11-10 | the rule that a server's absolute path is logged and never answered holds for these routes as much as for the others |

## What the TypeScript itself leaves open

| gap | state |
|---|---|
| no clobber is a check, then a rename that replaces | a file landing in between is replaced, on both sides (1232) |
| a move between the workspace and a library on another disk | the rename fails `EXDEV`, which both read as `EIO`: `the file could not be changed (EIO)` |
| a dangling link | refused by the gate as escaping; it cannot be recycled through these routes |
| a library's refs repointed in this workspace only | another workspace of the account keeps the old ref |
| a name the disk changes (`x.` on Windows) | answered as asked (1236) |
| the first half of an upload makes folders a refused body leaves behind | kept as the TypeScript does |

