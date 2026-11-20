# Parity: where the Rust backend matches node's JavaScript

The JavaScript number formats a file holds (`toFixed`, `String(number)`, `Math.round`, live-sync's `textHash`) and live-sync's
`TextBases` are native Rust in `k7-js`, checked against node's vectors. No JavaScript engine runs in the backend.

| module | what it is | for |
|---|---|---|
| `k7-js::bases` | native Rust | live-sync's `TextBases`, the versions a late whole text merges against |
| `k7-js::number` | native Rust | `toFixed`, `String(number)`, `Math.round`, live-sync's `textHash` |

## The choices

| choice | why |
|---|---|
| `toFixed`, `String(number)` and `Math.round` are native Rust | each is a closed algorithm ECMA-262 pins down, and the vectors below prove the port |

## What each proof covers

Every vector file is written by node from the TypeScript (`fixtures/make*Vectors.ts`) and committed; the Rust tests fail when
Rust disagrees. A vector file is regenerated on purpose, never as a side effect of a test.

| proof | covers | does not cover |
|---|---|---|
| `number-vectors.json`, `k7-js/tests/all/number.rs` | 30 edge cases, each through `toFixed(0, 2, 4, 6)`, `String` and `Math.round`, as node printed them: a snapshot, not a generator | the bulk of the doubles and the texts a larger set once covered |


## The phones

| platform | state |
|---|---|
| Android | `cargo build --target aarch64-linux-android` builds k7-js, which needs no C toolchain |
| iOS | `scripts/agent/ios-check.sh` builds the front for the device and the simulator |
| desktop | Windows built and tested; Linux and macOS take Cranelift's native backends |

## Keeping the vectors in step

The settle's and diff3's vectors are frozen: no generator is kept, and the committed vectors are the referee.
Regenerate the others on purpose (`npx tsx apps/k7/backend-rs/fixtures/make…Vectors.ts`).

## The settle

The merge of one op against a room's text is native: `k7-sync`'s `settle.rs` decides a whole text, a
removal and the diff3 merge ("The text merge", below), and its `k7x` module (`locate`, `diff`, `apply`) decides a
k7x op on `packages/k7x`'s document editor.  the committed
vectors, which node's TypeScript wrote (`settleFileOp`, `applyFileOp`, `fileOpFor`), are the referee.

### The choices

| choice | why |
|---|---|
| the k7x half is a stateless unit (`Native`) behind the `Js` seam | no thread, no pool, no queue: every room and every account's capture shares one, and a test stands a fake on the seam |
| a key path locates through `item_segments` over `LIST_KEYS`, a list item named by all its key fields | a path never holds an index, so a peer that reordered a list still lands an op on the same item |
| a replay splices a changed value's source verbatim and inserts through the editor's `set_in` / `insert_in` | a comment or a spelling between two peers must survive, and a re-printed value would rewrite the file |
| `settle` makes `next` an `Err` for an op `apply` cannot replay | the room parks what throws on apply; `next` and `apply` are never allowed to disagree |
| `TextBases` is native (`k7_js::bases`) | it is the room's state, read on every write: lengths in UTF-16 units, a JS `Map`'s order (a removed file that comes back goes last), `isBlobRef`'s exact shape |
| `ConflictSet` is left to the room | it holds no format, only ids and the `valueKey` join |
| `NeedsUpdate` is named in the error | the TypeScript class does not set `name` |

### Where it answers differently, on purpose

| case | node | Rust |
|---|---|---|
| a `text` op without `text`, applied | `next` is `undefined`, and the room would store it | `next` is `Err(())`: the room parks it |
| an error the engine itself throws (a malformed op's `TypeError`) | V8's message | Rust's message, same class; the vectors compare the class only |
| a settle that runs long | no limit | no limit: the work is linear in the file, so no timeout parks an op |
| a lone surrogate in a keyed value | kept | not reachable: serde_json cannot hold one, so no op carrying one reaches the room |

None of these changes what two peers converge on, so the sync protocol's version is unchanged (decision 4-11).

### What each proof covers

`fixtures/settle-vectors.json` (4 MB), written by node's TypeScript and frozen, 4,419 vectors, every
envelope byte for byte (334 past 800 characters as their SHA-256). `settle_vectors.rs` replays every settle and
apply, `file_op.rs` of `k7-replica` every `fileOpFor`, `text_bases.rs` the TextBases trace:


`fixtures/settle-vectors.json` (4 MB), frozen, 4,419 vectors, every envelope byte for
byte (334 past 800 characters as their SHA-256), each also rebuilt from the typed call:

| source | vectors |
|---|---|
| live-sync's own tests (`fileOp`, `textMerge`, `k7xOps`, `room`, `listOps`, `converge`), every `fileOp.ts` call they make, recorded while they ran; `converge.test.ts`'s random runs kept to their first 400 distinct calls per function | 1,350 |
| a fixed sample of 232 `.k7x` files of `k7-core`, `demos`, `e2e/fixtures` (one 51 KB `ui.k7x`): value edits, a keyed-list item edited, removed, added, each as `fileOpFor`, its replay, and its settle against the text, a moved value (`kept`), a removed item, no file | 2,584 |
| edge cases: CRLF, BOM, unicode keys and values, 18 number and string spellings through the printer, unknown and retired kinds, malformed ops, removes, root ops, whole-text merges clean and conflicting (diff3), CRLF lines, bases missing, older, or not the newest, blob refs, `.K7X` | 485 |
| the TextBases trace: 63 steps (the 20-version cap, the 2²⁰-unit cap with astral characters, k7x and blob refs never kept, `versions`/`restore`, a reload, the map's order), each followed by every file's versions and `find` probes | 63 |

The vectors do not re-read the corpus, so editing `k7-core` breaks nothing; they are never regenerated.

## The text merge

A whole text whose file moved, merged against its base (`mergeTexts`), is `k7_sync::diff3`: node-diff3 3.2.1's
`diff3Merge` ported step for step, called by `k7_sync::settle`.

### The choices

| choice | why |
|---|---|
| node-diff3's own algorithm, not a diff of our choosing: Hunt and McIlroy's LCS with node-diff3's choice of candidate, `diffIndices`, `diff3MergeRegions`' hunks sorted stably, its bounds arithmetic and the false-conflict rule | among repeated lines (blank lines, a chorus) several longest common subsequences exist; Myers' or patience diff picks other matches, hence other conflict regions, and an op node parks would land, or the reverse |
| the candidate search gallops where node scans, and skips at once the matches node's scan cannot use | a node merge is superlinear (a 2,000-line text with a blank line in 8 takes 0.3 s in node); the candidates' indices strictly increase, so the slot node's scan stops at is the one a binary search finds |
| lines interned to integers before the LCS | node keys its classes by the line in a prototype-less object, which is string equality; lines named `__proto__` or `constructor` are in the vectors |
| `lines("")` is `[""]` | `"".split(/(?<=\n)/)` is `[""]`, and an empty line against no line moves conflict bounds |
| a work budget: past 2²⁹ steps of the two LCS (about 1 s) the op is parked, as a conflicting one is | node-diff3 is cubic on a text of equal lines: one within TextBases' 2²⁰-unit cap would hold a relay thread for hours. The budget counts steps, not time, so a machine's load never changes an answer |
| a k7x op past its work budget (40 million bytes reparsed and items keyed per pass) is a whole text | the replay reparses the text per op and keys each list once per op; 2,000 transposed notes are 4,000 ops; no timeout bounds the work |
| 2²² candidates at most, then the same parking | each candidate is kept for the chain, and a hostile pair of texts could make one per step |

### Where it answers differently, on purpose

| case | node | Rust |
|---|---|---|
| a merge past the work budget | finishes, after an hour or more | parked |

### What proves it

| proof | what |
|---|---|
| `crates/k7-sync/tests/all/diff3.rs` | every one of the 241 non-k7x settles of `settle-vectors.json` (82 with a base) answered by the native settle as node's `settleFileOp` and `applyFileOp` answered, with a `Js` that panics if asked |

### Measured costs

Windows, release build (`cargo run --release -p k7-sync --example diff3_measure`); node 24 in WSL beside it.
One line changed near the top by one side, one near the end by the other:

| text | native | node 24 |
|---|---|---|
| 2,000 distinct lines (22 KB) | 1.2 to 1.6 ms | 12 to 20 ms |
| 5,000 distinct lines (57 KB) | 3.3 to 4.1 ms | 61 to 112 ms |
| 20,000 distinct lines (243 KB) | 23 to 49 ms | 1.7 to 1.9 s |
| 2,000 lines, a blank line in 8 (19 KB) | 1.4 to 1.7 ms | 220 to 314 ms |
| 5,000 lines, a blank line in 8 (50 KB) | 6.1 to 6.5 ms | 2 to 4.1 s |
| 20,000 lines, a blank line in 8 (215 KB) | 63 to 98 ms | 135 s |
| 200,000 equal lines | parked at the budget after 0.7 to 1 s | — |

The last line is the budget's cost, the most a hostile text holds a relay thread. The 20,000-line text with blank
lines takes 45 million of its 537 million steps; past roughly 70,000 such lines a merge would park, where node
would take over an hour.

## The replica

What is left of the replica (`crates/k7-replica`, `files/replica.md`) is native for whole texts, removals, hashes and
blob refs, with the k7x diff and replay native in `k7-sync` (step 4).

| proof | what it holds |
|---|---|
| `fixtures/settle-vectors.json`' 1,048 `fileOpFor` vectors | `crates/k7-replica/tests/all/file_op.rs`: the native `file_op_for` against node's, key order included |


