# Plan: a module's DSP written as k7x data

A module's DSP graph is a `dsp.k7x` beside its `module.k7x`: k7x data, parsed and never run,
decoded by the `song` crate into exactly the graph JSON `@studio/dsp`'s builder prints from a
`jsdsp.ts`. Nothing evaluates it, so nothing needs a sandbox: QuickJS's jsdsp context, oxc,
`k7-jsdsp`, `dsp-exec` and the node jail are gone. The decoder is `packages/song/rust/src/dsp_literal/`;
every module of k7-core, the demos and the e2e fixtures is written as data beside its `module.k7x`, and
`packages/song/rust/tests/all/dsp_vectors.json`, which `makeVectors.mts` prints from the builder, holds each
`dsp.k7x` to its graph byte for byte. The type is `dsp`, a `dsp.k7x` is one of a module folder's own files
(never a node), and the compile decodes it on every build.

Why it fits: the 23 modules are 242 lines of code, 5 to 25 each, and print 0.2 to 3 KB of graph.
Two hold a loop (the phaser's three all-pass stages, the EQ's five bands) and none computes a value
a literal cannot hold. A program is the wrong tool for a description this small, and the cost of
running one was a sandbox with four walls.

## The vocabulary

```js
// k7-core/modules/audio/phaser/engines/classic/dsp.k7x
params({
  Depth: { min: 0, max: 10, default: 5 },
  Rate: { min: 0, max: 10, default: 0.4 },
  Feedback: { min: 0, max: 1, default: 0.7 },
})
wires("fb")
graph([
  input().as("dry"),
  amul(aadd(amul(lfo("triangle", "Rate"), 0.5), 0.5), "Depth").as("octaves"),
  aadd("dry", amul("fb", "Feedback")).as("x"),
  allpass("x", 100, "octaves").as("x").repeat(3),
  allpass("x", 100, "octaves").into("fb"),
  output(aadd("dry", "fb")),
])
```

| written | means | why this way |
|---|---|---|
| `params({ Name: { min, max, default, labels } })` | the params, in file order | the builder's `g.param` as an object, the form k7x already gives names the project chose; a spec may also be a chain, `min(0).max(1).default(0.5)` |
| `wires("fb")` or `wires("a", "b")` | names read before a node writes them | a feedback loop is the one place order is not enough: declaring the wire is what makes reading it ahead legal |
| `graph([ … ])` | the nodes, in order | the engine runs them in declared order, so order is meaning and the graph is a list, never a map |
| `todb("Resonance")`, `lerp(-60, -15, "Sensitivity")` | a node: one builder word, its inputs as arguments | a call is an entry in k7x (`language.md`), so a node is the map `{ todb: "Resonance" }`; the words are the builder's methods |
| a call as an argument, `todb(envelope("Attack", "Release"))` | the inner node, emitted first | what a TypeScript author nests reads the same; no name is needed for a value read once |
| `.as("x")` | names the node | a name is a string, resolved against the params, the named nodes, the wires and `sr`, which share one namespace; a collision is refused |
| `"x"` as an input | the node, param or wire called `x`, declared ABOVE | data has no forward reference: a name further down is refused, which is also what refuses a cycle that no wire breaks |
| `.into("fb")` | this `allpass` or `delay` writes the wire `fb` | the builder's `into` argument; a wire has exactly one writer and must be written |
| `.repeat(n)` | the item, `n` times in series, rebinding its own `as` name | the one loop shape the modules hold; 1 to 64, and only the repeated node is named inside it |
| `"sr"` | the sample rate | the builder's `g.sr`; a Nyquist limit needs it and no literal can say it |

Every word, and what it records:

| family | words | the JSON |
|---|---|---|
| control, once per block | `envelope`, `todb`, `fromdb`, `lerp`, `linmap`, `geolerp`, `clamp`, `select`, `add`, `sub`, `mul`, `glide` | `{ type, in, out }` |
| on the buffer | `biquad(mode, freq, q[, gainDb])`, `gain`, `mix` | `{ type, in[, mode] }`, no value to read |
| per sample | `input([port])`, `lfo(shape, rate[, spread])`, `allpass(x, hz[, octaves])`, `delay(x, time, maxMs)`, `amul`, `aadd`, `saturate(x[, drive])`, `output(x)` | the engine's `ain`, `alfo`, `aallpass`, `adelay`, `amul`, `aadd`, `atanh`, `aout`, each with the builder's own key order |
| on the notes | `hold()`, `strike(step, gate)`, `pattern(mode, length, density, rotate)`, `accent(count, low, opposite, keep)`, `chance(p)`, `decay(d)`, `transpose(st)`, `velocity(scale)`, `channel(ch)` | `{ type, in }` on the one stream, which is implicit, as the builder's `g.midi()` was |

The modules use every word but `velocity`, `channel`, the `notch` biquad and the `saw` and
`square` LFOs; those are the builder's others, decoded all the same. The builder's chain methods
(`x.mul(…)`, `mix.from(1)`) have no spelling: a nested call says the same and keeps one form.

### The rules that make the graph byte-identical

| rule | what forces it |
|---|---|
| a node's inputs are emitted left to right, depth first, then the node itself | JavaScript evaluates a call's arguments in that order, and each builder call records its node as it returns |
| ids are `n0`, `n1`… in emission order; a sink, a note node and an `into` take none | the builder's counter moves only for a node that returns a value it names |
| each family writes its keys in the builder's order: `{ type, out, in, k }` for `amul`, `{ type, k, out, shape }` for `lfo`, `{ type, in, k, out, maxMs }` for `delay` | the JSON is compared as bytes, and `JSON.stringify` keeps insertion order |
| a second wire goes in `in`, a scalar in `k`; a fixed delay time is `k`, a modulating one is in `in` | the builder's two rates, which the decoder tracks in place of tsc's types and refuses when crossed |
| `input(0)` writes no `port`; `saturate(x)` writes `k: [1]` | the builder's defaults |
| a param spec is written `name, min, max, default, labels`, whatever order the file has | every module writes that order; a k7x map keeps file order, a spec does not need it |
| numbers print as JavaScript prints them (`js_number`), two-space indents, one item per line, a trailing newline | `JSON.stringify(graph, null, 2)` and the newline the jail adds |

`packages/song/rust/tests/all/dsp_literal.rs` holds every `dsp.k7x`, 23 of them, to the vector the builder printed
from its `jsdsp.ts`, which covers every word the modules use.

### Computed values

None of the 23 modules needs one. What looks computed is already a node (`sub(1, "Mix")`,
`todb(4)`), and the EQ's ``${name} Freq`` is a name written out. A literal says every number
the modules hold; the decoder refuses an infinite one, and keeps the builder's one check on
literals, a descending `geolerp`.

## What an author loses, and the answer

| lost | the answer |
|---|---|
| a `for` loop | `repeat` for stages in series, the only loop shape the modules hold; anything else is written out, as the EQ's five bands are (15 params and 5 lines) |
| a helper (`wah()` called twice) | the module is the unit of reuse: a module extends another, or is copied. DSP that wants functions and loops is Faust's (`plugin.type: faust`), which the engine already compiles |
| a computed value | none is needed today; a constant is written as its value, and arithmetic on a signal is a node |
| tsc's Scalar and Audio types | the decoder checks rates, names and arity, and names the file, the line and the key path: `graph[1].output[0].amul[0]: "dyr" names nothing` |
| completion in a TypeScript editor | the editor's own completion, from the vocabulary above, once the file has a kind |

What an author gains: the UI and live-sync can change a literal at its key path (`language.md`),
comments survive every edit, and a module reads the same on every host with no build step.

## The migration

| step | what | state |
|---|---|---|
| 1 | every module written as data: 21 in k7-core, the demos' and the e2e fixtures' crossfade; a test holding each `dsp.k7x` to the builder's graph; k7-core's version raised | done: version 4, which requires library level 2 |
| 2 | the compiler reads `dsp.k7x`: `plugin.type: dsp` (decision 1901), decoded on every compile with no cache, its error one finding on the module; the TypeScript compiler in node, which has no decoder, reads the `jsdsp.json` beside it, held equal by `k7coreJsDsp.test.ts`, and the QuickJS bundle asks the host's decoder (`packages/song/docs/graphModel.md`) | done |
| 3 | the file service and the editor: a file kind for `dsp.k7x` in `song`'s views, with the vocabulary as completion; the module builder's template | the file is a module folder's own and no schema reads it; completion and the `FileKind` are open |
| 4 | the browser: nothing to add. The decoder is in the `song` crate the browser's module is built from; a live DSP preview calls it | 71 KB of WebAssembly once something calls it, 24 KB gzipped |
| 5 | the accounts' own `jsdsp.ts` modules: a printer from a committed `jsdsp.json` to `dsp.k7x`, names `n0`…, byte-identical by construction, run over each library once (decision 1904) | open: until it runs, such a module plays its committed `jsdsp.json` and is never rebuilt |
| 6 | the removals: `k7-jsdsp` and its bundle, oxc, `dsp-exec.c` and the `dsp` user in the image, `jsDspJail.ts` and its tests, the build path of `jsDspGraph.ts`, `k7-service`'s jsdsp host, the build protocol (`jsdspBuilds`, `jsdspFailed`, `jsdspStale`) | done. Kept one library version: every `jsdsp.ts` and `jsdsp.json` of k7-core, the demos and the fixtures (the vectors' references and node's graphs), and `packages/dsp`, which go with the TypeScript compiler |

### How this finishes step 5 of the port

`docs/rust-wasm-port-plan.md`'s step 5 removes QuickJS from the hosts "but jsdsp's sandbox, which
stays until jsdsp builds in the browser". With the DSP as data there is no sandbox to keep and no
browser build to wait for: step 6 above deletes the exception, and step 5's gate becomes no
QuickJS runtime on any host. The phones lose `rquickjs-sys`'s bindgen for jsdsp at once; the other
QuickJS users (`k7-js`, `k7-compile`) go by the port's own steps.

## Measured against the Rhai prototype

The Rhai figures are the Rhai study's (branch `rhai-study`), on the same machine. Ours: release,
`opt-level = 3`, median of 2,000 decodes of the file's text, parse included; WebAssembly in
node 26 with the Rhai study's profile (`opt-level = "s"`, LTO, `panic = "abort"`, stripped), from
a scratch crate that exports the decoder, against the same crate exporting the k7x reader alone.

| | `dsp.k7x` decoder | Rhai |
|---|---|---|
| code | 786 lines of Rust (`json.rs` 65, `mod.rs` 397, `nodes.rs` 324), the whole builder | 476 lines (builder and sandbox), the three modules' words only, plus the `rhai` crate and its 20 dependencies |
| new dependencies | none: k7x's reader and `song`'s `js_number` | `rhai`, pinned |
| auto-wah, native | 27 µs | 44–49 µs engine kept, 97–164 fresh |
| phaser, native | 15 µs | not ported |
| crossfade, native | 5.8 µs | 10–13 µs kept, 48–74 fresh |
| auto-wah, WebAssembly | 32 µs | 81–149 µs kept, 184–370 fresh |
| crossfade, WebAssembly | 6.5 µs | 18–22 µs kept, 110–185 fresh |
| WebAssembly module, with the k7x reader | 186,982 bytes, 76,679 gzipped | 1,544,122 bytes, 381,612 gzipped |
| what the decoder adds to the k7x reader alone (116,323 bytes) | 70,659 bytes, 24,090 gzipped | — |
| a hostile file | refused by a limit before it runs: `repeat` past 64, 1,024 nodes, calls nested past 64, k7x's 256 levels; each in under a millisecond | needs the child, its rlimits and a counting allocator: no heap limit |

## Decisions for the user

| # | decision | recommendation |
|---|---|---|
| 1900 | a module's DSP as k7x data, rather than jsdsp in Rhai or TypeScript in QuickJS | go: the modules need no program, and data needs no sandbox on any host (approved by the user, 2026-11-10) |
| 1901 | the name: `plugin.type: dsp` with `dsp.k7x`, or `jsdsp` kept | `dsp`, with `jsdsp` still read until step 6: the module no longer holds JavaScript (approved by the user, 2026-11-10) |
| 1902 | references as plain strings in one namespace, or with a sigil (`"$x"`) | plain strings, collisions refused: a param's name is already a string everywhere else in k7x (approved by the user, 2026-11-10) |
| 1903 | `repeat` for series only, or a general form over a table (the EQ's bands, with names made from parts) | series only: a table form is a template language, and the one module that would use it reads plainly written out (approved by the user, 2026-11-10) |
| 1904 | the accounts' `jsdsp.ts` modules: converted from their `jsdsp.json` at the cut-over, or left playing a frozen graph | converted: byte-identical by construction; the `jsdsp.ts` stays beside it, unread (approved by the user, 2026-11-10) |
| 1905 | `jsdsp.json` once the compiler reads `dsp.k7x` | committed while the TypeScript compiler reads it, held equal by a test, then deleted: a decode costs microseconds, a cache costs a staleness rule (approved by the user, 2026-11-10) |
| 1906 | the limits: 64 copies per `repeat`, 1,024 nodes, 64 nested calls | these: the largest module has 19 nodes and nests 6 deep (approved by the user, 2026-11-10) |
| 1907 | DSP that needs loops, functions or computation | Faust, which the engine already compiles; no expression is ever added to k7x (approved by the user, 2026-11-10) |
| 1908 | the browser decodes `dsp.k7x` for a live preview | later, when the preview exists: the decoder is already in its crate (approved by the user, 2026-11-10) |
