// git.ts — the page's git rules from Rust: the bar's offers, a save's files, a merge's choices, an address's host and name.
// A reader would get wrong that none runs per frame: each is one JSON call when a state arrives, a field changes or a card draws.

import { call } from "./abi.ts";
import type { PortAnswer } from "./types/index.ts";

export * from "./ops/git.ts";
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
  DeviceCode,
  FetchError,
  FileChange,
  ForgeAccount,
  ForgeKind,
  ForgeProbe,
  ForgeStart,
  ForgeStatus,
  GithubRepo,
  GitMode,
  GitState,
  LastFetch,
  MenuView,
  MergeChoice,
  PollAnswer,
  PullState,
  RoamAnswer,
  RoamEntry,
  RoamState,
  SshHost,
  StatusEntry,
} from "./types/index.ts";

/** The ssh port field: undefined is the default, a whole port, or "bad". */
export function sshPort(typed: string): number | undefined | "bad" {
  const a = call<PortAnswer>({ op: "git.sshPort", args: { typed } });
  if (a.kind === "port") return a.port;
  return a.kind === "bad" ? "bad" : undefined;
}

/** The moment a Save is recorded under, `dd/mm/yyyy hh:mm` on the caller's clock. */
export const saveWhen = (at: Date): string =>
  call<string>({
    op: "git.saveWhen",
    args: { epochMs: at.getTime(), offsetMin: -at.getTimezoneOffset() },
  });
