# The server's own edits (step 3j)

What the front writes to a song file on its own behalf, with no verb behind it: a finished take filed, the format stamp, the
open's repairs, a library module moved or made, a project created, the bench, the account's chains, a node's presets. They are
native in `packages/k7-file-service/src/edits/` and `bench/`, over `song-edit`'s `Fs`; the front's half is in `native/` (`bracket.rs`,
`modules.rs`, `bench.rs`, `engine/filing.rs`). A verb is still the browser's Worker; these are the ones it never runs.

## Choices

| choice | why |
|---|---|
| a native operation lands whole or not at all: it is staged in a `StagedFs`, planned, then committed with a rollback | a failure halfway must not leave half a take or half a project; the change set's rule, one path for both |
| a staged filesystem splits its read mounts from its write mounts, and a write outside the write mounts is refused | a take may only touch its project and its own folder; a created project only its own folder |
| every `Env` root and every path song-edit sees is posix; the real spelling is made at one place, `edits::edge` | song-edit computes on `/C:/…`; a root left as `C:\…` is one relative segment and every confinement check is false |
| a created project's heavy bytes (the video sample) are read once, before the commit, and written after it through `write_atomic`; a failed write removes the project | a staged file is text, and a half project answered 400 would refuse a retry as "not empty" |
| a step id read from `bench-source.json` is kept only when it is one plain segment; otherwise a fresh id is minted | the id names `<id>.vstpreset`, so a stored id is a path an editor of that file could aim anywhere |
| a chain write copies its presets first, each atomically, and commits the chain last | presets are keyed by id so an orphan is harmless; the chain landing is the commit of the whole |
| once the chain has landed nothing is handed back: a decline is a refusal (409), a fault is 500, and the account's and the bench's revisions are bumped | a replayed request would answer "you already have a chain" for a write that succeeded, and an open editor must reload |
| a copy checks the chain name before it builds any path | the name arrives percent-decoded; a path built first is an existence oracle for any `.k7x` |
| the account's chain writes hold the journal gate, then the account lock, in a task of their own | a client that leaves must not release the lock while the write goes on; the order is the bench's |
| a take is filed with no journal step and never replaces an existing file | an undone take would need the renamed wav back; a free `_n` sibling costs nothing |
| the format stamp and the open's repairs run together, in the repairs' order | each reads what the others wrote |
| a COPY of a folder-layout chain, and of a chain that does not read, declines; `/bench/save` has no copy, so its decline is a 404 | the TypeScript's answer there depends on a layout the native side does not read |
| `scaffold_set_*.rs` are the `mix` archetype's source, edited as Rust and never regenerated | the Rust is the archetype |

## Tests

Goldens were harvested from the TypeScript before it became stubs (the writers are deleted, the goldens frozen) and are replayed over the whole tree of files, not
only the answer. The five archetypes are checked byte for byte against `e2e/specs/conformance/scaffolds.json`. The conformance
specs `serviceProjects`, `serviceWorkspaces`, `serviceModules`, `serviceChains` and `serviceOpen` run against a front with every
piece native.
