# The machine's engine, run by the front

On a device's own backend, the front answers the routes a page uses to know its machine — `/api/local`,
`/api/local/tray` — whatever runs the machine's engine. Where the host asks it to run that engine too (the desktop app), the `k7-local` crate hosts
`engine/k7-supervisor` over stdio and the front also answers `/api/local/engine*`. What these routes promise a
page is `localEngine.md`; this page is how the
front keeps those promises.

## The switches

| choice | why |
|---|---|
| `/api/local`, `/api/local/tray` the front's on every device (`STUDIO_LOCAL_USER=1`, an auth folder known), whatever `K7_LOCAL_ENGINE` says | they are a device's routes whoever hosts its engine: on Android the app hosts the supervisor, and its page still probes its host; `/api/local` answers `engine: false` there, and the desktop app's line says no engine |
| `K7_LOCAL_ENGINE=front` runs the supervisor, off by default | the desktop app turns it on, with the engine core (`apps/k7-desktop/docs/app.md`); the phones host their own supervisor (`supervisorHost.md`) |
| running it needs `STUDIO_LOCAL_USER=1`, `K7_SUPERVISOR_BIN` and a known auth folder | the family exists on a device only, the engine routes sit behind the session, and a front that cannot check sessions owns none; without them the machine runs no engine, and the front's log says why |
| a 6 s grace after `shutdown` | a supervisor on Windows takes about 4.1 s to deregister, stop its engine and exit; a shorter grace kills it at the end of its own clean stop. With the engine's proof after it, a stop takes 8 s at worst, which the desktop app's 15 s holds |

## Lifecycle

| choice | why |
|---|---|
| the supervisor starts once the front listens AND `/api/registry/hello` answers at the front's own address | its first heartbeat goes there, to the front's own registry (`engine.md`) |
| it is pinned to `http://127.0.0.1:<front's port>` | the address pages use (`localEngine.md`) |
| a stop stops the supervisor before anything else, while the front still serves | it deregisters through the front, which must still answer; the event streams are then ended, since a graceful shutdown waits for every answer still streaming |
| the supervisor's environment is the front's, then every setting the front was given, then the four the host sets | in the desktop app the front shares the desktop app's process, whose environment is not node's: the settings carry what the supervisor reads (`STUDIO_PORTS`, `STUDIO_ENGINE`) |

## The host

`k7-local` is the supervisor host (`supervisorHost.md`): death read from the process,
the 1 s→30 s backoff reset after 20 s, `shutdown` then the pipe closed,
6 s, then the supervisor killed and an engine still alive killed only on the pidfile's proof.

| choice | why |
|---|---|
| a crate of its own, with no HTTP in it | the desktop app links the pidfile proof for its quit's backstop (`apps/k7-desktop/src-tauri/src/pidkill.rs`), and may host the engine in-process later; a fake supervisor binary lives beside it for the tests |
| tokio's `process`, no other crate for the child | the front already runs on tokio; its child's `wait` is the process's own end, which a pipe held by the engine cannot delay |
| on Windows, `CREATE_NO_WINDOW` and `CREATE_NEW_PROCESS_GROUP`, and no job of the host's | Rust puts a child in no job, so the supervisor's own kill-on-close job is the only one, held by the host alone; the new group keeps a console's Ctrl-C from ending the supervisor before its deregister |
| `chrono` for the local time of the merged log's `HH:MM:SS` and macOS's process start | a time zone from every OS (Android and iOS included) without FFI; it is the desktop app's already |
| the pidfile proof in `k7-local`, which the desktop app links too | one verdict for both Rust callers, the one `PidFile.cpp` gives |
| a snapshot serialized key for key in the order of the supervisor's fixtures | the page and the conformance suite read fixed bytes; `fixtures/local-vectors.json`, committed, holds the supervisor's own fixtures to it |
| one SSE event per state frame | a snapshot carries `down: null` with it, so one frame says everything |

## The internal seam's local half

`GET /api/internal/engine/local`
(`engine/reach.md`), answered here while the front runs the engine and
declined otherwise, to the 404. Its gate is the seam's: the SOCKET's peer on loopback, then
`x-k7-internal` equal to the front's secret, compared in constant time. It sits outside the owned chain:
a server-to-server call has no `Origin`, and the seam's gate is stricter than the chain's.

## The answers

| choice | why |
|---|---|
| no `ETag` on `/api/local*` | the same as every JSON answer of the front (`routes.md`) |
| a compiled slot staged as `serde_json` writes it | the supervisor parses it; a number may be spelt otherwise than the file service wrote it (`1e21`), never valued otherwise |

## Tests

`k7-local`'s `tests/all/host.rs` runs the host against `k7-fake-supervisor`: snapshots, a crash relaunched, a
dead stdin, death read while an engine holds the pipe, and both pidfile verdicts. `tests/all/front.rs` runs
the front with the switch on, against a fake backend. `tests/all/real.rs` is ignored unless asked:
a built `k7-supervisor` run by the front, with a real backend behind it, the engine stopped, started, then
stopped with the supervisor, and its deregistration checked. The conformance specs are
`e2e/specs/conformance/localEngine.spec.ts`.

`fixtures/local-vectors.json` is the committed reference the Rust is held to (`crates/k7-front/src/local_vectors.rs`);
it changes on purpose only, with the supervisor's fixtures (`supervisorHost.md`).
