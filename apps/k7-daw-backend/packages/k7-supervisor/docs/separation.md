# Stem separation in the supervisor

A cut splits a mix into stems on the engine's machine: HT-Demucs run window by window on ONNX Runtime, the
overlapping windows folded back, four 24-bit WAVs written. The product rules (why a job, why 24 bits, why a
module) are `apps/k7-daw-engine/k7-supervisor/docs/stemSplit.md`; this page is what the Rust crate chose.

## Choices

| choice | why |
|---|---|
| a cut is a thread of its own, not a task of the core and not a tokio task | it burns every core for minutes and blocks on the runtime; the core's watch and the host lock's refresh never wait on it |
| the module `separation` is a library API (`run`, `terminate_all`, `cuts_in_flight`) with one part that ends the cuts at shutdown | `--separate` runs with no core at all, and the tunnel's `separate` job calls the same `run`; a part that only owns the shutdown keeps both honest |
| a job is keyed by its WORK (user, source, model, outputs) and polled once a second by whoever asked | a second request for the same cut joins the one running rather than burning another hour of arithmetic |
| the hour counts from the cut's start, after its place in the queue | the last song of a set list waits longer than an hour in the queue and must still be cut |
| the decode and the resample are the `waveform` crate, linked directly | one decoder for the page's analysis, the transcodes and the cut, held to ffmpeg's frame counts by its own tests; no C ABI between two Rust halves |
| ONNX Runtime goes through the `ort` crate with `load-dynamic`, loaded at the first cut | a supervisor missing the library still starts, holds the tunnel and runs the engine; only the job says what is missing |
| the library is looked for in `$STUDIO_ONNXRUNTIME_DIR`, then beside the executable, in `onnxruntime/`, `resources/onnxruntime/` and the macOS bundle's `Resources` | the places a build, a package and the desktop app put it |
| `api-24` | the oldest runtime shipped is DirectML's 1.24.4; `ort` demands at least the API level it was built for, and the 1.30 runtimes serve it |
| the feature `dynamic-ort` is on by default; off, `ort` links the runtime its build is pointed at | iOS has no library to find: its package is a static library, linked by the build that sets `ORT_LIB_LOCATION` |
| a provider that fails to register is an error, never a quiet CPU | the first provider that really builds wins, and the line `execution provider: …` says which and what gave way |
| CoreML is a Cargo feature on Apple targets and is refused on iOS; DirectML is a feature on Windows | CoreML cannot compile HT-Demucs on a phone; DirectML needs the memory pattern off and sequential execution |
| a window that fails is retried once on a fresh session, then on the CPU, and the provider is remembered as lost for the process | a lost device kills the session rather than the GPU |
| a phone refuses a cut it cannot hold before decoding | iOS kills the app past its limit with no word |
| ONNX Runtime's telemetry is off | the supervisor sends nothing nobody asked for |
| Android builds none of it | ONNX Runtime ships no binary there; every open says so |

## What the tests hold

The arithmetic, the budget, the slots and the decode run in every checkout. A cut on a stand-in model written by
hand (`tests/all/onnx_model.rs`, stems that are the mix and its negation) needs ONNX Runtime, which is a download:
those tests pass with a line saying they skipped when `$STUDIO_ONNXRUNTIME_DIR` names no library.
