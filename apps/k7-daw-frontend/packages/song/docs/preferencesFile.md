# The machine's preferences file

The preferences are `<state>/preferences.k7x`. `src/preferences.ts` is the only TypeScript that reads or
writes it: every caller, the file service, the commands and the stage scripts, goes through
`loadPreferences` and `savePreferences`, each a call into the `song-edit` crate.

## Choices

- **k7x, not YAML.** A native reader (the Rust compiler's rig) must read the file without a second
  YAML dialect next to the JavaScript one, and k7x is already linked. `stringify` then `parse` of a
  full `Preferences` is exact, accented text, `#`, colons and numeric-looking labels included, and a
  test holds it.
- **Whoever edits the file by hand edits `preferences.k7x`.** The syntax is k7x's
  (`apps/k7-daw-frontend/packages/k7x/docs/language.md`), not YAML's.
- **Every write is atomic.** `savePreferences` writes through the crate's atomic write, so a reader, native or
  not, sees the old file or the new one and never half of one. A symlinked k7x is written through,
  so a dotfiles setup keeps its link; a link to nothing is refused.
- **Only known fields are kept.** The text written is `normalizePreferences` of the values, so a key
  this code does not know is dropped at the first save.
- **A read answers defaults, a change never does.** A file that does not read lets every project
  open, but every mutator (`set*`, `clear*`) refuses with an error naming the file: saving would
  replace the person's audio device, labels, pairs and chains with the defaults. A transient read
  error (EBUSY, and EPERM or EACCES on Windows, while another process renames the file) is retried
  for a few hundred ms before the file counts as unreadable: a read never blocks for seconds.
- **An empty k7x is unreadable, not the defaults.** It is what a write that never finished leaves:
  every change refuses naming the file and the file stays. A k7x that is a link to nothing is
  reported as that.

## What each failure leaves

| case | a read answers | a change | on disk |
|---|---|---|---|
| the k7x does not read, or is a link to nothing | the defaults | refused, naming the k7x | the k7x stays as it is |
| the k7x is empty | the defaults | refused, naming the k7x | the empty file stays |
| the file is absent | the defaults | the file is made | the new k7x is whole |
| a save cannot write | n/a | the call throws | the old k7x is whole, and no temp file is left |

An unreadable file costs the preferences only: nothing else in k7x depends on it to start.
