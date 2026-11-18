// logState.ts — the Commits list: pages of the log appended by their cursor, a refresh that restarts at the head.
// What a reader gets wrong: a page shorter than asked is the end of the log, not an error.

import type { CommitInfo } from "./gitTypes";

export const PAGE = 30;

export interface LogList {
  commits: CommitInfo[];
  /** The log has no older page. */
  end: boolean;
}

export type LogEvent =
  | { type: "first"; commits: CommitInfo[] }
  | { type: "more"; commits: CommitInfo[] };

export const emptyLog: LogList = { commits: [], end: false };

export function logAfter(s: LogList, e: LogEvent): LogList {
  const end = e.commits.length < PAGE;
  if (e.type === "first") return { commits: e.commits, end };
  // A page that overlaps what is held (a push moved the head meanwhile) adds only what is new.
  const have = new Set(s.commits.map((c) => c.id));
  return {
    commits: [...s.commits, ...e.commits.filter((c) => !have.has(c.id))],
    end,
  };
}

/** The cursor for the next page: the id of the oldest commit read, or undefined at the start. */
export const cursor = (s: LogList): string | undefined => s.commits.at(-1)?.id;
