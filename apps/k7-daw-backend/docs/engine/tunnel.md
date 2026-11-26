# The engine tunnel

An engine sits on a musician's machine, behind a home router, and the server that lists it cannot
dial it. So the engine's **supervisor dials out**: one WebSocket to the registry it already
heartbeats to, `<registry>/api/engine-tunnel`, and the server opens virtual channels to the engine
through it. Every server-side process that needs an engine — the relay a browser uses, a hosted
file service — reaches it that way, however far apart the two are.

The server's end, the **hub**, is the Rust front's (`apps/k7-daw-backend/packages/k7-engine/src/tunnel/`). The machine's end, the **agent**, is the
supervisor's (`apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/agent.rs`); both read the hub's own frame code, and `apps/k7-daw-backend/packages/k7-front/tests/tunnel/` runs the one against the other.

## Who may hold a tunnel

Only an engine the registry lists, presenting that engine's token in `x-studio-engine-token` and,
once the id is bound to one, its machine's tunnel secret in `x-studio-tunnel-secret`
(`apps/k7-daw-backend/docs/engine/token.md`). Headers and never the query string, so no access log
keeps them. One tunnel per engine: a reconnect that passes that check replaces the socket it
outlived rather than adding a second; one that fails it is refused before the live tunnel is
touched, so nobody holding the token alone can take an engine's channels over.

The supervisor also states the engine wire's version in `x-studio-engine-protocol`, and a hub
refuses one below its minimum with a `426` whose body says to update k7 on that machine
(`apps/k7-daw-frontend/packages/engine-protocol/docs/version.md`).

Both ends ping every 20 s and drop a peer that missed one. A tunnel that died without a FIN would
otherwise look alive for ever, and its channels with it.

## Nothing on a tunnel is trusted for shape

Holding a tunnel needs no account, and the heartbeat that lists an engine is open:
whoever runs a supervisor can send the hub anything, and a hub that stopped on one bad frame would
take every engine's tunnel with it. So:

- every agent text frame goes through `parse_agent_message` and every binary header through
  `decode_binary` in the hub (`tunnel/frames/`), as the hub's frames do in the agent (`tunnel/wire.rs`) — a
  non-null object, a known `t`, integer `ch`/`req`, `paths` an array of strings — and a frame of any other
  shape is ignored, never read, and said once per type, on the hub and on the agent, so a peer newer than its
  reader shows in a log;
- nothing read off a socket reaches a panic, on the hub, the relay or the agent: a fault ends that socket alone;
- the hub and the relay accept frames up to `MAX_FRAME_BYTES` (16 MiB). The largest legitimate
  frame is a file chunk, `CHUNK_BYTES` plus its header, or an engine frame passed through — a
  document or a plugin list — which stays well under it; the `ws` default would let one frame
  hold 100 MiB of the server's memory.

## A channel looks like a WebSocket

The hub's channel (`tunnel/channel.rs`) is driven as the relay drives a socket, so the relay pipes
one without knowing it is not a socket. A process outside the front, which only speaks `ws://` URLs, gets a **loopback
address bound to an engine** (`Hub::bind_local`, asked through the internal seam's `POST /bindings`,
`apps/k7-daw-backend/packages/k7-front/src/routes/engine/internal.rs`): the key in the path is the only credential, and it is
refused to anything that is not loopback.

## A close comes after the frames sent before it

Either end closes a channel right behind its last frames: a share visit ends with a transport stop and
`drop-session`, then lets go. Those frames matter most, so a close never overtakes them, in either
direction:

- **the hub** writes a channel's frames and its close onto the one socket, in order. What it holds is
  what a loopback client on a bound path sent before the engine answered (`backpressure.md`); a client
  gone by then has those frames delivered once the channel opens, then the channel closed;
- **an agent** holds a queue per direction (a frame waits there on its files), so on the hub's close it
  delivers what is queued for the engine and only then closes the engine's socket, and on the engine's
  close it delivers to the hub what the engine sent and only then says `close`.

The wait is **bounded**: `CLOSE_DRAIN` (`apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/wire.rs`, and in the hub's `tunnel/hub_tunnel.rs`), 3 s, after which the channel is
cut and what is left is dropped, never sent behind the close; and an agent's direction holding more than
`HIGH_WATER_BYTES` when the close comes is cut at once. A stuck engine, or a download that never ends, must not hold a channel and
its engine session for ever, and past a few seconds the person who closed it has moved on; the engine's
own idle-session reaping is what cleans up after a cut. A jobs channel is not drained: a request whose
caller left has nobody to answer.

`apps/k7-daw-backend/packages/k7-front/tests/tunnel/hub_drain.rs` pins both directions, a client gone before its channel opened, and the bound.

## Server-owned files, played from a local copy

The engine opens files by local path and cannot do otherwise without playing silence, so a
server-owned project is played from a **copy on the engine's machine**, under
`<state>/remote/<server host>/<root name>`. The server shares **roots** with a channel when it
opens it; the agent rewrites every JSON string under a root as frames cross:

- **server → engine**: the file is downloaded **before** the frame naming it is delivered. A
  whole document (`project`) is re-checked against the server; a delta trusts what the last
  document settled, since a delta is meant to be sent at note rate.
- **engine → server**: a file the engine changed (a take, a plugin snapshot) is uploaded
  **before** the frame naming it is delivered, so a server that files the take finds it there.

One queue per direction per channel: a frame waiting on a download is never overtaken by the one
after it, which is what keeps a `transport play` behind the document it plays.

Every channel shares one socket, so file bytes — a download, an upload, a `sync` copy — are also
what every OTHER channel's frames wait behind: a relayed probe, a transport command. The senders
stop at **1 MB** unsent (`HIGH_WATER_BYTES` in `apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/wire.rs`) rather than several: on a home uplink a few megabytes
ahead of a `whoami` is seconds, longer than a probe waits before calling the engine unreachable.
The kernel's own socket buffer keeps the pipe full past that.

**A file is served only from INSIDE its root**, by real path (`still_inside`, the hub's `tunnel/files/`), so a
link cannot aim the engine's machine at anything else on the server. The two library roots are
`perChild`: each top-level folder there is one library's clone, and containment is checked against
THAT folder's real location. A clone may then be a link — an admin's edit clone pointed at a
checkout, as a dev machine has it — while a link inside a library still cannot leave the library.
Without it every file of a linked clone answers `missing`, and the agent makes the folder and
nothing in it.

That same queue is why a device's file service opens its engine channel **`synced`**: frames to the
engine are then only rewritten, never held for a download, because every file a document names was
copied before the document was sent, by a **`sync`** request on the jobs channel. The agent answers
that one itself: it stats the files, copies the ones that differ (eight at a time), answers
`syncing` after each one with the counts of what it had to copy, and `synced` with how many failed.
The decision and its failure modes are `apps/k7-daw-backend/docs/file-engine.md`. An agent that does
not know the flag downloads as above, which is what keeps an old machine working against a new
server.

Only **whole strings** are rewritten, never substrings, and only under a shared root. A plugin
path, a device name, anything of the engine's machine is left alone because no root covers it.

**A server path maps only when every segment can be a file name on any engine's machine**
(`safe_segment`, `apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/segments.rs`, used by `apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/path_map.rs`): no empty, dot-
or space-only segment, no control character, `\` or `:`, no Windows device name, and on Windows no
trailing dot or space. The hub refuses the same table on its side, because a path an agent would open is a
path the server could aim at the machine.
It starts from the tree's own segment rule, `isPlainSegment` (`apps/k7-daw-frontend/packages/web-protocol/docs/paths.md`),
and refuses more: the rest of the table is what an engine's machine would mis-open.

**A path under a root is spelt `<root>/<segment>/…` by the SERVER, whatever its separator**, and the
path maps do not read `\` as one: on a POSIX server `\` is a legal file-name character, so only the
server knows which it meant. A Windows server respells a job's paths (`apps/k7-daw-backend/packages/k7-engine/src/tunnel/spelling.rs`), and what a
browser hands an engine is built in that spelling to begin with — a deck's record
(`playablePath`, `apps/k7-daw-frontend/packages/k7-plugins/src/turntables/service.ts`).

**A file the engine holds open is never waited for.** On Windows a rename over a file another
process has open is refused, and the engine keeps open the clips of what it played. The new version
is then placed BESIDE it, `clip~<server mtime>.wav`, and the map names that copy from then on:
`toLocal` follows the redirect, `toRemote` reads it back as the canonical path, so nothing mistakes
it for a file to upload. The ledger stays keyed by the canonical path; a version placed that way is
found again by its name after a restart, and the copy it replaces is removed once nothing holds it.

A download lands beside its target and is renamed over it, stamped with the server's mtime: a
file whose size and mtime match is already here, including across a supervisor restart, when the
in-memory ledger is gone. Reads of a file the engine is still writing are not guarded against;
the frames that name engine-written files (`recorded`, `pluginBaseSaved`) are sent after the
engine closed them.

## The server confines every path

The hub answers file requests only inside the roots the channel was opened with, checked on the
**real** path: a cloned repository may carry a symlink pointing anywhere on the server, and a
lexical check would follow it. Every segment below the root must also be a name `safe_segment` takes, the agents'
own rule: on a Windows server a device name (`CON`, `nul.wav`), a stream (`a.wav:x`) or a trailing dot
opens something other than the file named, and no agent ever sends one. Only roots opened `writable` accept an upload, written beside the
target and renamed over it when complete, so a dropped tunnel never leaves half a file under a
real name. The part file's name carries random bytes and is created exclusively (`wx`), so two
uploads of one path never write into the same part.

Holding a tunnel needs no account, so what it may upload is **bounded** (`UploadLimits`,
`tunnel/uploads.rs`): 16 uploads open at once per tunnel; 2 GiB per upload,
well above an hour-long multitrack take; 20 GiB per tunnel per
hour; and 60 s without a chunk. An upload past any of them is
aborted, its part file deleted, the agent answered a `put-reply` with the reason, and its later
chunks ignored. Without them an agent opening request after request would hold a write stream and
a growing file each, until the tunnel dropped. The refusal is told at once, while the deletion waits
its turn in the upload's chain, behind the part file's creation: an agent may see the part for a
moment after the reply, never for good, so the test overruns an upload only once its part exists.

## What it does not do

- **Evict.** The cache grows with every project opened on the machine. Deleting
  `<state>/remote/<host>` is always safe; the next open fetches again.
- **Relay a file service over HTTP.** Nothing reaches a file service on the engine's machine: a
  project has none there — a device's account runtime is in its own front, and a server holds none; see `apps/k7-daw-backend/docs/files/hub.md`.

## What the lifecycle tests pin

`apps/k7-daw-backend/packages/k7-front/tests/tunnel/hub_lifecycle.rs` runs the supervisor's agent against the hub on real sockets, and pins:

- **A dropped tunnel ends every channel it carried**, the engine's sockets behind them included,
  and a local client on a bound path is closed with it. The agent's redial, under the same id and
  secret, serves new channels; an old one stays closed, and what is sent on it goes nowhere.
- **A bound path outlives its tunnel**: 503 while the engine has none, served again once it is
  back, with no new binding.
- **The newest verified tunnel wins**: the incumbent's channels end and the next channel reaches
  the newcomer's engine. A newcomer the check refuses (another secret) is a 403 and leaves the
  incumbent's channels working.
- **A local path is 403** for a key never bound or since unbound, and for a peer that is not
  loopback — the address the front vouched for counts, not the socket's — even with the tunnel up.

The secret's trust on first use and the protocol's 426 are `k7-engine`'s tests'.

`hub_confinement.rs` attacks the hub's files with a bare agent: what no agent would send (dot
segments, foreign separators, a prefix of a root, links out, device names) and uploads that stop or
collide; `hub_uploads.rs` overruns the limits above. These suites run against the hub's testbed, which `cargo test`
builds and spawns (`apps/k7-daw-backend/docs/engine.md`, "The testbed"); the links they need make
`hub_confinement.rs` Unix only.
