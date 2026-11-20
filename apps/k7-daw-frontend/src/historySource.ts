// historySource.ts — what the history panel offers its viewer.
// What a reader gets wrong: a failed read never turns checkpoint or restore on.

import { lockedBy } from "./git/gitState";
import type { GitState } from "./git/gitTypes";
import type { RestoreAnswer } from "./syncHistory";

export interface HistoryOffers {
  /** Naming a point and restoring to one need a workspace no merge waits on; list and diff work always. */
  write: boolean;
}

/** A workspace whose merge waits for choices takes no checkpoint and no restore. */
export function historyOffers(git: GitState | null): HistoryOffers {
  return { write: !lockedBy(git) };
}

/** The binary files a restore left as they are; a restore of texts alone names none. */
export const restoredKept = (r: RestoreAnswer): string[] => r.kept ?? [];
