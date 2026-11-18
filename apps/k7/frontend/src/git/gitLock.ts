// gitLock.ts — what keeps the editors still while a merge waits: the state's next value after a read (it fails
// CLOSED), the gate every write passes, and the sentence a refused write says. See ../../docs/git.md.
// What a reader gets wrong: a failed read never unlocks; only a good answer or a missing route does.

import { setWriteGate, type WriteAsk } from "@studio/project-shell/fileApi";
import { m } from "../paraglide/messages.js";
import { GitError } from "./gitApi";
import type { GitState } from "./gitTypes";
import { classOfLine } from "./verbWrites";

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

/** The routes the server's file service refuses while a merge waits (`routes::writes`, `/upload`); every other route passes. */
const WRITING_ROUTES = new Set([
  "PUT /file",
  "POST /folder",
  "POST /rename",
  "POST /move",
  "POST /library/move",
  "POST /entry/delete",
  "POST /module/clone",
  "POST /node",
  "POST /project/create",
  "POST /module",
  "POST /undo",
  "POST /redo",
  "POST /changes",
  "POST /upload",
  "POST /verb-host/playlist-stems",
  "POST /verb-host/take/save",
  "POST /verb-host/picture",
  "POST /verb-host/extract",
  "POST /verb-host/render",
]);

/** Whether the server would refuse this file route; a route it does not name is a read. */
export function refusesFileWhileLocked(method: string, path: string): boolean {
  return WRITING_ROUTES.has(`${method.toUpperCase()} ${path.split("?")[0]}`);
}

/** Whether a command line is refused while a merge waits: a verb that writes the workspace, or one unknown. */
export function refusesWhileLocked(line: string): boolean {
  const [verb = "", second = ""] = line.trim().split(/\s+/);
  if (verb === "ui") return false;
  if (verb === "selection" && second === "copy") return false;
  return classOfLine(line) === "write";
}

export type Gate = (ask: WriteAsk) => string | null;

/** What a waiting merge refuses: the writes the server would refuse with 423, said before the request leaves. */
export const mergeGate: Gate = (ask) => {
  const refused =
    ask.kind === "command"
      ? refusesWhileLocked(ask.line)
      : refusesFileWhileLocked(ask.method, ask.path);
  return refused ? m["git.lockedByMerge"]() : null;
};

/** Installs the merge's gate while one waits (none installed otherwise). */
export function installGates(merging: boolean): () => void {
  setWriteGate(merging ? mergeGate : null);
  return () => setWriteGate(null);
}

/** The sentence a refused write says. */
export const lockSentence = (): string => m["git.lockedByMerge"]();
