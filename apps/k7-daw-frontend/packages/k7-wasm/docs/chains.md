# `@studio/k7-wasm/chains`: the Chains tab's rules, from Rust

## Choices

| choice | why |
|---|---|
| The rules live in the `chains` crate (`apps/k7-daw-wasm/chains`), which depends on `serde`, `serde_json` and `indexmap` and on no other k7 crate | the page's module carries it in every host, and a native interface links the same crate; a dependency on the compiler (`song`) would put the compiler in the page's small module |
| The crate reads the page's graph through its own lenient structs (a field it does not read is ignored, a missing one defaults), not through the compiler's `Web*` types | the graph reaches the page as JSON that a test or a plugin may build half-filled, and the compiler's types only serialize |
| Each rule is one JSON op in the module every host already loads: `widget.*`, `flow.*`, `lane.*`, `bindings.*`, `place.*` | one ABI, no typed numeric export: a widget call costs about a microsecond, under what a pointer move can spare |
| A call sends what it reads and nothing else: a widget's six scale fields, a lane read's cables and step names, a binding's matching widgets only | a 1,200-step graph is 1.9 MB of JSON; sent whole, a lane read cost 5 ms and sent slim 0.26 ms, a binding join 19 ms and 1 ms |
| A control or a node is answered by its id or its index, and the facade hands the caller's own object back | the Rust side never sees a React-held object, and a callback cannot cross the ABI |
| The widget maths follows JavaScript where the two differ: a tie rounds up (`Math.round`, `toFixed`), `Number()` reads `0x10`, a NaN poisons `min` and `max` | a knob's text and a typed value must not change when the maths moves |
| A NaN answer crosses as `null` and the facade reads it back as NaN; a typed text that would be NaN answers "no number", so nothing is sent | JSON has no NaN, and sending NaN to a plugin is worse than sending nothing |
| A lane's own chains come first in a track's module row, then the others, each group in the order it was reached | the row starts where the signal does, and a rule that leans on a sort's stability gives another row on another engine |
| `liveVideoLanes` and `seedsEq` stay in TypeScript | one compares two JS maps by reference and the other scans a table the page already holds, both on every meter frame |
| The graph's build is one op, `graph.build`, which reads the whole `WebGraph` and answers the cards and cables; with `open` it folds the assemblies in the same call | a build reads every step's panel (its size, its fold, its narrow twin), so there is no slimmer graph to send; one call per document change, never per frame |
| A card carries its step's other fields as a `Carried` struct whose fields are the list, each typed in TypeScript from `WebChainStep` | "add a field" is one edit, and a field that reaches the page and stops at the node cannot happen unseen |
| The arrangements read a few fields of each card and answer only what changes: ranks, x positions, the seams, where a stack puts each card | the page already holds the nodes; sending a panel there and back would cost more than the arrangement |
| A card's panel keeps the shape the page reads: the step's own `ui` rides through untouched, and a folder dropdown lists `none` and the names in the folder's order | the `Intl.Collator` that orders them for people is the page's; the module has no ICU, so `buildGraph.ts` sorts those lists after the call |
| `insertFor` answers every cable at once (`graph.inserts`), and `jackRef` and `nodeSize` stay one call each, `nodeSize` kept per panel object | a model rebuild asks about hundreds of cables, a jack click about one, and a panel is measured once however often it is drawn |
| The layout constants (`NODE_W`, `CELL`, `TAB_COL_W`, ...) and the binds the build writes are generated into `consts.generated.ts` from the crate | the build that decides a card's size and the drawing that places its widgets read one number |

## What it costs

A call sends JSON, so a build pays for the graph twice: the page serialises it, the module reads it, builds, and the page reads the answer.
Measured in node on the golden graphs of `apps/k7-daw-backend/fixtures/golden` (medians; the TypeScript twin in brackets, run in the same process):

| graph | `buildGraph` | `moduleRow` | `stackLayout` |
|---|---|---|---|
| `shapes/mix`, 26 steps, 43 KB in and 60 KB out | 0.71 ms (0.28) | 0.40 ms (0.27) | 0.26 ms (0.26) |
| `k7-core/audio.overdrive`, 18 steps, 57 KB in and 79 KB out | 0.80 ms (0.18) | 0.48 ms (0.17) | 0.15 ms (0.34) |
| `shapes/mix` fifty times over, 1,300 steps, 2.0 MB in and 3.0 MB out | 27 ms (10) | 16 ms (9.7) | 23 ms (376) |

The build is two to four times slower than the twin it replaced: the twin shared the page's objects, the module copies them (a native
release run of the large graph takes 4.4 ms to read, 7.2 to build and 3.6 to write; the rest is the JSON on both sides). It runs once
per document change, and stays under a millisecond on a real project. `stackLayout` was quadratic in its cables and is 16 times faster
on the large graph. One `graph.inserts` call answers 1,901 cables in 2.8 ms; a `jackRef` or a `nodeSize` is under 5 microseconds.

## What stays in TypeScript

- The drawing geometry: `jackPoint`, `eventPoint`, `cablePath`, `widgetRect`, `stripWidgetRect`, `cubicSegments`, `buildCableGeom`, `smoothPath`,
  `autoTarget` (it reads a widget's rect to find where an arrow stops); they run for every cable on every frame, or on every pointer move.
- `liveVideoLanes` and `seedsEq`, read on every meter frame.
- `widgetTarget` and `moduleFieldValue`: they run once per widget on every render over the page's own node, and a call per widget would cost more than the rule.
- `graphLayout.ts` and dagre, until rust-sugiyama replaces them (M5b); `withOpenTabs`, a map over nodes the page holds.
- `chainsModel.ts`: it assembles the cable geometry above; only its `insertFor` call went to Rust.
