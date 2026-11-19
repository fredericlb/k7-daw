// diagnostics.ts — the located finding the editor squiggles, and the library modules that would not read.
// What a reader gets wrong: "accepted" must never quietly mean "ignored". See docs/loader.md.

import { loadModuleRefs } from "./moduleRefs";

/** One located finding for the editor's inline squiggles; mirrored on the wire by web-shared's Diagnostic. */
export interface Diagnostic {
  file: string;
  line: number;
  col: number;
  message: string;
  severity?: "error" | "warning";
}

// The modules of the inherited libraries that would not READ — a warning, becoming an error only once named (docs/graphModel.md).
export function libraryModuleIssues(dir: string): Diagnostic[] {
  const issues: Diagnostic[] = [];
  loadModuleRefs(dir, undefined, issues);
  return issues;
}
