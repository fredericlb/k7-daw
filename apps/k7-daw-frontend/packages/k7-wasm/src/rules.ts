// rules.ts — what a file's own text states, answered by the Rust crate (ops/rules.rs): a sequence's window and a project's tags.
// A reader would get wrong that a half-typed text throws: it answers null or an empty list, since this runs on every keystroke.

import { call } from "./abi.ts";
import type { SeqWindow, WindowSpan } from "./types/index.ts";

export type { SeqWindow, WindowSpan };

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `rules.${op}`, args });

/** The window `text` states in beats, or null when it states none: a half-typed edge reads as absent, not as 0. */
export const seqWindowFromText = (
  text: string,
  num = 4,
  den = 4,
): SeqWindow | null => ask("seq_window", { text, num, den });

/** Both edges as beats against the notes' extent; null when the span does not run forwards. */
export const windowSpan = (win: SeqWindow, extent: number): WindowSpan | null =>
  ask("window_span", { win, extent });

/** The tags of a project.k7x's text; none for a file that does not parse, since a listing must not fail on one. */
export const projectTagsOf = (text: string): string[] =>
  ask("project_tags", { text });
