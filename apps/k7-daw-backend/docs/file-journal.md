# The undo journal, kept natively (step 1d)

`PUT /file` with its base, `POST /undo`, `/redo`, `GET /history`, `/history/entries` and the journal's brackets of
every verb. The journal is a folder of files, read and written byte for byte by the Rust crate
(`crates/k7-file-service/src/journal`). Choices and what forced them; the code says how.

## The format is a contract (1720 to 1727)

| # | choice | why |
|---|---|---|
| 1720 | There is ONE journal, in `<studio>/.history/<project>-<hash>/journal.json` with `blobs/<id>-b<n>` and `blobs/<id>-a<n>` | an existing journal must stay readable; the hub's month sweep and the engine's `history-<slug>` root already name this folder |
| 1721 | `print` is `JSON.stringify`'s bytes: no white space, the file's key order kept (integer-like keys first, as JavaScript orders them), an unknown key kept at its place at every level, a number printed as `String(n)` prints it, strings escaped as `JSON.stringify` escapes them | a round trip that changes one byte is a file the other side reads differently; a key a later TypeScript adds must survive a native step |
| 1722 | A journal is read whole and kept with its stamp (length and mtime); every use compares the stamp first and reads again when it moved | a second process's write is read at the next use instead of being lost |
| 1723 | `store_for` is `historyStoreFor`: the shim's `resolve` of the directory, its basename with every run outside `[A-Za-z0-9_.-]` made one `_`, the first 12 hex digits of SHA-256 over its `windowsSpelling` | one project, one folder, whichever implementation asks; a canonical spelling would hash elsewhere |
| 1724 | A journal that is not JSON, or not this shape, is `Torn`: integer `nextId` above every entry's id, `cursor` within the entries, entries of kind `text` or `process` with string `label`, finite `ts`, and changes and blobs of `{path, before, after}` with `before` and `after` a string or null, `keptOn` absent, null or `{id, name}`. An integer is a non-negative safe integer. The text is read lossily first (1796) | a journal is written by programs and by a person's hand; a shape not held is never half read |
| 1727 | `text_hash` is `textHash`: cyrb53 over the UTF-16 code units, 14 hex digits | the base the browser sends is this hash of what it read |

The vectors (`tests/vectors/journal.json`, frozen: harvested from the TypeScript's own functions and
its path shim, with no generator kept) hold 433 journals and re-spellings with the bytes `JSON.stringify` prints for each, the 33
store names, 485 text hashes and the 432 answers of the TypeScript's own `PUT /file` route.

## What the journal decides

| choice | why |
|---|---|
| a mutation is journalled by diffing the project's text files before and after, with a callback for a binary the mutation is about to overwrite | an edit is caught however it was written; the callback, not size and mtime, is the authority for a same-size re-bounce |
| a file whose mtime has not settled is re-read and never kept | a same-size rewrite inside one tick reproduces the stamp, the two images diff to nothing and the edit is journalled nowhere |
| the journal is a ring of 1000 entries with a cursor; a push cuts the redo tail; an entry carrying blobs is `process` and the client confirms before undoing it | an undo of a process step restores an audio file |
| an applied undo or redo pushes the project to the engine | the rewrite happens outside any verb, and a restored event would stay silent until the next unrelated edit |
| a gesture is one step: `journal: "join"` folds a command's diff into the newest step, per file keeping its own `before` and taking the new `after`, and a file that ends as it began drops out | a module picked on a cable is two commands, and a first Ctrl+Z must not leave a node wired to nothing nobody saw |
| a join folds only into the newest step, never past an undo, into an empty journal or across audio | a step's blobs are kept and handed to an engine as one set |
| `journal: "skip"` runs and diffs the command and keeps nothing | the graph pinning where it laid itself out as a project opens is no step the first Ctrl+Z should undo |
| the journal lives in the account's state, never in the workspace | kept in the project it would be a thousand entries with audio that live sync carries to every replica |
| a journal that does not read is renamed `journal.torn-<ms>.json` and logged once, and a read that fails is no such file | a person's thousand steps are worth a file and only a person can say whether to mend it; a transient failure must not replace the journal |
| a journal is forgotten after 30 days unused, and a live one's blobs older than that | the hub's hourly sweep; an undo past a purged blob fails like one whose engine is gone |
| a step's audio is kept by the engine bound when it was recorded, the copy here deleted once the machine holds every blob, and an undo fetches it back from that engine | the engine's machine produced it; text and audio of a step go back together or not at all, so another bound engine is a 409 naming the one to switch back to |
| the supervisor deletes a kept file a month after it was last kept or sent back, and sending it back stamps it | a step still undone and redone stays |
| no engine bound keeps the blob here, under the same month | an import with no machine, a test |
| `POST /undo` and `/redo` take `confirm`, and a process step without it answers what it would revert and does nothing | the client confirms and posts again |

## The user's rulings, fixed in the TypeScript

| # | ruling |
|---|---|
| 1790 | An undo or redo stopped for a step's audio answers `409` with the sentence (`HttpError`), in node and in the native route (`Failure.status`) |
| 1791 | `PUT /file` answers an fs failure through `entryFailure`: a sentence that names no folder of the server, the thrown message logged |
| 1792 | A blob is restored through a temp beside the file and a rename |
| 1793 | A journal that does not read is renamed `journal.torn-<ms>.json` and logged once; the journal reads as empty. Only a parse or shape failure, or a folder where the file belongs, is a journal that does not read: an I/O error (EMFILE, EIO, EACCES) fails the call and leaves the file, and the next use reads it again |
| 1795 | An undo's paths go through the workspace's two path gates, all of a step's paths before its first write |
| 1796 | A journal holding invalid UTF-8 or a lone-surrogate escape is read lossily, never torn: each becomes U+FFFD, one character of a label changed. `Held::lossy` says which (amends 1724). One bad character in a label from a request body must not set a thousand steps aside, where the TypeScript reads them |
| 1797 | The walk stats a plain file from its listing (`DirEntry::metadata`) on Unix; a link, a name the shim spells otherwise (a `\`, bytes that are no UTF-8) and every file on Windows go by the joined path, as NTFS's listing can hold stale figures. The cached-images ceiling on `cap128` stays 30 ms (median 24 to 29 ms); the read ceiling stays 150 ms |
| 1798 | The TypeScript deletes an evicted step's or a dropped redo tail's blobs after `journal.json` is persisted, as the native side does, so a crash leaves orphans for the month's sweep and never a journal naming a missing blob |
| 1780a | A save's weight follows its size: 64 KiB, twice its text, twice its base's file and the text bytes of the root's last project image, within the account's share (the size part approved by the user, 2026-11-11; the image part is recommended, pending the user's ruling). Only a base file past half a share, or a project image past it, takes a save past half the share: a request body is at most 8 MiB. A `busy` decline is `shed`. |

## Records side by side, undo alone (1730 to 1739)

| # | choice | why |
|---|---|---|
| 1730 | `Journals` keeps, per workspace root, one gate: a record holds it shared from `begin` to `end`, an undo or a redo holds it alone; a record that begins while an undo waits queues behind it | two sets of records would let a native save land between a verb's two images and be undone with the verb |
| 1731 | A record's image and its token come from ONE `walk_project`; `Revisions::walked` settles the token of a walk another reader took and does not keep it as the memo | an image and a token from two walks name two instants; the memo's stamp would be read after the walk |
| 1732 | The images are the TypeScript's: every file the walk lists, a binary by `isBinary`'s names, a text file kept against `size:mtime` and never while its mtime is younger than 2,000 ms | a same-size rewrite inside one tick would otherwise diff to nothing and be journalled nowhere |
| 1734 | The gate is `tokio::sync::RwLock<()>` with owned guards, held across the record's life; a waiting undo is served before any record asked after it. The crate takes tokio's `sync` feature alone | a hand-made gate is code that can starve the undo; tokio's is FIFO |
| 1737 | When a record lands, what it wrote becomes the `before` of every record still in flight, text and binary, pre-images included (a hard link of the landed copy) | a save during a slow verb is its own step |
| 1751 | A step that fails to land (a blob not stored, the save refused) drops the journal held in memory, so the next use reads the disk | the stamp has not moved, so a pushed entry kept in memory would be written later naming blobs that were never stored |
| 1737 | Ends of one root go one at a time: a landing mutex is held from a record's removal from `records` to the push of its step, the walk inside it | a record out of `records` is rebased by nobody, so a second end landing in that window leaves its write in both steps; the TypeScript's single thread ends one at a time, and so does this (the alternative, reading `before` under the state lock, would need the walk inside it too) |
| 1738 | A record with no project answers the token twice and `files: null`, and still holds the gate shared | `recordNow`'s first branch |
| 1739 | A walk that reaches a limit, or a text past `max_text`, journals nothing for that record (`files: null`) and takes fresh tokens, said once per workspace | an image cut short would read as every file past the cut deleted |
| 1745 | A record carries an owner; `abandon(owner)` and `abandon_record(id)` remove records without journalling and remove their staged files | a verb's record would otherwise hold the gate for ever |

`diff_bin` takes a `stage_after` that copies the project's file at a rel path into a staged file of its own (`None` when it
cannot be read); a copy equal to its pre-image is removed by the caller through `unchanged`.

`begin_with` and `end_now` do the heavy work on the caller's thread; the front puts them on a blocking one.

## The blobs and the crash safety (1740 to 1755)

| # | choice | why |
|---|---|---|
| 1740 | Blobs are `<id>-b<n>` and `<id>-a<n>`; eviction and a dropped redo tail name the copies to delete, and the caller deletes them AFTER the journal is persisted | a crash between leaves orphan files, never a journal naming a missing blob |
| 1742 | A pre-image handed to `snapshot` is copied into `staged/<record>-<n>` and never held in memory; the after side is streamed into a staged file, compared with the pre-image by blocks, and both are renamed to their blob names at the push | a bounce's `.wav` is hundreds of MB |
| 1743 | `plan` says which blobs an undo lacks, `undo` answers `Retry` when the step on top changed or a blob is still missing under the gate | the fetch is an engine's download and must not hold the gate every record waits on |
| 1744 | The three sentences of `ensureBlobs`, whole, in `blobs.rs`, as a 409 (1790) | the specs and the client read them |
| 1750 | `write_atomic` is `writeFileAtomic`: the temp `.<base>.<pid>.<8 hex>.tmp` beside the file, the file's mode kept on Unix, `sync_all`, the rename retried 8 times from 10 ms doubling on Windows only | the journal, a project's text and a blob land whole or not at all |
| 1751 | A step's order is: the project's files, the blobs stored, then `journal.json` persisted | the write is what the person asked for; a step lost is the TypeScript's own failure mode |
| 1752 | Applying a side is idempotent and the cursor moves only after the files did | a failure part-way is finished by the next undo |
| 1753 | A blob is restored through a temp and a rename | 1792 |
| 1754 | A torn journal is read as empty, set aside as `journal.torn-<ms>.json` by the writer before its first persist, said once per store; reading changes nothing on disk; a read that fails is `Unreadable`, never torn, and nothing is saved over it | reading a journal must not move it |
| 1755 | A recorded path is confined before any write of its step: absolute or climbing is `history: unsafe path <rel>`, then the workspace's two gates | an undo writes what a journal names |

## `PUT /file` judged (1760)

`plan_put` runs the TypeScript's order: the base read first (the gate, a binary read as `""`, a folder's 400, absent as null) and
compared (`409 {error, stale: true}`), then the write's guards (the gate, the library, a binary name, a YAML name). An fs
failure of the write itself is the write's to find. The 432 answers of the TypeScript's own route replay in `journal_vectors`.

## The front (1700 to 1705, 1711, 1733, 1735, 1736, 1770 to 1773)

`native/journal.rs` (the save and the reads) and `journal_undo.rs` (the undo, which fetches a step's missing audio blob natively with the bound machine's `send-up` job).

| # | choice | why |
|---|---|---|
| 1733 | A save takes the journal's gate shared, then the account's lock; an undo takes the gate alone, then the lock; a fetch of audio is made before either | with the reverse order an undo holding the lock waits for a verb while a save holds it waiting for the undo |
| 1736 | A save and an undo run in a task of their own once begun | a client that leaves must not strand a written file without its step |
| 1764 | The stamp runs for a saved `.k7x` that contains the word `start` while its project's `project.k7x` declares a level under 2, under the account's lock, after the gate | the cheap test never says no wrongly, and spares the lock and a parse on nearly every save |
| 1726 | A first load of a store sweeps staged files older than ten minutes that no record in flight names; a staged copy is dated now, whatever its source's date | a copy of an old `.wav` would otherwise read as a crash's leftover |

## Gaps the TypeScript leaves and the native side copies

Two records writing one file at once: the later to land owns it. A write nobody brackets landing during a record is in that
record's step. A step lost when its persist fails after the write. A push rewrites the whole `journal.json`. Two processes
writing one journal at once: the last rename wins, each reads the other's at its next use.

A verb's writes bump no token: the token learns them from the watcher, or from the next bracket's walk, which sees them first.

## Measures

On this Mac at a load of 9 to 17, `cargo test -p k7-file-service --test all capacity:: -- --ignored --nocapture`.

| measure | cap128 (2,203 files of 250 bytes) |
|---|---|
| a record's bracket, every file younger than 2 s (both images read) | 110 to 140 ms |
| the same with the images cached (the edited file alone is young), median / slowest | 24 to 29 ms / 84 to 115 ms at a load of 9 to 11 (70 to 88 ms past a load of 23) |
| one `walk_project` of the project | 10 ms |
| one image from a walk, cached | 16 ms |

The walk's one `stat` per file is the floor of both images. Every file below the project is spelled once from the project's own spelling, since
`toRel` of each costs as much as the stat.

Hostile input (`journal_hostile`): a journal of 100 MiB loads in 0.26 s and a step persists in 1.3 s in all; a snapshot of
1 GiB is staged in 16 ms with 145 MiB resident; 10,000 texts inside the settle window are all read and none kept.

## Departures

| # | departs from | why |
|---|---|---|
| 1799 | the TypeScript, whose undo waits for a record in flight without end | an undo waits at most about 10 s, then answers a 409 "a command is still running on this workspace" (approved by the user, 2026-11-10); a verb past 10 s gives that 409 |
| 1796 | the TypeScript keeps a lone surrogate in a label | a Rust string cannot hold one; it becomes U+FFFD, and the next step prints it so; a test counts that byte as 1796's case |
| 1760 | node's `statSync(…, {throwIfNoEntry: false})` hides `ENOTDIR` | the shim does not; a path below a file with a base answers `(ENOTDIR)` rather than `(EEXIST)` |
| `Journals::begin` | the brief's signature | it takes the account's `Home`, which the store's folder is made from |
| `format.rs` | the brief's shape | each object also keeps `order`, the file's key order, so a key kept "at its place" is kept at it |

