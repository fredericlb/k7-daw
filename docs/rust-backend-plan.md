# Plan: k7's backend in Rust

The goal is ONE backend, in Rust, on every device: the tray, Android and iOS (no server hosts it).
No node runs on any device (no Termux node, no NodeMobile, no bundled `backend.mjs`): the
tray and the phones run the backend inside their own process. The frontend stays TypeScript.
The TypeScript the backend runs in QuickJS (the compiler, the verbs, the file service's logic) is
replaced later by Rust and WASM: `docs/rust-wasm-port-plan.md`, a feature request.

## What "the backend" is

Not `apps/k7/backend` alone (10k lines) but every TypeScript that runs server-side, about
90–110k lines: the backend itself; file-hub; file-service, with the song compiler
(`song`, `song-edit`, `translator`, `song-video`, `graph-model`, `take-render`, `engine-client`)
and the server half of `commands` and `k7-plugins`; engine-registry, engine-tunnel and
supervisor-host; audio-analysis, user-prefs, ffmpeg.
**Node leaves a device only when file-service and the compiler are ported**, which is why they are
a phase of their own and not left behind.

## The method

A strangler, never a big bang. The Rust front (`apps/k7-daw-backend/docs/front.md`) takes the
backend's address from the first phase and proxies whatever is not ported yet. A route family
moves when its gates pass, and its TypeScript is deleted in the same change. Every gate is a
check that runs against BOTH implementations:

- the black-box conformance suite (`e2e/docs/conformance.md`), which each family's in-process
  backend tests are rewritten into before that family is ported;
- the functional suite (`e2e/docs/e2e.md`) through the front;
- golden corpora for what is written to disk or to the engine: ProjectDocs, sidecars, k7x edits,
  the session cookie (`apps/k7-daw-backend/docs/sessions.md`), tunnel frames.

**A family moves with every route that shares its in-memory state.** The TypeScript backend keeps
some state in memory and writes it back whole: the API tokens (read once, `lastUsed` flushed over
the file), the supervisor it runs for the local engine. A Rust route
minting a token would be refused by every TypeScript route checking one, then erased by the next
flush. So the tokens move with the routes that check them, the local
engine with the file hub, and the readiness probe with the file services it
reports on. A family that reads its state from disk on every request (the preferences, the app
releases) or keeps it to itself (the cameras) moves alone.

That rule makes the engine core ONE move. The registry and the tunnel hub are held in memory by the
file hub (its binding and proxy); none of them
can move without the others, and a seam between them (TypeScript calling a Rust hub over loopback)
would be code written to be deleted. The file service is the exception that stays apart: it is a
process of its own and already reaches the hub over a socket.

## The phases

| phase | content | gate |
|---|---|---|
| 0. foundations | the Cargo workspace (`apps/k7-daw-backend`); the front and its peer secret; `k7-embed` for Android and iOS; the conformance suite | conformance identical direct and through the front; the embed running on a phone |
| 1. small surfaces | health, preferences, app releases, cameras | conformance, and each implementation reading what the other wrote |
| 2. engine trust, tested | the relay, ticket and tunnel lifecycle tests `docs/audit-moteur.md` §E lacks, written against TypeScript and kept as the gate of phase 4 | they pass, several runs in a row |
| 3. the session and the leaves | `/auth/*` and the API tokens; the analysis, uploads, AI organize | the on-disk corpus read and written byte for byte; a token minted by either implementation accepted by the other's gate |
| 4. the engine core, in one move | the registry, the relay and its gate, the tunnel hub; the file hub with its libraries, blob store and history log; the local engine and supervisor-host; the routes that hold them in memory — stems, engine slots, engine-local | phase 2's tests and `engine-tunnel`'s two-agent suite against both hubs; a TypeScript replica syncs against a Rust relay and the reverse; a security review |
| 5. file service and compiler | file-service, the compiler, jsdsp, renders, `.dawproject` import, picture; `/api/ready`; the file hub, left by phase 4 — the survey, order and decisions are `docs/rust-phase5-plan.md` | golden ProjectDocs over `k7-core`, the demos and the e2e fixtures |
| 6. cut-over (done) | the tray links the backend, Android and iOS load it, the backend is one binary; `apps/k7/backend`, node, Termux and `bundle.mjs` are gone | the functional suite on the web, the phone and the tray; one published version per switch before its TypeScript goes |

The front is the backend in the dev session, the tray and every phone build
(`apps/k7-daw-backend/docs/front.md`, `routes.md`). It owns the session (`/auth/me`),
the preferences, the engine registry and tunnel hub, the file hub and the file service, the libraries (frozen to the
bundled seed), the AI settings and the built page; the embed carries it on a phone
(`apps/k7-daw-backend/docs/embed.md`).

## What it costs

- **What the frontend needs stays TypeScript, so it is written twice**: the `song-schema` readers,
  the k7x reader and editor, the mediatek types and helpers, lyrics, panel-rules, web-protocol's
  helpers, and engine-protocol, which gets a THIRD spelling beside TypeScript and C++. Shared
  vectors bound the drift; nothing removes it.
- **The session cookie is a contract**, frozen by vectors (`apps/k7-daw-backend/fixtures`) and minted for the suite by `e2e/src/sessionCookie.ts`.
- **Feature work pays twice during the migration**: a backend change is written in TypeScript and
  ported later, or it waits.
- **The trust code is the riskiest to rewrite**: the relay gate, tickets, path containment, the
  loopback locks, CORS and the mount order are dense, partly untested, and a rewrite is where a
  gate goes missing unnoticed.
- **jsdsp's jail is redesigned**, not ported: node's permission model, the resolve hook and the
  `dsp` user do not carry over to an embedded JS engine, and on a phone the OS-user wall is gone.
- **Data and machines in the field**: journals, history logs, sidecars and
  k7x edits must stay byte-compatible, and replicas running the TypeScript backend must keep
  reading what Rust ones wrote.
- **Gaps in Rust's libraries**: no mature comment-preserving YAML editor (the sidecars), no
  node-diff3-identical merge, no official Anthropic SDK, no
  playwright: each is code of our own or a sidecar kept.
- **Iteration slows**: `tsx watch` becomes a compile, and the backend's 400 in-process tests are
  rewritten black box.
- **The build grows**: a Rust target for Android, a staticlib on iOS, cross-compiling on the
  Windows agent, and the supervisor's trust roots beside the front's, which must agree.

Each library a phase needs is proposed with its strengths and weaknesses before it is added.
