# A module's DSP as data

A module describes its DSP as a graph of blocks in a `dsp.k7x` beside its `module.k7x`. The file is data, never
run: the song crate decodes it into a JSON graph that the engine's graph processors run natively.

| Engine processor | File | Runs |
|---|---|---|
| DSP Graph (`studio_dspgraph`) | `engine/k7-runtime/engine/GraphDspPlugin.cpp` | audio graphs |
| MIDI Graph (`studio_midigraph`) | `engine/k7-runtime/engine/MidiGraphPlugin.cpp` | MIDI graphs |
| The graph model both share | `engine/k7-runtime/engine/DspGraph.h` | |

**Why a graph and not code.** A module's DSP must be editable by a user without a C++
build, yet it must run in the engine's audio callback with no garbage collector and no
interpreter. Describing the graph as data and running fixed native blocks gives both: the
author gets composition, the engine gets a static graph, and nothing evaluates the file.

## The vocabulary

The words a `dsp.k7x` may use (`params`, `wires`, `graph`, and the nodes: control rate, audio rate, on the buffer, on
the notes) are the table of `docs/dsp-literal-plan.md`. The engine's node types are `NODE_TYPES` in
`packages/song/rust/src/jsdsp/validate.rs`, which a test compares with `DspGraph.cpp`.

## How a module reaches the engine

A module opts in with `plugin({ type: "dsp", name })` and a `dsp.k7x` beside its module file
(fixed name: `DSP_TYPE` and `DSP_FILE` in `packages/engine-protocol/build.ts`). The file is k7x data,
not code: `params({…})`, `wires(…)` and `graph([…])`.

1. The loader (`packages/song/src/loader.ts`, the song crate's module reader) finds the source for each
   module folder; `dsp` is one of the folder's own file names, never a node.
2. Compiling the project decodes the file on every compile, with no cache: the song crate's
   `dsp_literal::decode` in every Rust host, and the same function in the WebAssembly song module. A file that does not decode plays a pass-through graph and is one error on the module, `line N: …`.
3. The graph travels inline on the plugin document. `dsp` maps to the engine plugin "DSP Graph",
   or "MIDI Graph" when the module emits MIDI.
4. Offline passes (`packages/audio-passes/src/processChain.ts`) read the same graph through the compile.

Nothing evaluates the file, so nothing needs a sandbox.

`packages/song/rust/tests/all/dsp_vectors.json` holds, for every module, the graph its `dsp.k7x` decodes to, frozen;
the song crate's `dsp_literal` test holds each `dsp.k7x` to it byte for byte, so a change to a module's DSP edits the
golden by hand. A `jsdsp` module of an account (type `jsdsp`, a `jsdsp.ts`) is never built: its committed
`jsdsp.json` plays.

## Adding a block

1. Implement the node in the engine's graph processor and its JSON reader (C++).
2. Add the word to the decoder (`packages/song/rust/src/dsp_literal/`) and its node type to `NODE_TYPES`.
3. Add a case to `dsp_literal.rs` and, in a module, the node to its `dsp.k7x`.
4. Verify by running the engine: the decoder cannot check that the C++ reader accepts the node.
