// read.ts — a k7x text read and a value printed, answered by the Rust crate (apps/k7-daw-wasm/k7x) through the module.
// A reader would get wrong that a text that does not read is a failed call: the op answers where it stops, thrown here.

import { call } from "../abi.ts";
import type { K7xSyntax, ReadAnswer, Seg } from "../types/index.ts";

export type K7Path = readonly (string | number)[];

export class K7SyntaxError extends Error {
  constructor(
    message: string,
    readonly offset: number,
    readonly line: number,
    readonly column: number,
  ) {
    super(`${message} (${line}:${column})`);
    this.name = "K7SyntaxError";
  }
}

export const syntaxError = (s: K7xSyntax): K7SyntaxError =>
  new K7SyntaxError(s.message, s.offset, s.line, s.column);

/** The 1-based line and column of a UTF-16 offset, the column in UTF-16 units: K7SyntaxError's convention. */
export function lineCol(
  text: string,
  offset: number,
): { line: number; col: number } {
  const before = text.slice(0, offset);
  return {
    line: before.split("\n").length,
    col: offset - before.lastIndexOf("\n"),
  };
}

/** The k7x reader's answer for one text: its data, or where it stops reading. */
export const readData = (text: string): ReadAnswer =>
  call<ReadAnswer>({ op: "k7x.data", args: { text } });

/** The plain value of a whole file; throws K7SyntaxError. */
export function parse(text: string): Record<string, unknown> {
  const a = readData(text);
  if (a.syntax) throw syntaxError(a.syntax);
  return a.data ?? {};
}

/** A plain value as JSON for the module, refusing what JSON would silently turn into null. */
export function toWire(value: unknown): unknown {
  JSON.stringify(value, (_k, v) => {
    if (typeof v === "number" && !Number.isFinite(v))
      throw new Error(`k7x has no ${v}`);
    return v;
  });
  return value;
}

/** Every path from `value` where `inline` says one line, as the printer asks them. */
function inlinePaths(
  value: unknown,
  inline: (path: K7Path, value: unknown) => boolean,
): Seg[][] {
  const out: Seg[][] = [];
  const go = (v: unknown, path: (string | number)[]) => {
    if (inline(path, v)) out.push([...path]);
    if (Array.isArray(v))
      for (let i = 0; i < v.length; i++) go(v[i], [...path, i]);
    else if (v !== null && typeof v === "object")
      for (const [k, x] of Object.entries(v))
        if (x !== undefined) go(x, [...path, k]);
  };
  go(value, []);
  return out;
}

/** What a whole file is printed with: comment lines on top, the nodes written on one line, comments above some nodes. */
export interface StringifyOptions {
  header?: readonly string[];
  inline?: (path: K7Path, value: unknown) => boolean;
  /** Comment lines printed above the node at each path. */
  lead?: readonly { path: K7Path; lines: readonly string[] }[];
}

/** Canonical text for a plain object: no comments but the header and the leads, and the `inline` nodes on one line. */
export function stringify(value: unknown, opts: StringifyOptions = {}): string {
  return call<string>({
    op: "k7x.print",
    args: {
      value: toWire(value),
      header: opts.header,
      inline: opts.inline ? inlinePaths(value, opts.inline) : undefined,
      lead: opts.lead,
    },
  });
}
