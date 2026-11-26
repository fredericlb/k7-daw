# The engine's C ABI

The Rust server reaches the C++ Tracktion host through one C interface, and through nothing else:
`apps/k7-daw-engine/k7-runtime/engine/capi/k7_engine.h` and one `k7_engine_<area>.h` per area. The
plan this belongs to is `docs/rust-engine-server-plan.md`.

## One symbol, a table of area tables

The engine exports one function, `k7_engine_api()`, which returns a table whose fields are the
area tables: core, project, transport, state, plugins, machine, render, cli. Each area is the
functions one milestone of the plan needed, and an area an engine lacks is `NULL`.

- **One symbol, not one per function.** A static library linked into three hosts (the desktop
  CLI, Android's `.so`, iOS's framework) is the place where a missing or renamed symbol is found
  last, at link time on the one platform nobody built. A table found at run time is checked once,
  on every platform, by the same code.
- **One area, one header, one `.cpp`, one Rust module.** An area's C header, its C++ implementation
  (`engine/capi/<Area>Api.cpp`) and its safe Rust module (`src/engine/<area>.rs`, with its fake in
  `fake.rs`) are written together by its milestone and touch no other area's files. That is what lets
  the plan's milestones run in parallel without conflicting in the ABI.
- **`ffi.rs` mirrors the headers field for field**, and `tests/all/engine_abi.rs` holds every table's
  field names to its header, in order. A header changed without its mirror fails a test, not a call.

## Append only, checked by `abi` and `size`

The top table starts with `abi` (`K7_ENGINE_ABI`) and `size` (`sizeof(K7EngineApi)` as the engine
was built). The server refuses a table of another `abi`, a table shorter than its own, and a table
without a core area (`AbiError`).

- **A function is appended, never inserted or changed.** Appending keeps `abi`; changing what a
  function takes or means raises it. The size check is what makes appending safe: a server newer
  than its engine refuses to read past the table's end instead of calling garbage.
- **The check runs once**, when the table is wrapped (`Api::new`), so no call site carries it.

## Nothing crosses with its ownership

A `K7Str` (UTF-8 bytes and a length, no NUL) or a struct handed across is borrowed for the duration
of the call, in either direction, and the receiver copies what it keeps. A string coming back is
handed to a `K7StrEach` callback with the caller's context rather than returned.

- **No allocator is shared.** Rust's and C++'s heaps never free each other's memory, so no function
  has a matching `free`, and a leak or a double free cannot be written by forgetting one.
- **No exception and no panic crosses.** Every C++ body runs inside `CApi.h`'s guard, which says the
  failure on stderr and returns the fallback its header documents; every Rust callback runs under
  `catch_unwind`.
- **The warning sink is the one long-lived pointer**: set before the server accepts, cleared with
  `NULL` after it stops, called from any thread. It is set once per run and never freed.

## The message thread: `Mt` and posted jobs

JUCE and Tracktion want most calls on their message thread, and that thread is the one that called
`host_create` and then `run` (Android's Looper thread, iOS's run loop). So the header splits each
area's functions into three kinds: the process's setup, any thread, and the message thread only.

- **A message-thread call needs an `Mt`.** The server reaches the message thread only by posting a
  job (`Engine::post`, or `Engine::call` for an answer); the job is handed an `Mt`, which is not
  `Send` and lives only for the job, and only `Mt` has the message-thread functions. Calling one from
  the wrong thread is a type error, not a JUCE assertion on one platform.
- **Any-thread answers read atomics or a lock of the host's own** (the audio timing, the message
  thread's age, the session ids), so a socket task never waits on the message thread for them, and
  the watchdog's age stays readable while that thread is wedged.
- **An `Engine` may outlive its host.** It holds the host behind a lock; once `Host` is dropped, a
  post returns false and an answer is `None`, never a call into freed memory.

## `drop`, for a job that never runs

`post` takes two functions and a context: `run` on the message thread, or `drop` once when the job
can no longer run (the host destroyed with the job still queued). Exactly one of them is called.

- **The context is a boxed Rust closure**, and it holds whatever the job captured: the reply
  channel of an `Engine::call`, a socket's handle. Without `drop`, a job left in JUCE's queue at
  shutdown would leak its box, and the task awaiting its reply would wait forever instead of seeing
  the channel closed.
- **C++ calls `drop` from the destructor of what JUCE's queue holds**, so the guarantee does not
  depend on the queue being drained in order or at all.
