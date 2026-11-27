// validate.ts — can the compiler read this text? The Rust reader that decodes a file for the compile judges the editor's buffer.
// A reader would get wrong that an issue holds the reader's FIRST finding only, its columns in UTF-16 units as CodeMirror counts.

import { call } from "./abi.ts";
import type { ModuleIssue, Validity } from "./types/index.ts";

export type { ModuleIssue, Validity };

/** Is `text` a document the compiler could read at `path`? A newer format first, then syntax, then the schema. */
export const validateFile = (path: string, text: string): Validity =>
  call<Validity>({ op: "file.validate", args: { path, text } });

/** The first thing wrong, phrased for a status line. Null when nothing is. */
export function firstIssue(v: Validity): string | null {
  const i = v.issues[0];
  if (!i) return null;
  const where = i.line ? `l.${i.line}${i.col ? `:${i.col}` : ""} ` : "";
  return `${where}${i.path ? `${i.path}: ` : ""}${i.message}`;
}
