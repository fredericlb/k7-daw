// git.ts — the page's git rules from Rust: the bar's offers, a save's files, a merge's choices, an address's host and name.
// A reader would get wrong that none runs per frame: each is one JSON call when a state arrives, a field changes or a card draws.

import { call } from "./abi.ts";
import type {
  Ago,
  Arranged,
  BarView,
  CommitView,
  Conflict,
  GithubRepo,
  GitState,
  MenuView,
  MergeChoice,
  PortAnswer,
  RoamEntry,
  RoamState,
  SshHost,
  StatusEntry,
} from "./types/index.ts";

export type {
  Ago,
  AgoUnit,
  Arranged,
  Autosave,
  BarAutosave,
  BarView,
  Blocked,
  CommitInfo,
  CommitView,
  Conflict,
  ConflictKind,
  FetchError,
  FileChange,
  GithubRepo,
  GitMode,
  GitState,
  LastFetch,
  MenuView,
  MergeChoice,
  RoamAnswer,
  RoamEntry,
  RoamState,
  SshHost,
  StatusEntry,
} from "./types/index.ts";

/** What the workspace bar offers on a state; a missing state, or a workspace with no repository, has no bar. */
export const barView = (state: GitState | null): BarView =>
  call<BarView>({ op: "git.bar", args: { state } });

/** What the workspace menu offers: connect with no remote, else the remote's name. */
export const menuView = (state: GitState | null): MenuView =>
  call<MenuView>({ op: "git.menu", args: { state } });

/** A remote as a person reads it: the host and the repository, never the user or the scheme. */
export const remoteName = (url: string): string =>
  call<string>({ op: "git.remoteName", args: { url } });

/** What a Save takes of a status: the files but the ones too large for a host. */
export const commitView = (entries: StatusEntry[]): CommitView =>
  call<CommitView>({ op: "git.commitView", args: { entries } });

/** The host and port an ssh address names, or null. */
export const sshHost = (url: string): SshHost | null =>
  call<SshHost | null>({ op: "git.sshHost", args: { url } });

/** Whether the address is a web one (`https`, `http`, `git`), which k7 never takes. */
export const isWebAddress = (url: string): boolean =>
  call<boolean>({ op: "git.isWebAddress", args: { url } });

/** The repository's own name in an address, or "". */
export const nameFromUrl = (url: string): string =>
  call<string>({ op: "git.nameFromUrl", args: { url } });

/** The name a clone starts with: its own, or the next free `name 2`. */
export const cloneName = (name: string, taken: readonly string[]): string =>
  call<string>({ op: "git.cloneName", args: { name, taken } });

/** A server's version without its build metadata. */
export const shortVersion = (version: string): string =>
  call<string>({ op: "git.shortVersion", args: { version } });

/** The ssh port field: undefined is the default, a whole port, or "bad". */
export function sshPort(typed: string): number | undefined | "bad" {
  const a = call<PortAnswer>({ op: "git.sshPort", args: { typed } });
  if (a.kind === "port") return a.port;
  return a.kind === "bad" ? "bad" : undefined;
}

/** The choices a conflict offers. */
export const offered = (conflict: Conflict): MergeChoice[] =>
  call<MergeChoice[]>({ op: "git.offered", args: { conflict } });

/** Conflicts still without a choice, given the ids that have one. */
export const remaining = (conflicts: Conflict[], chosen: string[]): number =>
  call<number>({ op: "git.remaining", args: { conflicts, chosen } });

/** Whether "Finish merge" is allowed: every conflict has a choice. */
export const canFinish = (conflicts: Conflict[], chosen: string[]): boolean =>
  call<boolean>({ op: "git.canFinish", args: { conflicts, chosen } });

/** The picker's two lists: k7 workspaces first, each most recently pushed first. */
export const arrange = (repos: GithubRepo[], filter: string): Arranged =>
  call<Arranged>({ op: "git.arrange", args: { repos, filter } });

/** How long ago a push was, in the largest whole unit. */
export const pushedAgo = (iso: string | null, now: number): Ago =>
  call<Ago>({ op: "git.pushedAgo", args: { iso, now } });

/** The roam entries to offer, in the list's order: the caller sorts them by name. */
export const roamOffered = (state: RoamState): RoamEntry[] =>
  call<RoamEntry[]>({ op: "git.roamOffered", args: { state } });

/** The moment a Save is recorded under, `dd/mm/yyyy hh:mm` on the caller's clock. */
export const saveWhen = (at: Date): string =>
  call<string>({
    op: "git.saveWhen",
    args: { epochMs: at.getTime(), offsetMin: -at.getTimezoneOffset() },
  });
