# The tunnel agent

The engine's machine dials out to every registry that lists it (`apps/k7-daw-frontend/packages/engine-tunnel/docs/tunnel.md`
says what the wire is and why). This page is what the Rust agent chose; the wire itself is the hub's and is not
restated here.

## Choices

| choice | why |
|---|---|
| frames are `k7-engine`'s (`tunnel/frames`), not a second copy | the hub's code is the one that already reads every agent's frames; an agent that spelled a header another way would be the drift `engine-tunnel`'s suites exist to catch. Only the shapes a hub SENDS are read here (`tunnel/wire.rs`), leniently, as the TypeScript agent reads them |
| tasks and queues, not threads | a channel is two `SerialQueue`s and a task for its engine socket, and the socket's reader never waits on a download |
| closing a queue drops the task it is running too | a task that waits on a download for ever is cancelled with its channel, and its half file goes with it |
| `Link` counts the unsent bytes | tokio-tungstenite has no `bufferedAmount`; the writer task says when a frame is out, and the shedding of meters and the upload's high water (256 KB, 1 MB) read that count |
| any frame from the hub proves the tunnel alive, not only a pong | a busy tunnel need not answer a ping to be known alive; a quiet one that misses its next ping is dropped, which is what the redial is for |
| the root cache folder, never the path, is what a root's `name` is | a name is checked against `[A-Za-z0-9][A-Za-z0-9._-]*` before anything is joined, and a refused one closes its channel with a reason |
| one agent per registry, whether or not the engine is up | the engine's port is asked at each channel, so a relaunch that moves it is followed; a channel opened while the engine is down closes with the dial's own error |
| no WebRTC | the browser reaches an engine on its own network or through the relay: a direct data channel would serve only a remote server's page, and no page is hosted (`docs/rust-supervisor-plan.md`, decision 1); an `rtc-offer` is a frame no agent reads |

## The jobs a jobs channel may ask

A jobs channel is answered by the agent itself, from a table by request `type` (`tunnel::Jobs`). **The table is the seam
later milestones plug into**: `src/parts/tunnel/table.rs` has one line per job, each a module that registers its
handler with `Jobs::add`, taking the part's `Ctx` for what it needs (the paths, the settings, the engine). A job:

- gets the request, an `Emit` (every reply goes back through the channel's path map, `id` echoed) and a `JobContext`;
- may reply as often as it has news, and ends by returning: an `Err` reaches the caller as an `error` reply, and a
  panic as the same, said in the log;
- asks `JobContext::fetch` for a file its request names too many of to be fetched before it starts, and checks the
  paths it writes with `require_writable`, and the ones it reads with `require_mapped` (`parts/tunnel/guard.rs`);
- runs blocking work on `spawn_blocking`, never on the task that serves the channel.

`sync` has no line: the path map answers it. The test jobs `where` and `upper` belong to `tunnel_harness` alone.

## What stays Windows's

`segments.rs` refuses a segment ending in a dot or a space only on Windows, and a download that cannot be renamed over
a file something holds open lands beside it (`clip~<mtime>.wav`) on every platform.

## How it is tested

`tests/all/tunnel_*.rs` run the agent against a hub and an engine on 127.0.0.1 (`tests/support/hub.rs`), and
`tests/tunnel_part.rs` runs the part inside the whole core. `tunnel_harness` is the agent alone, which
`engine-tunnel`'s suites spawn beside the TypeScript agent against the Rust hub (`testAgents.ts` finds it in the
backend's cargo target, or in `$TUNNEL_HARNESS_BIN`).
