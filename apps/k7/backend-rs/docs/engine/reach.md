# What the front asks of an engine

The studio's **one** engine registry is k7's Rust front's (`apps/k7/backend-rs/docs/engine/shared.md`):
the editor lists its engines from it, and every installed supervisor heartbeats to it. The engine slots
(`crates/k7-front/src/routes/engine_reach/`) live in the same front.

## What it asks of a machine: a slot

A machine's slots are jobs sent through the tunnel (`packages/engine-tunnel/docs/jobs.md`): there is no
file service on that machine for a browser to call, and a page on HTTPS could not call one there anyway
([slots.md](slots.md)).

## What it does NOT ask: an analysis

A `{file}.cache` sidecar is written by **the front**, out of its own decoder, because the front
holds the bytes (`packages/audio-analysis/docs/analysis.md`). Asking a machine for it would mean
downloading the master to that machine first and posting the sidecar back, to compute something the host
already has everything for. That is also why an engine may be absent without anything degrading
except playback on it.

## Reaching an engine

| choice | why |
|---|---|
| the slot routes ask `k7-engine` in-process (`K7_ENGINE_CORE=front`): the registry's view, a channel through the hub | the core is in this process; a loopback hop through its own seam would be a socket and a copy per frame for nothing |
| the slot routes are claimed only with the core (`Seams::start`) | without it nothing holds an engine to reach |
| one `Reach`, and a channel is one `Link` | each route is written once |
| an in-process channel holds what is sent before the engine answers (256 frames, 8 MiB), respells a jobs channel's text to the tunnel's spelling, and closes on a peer past the backpressure bound | what the hub's loopback pipe does for a seam client (`k7_engine::tunnel::local`); without it the hub drops a frame sent while the channel connects |
| `requestJob` ported (`job.rs`): an id per request, replies matched by it, the total and idle timeouts, the vectors' sentences | the machine's agent speaks this protocol whoever asks; the wording reaches the page |
| `/slot` asked of the seam's file half, five minutes allowed | the file hub is the front's; a slot is compiled by the account's file service, which takes its time |
| who asks (`caller.rs`) is a bearer the backend verifies over the seam's file half, else the session; a bearer that does not verify is refused, never downgraded to the cookie | `bearerOrSession` is the backend's guard for the routes a service may call |

## The slots

| choice | why |
|---|---|
| the slots behind the `/api` guard's bearer check, then the session | `requireAuth` follows `bearerOrSession` there: a bearer that verifies still needs a session |
| zod's messages reproduced for every body (`k7_engine::js`'s issues), the guard first, then the body, then the owner | `validate` sits after the guard and before the route |

## Proofs

`fixtures/engine-jobs-vectors.json`, node's answers kept as committed; `crates/k7-front/src/engine_jobs_vectors.rs`
checks Rust: zod's verdict on the slot bodies.
`engines_tests.rs` drives a `Link` and a job through a real hub against a fake supervisor on an in-memory socket.
The conformance spec `engineJobs` (`e2e/docs/conformance.md`) drives the routes through HTTP.
