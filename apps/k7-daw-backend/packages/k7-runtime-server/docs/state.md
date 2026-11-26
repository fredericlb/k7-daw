# What the engine says unasked

The state frame, a finished take, a plugin's parameters and an engine warning are pushed by the engine
without a request. They live in `src/server/state.rs` and `state_frame.rs`, over the ABI's state area
(`engine/capi/k7_engine_state.h`, `src/engine/state.rs`). The wire is every installed client's, byte
for byte (`protocol.md`).

## The timer stays in the engine

- **The engine's state timer is a JUCE timer on the message thread** and it owns the cadence: the 50 ms tick,
  the idle slowdown to 4 Hz, what a tick drains (waveform taps, recorders) and pumps (watched parameters).
  Rust does not run a second clock: it would have to ask the message thread for every number the tick reads
  in one breath, and a frame cut across two reads is a frame that never existed.
- **One call installs the timer and the three event sinks**, once per run, from a job on the message thread
  (`Mt::start_state`). The sinks are never freed, as the warning sink is not (`abi.md`).
- **A frame is flattened into arrays the engine lends for the call of the sink.** Nothing in it is owned by
  Rust; the sink copies what it keeps. A frame is built into JSON only when a socket is bound to its session,
  so an idle engine costs the arrays and nothing else.

## Who is sent what

- **The state frame is the one reply addressed by socket**, to the sockets whose first real message named its
  session (`Ctx::state_targets`), and is droppable: a socket more than 64 KiB behind skips it, since the next
  frame supersedes it. The subscription is bound once; a message naming another project does not rebind it.
- **A socket's first binding rewinds the recorders' pictures** (`rewind_recorder_envelopes`), because a new
  subscriber holds none of the take being recorded and the next frame must carry the whole of it.
- **A take, a plugin's parameters and a warning are replies to a project**, not to a socket: every trusted
  socket receives them stamped with the session, as `broadcast_to` does, and each client filters by
  `forSession`. A warning that names no project is a machine-wide `broadcast`, unstamped.
- **`plugin-params` means "persist this" and `plugin-params-live` is its own type**, so a consumer never
  rewrites the project at 20 Hz by forgetting a check; `watch-plugin-params` turns the live stream on and off,
  and the watch set is the engine's, so a second watcher is a no-op.

## Held to a fixture

- **The frame's bytes are `fixtures/state_frame.txt`**: each line is an engine snapshot as JSON and the exact
  frame the clients already read for it, frozen; `tests/all/server_state.rs` holds the frame to it through the
  fake engine. The cases include `-0.0` from a rounded level, a float
  master level printed whole, a 64-bit `from`, and escaped and non-ASCII names.
- **The rounding is the wire's**: a peak to four decimals, a level to a hundredth of a dB, computed on the
  float widened to a double; the playhead, needles and the master level keep their full precision.
- **The real timer is checked once on the real host** (`apps/k7-daw-engine/k7-runtime/tests/StateApi_test.cpp`, slow): a frame arrives on
  the message thread with a project's tracks. The take and parameter sinks are installed there but fired only
  in the fake, since a real recording or a plugin editor is not something a test of the door can make.

## What is not here

`get-plugin-params` (the full set with its ranges, deduplicated by the pending queries) belongs to the plugin
verbs; its engine call, `plugin_param_infos`, is in the state area already and answered by the fake.
The engine's warning sink is the process's, not the host's, so tests that say a warning through the fake
claim it again first, since another server in the same process may have taken it.
