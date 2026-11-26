# `@studio/k7-wasm/edit`: song-edit in the verbs' module

## Choices

| choice | why |
|---|---|
| One call, `edit.call {fn, args, env}`, named as the TypeScript names it (`song-edit/trackMix.editTrackStruct`); `edit.batch` runs calls in order and stops at the first throw | the 3e vectors name calls this way, so the replays and the module share one table (`song_edit::facade`) |
| The crate's file system is a set of imports (`k7_fs_*`) answered through `node:fs` | the host's own file system stays the only state |
| A path the host does not know is status `needs`, collected, and thrown as `needs_files` after the call returns | an exception through Rust frames skips destructors |
| A text crosses in `k7_fs_buf`, never in the call's input buffer | `k7_call` borrows its input for the whole call |
| Bytes are `{"$":"bytes","b64":…}`; ids come from `k7_new_id` (`setIdSource` pins them) | JSON carries no bytes, and the crate reads no random source |
| A trap fails the call and the next call runs on a new instance | `panic = "abort"` leaves thread-locals borrowed |
| A loop over a selection is one `edit.batch` (`stubBatch`): events remove, track delete, a lane's rename and its sound | the first throw ends the batch, as it ended the loop; a loop that tolerates a throw per item (assembly delete) stays single calls |
| The host's write is a temp beside the file, then a rename, keeping the file's mode and following a link | a reader sees the old file or the new one, and a read-only folder refuses |

## The proof

`npm run test:edit-replay -w @studio/k7-wasm` has the native replays write the calls they make (`K7_EDIT_PLAN`), then re-runs each through the module over a temp folder; the answers and the changed files must equal the native ones. `gate.sh` and `check-vectors.sh` run it.

On macOS the disk run departs on names APFS folds to NFD (a non-ASCII argument), counted as departures; the Vfs run has none.
