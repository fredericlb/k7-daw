// localEngine.ts — what the engine gate tells the editor under it about this device's engine: whether
// somebody else took its claim, and a way to look for it again. Null outside a gate (a share).
// See ../docs/shell.md.

import { onThisMachine } from "@studio/engine-browser/probe";
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

/** The engine on this machine among what a probe found: one that answered first, else one that did not. */
export function localEngineOf(ranked: RankedEngine[]): RankedEngine | null {
  const here = ranked.filter(onThisMachine);
  return here.find((r) => r.token !== null) ?? here[0] ?? null;
}
