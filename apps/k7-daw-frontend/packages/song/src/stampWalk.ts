// stampWalk.ts — the module stamp's walk: every entry's name mixed in order, and the newest mtime
// among the files that define a module. One thread, one syscall at a time, over the disk the
// process sees (the virtual one in a browser). See docs/moduleCache.md.

import { type Dirent, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

/** A file whose CONTENT says what a module is. Everything else is named in the stamp, not stat'd. */
const DEFINITION = /\.(k7x|ts|js|vstpreset)$/i;

/** `<type>/<category>/<module>/assets/<sub>` is the deepest real tree; also a floor under a link loop. */
const MAX_DEPTH = 6;

/** What a walk adds up to: the FNV-1a hash of the names, how many there were, the newest mtime. */
export interface Walked {
  hash: number;
  entries: number;
  newest: number;
}

/** The walk on this thread, one syscall at a time. */
export function walkInline(
  roots: readonly string[],
  files: readonly string[],
): Walked {
  let hash = 2166136261;
  let entries = 0;
  let newest = 0;
  const mix = (s: string): void => {
    for (let i = 0; i < s.length; i++) {
      hash ^= s.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
  };
  const stamp = (path: string): void => {
    try {
      const m = statSync(path).mtimeMs;
      if (m > newest) newest = m;
    } catch {
      /* raced away between readdir and stat — the next call sees it */
    }
  };
  const walk = (abs: string, depth: number): void => {
    let found: Dirent[];
    try {
      found = readdirSync(abs, { withFileTypes: true });
    } catch {
      return; // not there, or unreadable — the loaders treat it as empty too
    }
    for (const e of found) {
      if (e.name.startsWith(".")) continue;
      entries++;
      mix(e.name);
      if (e.isDirectory()) {
        if (depth < MAX_DEPTH) walk(join(abs, e.name), depth + 1);
        continue;
      }
      if (DEFINITION.test(e.name)) stamp(join(abs, e.name));
    }
  };
  for (const root of roots) walk(root, 0);
  for (const file of files) stamp(file);
  return { hash, entries, newest };
}

/** The stamp walk over `roots`, plus the mtime of each of `files`. */
export function walkStamp(
  roots: readonly string[],
  files: readonly string[],
): Walked {
  return walkInline(roots, files);
}
