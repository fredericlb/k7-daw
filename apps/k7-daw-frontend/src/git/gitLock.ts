// gitLock.ts — what keeps the editors still while a merge waits: the state's next value after a read (it fails
// CLOSED), and the sentence a locked workspace says. The server refuses the writes. See ../../docs/git.md.
// What a reader gets wrong: a failed read never unlocks; only a good answer or a missing route does.

import { m } from "../paraglide/messages.js";
import { GitError } from "./gitApi";
import type { GitState } from "./gitTypes";

/** A read's outcome: the state it answered, or what it threw. */
export type GitRead = { state: GitState } | { error: unknown };

/** A route that does not exist on this server: the one failure that clears a state. */
const noSuchRoute = (e: unknown): boolean =>
  e instanceof GitError && (e.status === 501 || e.status === 404);

/** The state after a read; a failure keeps the last one, so a blip never unlocks the editors of a merge. */
export function afterRead(prev: GitState | null, r: GitRead): GitState | null {
  if ("state" in r) return r.state;
  return noSuchRoute(r.error) ? null : prev;
}

/** The last state seen per workspace, so a bar that remounts starts from what the lock last was. */
const seen = new Map<string, GitState>();

export const lastSeen = (workspace: string): GitState | null =>
  seen.get(workspace) ?? null;

export function remember(workspace: string, s: GitState | null): void {
  if (s) seen.set(workspace, s);
  else seen.delete(workspace);
}

/** The sentence a refused write says. */
export const lockSentence = (): string => m["git.lockedByMerge"]();
