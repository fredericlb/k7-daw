# A module written in Faust

`plugin: {type: faust}` and a `dsp.dsp` in the module's folder. It is `jsdsp`'s brother, and this
page holds the difference between the two — plus the exact state of what is wired today, which is not
"it works".

## The state, first

**The engine compiles nothing.** `FaustPlugin` exists, loads, receives the source and *passes audio
through untouched*, saying so once on the warnings channel. Everything above it is finished: the
module loads, draws in the graph with the right colour, edits in the builder, and reaches the
compiled document and the engine. That order is deliberate — the seam is visible and exercised before
anyone decides what a compiler costs. See `engine/k7-runtime/docs/faust.md` for what is missing and the
three ways to fill it.

## The folder

```
modules/audio/moogladder/
  module.k7x        plugin: {type: faust}, params:
  dsp.dsp           the source           (FIXED name — FAUST_FILE)
  ui.k7x            the face
  presets.k7x       the named settings
  assets/           what its params name
```

The filename is a constant and not a field, exactly like `jsdsp.ts`: the type says the file is there,
and a module stays a folder you copy without rewriting anything inside it. No `.vstpreset` — a Faust
module's state *is* its parameters, so `presets.k7x` suffices, same as for a dsp module.

## What separates it from dsp: WHO compiles

That is the only difference, and it explains all the rest.

| | `dsp` | `faust` |
|---|---|---|
| the source | `dsp.k7x` | `dsp.dsp` |
| who compiles it | **here**, the song crate decodes the data (`graphModel.md`) | **the engine**, in the plugin's constructor |
| what goes on the wire | `dspGraph` — the decoded JSON | `faustDsp` — the source text |
| committed sibling artefact | none (node plays a `jsdsp.json`) | **none** |
| what runs on the audio thread | the `DspGraph` interpreter | compiled code |

The absent sibling artefact is not an oversight. A `dsp.k7x` decodes in microseconds, so no host caches it;
here the compiler is already on the other side of the wire: a cache of its output would be a second source of
truth for nobody's benefit. `readFaustSource` reads the file, full stop.

That holds only while nothing here needs the parameter list WITHOUT an engine: the panel and the
graph read the names from the `module.k7x`'s `params:`, which the author writes anyway. Needing them
otherwise means `faust -json` at compile time and a sibling `dsp.json` after all.

## The parameters, and what nobody checks

An `hslider("Cutoff", 1200, 20, 20000, 1)` gives name, minimum, maximum, default and step: the exact
shape of `DspGraphParam`, so `params:`, `ui.k7x`, `presets.k7x`, automation and `viewBindings`
carry on knowing nothing about Faust.

Neither side checks that the `module.k7x`'s `params:` and the source's `hslider`s speak of the same
names, and nothing here can: the parameters are declared by the compiler, which is in the engine. A
typo yields a setting nobody reads — the engine says so, one line per unknown name
(`applyPluginParams`, PluginParams.cpp). That is the only net, and it is at runtime.

## The imports

`import("stdfaust.lib")` has to resolve somewhere, and a module folder has to stay copyable. The
intended rule, unimplemented since nothing compiles yet:

- the **standard libraries** travel with the engine, never in the module's folder;
- what the module wrote itself goes in a `lib/` of its own;
- the include path is `[<module folder>, <engine libs>]`, in that order, and an import leaving the
  folder is refused as `resolveAssets` refuses a `../`.

Which assumes the engine knows the module's folder — it does not: what goes on the wire is the source
TEXT, not its path. A module importing a file of its own therefore wants either one more field in
`PluginDoc`, or that the compiler here flattens imports before sending. To settle with the compiler,
not before.

## The four gates

A `plugin.type` word must be known to four places, and the one you forget says nothing: forgetting the
second makes the module disappear *silently* from the compiled document.

| file | what it does |
|---|---|
| `loader.ts` `dspSourceFor` | resolves the source against the module's folder |
| `compile/plugins.ts` | the compiler's gate — an unknown type is dropped without a word |
| `graphDoc.ts` `known` | the node's colour: red means "the engine cannot load this" |
| `enginePlugin` (engine-protocol/build.ts) | the mapping onto the built-in plugin |

## Editing the source while it plays

`keyOf` (ProjectDiff.cpp) folds `faustDsp` into the plugin's identity, so saving `dsp.dsp` re-creates
it — mandatory, since compilation is in the constructor.

**`dspGraph` is not in there**, and that is a known gap on the dsp side: editing a `dsp.k7x` while
the engine runs reaches the document and changes nothing audible, until the next full rebuild.
