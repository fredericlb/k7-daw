// mergeState.ts — the merge screen's choices: one per conflict, what each offers, when "Finish merge" is allowed.
// What a reader gets wrong: "both" is offered only where the route says so (a keyed list item, a binary's copy beside).

import {
  canFinish as canFinishOf,
  offered,
  remaining as remainingOf,
} from "@studio/k7-wasm/git";
import type { Choice, Conflict } from "./gitTypes";

export interface MergeChoices {
  conflicts: Conflict[];
  chosen: Record<string, Choice>;
}

export type MergeEvent =
  | { type: "load"; conflicts: Conflict[] }
  | { type: "pick"; id: string; choice: Choice }
  | { type: "pickAll"; choice: "mine" | "theirs" };

export { offered };

export const emptyChoices: MergeChoices = { conflicts: [], chosen: {} };

export function choicesAfter(s: MergeChoices, e: MergeEvent): MergeChoices {
  switch (e.type) {
    case "load": {
      // What the device already saved stays chosen; a choice for a conflict that is gone is dropped.
      const chosen: Record<string, Choice> = {};
      for (const c of e.conflicts) {
        const kept = s.chosen[c.id] ?? c.choice;
        if (kept && offered(c).includes(kept)) chosen[c.id] = kept;
      }
      return { conflicts: e.conflicts, chosen };
    }
    case "pick": {
      const c = s.conflicts.find((x) => x.id === e.id);
      if (!c || !offered(c).includes(e.choice)) return s;
      return { ...s, chosen: { ...s.chosen, [e.id]: e.choice } };
    }
    case "pickAll": {
      // A side for every text and key conflict; a removed file or a binary still asks the person.
      const chosen = { ...s.chosen };
      for (const c of s.conflicts)
        if (c.kind !== "presence" && c.kind !== "binary")
          chosen[c.id] = e.choice;
      return { ...s, chosen };
    }
  }
}

/** The ids of the conflicts that have a choice. */
const chosenIds = (s: MergeChoices): string[] => Object.keys(s.chosen);

/** Conflicts still without a choice. */
export const remaining = (s: MergeChoices): number =>
  remainingOf(s.conflicts, chosenIds(s));

/** Whether "Finish merge" is allowed: every conflict has a choice. */
export const canFinish = (s: MergeChoices): boolean =>
  canFinishOf(s.conflicts, chosenIds(s));

/** The body of the resolve call. */
export const resolveChoices = (
  s: MergeChoices,
): { id: string; choice: Choice }[] =>
  s.conflicts.flatMap((c) => {
    const choice = s.chosen[c.id];
    return choice ? [{ id: c.id, choice }] : [];
  });

/** The conflicts of each file, in the order the route gave them. */
export function byFile(
  conflicts: Conflict[],
): { path: string; items: Conflict[] }[] {
  const out: { path: string; items: Conflict[] }[] = [];
  for (const c of conflicts) {
    const last = out.find((g) => g.path === c.path);
    if (last) last.items.push(c);
    else out.push({ path: c.path, items: [c] });
  }
  return out;
}

/** The next conflict after `from` that still has no choice (wrapping round), or null once every one has one. */
export function nextOpen(s: MergeChoices, from: string | null): string | null {
  const ids = byFile(s.conflicts).flatMap((g) => g.items.map((c) => c.id));
  const start = from ? ids.indexOf(from) + 1 : 0;
  for (let i = 0; i < ids.length; i++) {
    const id = ids[(start + i) % ids.length];
    if (!s.chosen[id]) return id;
  }
  return null;
}
