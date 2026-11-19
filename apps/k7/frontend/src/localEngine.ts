// localEngine.ts — what the engine gate tells the editor under it about this device's engine: whether
// somebody else took its claim, and a way to look for it again. Null outside a gate (a share).
// See ../docs/shell.md.

import { onThisMachine, reachedDirectly } from "@studio/engine-browser/probe";
import { createContext, useContext } from "react";
import type { RankedEngine } from "./types";

export interface LocalEngine {
  /** Somebody else holds the claim: the link hangs up and stays down — ../docs/shell.md, "When somebody else takes the engine". */
  detached: boolean;
  /** Look for the engine again, for a project opened while it was not running. */
  searchAgain: () => void;
}

export const LocalEngineContext = createContext<LocalEngine | null>(null);

export const useLocalEngine = (): LocalEngine | null =>
  useContext(LocalEngineContext);

/** Trusted without a click: reached directly on this machine, or registered by this device's own account (`owner`), never an unowned stranger's. */
const mine = (r: RankedEngine): boolean =>
  onThisMachine(r) && (reachedDirectly(r) || r.engine.access === "owner");

/** The engine of this device among what a probe found: one reached directly first, then one that answered, else none. */
export function localEngineOf(ranked: RankedEngine[]): RankedEngine | null {
  const here = ranked.filter(mine);
  return (
    here.find((r) => reachedDirectly(r)) ??
    here.find((r) => r.token !== null) ??
    here[0] ??
    null
  );
}
