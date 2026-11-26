# Rust first: TypeScript draws

k7's logic lives in Rust; its TypeScript is the page that draws it.

## Choices

| choice | why |
|---|---|
| a calculation, a rule, a file format, a wire protocol or a gesture's arithmetic is written in a Rust crate, and the page reaches it through the WebAssembly module (`apps/k7-daw-wasm/k7-wasm`) or a backend route | one implementation serves the page today and a native interface later; a TypeScript twin of a Rust function is two copies that drift |
| a TypeScript type that mirrors a Rust struct is generated from it with ts-rs, never written by hand | the wire and the files have one source; a hand-written mirror is checked by nothing |
| TypeScript keeps what only a browser does: React components, the UI kit (`@studio/ui`), CodeMirror, WebGL, the browser's WebSocket, audio and MIDI APIs, and the state that says what is on screen | those have no Rust side to share, and they are what a native interface would replace anyway |
| a TypeScript module that holds pure logic is moved to Rust when it is next changed, its tests with it | the footprint shrinks with the work already being done, not in a separate rewrite |
| the logic already in TypeScript leaves by the milestones of `rust-ts-inventory.md`, which gives every module its verdict | a move by clusters on disjoint files lets builders work in parallel, and the verdicts say what stays for good |
