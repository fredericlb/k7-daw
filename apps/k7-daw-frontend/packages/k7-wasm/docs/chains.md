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

## What stays in TypeScript

The cables' drawing geometry (`cubicSegments`, `buildCableGeom`, `smoothPath`), the sockets' points
and `cablePath`, which the canvas reads on every frame; the layout (`graphLayout.ts`, dagre) until the
layout crate replaces it; `buildGraph` with its measure steps (`geometry.ts`, `gridFit.ts`, `moduleRow.ts`,
`rowSeams.ts`, `stackLayout.ts`) and its cable rules `insertFor`, `jackRef` and `autoTarget`, which read the built nodes.
