# Plan: phase 4 of the Rust backend

`docs/rust-backend-plan.md` names phase 4 "the engine core, in one move". This page is what that
move is made of, why it is cut into three, and the two decisions taken before any code: the five
formats run as their own JavaScript and WASM inside the Rust backend, and the move ships in three.

## What the move holds

About 30,000 lines of TypeScript, more than phases 0 to 3 together:

| part | lines | the hard part |
|---|---|---|
| engine trust: `engine-registry`, the hub half of `engine-tunnel`, `upgradeGuard` | ~3,700 | the hub's channel multiplexing; `hubFiles`' confinement, which is all that keeps a remote machine out of the server's disk; the relay gate; the claim and role rules |
| file hub and live-sync | ~7,800 | the sync relay MERGES (`settleFileOp`): through the k7x document editor, `node-diff3` and a 53-bit hash over UTF-16 units; the log's format; device tokens and pairing with machines on older releases |
| local engine, supervisor host, remote forward and door | ~1,500 | killing an engine only on pidfile proof; the Windows job's single holder; shutdown order; the door forging a session that other families' routes consume |
| the médiathèque deferred from phase 3, and the rehearsal recorder | ~18,000 | comment-preserving `song.yaml` edits; TagLib tag writes; `{file}.cache` sidecars written with `toFixed`; the recorder's JSON-document SQLite store and 1 GiB multipart uploads |

## Five things no Rust library reproduces

The two implementations write the same files and answer the same machines, so each of these
must come out byte for byte what TypeScript produces:

1. **The k7x document editor** (`packages/k7x/src/edit.ts`, `print.ts`): the relay applies k7x
   operations through it; the Rust crate only parses.
2. **`node-diff3`'s merge**, as the relay calls it.
3. **The `yaml` library's `Document.toString()`**, which edits a sidecar and keeps its comments.
4. **TagLib's tag writes**, through `taglib-wasm`, per format.
5. **JavaScript's arithmetic where it is the format**: `toFixed` in the sidecars, `textHash`.

## Decision 1: how those five are reproduced

| option | for | against |
|---|---|---|
| **A. rewrite each in Rust, held by vectors generated from the TypeScript** | no second runtime; the end state of the plan as written | each is an open-ended parity project (the YAML printer and TagLib above all); a vector suite never proves the inputs it does not hold, so drift shows up as a rewritten file on a user's disk |
| **B. run the TypeScript and WASM themselves inside the Rust backend**: an embedded JS engine (QuickJS, through `rquickjs`) for the editor, `yaml`, `node-diff3` and the arithmetic, and a WASM runtime (`wasmi` or `wasmtime`) for `taglib-wasm` | identical by construction: the same code runs; a change on the TypeScript side reaches both; phase 5 needs QuickJS anyway, for the jsdsp sandbox | a JS engine stays in the backend (QuickJS: about 1 MB, no JIT, slower on large documents); a bridge to maintain; it is not "no JavaScript", only "no node" |

**Decided: B for the five, A for everything else.** The plan's goal is no node on a device, and
B keeps it; what B gives up is purity, for the five places where a divergence silently rewrites
people's files.

## Decision 2: one move, or three with seams

As one move, nothing ships until all 30,000 lines do, and every gate must pass on the same day.
The couplings that force "one move" are few and narrow, so it ships as three moves:

| move | content | the seams it needs |
|---|---|---|
| **4a. engine trust and the local engine** | registry, relay and gate, tunnel hub; supervisor host and the local engine routes; the remote forward and `server-engines` | TypeScript that stays (the médiathèque's engine routes, the share player, the file hub's binding) reaches the Rust hub through the loopback local channels and job channels the file services ALREADY use (`bindLocal`, `requestJobAt`), not in-process; and a loopback query for the few registry reads it makes (`get`, `holds`, `engineRole`) |
| **4b. the shares and the engine reach** | the author's side of shares, the share player, engine slots | the file hub's answers on bearers, sync credentials, chains and slots, and the device door's prefixes owned by the front |
| **4c. the rehearsal recorder** | `rehearsal-server` and its store | its loopback into `/api/mediatek` goes to the front |

**What stays for phase 5:** the file hub with the sync relay, favorites, the server copy (its library
half goes with `serverSongs`) and the device door (its médiathèque prefixes go with 4b). The relay merges through the k7x editor, so it belongs with the file service and the
compiler, which are built on that editor too. The door forges sessions for routes of every family,
so it moves last.

The seams of 4a are not throwaway: the local channels and job channels are the protocol the file
services already speak, and the registry query is a few lines. That is the difference from the seams
phase 3 declined.

## Gates

- **4a**: phase 2's lifecycle tests and `engine-tunnel`'s two-agent suite (the TypeScript agent
  and the C++ `tunnel_harness`) against the Rust hub; the C++ supervisor's heartbeat, deregister
  and tunnel against the Rust registry on all three desktop systems and the phone; `hubFiles`'
  confinement attacked with the cases of `PathMap_test.cpp`; a security review before merging.
- **4b**: the corpus of real libraries read, edited and re-read by both implementations with no
  byte changing that the edit did not change; tags written by each read by the other and by
  TagLib's own tools; the functional suite's médiathèque specs.
- **4c**: the recorder's specs, and a store written by each read by the other.

Each family's in-process tests become black-box conformance specs before its port starts
(`e2e/docs/conformance.md`).

## Libraries to approve

`rquickjs` and `wasmtime` (taken: taglib-wasm's module needs Wasm exception handling, which `wasmi` lacks — `apps/k7/backend-rs/docs/parity.md`). `russh` and `russh-sftp` for the hunt's
SFTP. `multer`'s equivalent (`axum`'s multipart). No YAML crate if decision 1 is B.

## Size

4a is the size of phases 1 to 3 together; 4b about twice that; 4c about 4a. Each ships alone.
