// useMergingWorkspaces.ts — which of the account's workspaces wait for a merge, for the home page's writes.
// A failed read keeps the last answer (the lock fails closed), as the bar's does; see gitLock.ts.

import { useEffect, useMemo, useRef, useState } from "react";
import { pollWhileVisible } from "../visiblePoll";
import { type GitTransport, git } from "./gitApi";
import { afterRead } from "./gitLock";
import { lockedBy } from "./gitState";
import type { GitState } from "./gitTypes";

const POLL_MS = 15_000;

/** One round of reads: each workspace's state after its answer, the previous one kept when a read throws. */
export async function readRound(
  t: GitTransport,
  names: string[],
  prev: ReadonlyMap<string, GitState>,
): Promise<Map<string, GitState>> {
  const states = new Map<string, GitState>();
  await Promise.all(
    names.map(async (n) => {
      const before = prev.get(n) ?? null;
      const after = await git.state(t, n).then(
        (state) => afterRead(before, { state }),
        (error) => afterRead(before, { error }),
      );
      if (after) states.set(n, after);
    }),
  );
  return states;
}

export function useMergingWorkspaces(t: GitTransport | null, names: string[]) {
  const [states, setStates] = useState<ReadonlyMap<string, GitState>>(
    new Map(),
  );
  const last = useRef(states);
  last.current = states;
  const key = names.join("\n");
  useEffect(() => {
    if (!t || !key) return;
    let live = true;
    const read = async () => {
      const next = await readRound(t, key.split("\n"), last.current);
      if (live) setStates(next);
    };
    void read();
    const stop = pollWhileVisible(read, POLL_MS);
    return () => {
      live = false;
      stop();
    };
  }, [t, key]);
  return useMemo(
    () => ({ merging: (name: string) => lockedBy(states.get(name) ?? null) }),
    [states],
  );
}
