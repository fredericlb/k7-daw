# The module graph in Rust (2k)

Sub-step 2k of [rustCompiler.md](rustCompiler.md), whose decisions 1 to 120, output rules, hostile-input rules and stage gate it keeps; 2e's, 2f's and 2g's
are in [rustCompilerLanes.md](rustCompilerLanes.md), [rustCompilerClips.md](rustCompilerClips.md) and [rustCompilerCurves.md](rustCompilerCurves.md).
It ports what the Chains tab DRAWS: the `WebGraph` of `@studio/graph-model`, every node's flags, panel and choices, every lane's clips and signal,
the warnings `validate` raises, and a library chain's graph. It builds no `ProjectDoc` and nothing here is wired to a route. Paths are relative to `apps/k7-daw-frontend/packages/song`.
The graph takes what the compile hands as plain data (`Handed`); `view/handed.rs` is the one file that reads 2h's `Compiled` and makes it.

## Corrections to the 2k row of `rustCompiler.md`

- The row omits the helpers `graphTypes.ts`' `isSink`, `chainHosting`, `chainAt`, `isNode`, `carries*`, `isStretch`, `kitOf`, `nodePlace.ts`, `moduleAssetChoices.ts`,
  `moduleAssetFiles.ts`' `listModuleAssets`, `listFiles.ts`, `basePreset.ts`' `basesIn` and `moduleBaseChoices`, `libraryFiles.ts`' `libraryFiles`, `song-video`'s shader listings,
  `ampModel.ts`, `modelCache.ts`' `missingModel` and `ownMix.ts`' `stageOf`; the `at` branch of `Workspace.graphDoc` is 2l's.
- `laneStrip.ts` is 335 lines of which about 40 reach a route; the rest writes files and is step 3's.
- `graphDoc.ts` depends on the compile through FIVE values, not two: `autoFreeze`, `freeze` and the three `…Of` functions over a `ProjectDoc`. All five arrive as `Handed`.
- 2h's `autoFreeze.ts` and `outputRecorder.ts` call `loadGraph`: the reverse dependency is unlisted.
- `What the compile reads` lacks the listings a graph walks (`assets/`, `base/`, `shaders/`, every library's `samples/`, `irs/`, `wavetables/`, `models/`) and the `.json` amp
  model it opens. Decision 68 ("a model is never opened") holds for resolution and not for this view (425).
- The 2,500 lines are low: the Rust is about 4,100 lines and its tests and scripts about 5,000, beside a corpus of 93 cases and a sweep of 1,500 projects.

## What the graph decides (2k)

Numbered on from 2e's table; 400 to 445 are the work order's, 446 on are the builder's. A row marked DEPARTS differs from the TypeScript and says why: it is `ts: false`
in the corpus or a unit test, so the gate's allow list stays empty. Each is named by its number in the test that holds it.

| # | decision | why |
|---|---|---|
| 400 | the code lives in `rust/src/graph/`; nothing but `view/handed.rs` imports 2h's `Compiled`, `ProjectDoc` or `AutoFreeze`; what the compile hands is `Handed` | 2f's rule: every function takes typed inputs |
| 401 | DEPARTS in the work only. A graph stands on the `Inputs` the compile loaded: `load_graph` reads the head, routing, nodes, vocabulary and families nothing more | the TypeScript reads each twice per `GET /graph` |
| 402 | the graph's chains are derived AGAIN from `Inputs.routing.segments` with the graph's own lane test (a name that is `kind: track` in the local-or-extra modules, or a picture lane), then those whose `from` is a disabled lane are dropped, then `apply_link_levels`. `Inputs.chains` serves only the wiring check | `buildGraph`'s second `parseChains` is wider than the compile's: `"@meta/track": [x]` is a lane in the graph and not in the compile (244) |
| 403 | the sound lanes are `lane_of` over the local modules then the extra ones, picture lanes left out; the picture lanes are the local picture modules, muted by `bypassed || disabled` | `lanesOf(resolved)` runs over the chain-extended map, `loadVideoTracks` over the project's files |
| 404 | the wiring check gets the RAW ports: `Inputs.nodes.nodes`, else the vocabulary's raw module. A node that only inherits its ports is unconstrained here, though the compile checks it | `loadGraphIn` hands `checkChainInputs` the unresolved map; a project the compile refuses can still draw, which is the point of `ifCompiles` |
| 405 | what refuses a graph is `GraphError::Refused`: the stops of the inputs minus the findings whose file starts `timelines/` minus the head's time-signature finding (447) and minus `settings.k7x`'s, which `loadGraph` never reads, or any wiring fault | `loadGraph`'s throws: the root, the chains, the nodes, the wiring. 249 stops a compile on a broken timeline and the graph has no reason to |
| 406 | DEPARTS. A file of `clips/`, `sequences/` or `timelines/` that does not decode refuses nothing: it is absent, every placement naming it reads `missing`, and its finding is in `Answer.findings`. The TypeScript throws on a SYNTAX error there and loses a schema error silently | one file costs that file (204, 248) |
| 407 | DEPARTS in the work only. `moduleRefs` is not built. Whether a ref exists is `Vocabulary.modules.contains_key`; the ancestry a walk climbs asks `resolve_ref` per distinct ref, memoised, each charged `weigh(m)`; its findings are dropped as the TypeScript's are | the resolved `extends` and `folderAbs` matter because an engine or an alternative may rewrite them |
| 408 | `Index` is built once per `GraphDoc`: the first chain hosting each step, the first chain by `from`, the nodes some chain feeds, a `Carries` and a stretch flag per chain, a lane's kinds, the disabled set (sorted, searched) | each was a linear `find` in a loop |
| 409 | `reachesSink` is one reverse reachability over the chain edges; `hasCycle` is one iterative three-colour walk with `Done` kept across starts, answering the first START in key order from which a cycle is reachable | recursion over 100,000 chains would overflow and a fresh walk per chain is quadratic |
| 410 | DEPARTS. `validate_issues` stops issuing at `MAX_ISSUES` (1,000), a name quoted in a warning is cut to `NAME_CUT` (200), the joined outputs of `outBadPort` too; past the cap `warnings` gets one extra sentence and `issues` nothing | `GraphWarningCode` is a closed union the screens exhaust; a 1 MB name in 100,000 warnings is 100 GB |
| 411 | the sentences outside `validate_issues` (levels not played, input labels) count under the same cap; the "have:" list is built once and cut to 300 bytes; a gain is printed by `clips::js_number`; the doubled "preferences.yaml preferences" is kept | the TypeScript's words |
| 412 | `disabled` is sorted by `str`'s `Ord`; a chain or lane whose name is integer-like is met in file order | decisions 165, 256 |
| 413 | a `GraphModule` borrows its `Resolved` and normalises only what validate and the view ask: `kind`, `inputs`, `outputs`, `emits`, `channel`, `monitor`, `automations` | `toModule` copied every field per module |
| 414 | a step carries `base` when the resolved module names one; the TypeScript on this branch does the same (`ts: true`) | the panel shows the current base |
| 415 | an automation carries its `trigger` when the file states one (`ts: true`) | `buildChains.ts` reads `a.trigger ?? s.module` for the cable |
| 416 | `moduleAssetMissing` for `ir` reads the resolved `LoadedModule.ir` (`ts: true`) | an `ir` that resolved is not missing |
| 417 | DEPARTS. A placement whose start is not finite is left out of `placements` with one warning finding named by the lane's file; a start `beats_to_position` refuses keeps its place and has `tc: ""` | JSON writes `null`; a position of `1e300` beats has no spelling |
| 418 | placements are read on a meter that defaults each half of `timeSignature` apart, as `buildGraph` does (`parseInt`, then `n \|\| 4`; a half past `u32` stays a float); a clip's `tc` is spelled on the header when it is `digits/digits` with no zero half, else on 4/4 (454); the tempo is the file's or 120 | `num \|\| 4` and `den \|\| 4`, and `graphDoc`'s own regex for the spelling |
| 419 | `Spend` charges every copy and every walk against `BUDGET` (1,000,000), a unit being 64 bytes of output or one walk step: a built step `serialized bytes / 64 + 1` per USE with its `pluginId`; a chain, an out, a track, a colour, a signal, a doc note, a tap, a mixer strip, a tap feed, a stack, a lane's alternatives (every name) `1 + bytes / 64`; a clip `1 + ref bytes / 64`; a curve, an assembly, a position by their serialized bytes; each listed entry, each shader's two stats, each ancestor resolved `weigh(m)`, an amp file `len / 64 + 1`; each name of a `FreezeMark` or `AutoMark` | a limit counts the output and the walks |
| 420 | past the budget: `capped`, one WARNING finding named by the module's file it stopped in (`graph.k7x` for a node the graph names), `Answer.capped`. Every chain keeps its `from`, `use` and `out`; a step not yet paid for is bare (`module`, `enabled` if any, `stub: false`, `hasNativePluginUI: false`, `params: {}`); `trackClips`, `trackAlternatives`, `trackMixers`, `tapFeeds`, `stacks` (the entries paid for stay), curves, assemblies, positions and frozen marks not yet paid for are absent | the topology is what the screen draws first |
| 421 | a step's body is built once per MODULE NAME, then cloned per use with its `pluginId` and `enabled`; the serialized size is measured once | a panel of 900 widgets over 100,000 steps is built once and refused by weight |
| 422 | every walk goes through the Source in one helper, `list_under`: dot names skipped, depth as the TypeScript's (assets 6, bases 3, library files 6), entries as `Source::list` reports them; a link is listed as what it points at | `Source` has no link kind (decision 1) |
| 423 | a walk whose folder the Source refuses as `Outside` answers empty and one WARNING named by the module's file, once per folder; `NotFound` and `Io` are silent and empty; `Missing` is `Err`, and so is a shader stage the Source answers `Missing` for; a stage it refuses as `Outside` is not offered and warns once for its shaders folder | decision 24's rule |
| 424 | `Walks` memoises per folder (assets, bases, shaders) and per family (library files), each a shared `Arc`; a library listing is the project's own `@project/<path>` for `samples` and `wavetables`, then each library root holding the family, sorted with the project's first | 100,000 rack steps over one library walk once and pay for their copies |
| 425 | `ampModel`: `.nam` answers `nam, 0` with no stat; `.json` is stat'ed and read through the caller's Source when at most 4 MiB, memoised per path per call, into a probe of five keys, every other key ignored unread; anything else answers none; the probe reads an object only (a sequence is none), a `null` `input_size` is the absent one, and the last of a repeated key stands | the panel's script needs the knob count |
| 426 | `missing_model` is pure, over `FILE_PARAMS` in order, the authored name present and the resolved field absent; only one with a `model_url` reaches the view | `missingModel` |
| 427 | `stage_of` lives in `view/flags.rs`; 2h reuses it if it ports `ownMix.ts` | two copies must not drift |
| 428 | curves are `read_curves`: every curve that decodes, in file order, a broken one absent with 2g's warning in `Answer.findings` | one file costs that file |
| 429 | `WebCurve` is `{type, shape, points, spec}`, `spec` the `CurveFile` as 2g fixed it | decision 201 |
| 430 | `Handed` is the seam: `extents`, `mixers` and `colors` are `None` when the project does not compile, `auto` is empty then and for any folder but the open project's, and `manual` is the graph's own for every folder, compiled or not | `buildGraphDoc`'s arguments already are that shape, and it computes `frozen` for every folder it draws |
| 431 | the frozen marks are computed from `Handed` plain: per plan in order every lane and module inside, then the point; a later plan may overwrite. Auto: `ready && !hosted` marks `autoFrozen`, `hosted` marks nothing, else `autoFreezeWaiting` | `graphDoc.ts` |
| 432 | a lane's extents are one map per lane keyed by the start's bits with `-0` made `0`, the last one winning; `len` is the compiled length when above 0, else 4, and absent when not finite | `new Map(...)` and `NATURAL_BEATS` |
| 433 | a library chain's graph has no code of its own: the host gives its FOLDER as `Roots::project` to `load_compiled_graph` (462) with `Folder::Other` and the compile of that folder, `None` when it does not compile | `graphDoc(at)` |
| 434 | `@in` and `@out` draw as any other name and warn as the TypeScript does | the folder chain's warnings are what it answers today |
| 435 | `node_places` skips a lane item, `place` is `chain · step n`, `chain › sub · step n` in a subchain, `n.k` for a parallel item; `node_stacks` is each called subchain's item names | `nodePlace.ts` |
| 436 | `lane_strip` finds the last strip of the lane's first run; only the picture lanes' strips come from here, and they override `mixers` | `{...mixers, ...laneStripsOf(g, pictureLanes(g))}` |
| 437 | the field rules of `WebGraph` and `WebChainStep` are the tables below, and are what the `serde` attributes say | the frontend reads presence, not only values |
| 438 | the serialized shape is proved three ways: (1) every `Web*` type derives `ts_rs::TS` and `scripts/graphTwinCheck.ts` holds each generated type assignable to the hand-written one, their keys equal up to the named absent list, with one `@ts-expect-error` proving it can fail; (2) the whole JSON is compared with `compare.rs` over the corpus, the negative corpus and the sweep; (3) `no_null` refuses a `null` or a float in an integer field, or in the numbers of an integer array, and runs over every drawn answer of the gate | a `#[ts(type = …)]` proves nothing about the field it names, so the leaves that keep the hand name (`ui`, `cc`, `spec`, and the string unions) rest on (2) |
| 439 | the known-absent keys of the key check: `WebChain.enabled` (nothing sets it) | a list of what is known absent, not a filter on what fails |
| 440 | `category` is `#[ts(type = "WebModuleCategory")]`, mirroring the `as` of `graphDoc.ts` | a cast, not a claim |
| 441 | no budget on `validate_issues` and `build`: each pass visits each chain, step, out, lane, placement, group member and assembly member at most once through an index, each copy is capped (410, 411); a chain's input landing is found once for the chain, not once per input | a pass with no product says why it has none (283) |
| 442 | DEPARTS. A kit pad's mute reads a NUMBER param only; a text param is not muted | porting `Number("0.7")` would widen `resolve/alternatives.rs` |
| 443 | the gate is in files of its own | decisions 171, 217 |
| 444 | no name of 2k becomes a path in Part A: nothing joins a node, lane or chain name to a folder | the rest are listings and paths 2c resolved |
| 445 | `Answer.findings` holds what the TypeScript lost in silence or threw away, never `validate`'s output; `WebGraph.warnings` holds `validate`, unplayed levels and input labels only | `warnings` is a contract of `WebGraph` |
| 446 | `load_graph` appends to `GraphDoc.findings` the findings of the three families (every severity) and of the assemblies, so a broken clip or a template that will not read reaches `Answer.findings` | 406 and 445 |
| 447 | the head's time-signature finding (a signature that makes no bar, such as `0/4`) does not refuse the graph, which draws on 4/4 | 418 says the graph draws; `Inputs::stops()` counts that finding, so 405 read alone would refuse. A question for the designer |
| 448 | the levels not played are found by the graph's own loop and not by `routing::unplayed_levels`, which copies every name before the cap can stop it | a name copied per item is a defect |
| 449 | an ancestor is charged `weigh(m)` as 407 says, where 419 says "one" | the stricter reading |
| 450 | DEPARTS. A `channel` number past `u32::MAX` is dropped | a channel is never `f64` (250) |
| 451 | an assets listing, a shaders listing and the library files are sorted as JavaScript's default `sort()` sorts (UTF-16 code units); bases by code point | as the TypeScript |
| 452 | `scripts/graphSweep.ts` holds the sweep's generator and is not in the layout | `stageDumpGraph.ts` would pass 500 lines |
| 453 | `Walks` lists a library root once to know which family folders it holds, and mounts nothing: the gate mounts the folders a staged library links its sounds into, as a host does; 2l's host MUST do the same (its decision 737 does), since `Roots::mounts()` does not and an unmounted linked family folder is `Outside`: one warning and no files, where the TypeScript lists them | decision 82; the disk Source confines a link out |
| 454 | DEPARTS. Under a time signature with a zero half (`0/4`, `3/0`) a clip's `tc` is spelled on 4/4, where the TypeScript spells it on the signature as written: `1:5` for `0/4`, and garbage (`1:NaN`, an infinite beat) for `x/0`, which is not copied | 418: the starts are read on the defaulted meter and agree; a nonsense meter has no spelling to keep |
| 455 | the dump lays a case's rig as the machine's preferences for the length of its dump; a case the TypeScript answers is read with the roots its dump used and drawn through `load_compiled_graph` (462) | the TypeScript reads the real `@meta` folder and the real compile, where the Rust-only corpus lays a stub of each |
| 456 | a shader is any folder under `shaders/` whose two stage paths exist, whatever they are: a dot folder is one and a stage that is a folder counts | `shadersIn` has no dot filter and `existsSync` accepts a folder |
| 457 | `handed_of(source, inputs, g, compiled)` (the open project's, which `load_compiled_graph` calls) takes the compile's `Compiled`: extents, mixers and colours from its document, `auto` from its freezes. The manual plans are NOT 2h's: they are computed over the GRAPH's chains and lanes (`g.chains`, `g.tracks`), as `graphDoc.ts` asks `activeFreezePlans` | 2h keeps its plans private, over the compile's chains and lanes and only when a document exists; the graph's differ in a project that does not compile, in the `@meta/track` corner (402) and for a disabled lane |
| 458 | a render exists when the Source stats it: an absolute path as it stands (`is_absolute`), else `join(project, render)`, then `render_ready`. `Outside`, `NotFound`, `Io` and a NUL answer false, `Missing` is `Err`; a climbing path is not refused by this code, the Source decides | 2h's 335, the one spelling of a render's stat; `existsSync(resolve(dir, r))` accepts both. On Windows `spell` makes a backslash a separator and the disk Source refuses a `:` past the drive |
| 459 | `handed_of` pays every copy it makes against a `Spend` of its own, a name by `1 + bytes / 64` BEFORE the copy: a track, a mixer, a colour, a plan, an automatic freeze. The plans are `freeze::freeze_plans`, the compile's own function, which pays every name a plan copies by its bytes under a freeze `Budget` of 3,000,000 and gives no plan when it stops | a lane of 1 MB in every plan is the product a count of names does not see, and the graph and the compile must not each have a way of counting it |
| 460 | the three maps of `Handed` come out in `Object.entries` order (`own_key_order`) | `clipExtentsOf`, `trackMixersOf` and `trackColorsOf` build JavaScript objects |
| 461 | not ported: a node named after a key of `Object.prototype` (`constructor`, `toString`, `__proto__`). In the TypeScript a MODULE so named changes the chain derivation (`chains/a/out/0/to` differs and `outIsPictureLane` is raised) and a LANE so named makes its compile refuse (`handed.extents` is null); the Rust derives and compiles both as it does any name | the TypeScript reads an inherited function off a plain object (`!out[t.name]`): a bug, and not a rule to copy; no screen names a node so |
| 462 | a host with a compile calls `load_compiled_graph(source, cache, roots, rig, Option<&Compiled>, Folder)`, which loads the inputs and the graph, makes the `Handed` and draws it: `handed_of` needs the `GraphDoc` that only this call holds, so no host can order the steps wrongly. `None` is a host that did not compile. `Folder::Open` draws the project's automatic freezes, `Folder::Other` (a library chain's folder, `graphDoc(at)`) none, as the TypeScript passes none there; the manual plans and the compile's values are drawn for both. `load_web_graph` serves a host whose `Handed` is given | the manual plans are the graph's own (457) |
| 463 | a product that does not fit the `Handed` spend is absent whole, never cut: `extents`, `mixers` and `colors` are `None`, `manual` and `auto` empty. The compile's values are paid first, then the plans, then the automatic freezes, and the answer is `capped` with one WARNING named `project.k7x` | a list cut at an arbitrary place reads as a project with fewer plans, and the freeze budget already gives none or all |
| 464 | a case with `renders: "ready"` and every other sweep project have their automatic renders made before they are drawn, by each side at the name its compile gives (343): the dump from its own first compile, the Rust test from its own | the two fingerprints differ, so one file cannot make both `ready` |
| 465 | `Session::model_need(source, ask, node)` answers `missingModelOf`: no node, the model there, or the file and its `model_url`; it loads the graph with no compile and holds nothing, so a model that arrived is seen at the next ask | the route's answer must not lag the disk, and a refused graph is `Err` for the host to leave to the TypeScript |

## The field tables (437)

`WebGraph`, the top level:

| key | present |
|---|---|
| `chains`, `tracks`, `videoTracks`, `trackColors`, `trackSignals`, `modules`, `warnings` | always |
| `stacks` | in a chain project, even empty |
| `positions` | when any survive |
| `trackMixers`, `trackClips`, `trackAlternatives`, `frozen`, `autoFrozen`, `autoFreezeWaiting`, `assemblies`, `tapFeeds`, `curves`, `issues` | when not empty |

`WebChain`: `from` and `fromPort` when set, `use`, `out`. `WebOut`: `to`, `gain`, `fromPort` when set. `WebChainStep`:

| key | present |
|---|---|
| `module`, `params`, `stub`, `hasNativePluginUI` | always; `params` is the module's plus `@mix`, `@input-volume`, `@output-volume` when numbers |
| `missing` | `true` when no module, else absent |
| `uses`, `place`, `enabled` (`false` only), `category`, `emits`, `ui`, `cc`, `alternatives`, `alternative`, `engines`, `engine`, `base`, `shader` | when set; `alternatives` and `engines` past one |
| `stage` | not empty; none for an offline module or the strip |
| `modelDownload`, `ampModel` | a missing model with a url; a model whose file answers |
| `inputs`, `outputs` | past one; `mandatory` only when `false` |
| `assets` | the module's |
| `assetChoices`, `bases`, `shaders` | not empty; `shaders` only for the `shaders` category |
| `events`, `notSupported`, `unknownPlugin`, `strip`, `rack`, `rackKit`, `defaults`, `pluginId`, `monitor`, `channel`, `camera`, `device`, `tap`, `automations` | as `graphDoc.ts` sets them |

## The seams

| neighbour | what 2k gives it | what 2k takes from it |
|---|---|---|
| 2c | | `Resolved`, `resolved_*`, `resolve_ref`, `Scan`, `Vocabulary`, `weigh` |
| 2d | | `derive_chains`, `is_parked`, `apply_link_levels`, `Segments`, `ChainProject` |
| 2e | | `Inputs`, `Lane`, `lane_of`, `resolve_input`, `monitor_mode`, `input_landing_track`, `check_chain_inputs`, `resolve_assemblies` |
| 2f, 2g | | `Grid`, `js_number`, `read_curves` |
| 2h, reverse | `load_graph` and `GraphDoc` | |
| 2h, forward | `handed_of`: `Handed`, filled from 2h's `Compiled`, and `render_exists` | `Compiled`, `AutoFreeze`, `Edges`, `render_ready`, `is_absolute`; `ownMix.ts` should it land first (427) |
| 2j | `Answer.findings` | |
| 2l | `load_compiled_graph`, `load_web_graph`, `GraphError`, the departures the allow list never needs: 406, 428, 417 and the wider findings | `Roots`, the host's `Handed`, the answer cache, and the mounts of a library's linked family folders (453) |
| step 3 | `GraphDoc`, `Index`, the strip finder | |

### What a host calls

A host that has the compile calls `load_compiled_graph` once and does not call `handed_of`: the `Handed` needs the `GraphDoc`, which the call builds, so a host cannot make it first.

| the host's case | the call |
|---|---|
| the open project, which compiled | `load_compiled_graph(source, cache, roots, rig, Some(&compiled), Folder::Open)`, `compiled` from the same `Roots` and rig |
| the open project, which does not compile | the same with `None`: the graph still draws the project's own manual plans, which `Handed::default()` would leave out |
| a library chain's folder (`graphDoc(at)`) | `Folder::Other` with `Some(&compile of that folder)`, or `None` when it does not compile: the compile's values and the manual plans are drawn and no automatic freeze |
| a `Handed` the host holds already (a test, a cached answer) | `load_web_graph` |

## The gate

`scripts/stageDumpGraph.ts` writes the TypeScript's `buildGraphDoc` answer, its `loadGraph` model and the `handed` its compile gives, per project, per
`ts: true` negative case and per sweep project; `rust/tests/all/stages_graph.rs` reads each through a `Recorder` and again through the bundle it made, one `Cache`
shared across projects, and compares with `compare.rs` (`issues`, `warnings`, `tracks`, `videoTracks` as sets; every other array in order) under an empty allow list.
**Which path each count covers**: the Rust compiles the project itself (`Via::Compile`), `load_compiled_graph` draws the graph over the `Handed` it makes, and that `Handed` (made by `handed_of`) is compared
with the dump's apart from the graph (`/handed`), so Part B is what the golden corpus, the sweep and every `ts: true` case check. The one exception: the two
`ts: false` cases that state a `Handed` of their own (`Via::Given`, read by Part A alone). The counts that name a `real` path below are of the first kind only.
Where the TypeScript threw, `Refused` or a finding must name the same file or the wiring. `rust/tests/all/graph_negative.rs` is the same corpus without the TypeScript and runs
wherever `cargo test` does. `scripts/graphTwinCheck.ts` is the type test of 438.

Over the staged corpus (177 projects, 5 of them chain projects), 107 negative cases (94 with the TypeScript) and 1,500 sweep projects (seed `0x2c0f1a5b`) the gate
finds no difference, with the allow list and the strip list empty: every project is read through a `Recorder` and again through its bundle (1,784 replays, which must agree on the graph, the model, the `Handed`, the refusal and every finding), and 119 times
the TypeScript threw, `Refused` or a finding named the same file or the wiring. What it saw:

| what | golden corpus | negative cases | sweep |
|---|---|---|---|
| drawn, refused | 177, 0 | 103, 4 | 1,385, 115 |
| chains, steps, outs | 563, 392, 568 | 138, 103, 139 | 7,347, 5,492, 10,751 |
| outs to a port, chains with `fromPort` | 4, 2 | 5, 4 | 2,424, 333 |
| lanes: single, group, strudel, picture; bypassed | 359, 5, 8, 9; 12 | 73, 5, 2, 4; 1 | 1,635, 560, 439, 476; 61 |
| clips: audio, midi, missing; with a compiled `len` | 281, 87, 1; 367 | 20, 5, 6; 2 | 698, 724, 425; 609 |
| signals midi, audio; colours | 87, 285; 365 | 0, 80; 53 | 512, 2,122; 825 |
| steps with `uses`, `stage`, `pluginId`, `events`, `defaults`, `engines` | 387, 345, 342, 92, 64, 223 | 13, 5, 65, 1, 1, 0 | 1,171, 380, 3,161, 0, 209, 0 |
| steps with a panel; a script, a `vertical`, tabs | 359; 148, 184, 51 | 5; 0, 0, 0 | 209; 0, 0, 0 |
| `stub` true, `strip`, `rackKit`; racks with sounds, empty racks | 35, 3, 10; 13, 1 | 2, 1, 2; 4, 2 | 191, 209, 187; 368, 0 |
| `assetChoices`, `bases`, `shader`, `place` | 8, 5, 4, 14 | 10, 1, 2, 4 | 4,013, 380, 180, 924 |
| `inputs` past one, `outputs` past one, `automations`, `camera`, `modelDownload` | 10, 1, 2, 1, 2 | 7, 2, 1, 1, 1 | 276, 96, 173, 234, 105 |
| curves, assemblies, `stacks`, `positions`, `tapFeeds`, `frozen` marks | 4, 2, 0, 0, 0, 0 | 5, 5, 2, 2, 1, 20 | 1,385, 510, 168, 781, 257, 777 |

What `handed_of` made and the TypeScript agreed with (the `real` counts, which are every project but the two `Via::Given` cases):

| what | golden corpus | negative cases | sweep |
|---|---|---|---|
| drawn over the Rust's compile: with a document, without | 175, 2 | 74, 27 | 693, 692 |
| with colours, extents, mixers handed | 173, 169, 3 | 47, 7, 0 | 662, 514, 16 |
| clips whose `len` the document gave | 367 | 0 | 609 |
| manual plans; `frozen` marks over a real render | 0; 0 | 9; 17 | 455; 777 |
| automatic freezes: ready and played, hosted, waiting; `autoFrozen`, `autoFreezeWaiting` marks | 0, 0, 0; 0, 0 | 2, 1, 1; 4, 2 | 9, 0, 3; 11, 4 |

The golden corpus holds no frozen module. The sweep freezes about a third of its plugins and lays a render for most of them, and every other project has its automatic renders
made first (464), so a plan in force, a render that stands in and a freeze that plays or waits are compared there as well; the negative cases hold the corners of the manual plans
(eleven `frozen-*` cases: a lane, integer-named plans that overlap, a node that feeds outside, a render that climbs inside the workspace and one that leaves it, a folder, a disabled
lane, an `@meta/track` step, a node on no chain, a render that is missing, and a manual plan beside an automatic one).

The most any graph spent is 8,078 of the 1,000,000 budget, and the gate fails past a tenth of it. The minimums sit below these counts and above zero, at what the kinds really
give and not at a round figure: 170 projects, 4 chain projects, 100 negative cases (88 with the TypeScript), 1,500 sweep projects, the golden counts of the table above for chains,
steps, lanes, clips and panels, every step key of the field tables at least three times over the three corpora (a MIDI map once), every warning code at least three times over the
negative cases and the sweep. The sweep alone, apart from the negative cases, must hold what its pool names: 200 strudel lanes, 250 assemblies, 90 nodes in two assemblies,
30 boxes named after a node, 80 stacks, 450 steps with a `place`, 400 positions, 120 tap feeds, 300 manual plans over a render that is there, 6 automatic freezes that play their render, 2,000 steps with `assetChoices` and 500 without (the assets vary by project, so the
count is a fact of the project and not of every step). The exceptions are the two codes no file can make (`outNegativeGain`, `chainDeadEnd`: `graph_validate.rs` raises them on a
graph changed in memory) and the two that reach once (`inputTrackGroup`, `inputTrackStrudel`). Every drawn answer of the gate passes `no_null` (438). `graph_negative.rs` runs the same cases without the TypeScript, and
holds each case's issue set, finding files, refusal and pointers exactly.

## Hostile input

Each answered fast, under five seconds (thirty on musl), asserting the answer as well as the time (`graph_hostile.rs`, `graph_capacity.rs`, `graph_choices.rs`, `graph_amp.rs`):
a ring, a ladder, 100,000 outs naming a hosted step, 100,000 outs to a picture lane, 100,000 from-less picture runs, 100,000 chains from a mixed lane, a module of 5,000
outputs left by 100,000 `fromPort`s, groups and assemblies of 100,000 members, names of 1 MB, 100,000 inputs on a rig of 4,096 labels, 100,000 lanes and disabled lanes,
100,000 inputs in one run over 100,000 destinations, a module name and a port name of 1 MB, a link loop under `samples/` (depth 6), a folder and an amp model a link takes out of the mounts, a Surge-sized panel stepped 100,000 times, a script of a megabyte stepped 1,000 times, a plugin id of a megabyte, 400 distinct heavy panels, a panel of 90,000 widgets, a
million assets, 50,000 shader folders, 30,000 samples under 500 rack steps, 17 levels of `extends`, an `extends` cycle, a model over 4 MiB, a nest of 300, one model stepped
100,000 times, 100,000 frozen modules in one run (the freeze budget stops the plans), a lane of a megabyte copied into every plan (paid by its bytes: past the budget there is no plan), 3,000 frozen modules named by 200 bytes (no plan, the compile's values whole, the answer capped), a document of 100,000 clips and of 100,000 tracks handed whole (`graph_handed.rs`), 100,000 clips and extents at one start, a start of `1e300`, refs of a megabyte, ten plans of 100,000 names, a curve of 200,000 points and 100,000 curves.
Capacity: 200 lanes of 8 steps (a 60-widget panel with an 8 KB script and 20 params each), 200 clips a lane and 50 curves spend 355,750 of the budget and are not capped;
twenty panels of 900 widgets and 14 KB spend 25,044. The resolve budget (entries, not bytes) stops first at about 990 distinct modules of such a panel: past it the graph is `Refused` by an error finding naming the module that crossed it, and is not capped with its topology whole.

## Risks

| risk | answer |
|---|---|
| a plan's render is stat'ed once per frozen module, through the Source | a stat is charged to the freeze budget as the compile charges it; an unreadable render is not in force |
| a link out of the mounts under a walked folder (a library's `samples/` linked into the repository) answers `Outside` on the disk Source and `Missing` from a bundle | the host mounts what a library links to (453); 2l adds the `refused` paths list of `rustCompiler.md` |
| a `.json` amp model of over 4 MiB, or not UTF-8, is not read, so a bundle holds nothing of it | none is offered; the same on both sides |
| a recorder keeps the text of a `.json` model, which a bundle replays | none |
| the sweep refuses about one project in thirteen (a wiring fault the generator makes by chance) | the refusals are compared too, and the drawn rest carry the codes |
| two codes (`outNegativeGain`, `chainDeadEnd`) cannot be made by a file | `graph_validate.rs` raises them on a graph changed in memory; the TypeScript differential never sees them |
