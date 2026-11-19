// roamState.ts — the workspaces added on the person's other devices: what the list route answers and which of them
// this device offers to clone. What a reader gets wrong: only an entry with a remote can come here, since git is the only
// way a workspace's files travel; the rest of the list is kept for the day it is connected.

/** One workspace of the list, as `GET /git/roam` answers it. */
export interface RoamEntry {
  id: string;
  name: string;
  /** The git ssh address, or null for a workspace that is not connected. */
  remote: string | null;
  addedAt: number;
}

/** `available` is false on a host with no list store (anything but iOS); `here` holds this device's ids. */
export interface RoamAnswer {
  available: boolean;
  entries: RoamEntry[];
  here: string[];
}

export interface RoamState {
  answer: RoamAnswer | null;
  /** Entries cloned from this section, hidden until a read says they are here. */
  cloned: readonly string[];
}

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

/** The entries to offer: connected, not on this device, not just cloned, by name. */
export function offered(s: RoamState): RoamEntry[] {
  const a = s.answer;
  if (!a?.available) return [];
  return a.entries
    .filter(
      (e) =>
        e.remote !== null && !a.here.includes(e.id) && !s.cloned.includes(e.id),
    )
    .sort((x, y) => x.name.localeCompare(y.name) || x.id.localeCompare(y.id));
}

/** The name a clone starts with: the workspace's own, and the next free `name 2` when this device has one of that name. */
export function cloneName(name: string, taken: readonly string[]): string {
  const has = (n: string) =>
    taken.some((t) => t.toLowerCase() === n.toLowerCase());
  if (!has(name)) return name;
  for (let i = 2; ; i += 1) if (!has(`${name} ${i}`)) return `${name} ${i}`;
}
