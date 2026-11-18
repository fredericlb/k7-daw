// gitState.ts — what the workspace bar offers, read off the route's state by one function (barView).
// What a reader gets wrong: Push is never offered over a branch that is behind, and Pull never over changes.

import type { GitState } from "./gitTypes";

/** Why a button is dimmed, said as a key of the strings. */
export type Blocked = "pullFirst" | "commitFirst" | null;

/** What the bar offers; every flag is one button, so a test reads the rules off it. */
export interface BarView {
  /** A workspace with no repository has no bar at all. */
  show: boolean;
  remote: string | null;
  branch: string;
  toPull: number;
  toPush: number;
  changed: number;
  /** Conflicts waiting for a choice (the bar then offers only the merge), or null when no merge waits. */
  merge: number | null;
  /** Commit has something to commit. */
  commit: boolean;
  /** Push has a commit to send and the branch is not behind. */
  push: boolean;
  pushBlocked: Blocked;
  /** Pull has something to bring and nothing uncommitted stands in its way. */
  pull: boolean;
  pullBlocked: Blocked;
  /** The last fetch failed: its sentence, said once by the host; null when it worked or never ran. */
  fetchError: string | null;
  /** The editors refuse every write while a merge waits. */
  readOnly: boolean;
}

const HIDDEN: BarView = {
  show: false,
  remote: null,
  branch: "",
  toPull: 0,
  toPush: 0,
  changed: 0,
  merge: null,
  commit: false,
  push: false,
  pushBlocked: null,
  pull: false,
  pullBlocked: null,
  fetchError: null,
  readOnly: false,
};

export function barView(s: GitState | null): BarView {
  if (!s || s.mode === "none") return HIDDEN;
  const toPull = s.behind ?? 0;
  const toPush = s.ahead ?? 0;
  const changed = s.changed ?? 0;
  const merging = s.merge !== undefined && s.merge !== null;
  const connected = s.mode === "connected";
  const base: BarView = {
    show: true,
    remote: connected ? (s.remote ?? null) : null,
    branch: s.branch ?? "",
    toPull,
    toPush,
    changed,
    merge: merging ? (s.merge ?? 0) : null,
    commit: false,
    push: false,
    pushBlocked: null,
    pull: false,
    pullBlocked: null,
    fetchError: s.lastFetch?.error?.error ?? null,
    readOnly: merging,
  };
  // While a merge waits only its own screen is offered: a commit would be a second history.
  if (merging) return base;
  return {
    ...base,
    commit: changed > 0,
    push: connected && toPush > 0 && toPull === 0,
    pushBlocked: connected && toPush > 0 && toPull > 0 ? "pullFirst" : null,
    pull: connected && toPull > 0 && changed === 0,
    pullBlocked: connected && toPull > 0 && changed > 0 ? "commitFirst" : null,
  };
}

/** Whether this state locks the writers of its workspace (a merge waits for choices). */
export const lockedBy = (s: GitState | null): boolean => barView(s).readOnly;

/** A remote as a person reads it: the host and the repository, never the user or the scheme (`github.com/o/r`). */
export function remoteName(url: string): string {
  const u = url.trim();
  const long = /^ssh:\/\/(?:[^@/]+@)?(\[[^\]]+\]|[^:/]+)(?::\d+)?\/(.*)$/.exec(
    u,
  );
  const scp = /^(?:[^@/\s]+@)?([^:/\s]+):(?!\/\/)(.*)$/.exec(u);
  const hit = long ?? scp;
  if (!hit) return u;
  const path = hit[2].replace(/^\/+/, "").replace(/\.git\/?$/, "");
  return path ? `${hit[1]}/${path}` : hit[1];
}

/** What the workspace menu offers on the git state: connect when there is no remote, else say it and offer to change it. */
export type MenuView =
  | { connected: false; remote: null }
  | { connected: true; remote: string };

export function menuView(s: GitState | null): MenuView {
  return s?.mode === "connected" && s.remote
    ? { connected: true, remote: remoteName(s.remote) }
    : { connected: false, remote: null };
}

/** The git lines of the workspace menu, in order; a connected workspace never offers a plain "connect". */
export const gitMenuEntries = (
  v: MenuView,
  // The demos are put back as shipped at each start, so they are never offered a remote.
  demos = false,
): ("connected" | "change" | "connect")[] =>
  demos
    ? v.connected
      ? ["connected"]
      : []
    : v.connected
      ? ["connected", "change"]
      : ["connect"];
