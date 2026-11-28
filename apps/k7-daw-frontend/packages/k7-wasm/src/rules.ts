// rules.ts — what a file's own text states, answered by the Rust crate (ops/rules.rs): a sequence's window and a project's tags.
// A reader would get wrong that a half-typed text throws: it answers null or an empty list, since this runs on every keystroke.

export * from "./ops/rules.ts";
export type { SeqWindow, WindowSpan } from "./types/index.ts";
