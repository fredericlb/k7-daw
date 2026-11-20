# Naming things

The rationale behind `apps/k7-daw-frontend/packages/song-schema/src/instanceName.ts` — how a thing this project makes
is named after what it came from. `apps/k7-daw-frontend/packages/song/docs/trackKind.md` covers the other naming rule,
a track's `[position.][prefix.]label`.

## `<owner>_<what>_<n>`

`moduleFork`'s copy of a shared module (`bs_lead_tape-delay_1`) and a node dropped on the canvas are
the same question — "what do I call this use of that" — so there is ONE rule and both go through it.
A rule spelled in two places is two answers (`apps/k7-daw-frontend/packages/song/docs/nodes.md`).

`namePart` keeps one segment of the module vocabulary: `createNode` and `createModuleFile` both
refuse anything else, and a lane is called `vo.jo`, so it is not cosmetic. A name that survives as
nothing at all becomes `"x"`, since an empty segment would make `_1` the whole name — a file called
nothing.

`owner` is what the thing is FOR — the lane whose chain will play it — and it is dropped when there
is none: a module the project does not play yet, a node dropped on the bare canvas. `4osc_1` rather
than `_4osc_1`, which is the shape `moduleFork` already answered with.

The increment is always written, including on the first. Two reasons, and the second is the one that
matters: `vo_reverb` and `vo_reverb_2` would read as different KINDS of thing rather than as the
first and second of a series, and a name that has to be renamed to make room for a sibling is a name
that breaks every chain that spelled it.

`taken` is injected rather than read here so the file touches no disk: the service asks the project,
the browser asks the graph it is drawing.

## Renaming a use when the thing under it changes

`renamedInstance` returns null when the name is not one this convention made, and that null is the
whole point. `node replace` swaps the module a node extends, and a node still wearing
`vo_jo_reverb_1` while it extends a delay is a name that lies — but one somebody deliberately called
`wash` means `wash`, and renaming it would rewrite every chain that spells it to undo a decision.

BOTH shapes count as ours, and the one that matched is the one that comes back: a node born on the
bare canvas is `reverb_1` with no lane in it, and it keeps having none after it is spliced into a
run. Following the module it names is what was asked for; growing a prefix it never had would be a
second change nobody asked for.

The counter is NOT carried over — `instanceName` picks the first free one for the new stem.
`vo_jo_reverb_2` becoming `vo_jo_delay_1` looks like a step backwards and is not: the number counts
uses of THAT module, and there is one of the new one.

The module's NAME is what a use is called after, so two modules that clean to the same word leave
nothing to follow. Without that guard, replacing a module with itself would walk the counter up by
one every time and rewrite the chains to say nothing.
