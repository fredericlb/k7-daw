// historySource.ts — what the history panel offers its viewer, and how far a rollback to a row walks the journal.
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

/** A journal row as the rollback reads it: when it ran, and whether it is undone. */
export interface JournalRow {
  id: number;
  ts: number;
  undone: boolean;
}

/** The undos (or redos) that bring the song back to the state right after `id`: zero when it is the state already. */
export function rollbackTo(
  rows: readonly JournalRow[],
  id: number,
): { redo: boolean; steps: number } {
  const at = rows.find((r) => r.id === id);
  if (!at) return { redo: false, steps: 0 };
  // An applied row: every applied row after it is undone; an undone one: it and every undone row before it come back.
  if (!at.undone)
    return {
      redo: false,
      steps: rows.filter((r) => r.ts > at.ts && !r.undone).length,
    };
  return {
    redo: true,
    steps: rows.filter((r) => r.ts <= at.ts && r.undone).length,
  };
}
