// headphonesHint.ts — when a live input becomes AUDIBLE (monitored, and given a channel), say once per project that the speakers feed it back.
// Said at the gesture or on opening such a project, never refused; the two tests are Rust's (song/src/reads/graph.rs). See ../docs/shell.md.

import type { WebGraph } from "@studio/graph-model";
import {
  hasAudibleInput as audible,
  givesChannel as gives,
} from "@studio/k7-wasm/reads";

/** A line that hands a live input a channel — the node card's pick. */
export const givesChannel = (line: string): boolean => gives(line);

/** Whether the graph holds an input that is monitored AND reaches a channel — the state that can feed back. */
export const hasAudibleInput = (g: Pick<WebGraph, "chains">): boolean =>
  audible(g);

const KEY = "k7.headphonesHint";

/** True the first time for `project` in this browser, false after — storage that fails says true, since the hint is cheap. */
export function firstTimeFor(project: string): boolean {
  try {
    const seen = JSON.parse(localStorage.getItem(KEY) ?? "[]") as string[];
    if (seen.includes(project)) return false;
    localStorage.setItem(KEY, JSON.stringify([...seen, project].slice(-50)));
  } catch {
    // Private mode or blocked storage: show it, once per load at worst.
  }
  return true;
}
