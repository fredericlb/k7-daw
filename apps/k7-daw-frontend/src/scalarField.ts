// scalarField.ts — one string field read out of a project file's text by a scan, half-typed files included.
// Apart from projectPaths.ts, which the home page imports, so the k7x reader it unquotes with is reached only in the editor.

import { parse } from "@studio/k7-wasm/k7x";

/** A string field's value, a call's `key("v")` or an object's `key: "v"`, found by a scan so a half-typed file still answers. */
export function readScalarField(text: string, key: string): string | null {
  const m = new RegExp(
    `(?:^|[^A-Za-z0-9_$-])${key}\\s*[(:]\\s*("(?:[^"\\\\\\n]|\\\\.)*"|'(?:[^'\\\\\\n]|\\\\.)*')`,
  ).exec(text);
  if (!m) return null;
  try {
    const value = parse(`v(${m[1]})`).v;
    return typeof value === "string" ? value || null : null;
  } catch {
    return null;
  }
}
