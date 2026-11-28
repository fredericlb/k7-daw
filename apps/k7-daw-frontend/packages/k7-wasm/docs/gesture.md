# `@studio/k7-wasm/gesture`: the page's gesture arithmetic, from Rust

## Choices

| choice | why |
|---|---|
| The arithmetic lives in the `gesture` crate (`apps/k7-daw-wasm/gesture`): the grid snap, the piano roll's drags and fit, the grid's caret, the clip panel's window, a take's counter, a picture drag, the edge drags, the tools' lines, the rulers' ticks and the anchored zoom | one implementation serves the page today and a native interface later; a TypeScript twin of a rule is two copies that drift |
| What runs on every pointer move is a typed call: numbers in, a number or a few back, no JSON (`typed*.rs`, `gesture*.ts`) | the JSON ABI parses a request per call; a few numbers cross for tens of nanoseconds, and the page already spends microseconds on a move |
| What runs once per release, click or scroll is a JSON op (`gesture.plan_group_move`, `gesture.warp_line`, `gesture.tool_line`, `gesture.picture_tool_line`, `gesture.crossfade_pair`, `gesture.ruler_ticks`) | these answer a structure or a text, and a few microseconds once per gesture is invisible |
| A typed answer comes back by shape: one number as the return, a few in the scalars, a list in the f64 output, a text in the output bytes; an array argument is written into the module's input buffer first | the module hands out memory, never a pointer the page keeps: any call may grow it |
| A number a caller may leave out crosses as NaN, and an answer that has no value is NaN | a typed call has no `null`; the wrappers read it back as `undefined` and `null` |
| Every function answers what JavaScript's would, down to `Math.round`'s half-up on a tie, the sign of zero and `toFixed`'s ties (`js.rs`) | a drag that lands a pixel off its old place is a regression nobody can see; the differential against the old code ran on random inputs, and the Rust tests hold the same cases |
| The tools' notes are a message id and its parameters; the page turns them into words with paraglide (`timelineTools.ts`) | the rule is Rust's, the words are the page's |
| Per-frame drawing maths (the playhead's glide, the meters, the needle) stays in TypeScript | a call per frame buys nothing |
| A tick list is copied in for each call and never held | the lists are tens of numbers, and a held view dies with the next call |

## What stays in TypeScript

| module | why |
|---|---|
| React event handling, DOM measuring, refs and the drawing | a browser has them and Rust does not |
| `DRAG_SLOP` (the 4 px that tell a click from a drag) | an interaction threshold of the page |
| The tools' names and labels (`TOOLS`, `TOOL_LABEL`) | the strip's tooltips are words, and `Tool` is generated from the Rust enum so a missing label fails the type check |
| `gridCaret.ts`: the handle the shell drives (`ClipGridHandle`, `PasteResult`) | commands over React state, not arithmetic |
| The typed calls' wrappers (`gesture*.ts`), written by hand while the JSON ops' are generated | an `extern "C"` signature is not visible to the bindings binary without parsing the source, and most wrappers reshape: a code for a mode, NaN for none, an answer read from the scalars or the f64 output, an array written into the input first |
