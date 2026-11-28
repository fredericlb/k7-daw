// validate.ts — can the compiler read this text? The Rust reader that decodes a file for the compile judges the editor's buffer.
// A reader would get wrong that an issue holds the reader's FIRST finding only, its columns in UTF-16 units as CodeMirror counts.

import { validateFile } from "./ops/file.ts";
import type { Validity } from "./types/index.ts";

export type { ModuleIssue, Validity } from "./types/index.ts";
export { validateFile };

/** Can the compiler read this text as a module's panel file, a `ui.k7x` beside its module? */
export const validatePanel = (text: string): Validity =>
  validateFile("modules/audio/x/ui.k7x", text);

/** The first thing wrong, phrased for a status line. Null when nothing is. */
export function firstIssue(v: Validity): string | null {
  const i = v.issues[0];
  if (!i) return null;
  const where = i.line ? `l.${i.line}${i.col ? `:${i.col}` : ""} ` : "";
  return `${where}${i.path ? `${i.path}: ` : ""}${i.message}`;
}
