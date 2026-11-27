# k7-supervisor — the engine machine's supervisor

One program on the musician's machine, this crate, between whoever runs it and the engine. It starts,
adopts and watches `k7-runtime`; keeps it listed at the studios it is connected to; holds the tunnel
a server reaches the machine through; and does the work a server asks of the machine — a stem cut,
a set list brought down, a file handed to the player, the log. The paces and the rules it follows
are in [supervisorPolicy.md](supervisorPolicy.md).

## Two ways to run it

| | run by | settings | stdout | stops when |
|---|---|---|---|---|
| **under an app** | k7's front under the desktop app (`apps/k7-daw-backend/docs/localEngine.md`, through `k7-local`) or the phone (`apps/k7-daw-android`) | `<state>/engine-host.json`, changed by the app's commands and saved | the app's protocol ([hostProtocol.md](hostProtocol.md)) | stdin closes — the app went away |
| **the command-line engine** | `k7-supervisor --config <file>`, a terminal, a service, the server's container | the file, read once and never written | nothing | SIGINT or SIGTERM |
| **inside the iOS app** | `k7_supervisor_run` on a thread of `apps/k7-daw-ios`, a library linked into `k7-embed`'s since no iOS app may run a program (`src/run.rs`'s `embedded`, `apps/k7-daw-backend/packages/k7-embed/src/supervisor.rs`) | as under an app | the same protocol, on a pipe of its own: the app's stdout is shared | the command pipe closes |

`--config <file> --port <n>` fixes the engine's port, which otherwise is only the first one tried
("The engine's port", below).

The config file has the settings' schema (`registryUrls`, `port`, `audioPriority`, `onnxProviders`,
`instruments`, `deviceTokens`, `lanOpen` — whether a device on this network drives the engine unpaired,
`apps/k7-daw-backend/packages/k7-runtime-server/docs/pairing.md`), **`stems` and `instrumental`** — the chain THIS machine separates
with, by what the cut wants, since which module and model it runs is that machine's CPU, GPU and
downloaded models, not an account's ([stemSplit.md](stemSplit.md)); its owner names them
from the studio's Engines card as well as by hand, through the `chains` job
(`apps/k7-daw-backend/docs/engine/chains.md`), plus a single
`deviceToken` for every studio the file names, and the two keys that otherwise come from the environment — `machineName` and
`extraCaCerts` — and `"instruments": ["all"]` for `$STUDIO_INSTRUMENTS=all`. A key the
file names wins over the environment; one it leaves out falls back to it, so `{}` is a valid config.
`engine.example.json` in this crate is the one the release archive carries.

The file is never written back because it is the user's: a hand-edited file rewritten by the program
loses its order and its blank keys, and a choice the supervisor makes on its own — the instruments a
first boot settles — lasts the run. With no app to own the session, the supervisor truncates the
merged log when it starts and echoes every line to stderr, which under an app would print each
line twice.

## A process of its own, not a part of the engine

A plugin that crashes takes the engine process down, and the supervisor is what brings it back —
while the tunnel and the studio listing stay up. A stem cut burns every core for minutes and
downloads write to the disk; beside the audio callback they would be heard. So the engine plays
audio and nothing else, and everything that can fail slowly or loudly lives here.

It is a crate of the backend's workspace, not a target of k7-runtime's: building it must never mean
building Tracktion and JUCE, and one lock with the front makes its TLS, WebSocket and HTTP crates the
versions already reviewed.

## The identity, and the one secret the engine never holds

`engine-id.json` (`src/identity.rs`) holds the id, the token and the **tunnel secret**. The engine
gets the id and the token, and echoes the token on `whoami` to whoever it trusts; the tunnel secret
goes only into the heartbeat body and the tunnel's upgrade header, which is what lets a registry bind
the id to this machine (`apps/k7-daw-backend/docs/engine/token.md`). A file from before the secret
keeps its id and token and gains one on the next start. The file is 0600, and its folder 0700 when
the supervisor creates it, since it is the machine's credential.

## An app learns we died from our stdout, so no child may hold it

An app watches one thing to know we are gone: our stdout closing. That EOF is what it reaps on,
logs, and relaunches us from — there is no second signal. A supervisor whose death goes unnoticed
leaves a machine whose engine still runs and whose studios no longer list it, since nothing is left
to heartbeat and a registry entry outlives its last heartbeat by `REGISTRY_TTL_MS`.

The engine outlives us by design, so a copy of the app's pipe inside it would block the app's read
forever, through a crash and through a rebuild alike. On POSIX every pipe the standard library makes
is close-on-exec and a child's stdin is null (`src/process.rs`); on Windows the three std handles are
made non-inheritable before anything is spawned ([windows.md](windows.md)).

An app that dies closes our stdout, and the next frame we write would raise SIGPIPE, whose default
action ends the process before the stdin EOF reaches the shutdown — no deregister, and an engine
nobody watches. Rust ignores SIGPIPE before `main`, so that write fails as a write. The engine is
started with SIGPIPE ignored too, which Rust's spawn would otherwise reset: its output is piped to us,
and an engine that outlives us would die at its next log line, before the next supervisor can adopt it.

## An engine is adopted by its PORT, and a probe that cannot ask says nothing

Whether the engine is up is asked of its port (`isServing`), and the answer has three values, not
two: up, down, or *could not ask* — `socket()` itself failed, which is what running out of file
descriptors looks like. Read as "down", that third answer cost a machine a whole night: the pidfile
named the live engine, the probe could not open a socket, the supervisor dropped the pidfile as
stale, and every engine it started after that died on the port the first one still held, every two
seconds, while the browser's transport kept being reset. So *could not ask* keeps the pidfile and
retries, and a start that finds the port already answered with no pidfile naming its owner adopts
that owner (its pid asked of `lsof`, or of the TCP table on Windows) rather than spawning an engine
that can only fail to bind.

## The engine's port

`port` is the FIRST port tried, not the port: the next free one of ten is taken when another program
holds it. With a fixed port, one program on the machine is enough to keep the engine from ever
starting — phare's desktop agent listens on 8766, for one — and an engine that
cannot bind exits at once, which the crash loop reads as a crash and answers with safe mode. Nothing downstream
needs the port fixed: the registry's presence, the snapshot and the tunnel all read
the one the start reports, and the pidfile records it so the next supervisor adopts the engine there.

What holds a port is judged by BINDING it, as the engine will, not by connecting to it: a connect
cannot tell an engine from any other listener. The bind is tried on the wildcard AND on 127.0.0.1:
on macOS a wildcard bind succeeds beside another program's loopback listener (adb's port forwards
sit on 127.0.0.1), and the engine, which binds 127.0.0.1, would then fail on the port it was handed.
A port answered by an engine no pidfile names is still
adopted, and is never skipped for the next one, or a lost pidfile would put a second engine on the
audio device. `--port` is for whoever wrote the port down — a firewall rule, a LAN address — and
there a held port is an error naming its holder, never a silent move.

In the app (a phone, or the iPad app on a Mac) the engine binds the port it is handed once per
process, so a held port is skipped before the engine is asked, never found by a failed bind.

What ran the descriptors out was the heartbeat: one started every five seconds whether or not the
last had come back, so a network gone dead — a sleeping laptop, a DNS lookup the request timeout
does not bound — stacked them, each holding its sockets and pipes, until the 256 a process gets
were spent. A heartbeat still in flight now makes the next tick skip.

## One wire, one set of tests

The server's half of the tunnel is `apps/k7-daw-backend/packages/k7-engine/src/tunnel/`, and the machine's half
is this crate's `src/tunnel/`, which reads and writes the hub's own frame code ([tunnel.md](tunnel.md)). Both are
held to ONE suite: `apps/k7-daw-backend/packages/k7-front/tests/tunnel` runs this crate's agent, as a library, against the
real hub, and the conformance suite's stand-in engine runs its `tunnel_harness` binary.

## A server reaches this disk only under its roots

A server names files by its own paths, and the map turns them into this machine's — a download
lands there, an upload is read from there. So a server that could spell its way out of a root
could write any file on the machine with bytes it chose, or read any file back up.

Each direction is held element by element, never as a string prefix (`src/tunnel/path_map.rs` and
`src/tunnel/segments.rs`, cases in `tests/all/tunnel_paths.rs`):

- **down**: the server's path below its root is split on `/`, and a segment is refused when it is
  empty, only dots and spaces (`.`, `..`, and `.. `, which Windows trims to `..`), or holds a `/`,
  `\`, `:`, NUL or control character — on Windows a `\` would be a separator and `C:\` or `\x` would
  replace the whole path — or is a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`9`,
  `LPT0`–`9`, the console names, any case, with or without an extension), which Windows opens as the
  device rather than as a file. A server file with such a name is the same refusal on every machine,
  a Mac's included. The joined path must then still lie under the root once lexically normalised;
- **up**: a local path maps only when every element below the root passes the same rule, so
  `<root>/./../..` maps to nothing rather than to the server.

The jobs that WRITE — `render`'s `out` or `outDir` and every stem in its manifest, `separate`'s
`outDir` — and `send-up`, which stamps a file so the channel uploads it, accept only a path the map
places in a root the server marked writable (`require_writable` in `src/parts/tunnel/guard.rs`), and are refused with an error
reply otherwise.

The jobs that READ a path the server names — `prefetch`, `separate`'s `source` and `model` —
accept only one under this machine's copies of the servers' roots, `<state>/remote`, by spelling and
again once links are resolved (`require_mapped`). `prefetch` does not count a path it refuses, so it
never tells a server whether some other path exists on this disk. `separate`
opens its source itself, in-process: a name is a file and nothing else — never an option, a network
protocol or a playlist.

`render` works in a folder of its own under the temp directory, created by the job itself with a
random name and owner-only ([machine.md](machine.md)): a name that can be predicted is a folder another user
of the machine can make first, or point elsewhere with a link.

Under `K7_ENGINE_IN_APP` (a phone, which may start no program) `render` asks the RUNNING engine instead: a
`render` command over its socket (`src/machine/render.rs`), into a scratch folder under `<state>/render-tmp`, since the
engine writes only inside its write roots and the temp directory is not one. The files are then copied out under
the same checks as above. The count of renders in flight rides the snapshot as `rendering` (absent at zero), which
`Busy` reads so that a render outlives the app going behind (`hostProtocol.md`).

The engine opens its socket to web pages only from the studios it should serve: the supervisor
starts it with `STUDIO_ENGINE_ALLOWED_ORIGINS` set to the origins of the studios this machine is
listed at (`engine::allowed_origins`), the rest of that rule being the engine's
(`apps/k7-daw-backend/packages/k7-runtime-server/docs/pairing.md`).

## The crates, and why each

| | what for | why this one |
|---|---|---|
| serde_json | every frame and file | the front's own |
| tokio | the core, the parts, the sockets | the front's runtime; blocking work goes to `spawn_blocking` |
| rustls with ring | the tunnel, HTTPS to the registries | one TLS stack on every platform, no C build and no process-wide state to guard |
| tokio-tungstenite, hyper | the engine socket, the tunnel, HTTP | the front's own, over one TLS configuration (`src/trust.rs`) |
| `ort` | stem separation | the binding every Rust user of ONNX Runtime has ([separation.md](separation.md)) |
| `waveform` | decoding a mix, resampling its stems | the next section |
| `zip` | unpacking instruments | the front's own |
| `windows` | what only Windows does | typed calls, no hand-written FFI ([windows.md](windows.md)) |

Every one is in the backend's `Cargo.lock`; none is found on the system.

## The cut decodes with the media library's decoder, not ffmpeg

A stem cut needs every format a record arrives in, among them Opus in WebM and AAC in m4a. JUCE, the
engine's decoder, reads no WebM; ffmpeg is one more program a musician would have to install. The
decoder (`apps/k7-daw-backend/packages/waveform`, symphonia and opus-rs) reads all of them, is held to
ffmpeg's frame counts by its own tests, and trims an mp3's encoder delay as ffmpeg does — so a stem lines
up with the record as it did. Its rubato resampler takes the mix to the model's 44.1 kHz and the stems
back to the record's rate. It is linked where separation is, the platforms ONNX Runtime ships for: a
computer's and iOS, not Android.

## Trust comes from the OS

The OS's roots are the trust anchors (`src/trust.rs`): `rustls-native-certs` on macOS and Linux, which
reads the Keychain WITH its trust settings — a certificate the admin or the user set to *Never Trust* is
dropped — and the distribution's bundle; on Windows the ROOT stores of the user and the machine, with the
`Disallowed` stores and the server-auth usage applied ([windows.md](windows.md)); the bundle the app ships
on iOS (`$STUDIO_SYSTEM_CA_CERTS`, since iOS lets no app list its roots) and the system's folders on Android;
and `$STUDIO_EXTRA_CA_CERTS` on top. A studio behind a private CA works because that CA is in the OS store.
A platform that gives no root at all falls back to Mozilla's (`webpki-roots`).

## One task owns the state

The core (`src/core/`) is one task that owns the engine's lifecycle and the snapshot; every other
subsystem is a PART, a task of its own fed events (`src/parts/mod.rs`). Anything that blocks — a start,
a probe, `lsof`, a kill — runs on `spawn_blocking`: a blocked core would freeze the crash watch and the
host lock's refresh. A panic in a part ends that part's task, not the process, once the panic hook has
written `fatal: …` to the merged log (`run::log_panics`).

The tunnel never blocks a socket's own thread: a frame that must wait for a download waits on its
channel's queue (`SerialQueue`), one per direction, which also keeps frames in order; a channel's
close drains that queue first, within a bound (`apps/k7-daw-backend/docs/engine/tunnel.md`). A job runs on
a task of its own. What a job changes in the settings goes through the core, the settings' single
writer (`Ctx::update_settings`) — a job writing the file beside the core's own copy would be undone by
the next save of that copy.

## One stem cut at a time

A cut holds the whole decoded song, per channel, and the overlap-add buffers of all four stems: past
a gigabyte for a ten-minute song. A set list sent for cutting asks for every song at once, on a
machine whose engine may be playing live — so the machine runs `$STUDIO_SEPARATIONS_AT_ONCE` cuts at
a time (1 when unset), read once per run, and the others wait in the order they asked
(`src/separation/slots.rs`). A waiting job reports the phase `queued` with a line saying how many are ahead
of it, so a server shows a queue rather than a cut that never moves. The hour a cut is allowed
counts from its start, not from its request: the last song of a long set list waits longer than
that.

The decode lands block by block in the per-channel buffers: no interleaved copy of the whole song
is held beside them.

**A cut does not take every core.** ONNX Runtime's default is one thread per physical core, at the
same priority as the engine's render pool, and a cut sent while a set plays would then compete with
the audio for every core and the memory bus. The session asks for **half the logical cores, and never
more than all but five** — the engine's four render threads and one for the rest — at least one
(`thread_count`, `src/separation/budget.rs`). `$STUDIO_ONNX_THREADS` sets the count outright. A cut on a
quiet machine is slower for it; that trade is unmeasured, as is the priority, which ONNX Runtime
offers no option to lower.

## A cut on a phone

| choice | why |
|---|---|
| half the cores, with no five kept back (`thread_count`, `src/separation/budget.rs`) | a phone has no render pool to spare, and "all but five" is one thread on six cores |
| a cut is refused up front when `os_proc_available_memory()` is below what the model and the song need, with the figures in the reason | iOS ends an app past its limit with no word; a refusal says what to close |
| `$STUDIO_SEPARATION_MIN_MEMORY_MB` raises that floor | the only way to see the refusal on a device with memory to spare |
| `k7_supervisor_cuts_in_flight()` counts cuts queued or running, and `Busy.swift` holds the app behind another while it is above zero | a cut is minutes of work and the app would suspend as soon as its audio idled |
| no increased-memory-limit entitlement | the capability is not offered to a free Personal Team, which is how the app is installed; it is a one-line addition to the app's entitlements for a paid team |

## ONNX Runtime is loaded, never linked

The library is looked for at the first separation — at `$STUDIO_ONNXRUNTIME_DIR`, then beside the
executable or in a `resources/onnxruntime` near it — and loaded then. Linked, a machine missing it could
not start the supervisor at all, engine and tunnel included, over one feature; loaded, only the separate
job says what is missing. The build lays it beside the binary from Microsoft's NuGet packages, hashes
pinned (`apps/k7-daw-engine/scripts/onnxRuntime.mjs`), and the staging
(`apps/k7-daw-engine/scripts/docs/buildEngine.md`) keeps it there.

**iOS is the one exception.** Microsoft's iOS package is a static xcframework, and a phone has no library
to find, so `k7-embed` builds the supervisor with the `dynamic-ort` feature off and the app links the
xcframework, taken from the NuGet package macOS takes (`runtimes/ios`). CoreML is left out on iOS: it
cannot compile HT-Demucs, so a cut is CPU.

Windows takes the DirectML package, whose last release is 1.24; macOS arm64 and Linux take 1.30,
and macOS 1.30 needs macOS 14. A platform ONNX Runtime ships no binary for builds without
separation. CoreML is offered and cannot compile HT-Demucs; the job falls back to the CPU and says
why ([stemSplit.md](stemSplit.md) is the page on providers and on the model).

## A crash says why

An app restarts a supervisor that died, so what a crash leaves is all there is to read. A panic writes
`fatal: <the panic>` to `.studio.log` (`run::log_panics`), and on Windows `crash-supervisor-<pid>.dmp`
lands beside it, written once by the first of a panic, the C runtime's `abort()` and an unhandled
exception (`src/win/crash.rs`, [windows.md](windows.md)). Open the dump in Visual Studio or WinDbg to read the stack.

## Two diagnostics

`k7-supervisor --probe <address>` asks a studio's registry the hello a connect would, and prints
why it did not answer — a refused certificate and a missing host read differently.
`k7-supervisor --separate <mix> <model> <outDir> [providers]` runs one stem separation on this
machine and prints its progress. Neither touches the state directory.

## What `.studio.log` says, and when

The merged log is where a machine's trouble is read after the fact, so what fails says so there —
not only on an app's status light, which is gone when the app is:

- **At start:** the engine id and the whole presence a heartbeat will carry (hostname, addresses),
  then the performance figures with the rating. When a registry refuses a heartbeat, the reason
  names a field of those two lines.
- **`[registry]`, per registry:** `listed at …` on the first accepted heartbeat; the FIRST failure,
  with the registry's own answer (`HTTP 400 {"error":"heartbeat refused — …"}`, or `no answer
  (…)`); again whenever the reason changes; a reminder once a minute while it keeps failing; and
  `accepted again after N failure(s)` when it recovers. A registry added, removed or refused on add,
  and a deregister that did not land. No registry configured at all is said once.
- **`[host]`:** an engine that did not start, and why.
- **`[tunnel]`:** why a tunnel failed or closed (the socket's error, or the close code), once per
  distinct reason rather than on every redial; a job the machine refused or that threw.

A heartbeat cannot fail silently: a registry refusing every heartbeat, unread, is an engine listed
and then unselectable with nothing in its log. On Windows the hostname is the wide DNS
name, as UTF-8, since `gethostname` answers in the ANSI code page and `PC-FRÉDÉRIC` there is not UTF-8. The registry's half is
`apps/k7-daw-backend/docs/engine/heartbeat.md`.

## The performance rating is compiled

The single-core figure of the rating is a biquad over a fixed budget, compiled rather than
JIT-compiled, so it reads several times higher than a JIT-measured one on the same machine. The
reference in `performance.rs` was measured under a JIT; ratings are
comparable between machines running this supervisor, not with a figure measured another way.
