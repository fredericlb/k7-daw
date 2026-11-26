# The k7 SDK — a plugin is a contribution, not a process

One `K7Plugin` a feature declares, through `definePlugin`, landed by the shell in registries that
**already existed**. The SDK has no runtime of its own, no loader, no sandboxing — the registries
are its whole surface, and a plugin is a set of contributions to them:

| a plugin may contribute | registry it lands in | who reads it |
|---|---|---|
| pane drawers (`PaneDrawerContribution`) | the shell's pane ring | the shell's pane and a device's mount |
| module folders (`ModuleContribution`) | the module library | the graph, the module builder |

## CHOICES

**A verb is never a plugin's.** k7's verbs are the Rust crate `apps/k7-daw-wasm/verbs`, run by the backend
(`docs/verbs-port.md` at the root): a feature that needs a verb adds a handler there and a row to `commands.yaml`, as
`track transcribe` does for transcription. A plugin brings what the page draws.

**A drawer is typed where it is mounted.** The SDK's `PaneDrawerContribution` takes `never` —
the bottom type, which any real drawer assigns to — because the props a drawer gets are the
SHELL's to know: the pane mount (hidden-and-shown, a long-lived ring slot) and a device's mount
(fullscreen, whole-ctx) pass different things, and both are the shell's invariants. Defining
them here would make the SDK import the shell or the shell import the SDK, and the second is
the one that is true; the first would drag app internals into the shared tier. The drawer a
plugin declares must be usable in BOTH mounts — its props are a structural subset of each ctx,
and what it needs beyond the subset (the graph fetch, the library base, the revision token) it
takes from the ctx the shell injects or builds from shared packages. Distinct per-mount drawers
would fork the registry's contribution shape, and no plugin has needed one yet.

**The engine is out of scope, stated once.** A plugin cannot add an engine module TYPE without
C++ work in k7-runtime. The honest paths for sound are what exist: library modules the plugin
ships into a module library, and `@studio/dsp`-authored modules the engine already executes.
`docs/audit-moteur.md`'s boundary applies unchanged.

**No runtime loading, yet.** Plugins are in-repo TypeScript the composition point imports.
Everything the four first examples need (transcription, turntables, timeline, video) is core
functionality, which needs attribution and versioning, not loading — and a loader without a
second consumer is machinery that nothing tests.

## Where installation happens

The shell's composition point (`apps/k7-daw-frontend`) imports `@studio/k7-plugins/browser` and registers each plugin's
drawers through its pane registry.
