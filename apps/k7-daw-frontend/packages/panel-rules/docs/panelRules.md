# The panel's rules live in Rust

What decides whether a module's panel is valid, how it is laid out and printed, and how AI organize's answer is
judged is the song crate's `modules/ui` (`apps/k7-daw-wasm/song/src/modules/ui`): groups framing what lies wholly
inside their box (`geometry.rs`), the horizontal and vertical layouts offering the same controls (`geometry.rs`), the tabs
and the clipboard (`edit.rs`), the proposal from a parameter list (`auto.rs`), a panel read from and written to its k7x text,
a widget a line (`write.rs`), the union of a module's params (`rows.rs`), and AI organize's prompt, answer and check
(`ai_prompt.rs`, `ai_answer.rs`, `ai_check.rs`). What the rules ARE is the builder's to explain:
`apps/k7-daw-frontend/packages/ui-module-builder/docs/builder.md`, `layouts.md` and `aiOrganize.md` there.

## The choices

| choice | why |
|---|---|
| the rules are written once, in Rust, and the page reaches them through `@studio/k7-wasm/panel` | the grid, the library check and the host's AI loop must agree on every rule; three implementations would have to be kept equal by hand |
| a panel travels as loose JSON, not as the loader's strict types | the grid holds widgets mid-edit (a fraction, a missing field), and a rule must still answer; `validate.panel` is the loader's reader and judges what is saved |
| every call answers a new value and edits nothing in place | the page's state is React's; a rule never holds a panel between two calls |
| the rows of a module's params come back unsorted | the page orders names with its own collator, which Rust has no copy of |
| the one TypeScript file this package keeps reads the NDJSON stream of a run (`src/organizeStream.ts`) | a stream is a browser API; the events it reads are generated from the Rust enum |
| the host reads a script's strings, comments and brackets, not its syntax | the host has no JavaScript engine; the page compiles the script for real where the panel lands (`apps/k7-daw-frontend/docs/aiOrganize.md`) |
