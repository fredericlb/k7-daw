# DSP as data

Some modules do not host a plugin: their sound processing is described in a file inside the module
folder, which you can open and change.

| Module type | File | Language | Runs as |
|---|---|---|---|
| `dsp` | `dsp.k7x` | k7x data, describing a graph of DSP blocks | The engine's DSP Graph (audio) or MIDI Graph (notes) processor |

It is edited in the [module builder](./45-module-builder.md)'s **dsp** tab. A module of the older type
`jsdsp`, with a `jsdsp.ts`, still plays the `jsdsp.json` beside it, but k7 no longer builds the TypeScript:
write its graph as a `dsp.k7x` and change its type to `dsp`.

## DSP as data

A `dsp.k7x` is read, never run. It **describes** a graph of blocks (filters, gains, delays, envelopes)
and the engine runs that graph natively. There is no per-sample code and no loop: what the graph needs
is spelled out, and a file that does not read says which line is wrong.

### A first module

`module.k7x`:

```js
plugin({ type: "dsp", name: "Trim" })
params({ Trim: 0 })
```

`dsp.k7x` beside it:

```js
params({ Trim: { min: -12, max: 12, default: 0 } })
graph([output(amul(input(0), fromdb("Trim")))])
```

`params` declares what a panel can bind, with its range and default. `labels: ["up", "down"]` in a
parameter turns it into a choice of named settings (a dropdown). A name in quotes is a parameter, a
node named with `.as("x")` above it, a wire from `wires("fb")`, or `"sr"`, the sample rate. A call
inside a call is a node whose result is read once.

### The blocks

| Group | Blocks |
|---|---|
| Control values (computed once per block) | `todb`, `fromdb`, `lerp`, `linmap`, `geolerp`, `clamp`, `select`, `add`, `sub`, `mul`, `glide` |
| Analysis | `envelope(attackMs, releaseMs)` |
| Audio, per sample | `input(port)`, `output(x)`, `amul`, `aadd`, `saturate(x, drive?)`, `delay(x, timeMs, maxMs)`, `allpass(x, baseHz, octaves?)`, `lfo(shape, rateHz, spread?)` |
| Audio effects | `biquad(mode, freq, q, gainDb?)` with modes `lowpass`, `highpass`, `bandpass`, `notch`, `peak`, `lowshelf`, `highshelf`; `gain`; `mix(wet)` |
| MIDI | `hold`, `transpose`, `velocity`, `channel`, `strike(stepBeats, gate)`, `pattern`, `accent`, `chance`, `decay` |
| Shape of the file | `.as("name")` names a node, `.into("wire")` makes a `delay` or `allpass` write a feedback wire, `.repeat(n)` puts the same node `n` times in series |

A module whose type folder is `transformers` (MIDI in, MIDI out) builds a MIDI graph; any other
builds an audio graph. A loop, a function or a computed number is not in the vocabulary.

### Examples in k7-core

Read these modules' `dsp.k7x` to see the blocks in use: `audio/auto-wah`, `audio/flanger`,
`audio/zz_utility/gain`, `trim`, `fade` and `crossfade`, the `classic` engines of `chorus`,
`phaser` and `tremolo`, the `one-knob` engines of `chorus`, `phaser`, `lowpass` and `highpass`,
`audio/eq/engines/parametric`, `audio/filter/engines/sweep`, `audio/delay/engines/echo`,
`audio/vibrato/engines/wobble`, `transformers/note-repeat`, `transformers/transpose/engines/pitch-shift`
and `cold/normalize`.

To start your own, create a module (`module new audio/mytrim`), set its `plugin` to
`{ type: "dsp", name: "..." }`, and write a `dsp.k7x` beside it, starting from one of these.
