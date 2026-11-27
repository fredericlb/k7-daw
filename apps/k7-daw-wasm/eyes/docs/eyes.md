# eyes — what the compositor draws, as Rust

`ui-eyes` is the page's WebGL compositor. Its input, and every rule that decides what it draws,
are in this crate; it keeps the drawing.

## Choices

| choice | why |
|---|---|
| the scene's types are Rust structs, written out to TypeScript by ts-rs into `k7-wasm`'s `types/` | the server builds the scene (`verbs`) and the page reads it: one definition, so a field cannot be added to one side |
| which shots a layer shows when is a **schedule**, a table of spans worked out once per scene and carried in each layer; the page looks `t` up in it | the rule runs per frame per layer, where a call into the module buys nothing; a table is read, not computed |
| a span holds the SHAPE of a frame (which pictures, which program, which stretches); a blend's progress is a **ramp** (start, length, rising) the page evaluates at `t` | the shape changes only at a handful of instants, the numbers every frame; a ramp is the whole difference |
| every instant at which the shape can change is a breakpoint taken from the clips (starts, ends, the two ends of each fade's window), and the shape is read at each | the rule compares `t` only against those, so one read per interval says it all |
| the two built-in transitions travel in the scene's `transitions` map, a song's own module of the same name replacing one | the compositor holds one map and no built-in of its own |
| a `.cube` is read here and its floats are handed to the page as a view of the module's memory, not as JSON | a 65³ LUT is over a million numbers, uploaded once to a texture |
| the grip drag is a typed numeric export, not a JSON op | it runs on every pointer move (`docs/rust-ts-inventory.md`, choices) |
| which lens a label names is the rule of `cameras.rs`; the hub's rule for remote cameras stays in `k7-front` with its own vectors | the two answer different questions: devices the browser lists, cameras the account has on |
| hit-testing a click against layers stays TypeScript | it reads what is on screen, and a native interface has its own |

## What stays out

Per-frame drawing maths (springs, glides, a keyframe's value at a time) is TypeScript: a document
drives it, and Rust gives it a schedule where it can (decision 12 of the inventory).
