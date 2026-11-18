// thawed.ts — the modules a person is EDITING, left live so a knob reaches a real plugin. In memory
// and per process: a state of the session, never of the song. Rationale: docs/autoFreeze.md.

import { resolve } from "node:path";

// By project dir.
const thawedByDir = new Map<string, Set<string>>();

/** Leave `modules` live in `dir`'s project until thawed again with an empty list — docs/autoFreeze.md. */
export function setThawed(dir: string, modules: Iterable<string>): void {
  const set = new Set(modules);
  if (set.size) thawedByDir.set(resolve(dir), set);
  else thawedByDir.delete(resolve(dir));
}

/** The modules left live in `dir` by `setThawed`. */
export function thawedIn(dir: string): Set<string> {
  return thawedByDir.get(resolve(dir)) ?? new Set();
}
