# The file hub in the front (`K7_FILES=front`)

The front holds the file hub (phase 5a, `docs/rust-phase5-plan.md`): the ticket and the routes that hand each
account to its own runtime, the libraries,
`/api/ready` and AI organize. No TypeScript hub is left
and the hub starts no node process: every account's file service is the front's own (`service.md`). The pages of each part are under `files/`: [the hub](files/hub.md),
[the history](files/history.md).

## The switch

| choice | why |
|---|---|
| `K7_FILES=front` in the front's settings (`ENV_KEYS`), followed only with the auth folder known and `K7_ENGINE_CORE=front`; every host sets it (the tray's `backend.rs`, Android's `BackendHost.kt`, the image's `start.sh`, `dev-front.mjs`) | the registry the front holds needs the hub's runtimes; without the switch no process answers the family |
| the backend reads no switch and runs no hub: it answers `GET /api/internal/files/hub` with `runs: false` always (`apps/k7/backend/src/media/internalFiles.ts`), and holds none of the family's paths | one owner of the runtimes |
| the front asks `GET /api/internal/files/hub` every second until the backend answers, and takes the family only on `runs: false`; a backend that answers 404 is a previous release's, which runs a hub of its own | the previous release is the rollback (`docs/rust-phase6-plan.md`, decision 3): pairing it with this front still leaves one hub, the backend's |
| the family's paths move together on every host: `/api/files`, `/api/files-hub`, `/api/ready` and the seam's file half (`routes/files/mod.rs`) | they share the hub's memory: the runtimes by secret |
| the seam's file half is asked of the front's own port once the switch is on (`files::seam_addr`), by the registry and the recorder alike | whichever process holds the hub answers, and a test bed sees the same calls a host makes |

## The hub

The ticket, the routes to a device's account runtime, the libraries and a server's 410 for every other workspace path: `crates/k7-hub` (tickets, the allow-list, the
account names and environments, the engine bindings, the caches' sweep, with no HTTP framework) and
`crates/k7-front/src/routes/files/hub/` (the routes). The ticket, the allow-list and the claim are
[files/hub.md](files/hub.md); this table is how the front does it.

| choice | why |
|---|---|
| a front that is not a device (`STUDIO_LOCAL_USER` unset) makes no account runtime: `/api/files` answers 410 but for the libraries, `warm` does nothing, the cut resolves in the libraries' context ([files/hub.md](files/hub.md)) | a server edits no workspace; devices share them over WebDAV or SFTP |
| the hub starts nothing: no node child, no port, no cap, no crash backoff, no dev watch; a host still naming `K7_NODE`, `STUDIO_FILE_SERVICE_ENTRY`, `STUDIO_LIBRARY_SERVICE_ENTRY`, `STUDIO_MAX_FILE_SERVICES` or `STUDIO_FILES_WATCH` gets one log line | every account's runtime is the front's own, so a process per account would be a second owner of the account's files (`service.md`) |
| an account's environment is `k7_hub::env`'s allow-list taken from this process's environment, overlaid by the host's `Config.env`, then the account's own values (`Names`) | a host that embeds the front hands its settings through `Config.env` alone; every name a runtime reads is therefore in `ENV_KEYS` too |
| without a code root every account's files and the libraries answer a 503 naming it, never a start of something else | the files are the front's or nothing |
| `account_key` lives in `k7_hub::account`, and `hub-vectors.json` holds it | every folder of a files root names an account by the same function |
| the hub's own calls (the engine seam) are `k7_hub::call`, one pooled hyper client | a seam's answer is JSON read whole |
| the engine seam is called over loopback at the front's own port with `K7_FRONT_SECRET`, kept bindings re-posted under their key on a new epoch | the core answers it in this process today, but the contract is the seam's (`packages/engine-registry/docs/internalSeam.md`) |
| the ticket's key is `${SESSION_SECRET}:files`, else 32 random bytes per process | an open page keeps its ticket through a deploy, and a ticket the previous release minted reads here |
| the shares' bearer each runtime is handed (`MEDIATEK_TOKEN`) is minted by the front: an ephemeral token (@studio/oidc's format, `api_token_secret`, held in memory, listed nowhere), which `routes/engine_reach/caller.rs` accepts before it asks the backend | only the process that verifies a bearer can hold an ephemeral one; `MEDIATEK_URL` is the front's port |
| `GET /api/files-hub` and `/api/files` read the request's own session | one identity for every route of the family |
| `routes/auth/warm.rs` warms this hub on a signed-in `/auth/me` once the front holds the family, which does nothing on a server; a device's local user is warmed at launch | a page asks who is signed in before it lists anything, so its account's runtime is up by its first list |
| `exts.json` is written through `k7_hub::json` (`JSON.stringify`'s spelling), a copy of `k7-mediatek`'s | `k7-mediatek` brings Wasmtime, which the front does not carry |

### The libraries

The store of `libraries.json` and the library folders, over the seed the app bundles. No relay carries a library and
nothing is fetched. They answer `/account/libraries*` (the admin flag is the ticket's or the session's) and
`/libraries/sync`. Their choices are [libraries.md](libraries.md).

| choice | why |
|---|---|
| one runtime for the whole origin, made at launch where a code root is, in the front ([libraries.md](libraries.md)) | the folders have one writer |
| on a standalone (`STUDIO_LOCAL_USER=1`), a WRITE to `/api/files` answers 503 `k7 is starting: its libraries are being installed` with `Retry-After: 1` until the libraries say their first `ensure` settled, or 45 s have passed; `/api/files-hub` and every READ answer at once | a workspace created before the bundled library is installed has none, which is the gate's whole subject; the page a phone opens first asks for its workspace list before those libraries could matter, and a refusal meant for creation must not wear a read's face |
| the file family is launched before the supervisor's and the recorder's (`lib.rs`, `serve_seen`) | the page a phone opens on startup asks for its workspace list before anything else, and each family blocks a worker while it starts: launched last it queued behind a second of the engine's own starting |

### Answers chosen on purpose

| difference | why |
|---|---|
| a render machine the seam does not answer leaves `STUDIO_RENDER_JOBS_URL` unset, and the service starts | a service that renders nowhere is still the account's files |
| `/api/files-hub` with a method the hello does not take goes to the backend, which answers its 404 | one 404 for every path nobody serves |

### Security review

| trust decision | what proves it | pinned by |
|---|---|---|
| a ticket is HMAC-SHA256 under `${SESSION_SECRET}:files`, compared in constant time, refused past its `exp`, with an empty or missing `u`, or malformed; the engine after `~` is unsigned and binds nothing without a claim | `k7_hub::ticket` | `tests/all/vectors.rs` (TypeScript-minted tickets, read and minted byte for byte), `ticket.rs`; `filesHub.spec.ts` |
| the identity a client sends is ignored (`x-studio-user`, `x-studio-hub-secret`, `x-studio-engine-token`, `x-studio-admin`): the account served is the ticket's or the session's, the admin flag only the ticket's or the admin groups' | `hub/proxy.rs` `files` | `tests/files_hub.rs`; `filesHub.spec.ts` |
| each runtime's secret is 32 random bytes, handed in its environment alone, and is its gate | `k7_hub::account::random_secret` | `tests/files_hub.rs` (a forged secret is a 401) |
| `/api/files-hub/internal` answers only a held runtime's secret, compared in constant time, whatever else the request carries | `hub/answers.rs` `internal`, `State::account_of_secret` | `tests/files_hub.rs`; `filesHub.spec.ts` |
| a server answers 410 to every `/api/files` path but the libraries', and its cut makes no account folder | `hub/proxy.rs` `files`, `hub/answers.rs` `cold_run`, `Services::cut` | `tests/files_hub.rs`; `serverFiles.spec.ts` |
| the libraries have their own secret; their admin flag comes from the ticket or the session alone | `hub/proxy.rs` | `tests/files_hub.rs` |
| an account's environment is the allow-list: no session, token, OIDC, render or front secret reaches it, whatever its name | `k7_hub::env` | `tests/all/vectors.rs` (`hub-vectors.json`) |
| only a claim binds an engine: the registry is asked `holds` for the account before any path is bound | `k7_hub::binding` | `filesHub.spec.ts` (an unclaimed engine leaves the files reachable) |
| the shares' bearer the front mints is ephemeral, held in this process, accepted only with `exp: 0` and its id held for its subject | `hub.rs` `verify_service_token` | read in the code |
| the seam's `cold-run`, `slot` and `hub` sit behind the seam's own gate (loopback and `K7_FRONT_SECRET`) | `routes/engine/internal.rs` `gate` | `tests/files_hub.rs` |

`fixtures/hub-vectors.json` holds account keys, tickets minted and read, the allow-list and the sync target as the
TypeScript hub answered them (its cap and port-range cases are no longer replayed); it is the reference as committed, nothing regenerates it,
and `crates/k7-hub/tests/all/vectors.rs` fails when Rust disagrees.

## The settle

A k7x op's settle (`settleFileOp`, then `applyFileOp`) is `k7_sync::k7x`; a whole text, a removal, the diff3 merge
of a whole text (`k7_sync::diff3`) and the room's `TextBases` are native too. The choices, the API, the proofs and the costs are in `parity.md`, "The settle"
and "The text merge".

Where it answers differently, on purpose: a `text` op without `text` is parked (node would store `undefined`);
a text merge past its work budget is parked, where
node might have merged.

| trust decision | what proves it | pinned by |
|---|---|---|
| the merge writes what node writes, byte for byte | 4,419 vectors from node, every envelope compared | `tests/all/settle.rs` |
| the text merge writes what node writes, and parks what node parks | 3,032 `diff3Merge` vectors from node-diff3, and every non-k7x settle vector answered natively | `k7-sync/tests/all/diff3.rs` |
| a hostile text cannot hold a thread in the merge | a work budget and a cap on candidates, counted, not timed | `diff3.rs`' `a_degenerate_text_runs_out_of_budget_instead_of_hours` |

## AI organize

`POST /api/ai/organize/turn` (`crates/k7-front/src/routes/ai/turn.rs`) makes ONE model turn with the account's key
and model; the AI settings and `GET /api/ai/organize` are the front's phase-3 routes. The conversation, the
prompts and every check run in the page (`apps/k7/frontend/docs/aiOrganize.md` holds those choices). The backend
has no `/api/ai` route: a request the front hands it there meets its `/api` guard.

| choice | why |
|---|---|
| the turn takes a prompt, the answer's schema and up to five messages that start and end with the user, and nothing else of the browser's body reaches Claude | the key, the model, `max_tokens` and the effort are the account's and the server's; the page decides what to ask |
| the Messages API by hand over the front's hyper and rustls client (its roots, `front.md`), `ANTHROPIC_BASE_URL` read as the SDK reads it | no SDK crate is approved; the call is one streamed POST |
| the request body is the SDK's byte for byte: `model`, `max_tokens`, `system`, `messages`, `output_config`, `stream`, compact, and a repair's assistant turn is the streamed blocks with their fields in the server's order | a cached system prompt and a repair that reasons from its own turn depend on what is sent; the vectors compare the bytes |
| `@anthropic-ai/sdk` 0.128's defaults: two retries on a connection error, a timeout, 408, 409, 429 or 5xx, `x-should-retry`, `retry-after-ms` and `retry-after` obeyed, else 0.5 s doubled up to 8 s with up to 25 % jitter; the 600 s timeout runs until the headers only | the SDK's defaults are what the vectors were written with; once the answer streams, a cut is the route's error, as in the SDK |
| the SSE decoding and the message accumulation follow the SDK's `LineDecoder`, `SSEDecoder` and `MessageStream`: events dispatched by name, `ping` skipped, an `error` event an API error, the message the one `message_stop` sealed | the errors streamed to the browser are the SDK's sentences (`request ended without sending any chunks`, `stream ended without producing a Message with role=assistant`) |
| a stream past 8 MiB is cut with an error | `max_tokens` bounds a real answer; an upstream that never stops costs one call |
| the NDJSON (`status`, `ping`, `turn`, `error`), the heartbeat (the last status, else a ping, every 15 s) and the abort on a closed tab are the route's | the button reads the same lines; a tab closed drops the request to Claude |

### Where it answers differently, on purpose

| difference | why |
|---|---|
| an SSE `data:` that is not JSON is reported with serde's message, not V8's `SyntaxError` | the API never sends one |
| the request headers beside the compared seven carry `user-agent: k7-front/<version>` and no `x-stainless-*` platform headers | they are the SDK's telemetry; the API reads neither |
| `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_CUSTOM_HEADERS` are not read | the route passes the account's key; the SDK would add those two from its environment, which k7 never sets |
| a `tool_use` block's `input_json_delta` is not folded | the request offers no tool |

### The proofs

`fixtures/organize-vectors.json` was written by a server-side route that ran the whole conversation, with the real
SDK, against a fake Messages API, for 16 cases: a good panel, mistakes then repaired on an update, not JSON then
repaired on Haiku (no `effort`), mistakes twice, a refusal with and without its reason, `max_tokens`, an
HTTP error in JSON and in text, a 529 retried after its `retry-after-ms`, an `error` event mid-stream, a
dropped stream, a stream that ends before `message_stop`, an empty stream, and two bodies zod refuses. Every
stream is written in 97-byte pieces, a character split across two at times. It holds the replies, each request the SDK sent (seven headers and the body) and the NDJSON streamed back,
and is the reference as committed: the route and its generator are gone.
`crates/k7-front/src/turn_vectors.rs` plays the same replies from a Rust twin of the fake, turn by turn, and requires the
same requests byte for byte and the NDJSON each turn is the server's part of; `packages/panel-rules/src/aiLoop.test.ts`
drives the page's loop with the same replies. `crates/k7-front/tests/all/ai_turn.rs` runs the guards through a whole front.

Not proven: the abort on a closed tab (timing, read in the code); a timeout or a connection error (the messages are
the SDK's constants); a real Claude answer.

### Security review

| trust decision | what proves it | pinned by |
|---|---|---|
| the account's key leaves only to `ANTHROPIC_BASE_URL` (the API's own host otherwise), over TLS with the front's roots, and never to the page | the vectors read `x-api-key` at the fake; the page's body names a model and a key and neither is used | `turn_vectors.rs`, `ai_turn.rs` |
| a body is parsed under 2 MB before the session is read, the session checked before the key, the key before the shape | the order of `post_turn` | `ai_turn.rs` |
| a turn is bounded: five messages, a 200 KB prompt, 64,000 output tokens, 8 MiB of stream | the shape check and the stream cut | `turn.rs`' tests, `turn_vectors.rs` |
| a request with a bearer goes to the backend, which answers no `/api/ai` route | `claim`'s bearer rule for `/api/ai/*` | `routes.md` |

## Answers chosen on purpose

| difference | why it is accepted |
|---|---|
| an unreadable volume's message in `/api/ready` is the system's | a person reads it; the status and the keys are the previous release's |

## Ready

| choice | why |
|---|---|
| loopback by peer AND by the Host it names, refused with the words of the backend's loopback guard (`apps/k7/backend/src/loopback.ts`) | a proxy on the same machine would otherwise expose it |
| the volumes are the raw `STUDIO_FILES_ROOT`, `STUDIO_MEDIATEK` (the instance's data folder), `STUDIO_PREFS_DIR`, unset ones skipped, each probed with a file it removes, 3 s each | each is a volume a write must reach; the hub's figures keep node's shape and never make a host unready |

## The history rooms

A device's workspace history is an ordered log in a room folder, kept by `crates/k7-sync` (`room`, `log`, `relay`, `settle`,
`history`) and driven by the capture (`k7-capture/src/history/`). Why each is what it is: [files/history.md](files/history.md);
No socket, blob store or change stream is left: nothing dials a room.

| choice | why |
|---|---|
| the Room's ordering, its conflicts, its text bases and every file it writes are native; an op's settle is native (`k7_sync::settle`, its `k7x` module for a k7x op); a whole text whose base moved is merged by a native port of node-diff3 (`parity.md`, "The text merge") | a whole text or a removal is a hash comparison, and node-diff3's merge is quadratic where it is run as written |
| an entry stays the JSON object it arrived as (`serde_json` with `preserve_order`), written by a `JSON.stringify` of our own (`k7_sync::json`) | a batch's fields, unknown ones included, travel in its entry as they came, and every byte of `entries.jsonl`, `room.json`, the segments, the index and the snapshots is what an older log, read by a newer build, expects: numbers as `String(number)`, key order kept |
| the rooms are this process's alone | the log folder of a workspace has one writer |
| a room is loaded outside the rooms' map lock, one load per room at a time, and the idle pass takes one room at a time | a big room's replay or a slow room would otherwise hold up every other room's batches |
| the history reads a room's folder with the room locked | a save rolls `entries.jsonl` into a segment; a page read meanwhile would miss its entries |
| `fixtures/sync-vectors.json`, written by the retired TypeScript relay and never regenerated: the history functions' pages and files at every point | the reference as committed; `crates/k7-sync/tests/all/vectors.rs` fails when Rust disagrees |

### Security review

| trust decision | what proves it | pinned by |
|---|---|---|
| a room folder cannot climb out of its parent: the name is a single folder name and is ENCODED into the folder | `room_name_error`, `is_workspace_name`, `room_dir` with `encodeURIComponent` | `path.rs` tests |
| an op's path stays in its workspace, checked on the batch and again on submit | `sync_path_error` in `is_file_op` and `Room::submit` | `protocol.rs`, `room.rs` tests |
| a log written by a newer k7 is refused, never replayed | `check_version`, `parse_entry_line` | `log` tests |

A room's folder name keeps the names Windows reserves (`CON`, `NUL`, `COM1`…) and a trailing dot
(`k7-sync/src/path.rs`): known and accepted for now. A room Windows cannot open fails that workspace's own history, never another's.

## The proofs

| what | how | seen |
|---|---|---|
| the 85 specs of 5a against the previous release's TypeScript hub | `p5a-bed-up.sh plain`, specs at the backend and at the front | 85/85 at both: the specs hold either hub to one contract |
| the 85 specs with `K7_FILES=front` | `p5a-bed-up.sh front`, specs at the front with `CONFORMANCE_FRONT=1`, on a fresh and on a reused files root | 85/85, five runs in a row |

## The security review, by subject

| subject | where its rows are |
|---|---|
| the files ticket, the identity headers, the runtimes' secrets, the environment allow-list, the claim before a binding | "The hub", security review |
| room names, newer logs refused | "The history rooms", security review |
| an op as data | "The settle" |
| the Anthropic key and the answer check | "AI organize", security review |

## What a host gives the front

| requirement | why |
|---|---|
| `K7_FILES=front`, with `K7_ENGINE_CORE=front` and the auth folder | the switch is the front's alone; the backend runs no hub either way |
| `STUDIO_FILES_ROOT`, and what the runtimes read, in the front's settings (`ENV_KEYS`), with a code root (`K7_CODE_ROOT`, or a checkout above the front) | an embedded front has no environment of its own, and without a code root every account's files are refused |

## What stays TypeScript

The backend process holds no hub: no service, no libraries, no readiness and no AI route. What it still runs is `verify-token` on the seam, for the bearers. Node starts no child for the hub: every account's files and the libraries are the front's own (`service.md`).
