// gitTypes.ts — the shapes the git routes answer (apps/k7-daw-backend/docs/files/git.md).

export interface GitState {
  mode: "none" | "local" | "connected";
  branch?: string;
  remote?: string | null;
  user?: string | null;
  ahead?: number;
  behind?: number;
  changed?: number;
  /** Conflicts still without a choice; absent when no merge waits. */
  merge?: number | null;
  /** True when connected, a fetch worked, and the remote has no branch yet: nobody has pushed. */
  remoteEmpty?: boolean;
  lastFetch?: {
    /** Milliseconds since the epoch, like Date.now(). */
    at: number;
    error?: { code: string; error: string } | null;
  } | null;
}

export interface StatusEntry {
  path: string;
  change: "added" | "modified" | "removed" | "renamed";
  from?: string;
  size: number;
  binary: boolean;
  lfs: boolean;
  /** Over what a git host takes: a commit leaves it out and the status keeps naming it. */
  tooLarge?: boolean;
}

export interface CommitInfo {
  id: string;
  subject: string;
  author: string;
  /** Seconds since the epoch. */
  time: number;
  device: string | null;
  files_changed: number;
}

export type PullState =
  | "up-to-date"
  | "fast-forward"
  | "merged"
  | "conflicts"
  | "empty-remote";

export type Choice = "mine" | "theirs" | "both" | "keep" | "remove";

export interface Conflict {
  id: string;
  path: string;
  kind: "k7x" | "text" | "binary" | "presence";
  keyPath?: string;
  line?: number;
  base: string | null;
  mine: string | null;
  theirs: string | null;
  /** Whether "both" is offered. */
  both: boolean;
  choice: Choice | null;
}

export type MergeView =
  | { pending: false }
  | { pending: true; theirs: string; conflicts: Conflict[] };
