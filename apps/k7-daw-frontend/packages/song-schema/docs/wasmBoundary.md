# The browser's Rust: crates, the C ABI, the generated types

The Rust and WASM port (`docs/rust-wasm-port-plan.md`) replaces the TypeScript the browser and the
backend share by one Rust source. This page fixes how that Rust is cut into crates, how the browser
calls it, and how its types reach TypeScript. The curve arithmetic and the timecode are the first
code on it; every later piece (the compiler, the verbs) follows the same rules.

Paths below are relative to the package named in the row: `@studio/k7-wasm` is `apps/k7-daw-frontend/packages/k7-wasm`,
the song-schema crate is `apps/k7-daw-wasm/song-schema`.

## The crates

| choice | why |
|---|---|
| a library crate per domain package, in that package's `rust/`, named after it: `song-schema` in `apps/k7-daw-wasm/song-schema` | the Rust sits beside the TypeScript it replaces and the docs it is described in; the name says which package's vocabulary it is |
| a library crate is `rlib` only and holds no FFI: no `extern "C"`, no `#[unsafe(no_mangle)]`, no `thread_local!` buffers, no `serde_json` | the backend links the same crate natively; a C ABI inside it would be a second API to keep and a symbol clash the day two crates export one |
| ONE browser facade, `@studio/k7-wasm`, crate `k7-wasm` in its `rust/`, `cdylib` + `rlib` | a page loads one module, one memory and one allocator; a module per crate would ship serde and the allocator once per crate |
| the facade re-exports nothing as Rust: it holds the ABI, the operations' dispatch, the handle table and the error type | the types belong to the library crates, where the native backend reads them too |
| the dispatch (`apps/k7-daw-wasm/k7-wasm/src/ops.rs`) is plain Rust, `Session::call(bytes, out) -> Result<(), WasmError>`, and `apps/k7-daw-wasm/k7-wasm/src/abi.rs` is the only file with `extern "C"` | a host with no WebAssembly (NodeMobile) can install the same dispatch natively, as the k7x reader is installed (`apps/k7-daw-frontend/packages/k7x/docs/rust.md`) |
| each crate is its own Cargo workspace with its own `Cargo.lock`, the facade's lock being the one the module is built from | it is how `apps/k7-daw-wasm/k7x` and `apps/k7-daw-backend/packages/waveform` already are; `lint:locks` holds each lock to its manifests |
| `apps/k7-daw-wasm/k7x` and `apps/k7-daw-backend/packages/waveform` stay separate modules | they already serve hosts the facade does not (the iOS addon) and answer under parity rules of their own |
| the verbs' module (the `song` feature) is the one build with imports: module `k7`, `k7_fs_*` for the files (`node:fs`, so the Vfs in the Worker) and `k7_new_id`, declared in `abi.rs` | `song-edit` reads and writes files while it runs, and the Worker's Vfs is the only state; the curves' module stays import-free (`apps/k7-daw-frontend/packages/k7-wasm/docs/edit.md`) |

## The C ABI

Every export is `extern "C"` and named `k7_*`. JSON text crosses it, UTF-8 in and UTF-8 out, except
for one hot call whose answer is two numbers.

| export | does |
|---|---|
| `k7_input(len: usize) -> *mut u8` | resizes the input buffer to `len` bytes and returns it |
| `k7_call(len: usize) -> u32` | runs the request in the first `len` bytes of the input; `0` OK (the answer's JSON), `1` ERROR (a `WasmError`'s JSON) |
| `k7_output_ptr() -> *const u8`, `k7_output_len() -> usize` | where the last answer is |
| `k7_curve_playhead(handle: u32, beat: f64) -> u32` | `0` OK (x, y in the scalars), `2` NONE (an envelope, a negative beat), `3` UNKNOWN_HANDLE |
| `k7_scalars_ptr() -> *const f64` | a fixed `[f64; 4]`, the numeric answer of a hot call |

| choice | why |
|---|---|
| a request is `{"op": "<family>.<verb>", "args": {…}}`, one serde enum adjacently tagged (`tag = "op"`, `content = "args"`) | one parse for the dispatch and the arguments, and ts-rs gives TypeScript the request's type, so a wrong argument is a compile error in TS |
| an answer is the operation's value itself; an error is `{"code", "message"}` with the status saying which | no `{ok}` wrapper to unwrap on every call; the status costs nothing |
| error codes: `bad_request` (the JSON does not decode, an unknown op, an argument out of range), `bad_timecode`, `bad_curve` (a spec with a non-finite number), `unknown_handle`, plus `panic` and `unavailable` raised by the TS side | each is something a caller does differently: a `bad_timecode` message is the one `timecode.ts` throws, word for word, since the curve editor shows it |
| Rust owns every buffer; JS never frees, never allocates in the module's memory, and never holds a pointer across a call | one owner, no leak by a forgotten free; the buffers are reused and keep their capacity |
| **any call may grow the memory, which detaches every view of `memory.buffer`**: a view is made after the last call that may allocate (`k7_input` included) and dropped before the next call | a view made before a growth reads zeros or throws, and it reads right on every small test, so the bug shows only on a big request |
| a `Float64Array` view is allowed for a fixed-size numeric answer of a call made every frame (the playhead), and later for a bulk numeric array whose JSON would dominate (a bake's breakpoints), and then it is copied (`.slice()`) or read before any other call | anything structured goes as JSON: `JSON.parse` builds JS values faster than any binding (`apps/k7-daw-frontend/packages/k7x/docs/rust.md`); the per-frame call has no structure to build |
| no NaN or infinity in an answer: inputs that would make one are clamped or refused | serde_json writes a non-finite `f64` as `null`, which TypeScript then reads as a number |
| a panic is a bug: `panic = "abort"` traps the instance, and the TS side throws it away, makes a new one and raises `panic` | the instance's `RefCell`s may be mid-borrow; nothing in it can be trusted after a trap. Non-test code denies `clippy::unwrap_used`, `expect_used` and `panic` |
| the module is compiled synchronously where the runtime allows it, else asynchronously; `wasmReady()` resolves when it is ready and k7's frontend mounts after it settles | a browser's main thread refuses a synchronous compile past a size; nothing has a TypeScript fallback to answer meanwhile |
| serde_json is built with `float_roundtrip` | its default float parser can land one ulp off, and a saw or a square sits on a knife edge at each cycle: one ulp in a frequency flips a sample from the top of the wave to the bottom |
| no wasm-bindgen, no tsify | the repo's two modules already cross a plain C ABI with JSON, a dependency-free loader, and no JS glue to generate or version |

## The generated TypeScript

| choice | why |
|---|---|
| ts-rs derives the TS type of every value that crosses: requests, answers, errors, and the specs they carry | the frontend's types cannot drift from what the Rust decodes |
| the derive is behind a `ts` feature of each crate; the facade's `bindings` binary (`required-features = ["ts"]`) exports them all into `apps/k7-daw-frontend/packages/k7-wasm/src/types/`, one file per type, plus an `index.ts` | the native backend never compiles ts-rs; one binary is the only writer, so nothing else needs to know the folder |
| TypeScript imports them from `@studio/k7-wasm/types` and nowhere else | one door; a deep import fails by the package's `exports` |
| the WebAssembly ships checked in, base64, in `apps/k7-daw-frontend/packages/k7-wasm/src/k7Wasm.generated.ts`, with `SOURCES`, the hash of every Rust source it was built from, and `TYPES`, the hash of the types folder as written | every host loads packages as raw TypeScript, with no build step (`apps/k7-daw-frontend/packages/k7x/docs/rust.md`) |
| the checked-in module is built on one machine and is not byte-identical across platforms (the WSL and Windows builds differ by a few KB) | only the sources are hashed, so any platform's build satisfies the test, and a rebuild before a release is not required |
| a vitest fails when the sources no longer hash to `SOURCES` or the types folder to `TYPES`, and when a `path` dependency of the facade is missing from the hashed crates | a Rust change without a rebuild, or a hand edit of a generated file, cannot land; the test hashes files and needs no cargo |
| `npm run build:wasm -w @studio/k7-wasm` is the one command that writes all of it, then formats the types folder with biome | the generated files pass `npm run lint` as written |
| a Rust spec type keeps the JSON shape of its TypeScript twin in `@studio/graph-model`, and a type test holds the twin assignable to the generated type | call sites pass the value they already have; which of the two becomes the source of truth is decided when the compiler moves |

## The curve API

A curve as the file holds it (`CurveSpec`, times as timecodes) is resolved once, against a time
signature, into a `Curve`: beats parsed, points sorted, span computed. Every answer is computed from
the resolved curve.

| op or export | args → answer | called |
|---|---|---|
| `curve.preview` | `{spec, num, den, n?, playhead?}` → `CurvePreview` | per draft change by the curve editor (`apps/k7-daw-frontend/src/curveDraft.ts`): one call where it made three |
| `curve.open` | `{spec, num, den, n?}` → `{handle, preview}` | once per spec by the graph's curve card (`apps/k7-daw-frontend/packages/ui-chains-react/src/widgets/Spark.tsx`) |
| `k7_curve_playhead` | `handle, beat` → x, y or none | every frame, every card, while the song plays |
| `curve.close` | `{handle}` → `null` | when the card unmounts or its spec changes |
| `timecode.parse` | `{tc, num, den, position}` → beats | not yet by the frontend: the timecode's own API, held to `timecode.ts` |
| `timecode.format` | `{beats, num, den, position}` → timecode | idem |

`CurvePreview` is `{span: {beats, solid}, solid: XY[], faded: XY[], beats: BeatLine[], playhead: XY | null}`.

| choice | why |
|---|---|
| times cross as the file writes them, and the crate parses them | the timecode is ported into the same crate, so there is no second parser to keep in agreement inside the browser's path |
| sorted points are an invariant of the type: `Points` has a private `Vec`, built only by a stable sort by beat, and lookups use `partition_point` | no caller can hand an unsorted list to a function that assumes one, and a sample is a binary search instead of a scan |
| the caller's own point order is untouched | the editor's circles index the draft's points as written |
| a handle is a slot in a table of 256 curves with a generation, the least recently used one evicted when full; `unknown_handle` makes the TS `CurveHandle` reopen from the spec it kept and retry once | closing is an economy, never a correctness rule: a missed close, a React double mount or a trap that reset the instance all heal on the next call |
| no `previewSpan` / `previewParts` / `previewBeats` / `previewPlayhead` shims with the old signatures | each would resolve the curve again for one piece of an answer that leaves in one call |
| `n` defaults to 160 and is refused above 4096; `num` and `den` are integers of at least 1 | the default is the editor's resolution; the bounds keep an answer finite and small |
| a timecode too large to be a finite beat count is `bad_timecode` wherever it is met, in a curve's point or duration as in `timecode.parse` | one code for one cause: the editor shows the same message whichever call found it |
| `timecode.format` clamps a beat before the start to the start and refuses only a time beyond 9e15 ticks | the TypeScript answers `1:1` for a negative beat; a time no float counts exactly has no spelling |
| a picture's beat lines are at most four per sample (counting at least 160), thinned to a whole number of bars past that | a span of millions of beats (a tiny frequency, a huge duration) would otherwise allocate past what a wasm32 memory holds and trap |
| a non-positive frequency is drawn as a frequency of 1 | where the TypeScript drew infinity; the sweep below leaves it out |
| the compiler samples its curves in Rust (`apps/k7-daw-wasm/song/src/curves/bake.rs`), the editor's preview through the wasm module | the compiler samples point by point, which a call per sample would make slower |
| the curve vocabulary (`LFO_SHAPES`, `CURVE_CUTS`, `ADSR_DEFAULT_LEVEL`) is plain TS in `apps/k7-daw-frontend/packages/song-schema/src/curveVocab.ts` | ts-rs emits types, not values; the parity test holds each listed value accepted by the Rust |
