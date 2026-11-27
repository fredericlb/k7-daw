// roamState.ts — the workspaces added on the person's other devices: what the list route answers and which of them
// this device offers to clone. What a reader gets wrong: only an entry with a remote can come here, since git is the only
// way a workspace's files travel; the rest of the list is kept for the day it is connected.

import {
  cloneName,
  type RoamAnswer,
  type RoamEntry,
  type RoamState,
  roamOffered,
} from "@studio/k7-wasm/git";

export type { RoamAnswer, RoamEntry, RoamState };

export type RoamEvent =
  | { type: "loaded"; answer: RoamAnswer }
  | { type: "failed" }
  | { type: "cloned"; id: string };

export const emptyRoam: RoamState = { answer: null, cloned: [] };

export function roamAfter(s: RoamState, e: RoamEvent): RoamState {
  switch (e.type) {
    case "loaded":
      // What a read says is here is no longer pending from a clone.
      return {
        answer: e.answer,
        cloned: s.cloned.filter((id) => !e.answer.here.includes(id)),
      };
    case "cloned":
      return s.cloned.includes(e.id)
        ? s
        : { ...s, cloned: [...s.cloned, e.id] };
    default:
      // A failed read keeps the last answer: the section is a convenience, never a gate.
      return s;
  }
}

/** The entries to offer: connected, not on this device, not just cloned, by name (the browser's collation). */
export const offered = (s: RoamState): RoamEntry[] =>
  roamOffered(s).sort(
    (x, y) => x.name.localeCompare(y.name) || x.id.localeCompare(y.id),
  );

export { cloneName };
