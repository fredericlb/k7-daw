# atomicWrite.ts — a file is replaced whole or not at all

A file written in place (`writeFileSync` straight onto its name) is truncated first and filled
after. A process killed between the two — a quit, a crash, a power cut — leaves a short file, and
in a workspace a short file is not a local accident: live capture reads it as an edit and sends it
to every replica. So every write that REPLACES a file the user or the sync depends on goes through
`writeFileAtomic`: the bytes go to a temp beside the file, are fsynced, and the temp is renamed
over the name. A reader sees the old bytes or the new ones.

## Decisions

- **It lives in `@studio/song`.** The workspace's own saves (`yaml/workspaceFile.ts`) are here,
  and k7's backend sits above this package, so it
  imports it without a cycle. It has no dependency of its own. The backend's JSON stores (shares,
  settings, the analysis sidecar) go through it like a workspace file.
- **The temp is hidden: `.<name>.<pid>.<8 hex>.tmp`, in the same folder.** Same folder, because a
  rename is atomic only within one file system. A leading dot, because the workspace's `skip`
  (`apps/k7/backend-rs/crates/k7-file-service/src/filter.rs`) hides every dot name from live capture, the
  pattern watcher and every listing: a temp is never captured, so never synced, even when a
  process dies and leaves one. A store that lists every file of its folder without that filter
  (the confirmed mirror, `liveConfirmed.ts`) asks `isAtomicTemp` instead.
- **The file keeps its mode.** An existing file's permission bits are copied onto the temp, since
  the umask would otherwise reset them on every save.
- **On Windows a refused rename is retried** a few times with a growing pause, on `EPERM`, `EBUSY`
  and `EACCES`: an antivirus or the search indexer holding the target for a moment refuses the
  rename, and a save must not fail for that. Elsewhere a refusal is real and thrown at once.
- **An explicit mode is applied, as an existing file's is.** `opts.mode` is set on the temp with a
  `chmod`, not left to the umask, since a private file written by a caller must stay private.
- **`createFileExclusive` makes a file only if the name is free.** The bytes go to the temp and the
  temp is hard-linked to the name, which refuses an existing one: the file appears whole or not at
  all, and the caller learns that another process was first. Where hard links are refused, a caller
  that holds a lock (`heldLock`) gets the temp renamed into place after the name is seen free, so
  the file is still whole when it appears; any other caller falls back to an exclusive open, which a
  reader can see part-written, and only a lock file may use that. Lock files and the first
  `preferences.k7x` use it.
- **`readTextRetrying` and `fsyncDir` are the read and flush a rename needs around it.** A read that
  meets another process's rename (EBUSY, and EPERM or EACCES on Windows) is retried like the rename
  is, for a few hundred ms at most, since a read that waits seconds freezes the server; a folder
  flush makes a rename or a link survive a power cut, and where a folder cannot be opened there is
  nothing to flush.
- **A failed write removes its temp and throws**, and the file is left as it was.
- **Synchronous**, like the call sites it replaces.
- **`durable: false` skips the fsync, and only a store rebuilt from elsewhere asks for it**: the
  confirmed mirror and the engine catalogue's cache. The fsync is about 4 ms a file on NTFS, and a replica applying a library's first batch confirms thousands of
  files in one turn of the event loop: 2151 for `k7-core`, nine seconds of a backend that answers
  nothing. The mirror is what a restart diffs the workspace against, so a file it lost to a crash
  costs one resend of text the room already holds; a workspace file has no such second copy, and
  keeps the fsync.
- **Append-only logs stay appends.** A torn trailing line is skipped on load; only a rewrite or a
  compaction of such a log goes through here.
