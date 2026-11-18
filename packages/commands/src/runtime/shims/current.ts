// current.ts — the one Vfs the shims read: a runtime sets it once, before any verb runs.
// What a reader gets wrong: there is one per Worker, so two runtimes in one process share it (tests make one at a time).

import { Vfs } from "../vfs.ts";

let held: Vfs | undefined;

export function setVfs(v: Vfs): void {
  held = v;
}

/** The vfs in use; until a runtime sets its own it is an empty disk, which is what a test under the aliases wants. */
export function current(): Vfs {
  held ??= new Vfs(true);
  return held;
}

/** Runs `f` with `v` as the vfs, then puts back the one that was. */
export function withVfs<T>(v: Vfs, f: () => T): T {
  const prev = current();
  setVfs(v);
  try {
    return f();
  } finally {
    setVfs(prev);
  }
}
