// useRoamList.ts — the list of workspaces from the person's other devices, read when the home page opens, every 20 s
// while it is visible, and after a clone; nothing without the file service or on a host with no list.

import { useCallback, useEffect, useReducer } from "react";
import { pollWhileVisible } from "../visiblePoll";
import type { GitTransport } from "./gitApi";
import { git } from "./gitApi";
import { emptyRoam, offered, type RoamEntry, roamAfter } from "./roamState";

const EVERY_MS = 20_000;

export function useRoamList(t: GitTransport | null): {
  entries: RoamEntry[];
  cloned: (id: string) => void;
  reload: () => void;
} {
  const [state, send] = useReducer(roamAfter, emptyRoam);
  const read = useCallback(async () => {
    if (!t) return;
    try {
      send({ type: "loaded", answer: await git.roam(t) });
    } catch {
      send({ type: "failed" });
    }
  }, [t]);
  useEffect(() => {
    if (!t) return;
    void read();
    return pollWhileVisible(read, EVERY_MS);
  }, [t, read]);
  return {
    entries: offered(state),
    cloned: (id) => send({ type: "cloned", id }),
    reload: () => void read(),
  };
}
