# Inventory of dead, duplicated and built-ahead code

What the repo contains and nothing reaches. Every entry carries enough to decide: *who calls it* is
the question, not *does it work*.

> **Read §0 before deleting anything.** Several perfectly live modules are invisible to an `import`
> sweep. The trap recurs at every survey.

`npm run lint:dead` (knip, `knip.json`) does passes 1, 2 and 4 below in thirty seconds. It is
**non-blocking** — a calibration is read, not suffered — and its exception list is §0 translated
into configuration. What it does not see: the C++ (`biome.json` excludes it too) and everything §0
describes — knip believes what it is told.

Current output: **0 files, 0 dependencies, 0 binaries, 0 exports, 0 types and 0 duplicate
exports**. On a checkout where the paraglide
catalogues have not been compiled, it also lists every `paraglide/messages.js` import as unresolved:
those files are generated, not missing.

> **A rising count can measure a `package.json` that lies less.** A symbol published in an `exports`
> map is invisible to knip, which takes it as consumed from outside; the same symbol turned internal
> becomes a plainly visible "unused export". Shrinking a published surface therefore moves findings
> out of the shadow and into the list. The count never reads alone: it reads against the `exports`
> maps of the same day.

## How this survey is made

Four passes, all reproducible:

1. **Published surface** — every subpath of every package's `exports` map, against the specifiers
   that actually appear in the source.
2. **Reachability** — a walk of the import graph from the real entry points: everything outside
   `apps/k7-daw-frontend/packages/`, files reached from another package by a published specifier, the `bin`/`main`, the
   declared `entry` globs, and the tests.
3. **Config** — dependencies imported without being declared, missing scripts, environment variables
   read without being documented.
4. **Symbols** — every `export`ed name, against every identifier appearing in the other files. A name
   nothing else pronounces, not even its own file, has no possible caller left. Deliberately crude: a
   homonym saves a dead symbol, never the reverse, so the list is a floor.

**Two SURVEY traps**, about the page rather than the code:

- **A file rename stales a path without staling the finding.** An entry can be wrong twice: the
  symbol has moved file, and it has been resurrected meanwhile. Check both, separately.
- **A corrected finding does not withdraw itself.** A commit updating the prose *around* an entry
  without revisiting the entry leaves an indictment of a repo that has moved. Replaying the passes
  is the only defence.

Corollary: **`docs/audit-moteur.md` §C5 keeps its own dead-code list**, over a perimeter overlapping
this one without being identical. The two are read together, or an entry stales on both sides
without anyone seeing it.

**Three method traps:**

- **Extensions count.** An import written `from "./addresses.ts"` escapes a search on
  `"…/addresses"`, which then declares a live subpath dead.
- **A specifier does not span two lines.** Too permissive a regex catches the word `from` in prose
  and invents dozens of phantom dependencies.
- **A NUL byte makes a file INVISIBLE.** ripgrep classes any file containing one as binary and skips
  it without a word: the file stops existing for the whole reachability pass. The case arises as soon
  as a NUL is taken as a composite-key separator — the six-character escape gives the same string at
  runtime and stays visible to every tool. `npm run lint:bytes`
  (`scripts/check-source-bytes.mjs`) refuses it in CI, and only helps if it is RUN. The trap costs
  more than it looks: knip reads source the way ripgrep does, so a file carrying a NUL is invisible
  to the dead-code detector itself, which will return a perfectly false "nothing to report". **Run
  `lint:bytes` BEFORE `lint:dead`**, or the second lies about the first. (This page does not write
  the byte literally: copying it from source to describe it traps the page itself.)

---

## 0. Known false positives — do not re-kill

### Loaded by PATH, never imported

| what | how it lives |
|---|---|
| `@studio/dsp` in `@studio/song` | the reference builder: `k7coreJsDsp.test.ts` imports it, and the vectors (`makeVectors.mts`) print the graph it builds from each `jsdsp.ts`, which every `dsp.k7x` must decode to. No compile runs it. |
| a library module's `jsdsp.ts` | kept one library version beside its `dsp.k7x` as the vectors' reference; the compiler never reads it (the type is `dsp`). Nothing imports it, and `knip.json` declares `k7-core/modules/**/jsdsp.ts` an entry. |
| `demos/**/jsdsp.ts` | same: a demo module's reference for its `dsp.k7x`. |
| `apps/k7-daw-frontend/packages/ui-chains-react/src/DomChains.stories.tsx` | picked up by `.storybook/main.ts`'s glob. |

`@studio/dsp` is also the author-facing API published to users
(`apps/k7-daw-frontend/user-docs/25-syntax-instruments.md`), and the C++ pastes its generated JSON
(`apps/k7-daw-engine/k7-runtime/engine/GraphDspPlugin.cpp`).

### `detectTransients` is called once per file, by the analysis

`apps/k7-daw-frontend/packages/audio-probe/src/waveform.ts` exports it and only `@studio/audio-analysis`'s `buildEnvelope`
calls it, while it builds a file's envelope; the result goes into the `{source}.cache` sidecar.
Nothing calls it per clip region, and nothing should.

### A deep import bypasses the `exports` maps

`apps/k7-daw-engine/k7-runtime/scripts/render-once.ts` → `apps/k7-daw-frontend/packages/take-render/src/renderProject.ts`. The file looks
unreachable if you look only at `exports`.

---

## 1. Published surface with no external consumer

**3 subpaths** have no importer outside their own package. That is a finding about the
`exports` map, not about the code: all 3 are **alive inside** their package, reached by relative
import.

| package | subpath |
|---|---|
| `engine-tunnel` | `/rtc` |
| `project-shell` | `/files` |

Shrinking the map deletes nothing: it documents the real surface.

The desk (`apps/k7-daw-frontend/packages/k7-plugins/src/turntables`) publishes **1** subpath, `/turntables/engine`. Its
other plumbing stays reachable by relative import inside the package.

## 2. Genuinely orphaned — nothing reaches it

| what | where | note |
|---|---|---|
| `probeLocalEngine` | `apps/k7-daw-frontend/packages/engine-browser/src/probe.ts` | the registry does this work. Deleting a loopback probe nothing replays is to be decided, not left to rot. |

### SYMBOLS, not files

The file lives, the function does not: nobody pronounces that name anywhere, not even the file
declaring it. From pass 4, over `apps/`; `probeLocalEngine`, above, is
not repeated. Pass 4 finds nothing else but the named stories of the `*.stories.tsx` files, which
Storybook reads by export rather than by name.

| symbol | where | note |
|---|---|---|
| types `LcdDraw`, `LcdPointer`, `PanelVisible` | `apps/k7-daw-frontend/packages/ui-chains/src/lcdScreen.ts` | the contract a module's `widgets.js` is written against (`../apps/k7-daw-frontend/packages/ui-chains-react/docs/lcdScreen.md`), kept for its reader: a `widgets.js` is plain JavaScript, so no file in the tree names them. |

**A further ~480 symbols are pronounced only by their own test.** That is not dead code — it is the
choice, taken everywhere here, of testing a constant or a computation step rather than the single
public function wrapping them. Counted so the figure does not surprise the next pass:
`ui-chains/src/cableCanvas.ts` (13), `song-schema/src/trackKind.ts` (7), `dsp/src/sweep.ts` (6).

**Tooling with no entry point** — programs, not dead code, but nothing wires them:

- `apps/k7-daw-frontend/packages/song/scripts/` — run by hand, apart from `checkPanelBinds.ts`, which is `npm run lint:panels`.
  `projectFingerprint.ts` records what a
  real project compiles to, to diff before and after a refactor.
- `apps/k7-daw-frontend/packages/k7-plugins/scripts/turntables/put-on.ts` — this one is **documented** and run by hand
  (`../apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/hosting.md`); it measures a SOUND, which no unit test does.

## 3. Built ahead — not to be confused with an oversight

### The `server` engine mark

The `server` engine mark (`apps/k7-daw-frontend/packages/engine-browser/src/choose.ts`, `mayAutoPick` rule 3) stays until engine ownership is reworked (D3, deferred).

### Asking a plugin what it has

The full chain is alive: `App.tsx` (`onScanParams={api.scanPluginParams}`) → `api.ts`
(`GET /plugin-params`) → the front's `/plugin-params` route (`apps/k7-daw-backend/docs/file-engine-jobs.md`) → `listPluginParams` →
`list-plugin-params` → `pluginParamsOf` (`apps/k7-daw-engine/k7-runtime/engine/ParamDump.cpp`). What has no caller is
the **browser half** only: `listPluginParams` (`engine-browser/src/engineQueries.ts`) and
`onPluginParamList` (`engine-browser/src/engineEvents.ts`).

Two reasons to keep the whole. It is the **only** way to learn an installed plugin's parameters — a
plugin registers them in its constructor, and the scan cache carries none. And three real failures
are legible in it: the engine rebuilt a `te::Engine` per question and forty questions in a row killed
it (measured, twice); it asked a plugin for its list before the asynchronous load had built it; and an
empty list confuses "I cannot load this plugin" with "it loads and publishes none".

That last point stands on its own: **a plugin with no parameter is almost always a plugin refusing to
load** (`getLoadError` says so, if asked). Answering "this synth has no parameters" states a fact
about the plugin where it is a fact about the machine — a licence, an installation — and sends nobody
to the only place it is fixable.

A panel generator derived from a parameter list can only produce buttons and dropdowns — never a
fader, a `meter`, a file picker or a layout. What would relight the browser half: a module builder
offering the list of a VST's parameters to bind.

## 4. Duplication

### A MIDI note's name, twice

The same chromatic table and the same arithmetic (`((k%12)+12)%12`, `Math.floor(k/12)-1`):

| where | constant |
|---|---|
| `apps/k7-daw-frontend/packages/ui-editor/src/noteGrid/RollGrid.ts` | `NOTE_NAMES` |
| `apps/k7-daw-frontend/src/noteNames.ts` | `NAMES`, behind `noteName` |

One aggravating detail: `noteNames.ts` calls itself "one spelling of a MIDI key, for every view" — true within k7,
false the moment you step into `@studio/ui-editor`, which k7 already depends on and which has one of
its own. The predicate follows: `isBlack = [1,3,6,8,10].includes(...)`, kept by the roll grid.

Not addressed: it is a decision about **where to put the table**, not a cleanup.

### STRUCTURAL copies — the ones an import cannot resolve

A duplication that cannot be removed without inverting a dependency or poisoning a bundle.
Written here so it stops being mistaken for laziness.

| what is doubled | why it stays |
|---|---|
| The `-100 dBFS` floor: `PEAK_FLOOR_DB` (`audio-analysis/src/peak.ts`), `MIN_DB` (`audio-probe/src/trimGate.ts`), and three times in `@studio/dsp` | `trimGate` is **browser-safe** — k7's clip panel imports it — and `peak.ts` spawns a decoder. Sharing the constant would bring a process launcher into the browser to save one number. The generated JSON cannot import at all |

### Cosmetic, but real

- **The `piano-16` glyph**, SVG body identical byte for byte:
  `apps/k7-daw-frontend/src/trackIcons.tsx` ≡ `apps/k7-daw-frontend/packages/ui-chains-react/src/moduleIcons.tsx`.
  (The rest of the glyph-table duplication is argued and accepted — see `moduleIcons.tsx`.)
- **The full-screen spinner**, three copies of the same block here:
  `apps/k7-daw-frontend/src/components/{AuthGate,EngineGate}.tsx` and
  `apps/k7-daw-frontend/packages/project-shell/src/GateScreens.tsx`. Left as is: short presentation blocks, no risk of silent drift — but
  one grows per gate, which makes this count a thing to re-check.

### Left on purpose

`AuthUser` is declared in k7's frontend's `api.ts`, carrying `email?: string | null`.

## 5. Config

| finding | where |
|---|---|
| With no test file at all: `graph-model`. | — |
| Read in code and absent from **every** `.md`: `STUDIO_MODELS`, `STUDIO_SCRATCH`, `CHROME`. | `apps/k7-daw-frontend/packages/song/src/modelCache.ts`, `apps/k7-daw-engine/k7-runtime/scripts/` |
| Imported without being declared: `ui-chains-react`, `score` and `k7-plugins` (`@storybook/react-vite`, in their `*.stories.tsx`; the root declares it). | — |
| `apps/k7-daw-frontend/packages/web-protocol` depends on `@studio/engine-protocol` alone, which has no dependency of its own, and that is RIGHT — what it takes from it is types. That absence of a runtime dependency is what lets `engine-client` take it to read `ENGINE_OUTBOX_MAX` without weighing down the supervisor's bundle. | `apps/k7-daw-frontend/packages/web-protocol` |
| Read in code, absent from `env.example.yaml`: `STUDIO_REGISTRY_URL`, `STUDIO_DEMOS`, `STUDIO_MODELS`, `STUDIO_WEB_PORT`, `STRUDEL_SAMPLES`, `DIRT_SAMPLES`, `STUDIO_ENGINE_TOKEN`, `STUDIO_SCRATCH`, `CHROME`. | `env.example.yaml` |

## 6. A deck has no effects chain

Mix is a desk: two records and a crossfader. A deck's effects chain has no place on it, and the
desk's only host (k7's `turntables` surface) draws no per-deck graph, so there is no component for
one: an option nothing can mount would cost code and a published subpath that lies about the
package's surface. See
[../apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/mix.md](../apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/mix.md).

One leftover name: `m9`, an application that does not exist, is a session id in the
`SessionRegistry_test.cpp` fixtures — any string would do there.

> **A test file does not die with its subject; its tests die one by one.** A test file named after
> a module regularly covers another one through a shared helper: a `describe` about one component
> can hold tests of `deckDoc` — `deck.ts`, very much alive — the module order (audible), the wire
> form of `plugin:`, the bypass. Deleting the file "because it tests dead code" would lose them
> silently. Those live in `deck.test.ts`, under "a deck's modules, in the document".

## 7. Kept after the OIDC sign-in left the front

The front serves `/auth/me` and nothing of a provider. What stays, and why it is not dead:

| kept | where | why |
|---|---|---|

## Left on the phones and the supervisor after the pairing left

- **`weburi.add`, `weburi.pair` and `weburi.remove` in `apps/k7-daw-engine/k7-supervisor`** stay: the CLI engine and the Pi image ship the supervisor unchanged, and the phones no longer send them. They go with the CLI engine's rework (`registryUrls`, the registry URL in `Url.cpp`).
- **`CameraMode.tsx` and the `camera mode` verb** stay behind `CAMERA_MODE_LISTED` in `homeCommands.ts`: cameras return as an app feature.

## 8. Deferred reworks, and the code that waits for them

k7 runs on a device that holds one local account. The admin stamp, the engine ownership records, the
share lists, the render key and `STUDIO_REGISTRY_REQUIRE_DEVICE` are gone. What still carries the shape of
a multi-account server is kept whole, tested, and listed here so `lint:dead` debt is explained.

| name | what stays | what removes it |
|---|---|---|
| **D2** the accounts | the multi-account file hub (`k7-hub` accounts: a slug and a home per user, the cache sweep over every account), the session code (section 7), the libraries' `draft` folders (`k7-libraries`' `ensure(.., draft)`, the `draft` version of a pin, `song-edit`'s `overlays`) | a slice that makes the hub single-account and drops the library draft model |
| **D3** engine claims | the claim's `holder` strings and the displacement rules between holders (`routes/engine/registry.rs`, `k7-engine/src/registry/claims.rs`, `apps/k7-daw-backend/docs/engine/claim.md`), the `unowned` listing of an engine off this machine | the engine registry's rework with the remote engines, which is tier 4's |
| the app's **second-device rework** | `/external` (`ExternalIndex`, `apps/k7-daw-frontend/docs/external.md`), the camera mode behind `CAMERA_MODE_LISTED`, the engine tunnel hub, `weburi.add`/`pair`/`remove` in the supervisor, `engine.example.json`'s `registryUrls`, the registry URL in `Url.cpp`, `STUDIO_REGISTRY_URL`, `k7-local`'s settings | the rework that makes a second device and a remote camera features of the app; the CLI engine and the Pi image ship unchanged until then |

`k7-sync`'s `relay/` module may be dead, since only the capture's history uses the crate: a
`lint:dead` question of its own.
