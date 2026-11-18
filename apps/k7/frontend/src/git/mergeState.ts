// mergeState.ts — the merge screen's choices: one per conflict, what each offers, when "Finish merge" is allowed.
// What a reader gets wrong: "both" is offered only where the route says so (a keyed list item, a binary's copy beside).

import type { Choice, Conflict } from "./gitTypes";

export interface MergeChoices {
  conflicts: Conflict[];
  chosen: Record<string, Choice>;
}

export type MergeEvent =
  | { type: "load"; conflicts: Conflict[] }
  | { type: "pick"; id: string; choice: Choice }
  | { type: "pickAll"; choice: "mine" | "theirs" };

/** The choices a conflict offers: a removed file keeps or goes; the rest take a side, and both where it is offered. */
export function offered(c: Conflict): Choice[] {
  if (c.kind === "presence") return ["keep", "remove"];
  return c.both ? ["mine", "theirs", "both"] : ["mine", "theirs"];
}

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

/** Conflicts still without a choice. */
export const remaining = (s: MergeChoices): number =>
  s.conflicts.filter((c) => !s.chosen[c.id]).length;

/** Whether "Finish merge" is allowed: every conflict has a choice. */
export const canFinish = (s: MergeChoices): boolean =>
  s.conflicts.length > 0 && remaining(s) === 0;

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
