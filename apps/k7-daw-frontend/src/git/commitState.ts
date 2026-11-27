// commitState.ts — what a Save takes: the whole tree but the files too large for a host, and whether it can be signed.
// What a reader gets wrong: a too-large file is left out of the commit and stays in the workspace; it is not a change waiting.

import { type CommitView, commitView as commitOf } from "@studio/k7-wasm/git";
import type { Identity } from "./gitApi";
import type { StatusEntry } from "./gitTypes";

export type { CommitView } from "@studio/k7-wasm/git";

/** What a Save takes: every change but the files too large for a host. */
export const commitView = (entries: StatusEntry[]): CommitView =>
  commitOf(entries);

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
