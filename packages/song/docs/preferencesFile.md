# The machine's preferences file

The preferences are `<state>/preferences.k7x`. `src/preferences.ts` is the only TypeScript that reads or
writes it: every caller, the file service, the commands and the stage scripts, goes through
`loadPreferences` and `savePreferences`, each a call into the `song-edit` crate. The upgrade from a
`preferences.yaml` is the Rust server's alone (`k7-file-service`'s `preferences/upgrade.rs`); the
crate, in the Worker or in node, reports a YAML it finds as a problem and never reads it as the defaults.

## Choices

- **k7x, not YAML.** A native reader (the Rust compiler's rig) must read the file without a second
  YAML dialect next to the JavaScript one, and k7x is already linked. `stringify` then `parse` of a
  full `Preferences` is exact, accented text, `#`, colons and numeric-looking labels included, and a
  test holds it.
- **Whoever edits the file by hand edits `preferences.k7x`.** The syntax is k7x's
  (`packages/k7x/docs/language.md`), not YAML's.
- **Every write is atomic.** `savePreferences` writes through the crate's atomic write, so a reader, native or
  not, sees the old file or the new one and never half of one. A symlinked k7x is written through,
  so a dotfiles setup keeps its link; a link to nothing is refused.
- **Only known fields are kept.** The text written is `normalizePreferences` of the values, so a key
  this code does not know is dropped at the first save and at the upgrade.
- **A read answers defaults, a change never does.** A file that does not read lets every project
  open, but every mutator (`set*`, `clear*`) refuses with an error naming the file: saving would
  replace the person's audio device, labels, pairs and chains with the defaults. A transient read
  error (EBUSY, and EPERM or EACCES on Windows, while another process renames the file) is retried
  for a few hundred ms before the file counts as unreadable: a read never blocks for seconds.
- **A file that is absent is the one that is not there twice.** The k7x is looked at again after the
  YAML is found missing, since an upgrade removes the YAML last; a process that lost a race to an
  upgrade reads the k7x it made instead of answering defaults.
- **The upgrade is one process at a time, and only a change waits for it.** It takes an exclusive lock
  file (`preferences.k7x.lock`) holding the owner's pid, host and time. The lock is
  broken at once when its pid is gone on this host or when it is older than ten seconds (which no
  upgrade takes); a release removes only the text its owner wrote. A lock dated ahead of this clock is
  broken only past a ten-second margin: a drifting clock (NFS, SMB, a VM) makes every live lock look a
  little ahead, and breaking it lets two processes hold it. A read never waits: it is called during
  every compile, on the thread that serves requests, so a busy lock answers the YAML's values at once.
  A change waits 300 ms at most and, where the host cannot block, not at all and
  never by spinning a CPU. Past that it refuses with "try again" instead of saving "the YAML plus my
  change" over whoever held the lock; an empty k7x beside the YAML gets the same refusal. Everything is
  read again under the lock; the lock only spares work, so the k7x is still created exclusively. Where
  the folder cannot take a lock or a file, the YAML answers and a later read upgrades.
- **The k7x is never visible before it is whole.** It is created with a hard link, which refuses an
  existing name. Where hard links are refused (vfat, exFAT, some SMB and NFS mounts) a temp written
  in full is renamed into place under the lock, after the name is seen free. Only the lock file may
  be created by an open-then-write, since a reader treats a half-made lock as held.
- **An empty k7x is unreadable, not the defaults.** It is what a write that never finished leaves.
  Beside a YAML, the YAML answers and the upgrade replaces the empty file; alone, every change
  refuses naming the file and the file stays. A k7x that is a link to nothing is reported as that.
- **The YAML is set aside, never deleted, and does not outlive the upgrade.** After the k7x is
  written, flushed with its folder and read back equal, `preferences.yaml` is renamed to
  `preferences.yaml.pre-k7x` in the same folder, or to the first free `preferences.yaml.pre-k7x.1`,
  `.2`… when one exists, so an older backup is never overwritten. The rename is retried on Windows
  as the other renames are. The backups are kept for good and never read by k7x. They hold what the
  k7x cannot: comments, unknown keys, and a file of a shape the upgrade read as the defaults. A YAML
  an antivirus holds past the retries stays, and the k7x wins.
- **A copy that reads back different is taken back only if it is the one this call wrote.** The k7x
  is renamed aside under a unique name and compared there; its bytes are removed only when they are
  the ones this call wrote, and anything else is another process's save and is put back. The YAML
  then stays too.
- **The k7x takes the YAML's permission bits**, so a private file does not become world-readable.
- **The flush is real on every host**, so the k7x is on disk before the YAML is moved, and a file
  at 0444 or 0400 is written before it is locked down.

## What each failure leaves

| case | a read answers | a change | on disk |
|---|---|---|---|
| both files exist | the k7x | the k7x is changed | the YAML is left untouched, and never read again |
| the YAML does not read, no k7x | the defaults | refused, naming the YAML | the YAML stays and no k7x is made, so a repair by hand is honoured |
| the folder is read-only | the YAML's values | the save throws | the YAML stays; the next read upgrades it |
| the copy does not read back the same | the YAML's values | follows the next read | the copy is taken back if untouched, the YAML stays |
| the k7x does not read, or is a link to nothing | the defaults | refused, naming the k7x | the k7x stays as it is, and the YAML beside it is not consulted |
| the k7x is empty, a YAML beside it | the YAML's values | follows the next read | the upgrade replaces the empty file |
| the k7x is empty, no YAML | the defaults | refused, naming the k7x | the empty file stays |
| the lock is held (an empty k7x included) | the YAML's values, at once | refused, "try again" | nothing is written |
| a read keeps failing | the defaults | refused, naming the file | nothing is written |
| a save cannot write | n/a | the call throws | the old k7x is whole, and no temp file is left |

An unreadable file costs the preferences only: nothing else in k7x depends on it to start.

## Going back to an older k7

The upgrade is one way. An older k7 reads only `preferences.yaml`, so after the upgrade it finds none
and starts from the defaults, and what it saves is a YAML the newer k7 never reads while the k7x
exists. The way back is manual: put `preferences.yaml.pre-k7x` back as `preferences.yaml` (it holds
the values as of the upgrade, not later changes) and, to keep the newer k7 from preferring its k7x,
move `preferences.k7x` away.
