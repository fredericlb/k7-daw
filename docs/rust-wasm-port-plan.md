# Plan: the TypeScript the Rust backend still runs, in Rust and WASM

A feature request, not scheduled. The Rust backend (`docs/rust-backend-plan.md`) runs about 55,000 lines of
TypeScript in QuickJS: the compiler (`song`, `song-schema`, about 18,700), the verbs and edits (`commands`,
`song-edit`, about 14,700), the file service's own logic (`packages/file-service`, about 13,300) and the rest
(translator, `song-video`, `dsp`, live-sync's settle, the médiathèque's edits, about 6,000). QuickJS has no JIT:
on heavy projects a verb takes about 3 s against node's 1 (`apps/k7-daw-backend/docs/service.md`, Measurements).
This plan replaces that TypeScript by Rust: native in the backend, compiled to WASM where the browser needs it,
as `apps/k7-daw-wasm/k7x` and `apps/k7-daw-backend/packages/waveform` already are. A module's DSP is data, a `dsp.k7x` the song crate decodes (`docs/dsp-literal-plan.md`): nothing
builds it, so no sandbox is kept.

## The decisions

| choice | why |
|---|---|
| one Rust source for the backend and the browser | removes the JavaScript engine from the hosts and the question of two implementations agreeing |
| a working port, not a faithful one: no byte-for-byte parity, no legacy syntax, journals and capture state reset, a new sync protocol version | parity was the most expensive part of every earlier port; existing projects may break and are redone |
| the golden corpus and the vectors become smoke tests (it compiles, the engine accepts it, it plays) | they are no longer the referee once output may differ |
| the compiler first (step 2, before step 1), one crate native in the backend and built for WASM from its first commit; the file service stays QuickJS's meanwhile and calls it through one host function | the compiler is the half of the work both hosts need, and the file service's native pieces then call it directly instead of bridging back into QuickJS (`apps/k7-daw-frontend/packages/song/docs/rustCompiler.md`) |
| then the file service, as plain native Rust; then the verbs, native first, with a WASM half only where a browser use needs it | the browser does not use the file service, and nearly all the time a heavy project costs is the server's compile and verbs (`docs/verbs-port.md`) |
| the browser's compile is narrow and comes after the server answers natively: `song.validate` first, then the project's handle with `want: errors` on a pause; the `web` and `graph` views move only if draft preview becomes a feature | a keystroke costs under a millisecond today and compiles nothing, and the compiler is 3.5 MB of WASM plus a bundle of 2 to 5 MB per project |
| the browser's calls designed for WASM: coarse operations (compile a project, run a verb), the per-keystroke ones batched | a call into WASM copies its data; a chatty API would be slower than today's TypeScript |
| the frontend's types generated from Rust (`tsify` / `wasm-bindgen`) | the React app imports these types everywhere |

## What stays a contract, whatever the variant

| contract | why it cannot break |
|---|---|
| the engine's `ProjectDoc` schema | the C++ engine plays it; changing it means changing the engine too |
| the file service's routes and their answers | the frontend talks to them; a changed route changes the frontend with it |
| what the frontend calls and the types it uses | the WASM API replaces them call site by call site |

## The two variants

| | A: one switch per piece, no soak | B: no switches, a single cutover |
|---|---|---|
| how it lands | a switch each for the file service, the compiler and the verbs, turned on in production as soon as the suites pass | the TypeScript replaced in one go |
| how songs sound | compared with the engine's output where it is cheap | not checked |
| debugging | the old implementation runs beside the new one, so a difference points at its cause | only the UI and the suites find a bug |
| rollback | turn the switch off | the previous release and backups; data the new version wrote may not read back |
| rough time | 3½ to 4½ weeks of agent work counted before the steps' designs; step 2's design alone puts the compiler at 3½ to 4 (`apps/k7-daw-frontend/packages/song/docs/rustCompiler.md`) | 3 to 4 weeks |

Variant A is the one taken: the switches cost about half a week and buy a rollback and the side-by-side
comparison that found most bugs in the earlier phases. Most of the time in both goes to the frontend's integration and the
compiler's semantics, which neither variant shrinks. The estimates come from the earlier phases' pace and are rough.

## How the work is run

| choice | why |
|---|---|
| the agents' places, worktrees, merges and checks follow `docs/agent-playbook.md` | the rules were tuned as the port measured them; one page holds them, and this plan would contradict it |
| one Sonnet agent builds, one Opus agent designs each step, reviews it before its merge, and takes over a stuck problem | most of the port is bounded work against specs (the file service's plumbing, the verbs one family at a time, the frontend's call sites, tests); the WASM API's shape, the compiler's semantics and the hard races are where a mistake stays quiet |
| a pilot before the bulk: Sonnet ports one well-tested piece (one verb family, or the workspaces routes), compared with what Opus produces, before it takes the rest | whether it keeps this repository's conventions (one-line comments, the docs rules, the merge procedure) is not known in advance |

## The order

The steps run 0, 2, 1, 3, 4, 5: the compiler before the file service.

| step | what | gate |
|---|---|---|
| 0 | profile the actor on heavy projects; fix the WASM API's shape with the frontend's real call sites | the WASM API's shape: `docs/verbs-port.md` |
| 1 | the file service in Rust (the routes, workspaces, the journal, live capture and the relay link, the engine link, media), on the existing Rust I/O and the compiler's crate (`apps/k7-daw-backend/docs/file-service-rust.md`) | the 5d conformance specs (rewritten where output may differ), the web suite, the Mac's `e2e macos` |
| 2 | the compiler in Rust, native and built for WASM; the file service's compile calls moved to it (`apps/k7-daw-frontend/packages/song/docs/rustCompiler.md`) | every archetype and demo compiles and the engine plays it; the web suite |
| 3 | the verbs and edits, native; `/complete` stays a route; a WASM half only for a browser use that needs it | the verb specs; the web suite |
| 4 | the rest (translator, `song-video`, `dsp`, the settle, the médiathèque's edits) | their specs |
| 5 | QuickJS removed from the hosts; a module's DSP is data and needs no sandbox | done: no QuickJS runtime on any host, no switch, twin or shadow in the backend |

## What it costs

- Existing projects may not open, and songs may sound different; they are redone.
- Every device updates with the server: older machines stop syncing.
- The frontend's compile calls change one at a time, as each moves to WASM.
- With variant B, a bug reaches production with no switch to turn it off.
