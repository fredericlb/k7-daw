# What is left of the replica, a native library

`apps/k7-daw-backend/packages/k7-replica` holds what the capture still uses of live sync's replica: the ops a change is made of (`ops.rs`),
the blob refs (`blob.rs`) and the `Confirmed` store the capture's confirmed folder implements (`lib.rs`). A device runs no
replica: no workspace joins a relay room, so nothing orders its batches but its own disk. The k7x diff and replay are native in
`k7-sync`.

## The choices

| # | choice | why |
|---|---|---|
| 1100 | a crate of its own, depending on `k7-sync` and `k7-js`, changing neither | the relay's crate stays the relay's; the front links both |
| 1102 | the k7x diff and replay are `k7-sync`'s, behind `K7x`, beside whole texts, removals, hashes and blob refs | every peer replays a k7x op to the byte, as the room does |
| 1103 | `file_op_for` asks the seam only for a k7x change whose two texts differ; the rest is decided natively, `before_hash` honoured | the only k7x-specific part of `fileOpFor` is the path ops |
| 1104 | a failing k7x diff answers the whole text, never an error | a whole text is always correct and costs only bandwidth |
| 1105 | ops, batches, entries and conflicts are `serde_json::Value` objects in the TypeScript's key order | the room's types take `Value`; a newer room's unknown fields survive |
| 1113 | the native `file_op_for` is held by the 1,048 `fileOpFor` vectors of `fixtures/settle-vectors.json`, 105 of them answered by a seam that panics | the panic proves the native half never crosses |
| 1114 | blob refs are native; `blobs_of` keeps the first-seen order of a hash | a ref is file text on the wire, and a capture lists a batch's blobs in order |
| 1115 | no dependency outside the workspace's | nothing links the crate but the capture and its tests |
| 1116 | `sha2` is a dev-dependency | the vectors keep a digest of an answer too long to store |
| 1118 | the `Confirmed` store is a trait whose calls can fail, and a failing `files()` is an error, never an empty list | a confirmed folder that cannot be read must not read as one with nothing in it |

## What each proof covers

| proof | what it holds |
|---|---|
| `apps/k7-daw-backend/packages/k7-replica/tests/all/file_op.rs` | the 1,048 `fileOpFor` vectors: 105 not k7x, 943 k7x, 16 with `beforeHash`, equal as `Value`s, key order included |
| `apps/k7-daw-backend/packages/k7-replica/tests/all/blob.rs` | a blob ref's spelling and parse, and the first-seen order of `blobs_of` |

Not ported: `fileOp.test.ts`' other seven, `k7xOps.test.ts`, `k7xCorpus.test.ts` and `listOps.test.ts` test the k7x half (their
calls are in `settle-vectors.json`); `textMerge.test.ts` tests diff3 (`apps/k7-daw-backend/packages/k7-sync/tests/all/diff3.rs`).

## Risks

| risk | answer |
|---|---|
| the k7x half and the native half of `fileOp` drift when `fileOp.ts` changes | `parity.md`'s regeneration rule; `rustParity.test.ts` fails on a stale file |
| 1102 outlives step 1 | step 3's k7x editor is the moment to move the replay, held then by `settle-vectors.json` |
