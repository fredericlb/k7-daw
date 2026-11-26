# The command line

`k7-runtime`'s `main` hands argv to `k7_server_main` (`src/entry.rs`), and `src/cli.rs` reads it: a
one-shot mode, run through the ABI's `cli` area (`engine/capi/k7_engine_cli.h`), or the port to serve on.
What each mode does is the engine's (`apps/k7-daw-engine/k7-runtime/docs/cliModes.md`).

## argv is read as it always was

Every caller of a mode — the supervisor's scan and render, `@studio/audio-analysis`'s `--decode`, the
generators' `--dump-*`, the harnesses — spells the arguments it has always spelt, so the reading keeps
their shape:

- **`--settings-dir <folder>` is taken out first, wherever it sits**, then `$K7_SETTINGS_DIR` is read
  by the engine (`apps/k7-daw-engine/k7-runtime/docs/settingsDir.md`).
- **A mode short of its arguments falls through to the port**, and the port is read as C's `atoi`
  reads it, so `k7-runtime --decode` serves on port 0 as it did; only `--render` and
  `--multi-edit-test` answer a usage line and exit 2.
- **The layout check runs before any mode**, and exits 4 for a half-rebuilt engine
  (`apps/k7-daw-engine/k7-runtime/AGENTS.md`).

## A document is checked in Rust, read in C++

`--render` and `--multi-edit-test` read a `ProjectDoc` from a file (or stdin for `-`), bare or in a
`{ "doc": … }` envelope. It goes through the same parse as a socket's `project` (`project.md`): nesting,
bounds, repeated ids, paths. The C++ side reads the validated JSON with `engine/DocReader.cpp` and no
check of its own, so there is one parser of the wire, not one per caller.

- **No roots are set in a mode**, so every path but a UNC one passes, as the engine's roots say for a
  process that never served (`apps/k7-daw-engine/k7-runtime/docs/pathRoots.md`).
- **A bad file says why on stderr and exits 2**: `cannot open`, `bad json:` or `bad project:`.

## The other modes pass through

The scan, its child, the parameter dumps, the device list and `--decode` take strings and return an exit
code; the cli area is a door to the engine's functions of the same names and nothing more.
