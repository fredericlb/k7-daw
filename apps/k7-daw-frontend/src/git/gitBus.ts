// gitBus.ts — "this workspace's git state moved": the one word the menu, the home row and the tree's bar share,
// so a connect made in a menu shows on the row at once. What a reader gets wrong: it carries no state, only a nudge to read.

type Listener = () => void;

const listeners = new Map<string, Set<Listener>>();

/** Calls `fn` on each announcement for `workspace`; returns the way to stop. */
export function onGitMoved(workspace: string, fn: Listener): () => void {
  const set = listeners.get(workspace) ?? new Set<Listener>();
  set.add(fn);
  listeners.set(workspace, set);
  return () => {
    set.delete(fn);
    if (!set.size) listeners.delete(workspace);
  };
}

export function announceGitMoved(workspace: string): void {
  for (const fn of [...(listeners.get(workspace) ?? [])]) fn();
}
