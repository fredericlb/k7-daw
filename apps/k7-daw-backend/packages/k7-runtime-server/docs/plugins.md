# The plugin verbs

The installed catalogue and its scan, a plugin type's parameters, the live instances of a project, their native
windows, presets and waveform taps. They live in `src/server/plugins.rs` over the ABI's plugins area
(`engine/capi/k7_engine_plugins.h`, `src/engine/plugins.rs`). The wire is the C++ server's, byte for byte
(`docs/rust-engine-server-plan.md`).

## Blocking calls and the message thread are separate kinds of function

- **The ABI marks each function BLOCKING or message-thread.** A scan and a probe load binaries for seconds and
  never run on the message thread; the engine hands the creation to that thread itself and waits. Everything
  else (instances, presets, windows, taps, one parameter) is called from a posted job, through `Mt`.
- **A blocking call runs on a blocking task**, not a socket's task: a handler never blocks. The task may be
  abandoned at stop; the server cannot join a plugin that is still loading.

## One scan, said in a fixed order

- **One scan at a time for the whole engine**: a second waits at a lock, so a rescan and the start-up scan never
  run two children at once. Progress lives on one board every socket may ask (`plugin-scan`).
- **The end is said after the catalogue**, a scan that failed included, so a client re-reading on the end finds the
  new list already sent, and a gate waiting on the end never waits for nothing.
- **A load starting is said at once, anything else at most four times a second**: a warm scan stats hundreds of
  files in a second.
- **The scan runs at every start** (`ServeConfig::startup_scan`, on in `from_env`, off in a test), because a plugin
  installed since the last run would otherwise be unknown until someone rescanned.

## Four probes at once

A parameter probe and a window load take a slot or are refused with an error; they are never queued, so a client
that loops cannot pile up threads. The slot is a guard, returned however the load ends.

## Pending queries

A read-only query whose answer is a broadcast joins an identical one still waiting on the message thread. A key
leaves when its job starts, not when it ends, so a query asked while one runs is queued again. A ticket whose job
never ran frees its key on drop. The set is the context's, for the machine's queries to share.

## Held to the C++

`fixtures/plugin_replies.txt` is the C++'s own output for each reply (`tests/PluginReplies_test.cpp`, whose builders
are copies of `ServePlugins.cpp`'s, so it proves the fixture, not the file). `tests/PluginsApi_test.cpp` calls the
area on a real host: the cache, a type the catalogue does not list, and the message-thread calls with nothing
loaded. A real scan is not run there: it spawns the binary's own children for every plugin of the machine.
