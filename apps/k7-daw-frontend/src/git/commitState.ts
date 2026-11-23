// commitState.ts — what a Save takes: the whole tree but the files too large for a host, and whether it can be signed.
// What a reader gets wrong: a too-large file is left out of the commit and stays in the workspace; it is not a change waiting.

import type { Identity } from "./gitApi";
import type { StatusEntry } from "./gitTypes";

export interface CommitView {
  /** The changes the commit takes. */
  sent: StatusEntry[];
  /** The files over the host's limit: sent as the commit's exclusion and named in a toast. */
  leftOut: StatusEntry[];
  /** How many of the sent go through LFS. */
  lfs: number;
}

export function commitView(entries: StatusEntry[]): CommitView {
  const sent = entries.filter((e) => !e.tooLarge);
  return {
    sent,
    leftOut: entries.filter((e) => e.tooLarge),
    lfs: sent.filter((e) => e.lfs).length,
  };
}

/** A size as a person reads it: 1.5 MB, 800 B. */
export function formatSize(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  const units = ["kB", "MB", "GB", "TB"];
  let v = bytes;
  let i = -1;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v >= 100 ? Math.round(v) : Math.round(v * 10) / 10} ${units[i]}`;
}

/** Whether a commit can be signed: git needs a name and an email. */
export const identityReady = (id: Identity | null): boolean =>
  Boolean(id?.name.trim() && id.email.trim());
