// commitState.ts — what the Commit dialog may do: its file rows, the refusals before any call, the size a row shows.
// What a reader gets wrong: a commit takes the whole tree except the files too large for a host, which it leaves out.

import type { Identity } from "./gitApi";
import type { StatusEntry } from "./gitTypes";

/** Why Commit is dimmed, as a key of the strings, or null. */
type CommitBlock = "noChanges" | "noMessage" | null;

export interface CommitView {
  rows: StatusEntry[];
  /** The files over the host's limit: sent as the commit's exclusion and named in a line. */
  leftOut: StatusEntry[];
  /** How many rows go through LFS. */
  lfs: number;
  block: CommitBlock;
}

export function commitView(
  entries: StatusEntry[],
  message: string,
): CommitView {
  const leftOut = entries.filter((e) => e.tooLarge);
  const block: CommitBlock =
    entries.length === leftOut.length
      ? "noChanges"
      : message.trim() === ""
        ? "noMessage"
        : null;
  return {
    rows: entries,
    leftOut,
    lfs: entries.filter((e) => e.lfs).length,
    block,
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

/** What to store as this device's identity: nothing while the fields are still what the route prefilled. */
export const identityToSave = (
  loaded: Identity | null,
  now: Identity,
): Identity | null =>
  loaded && loaded.name === now.name && loaded.email === now.email ? null : now;

/** Where the one act stands: the commit, then the push, and the way each can stop. */
export type CommitPhase =
  | { kind: "ready" }
  | { kind: "running"; step: "commit" | "push" }
  | { kind: "failed"; step: "commit" | "push"; error: string }
  | { kind: "pullFirst" }
  | { kind: "done" };

export interface StepRow {
  id: "commit" | "push";
  state: "pending" | "active" | "done" | "warn" | "error";
  /** The sentence under a warn or error step. */
  detail?: string;
}

/** The rows of the step list: no commit row when only a push is left, no push row on a workspace with no remote. */
export function stepRows(
  phase: CommitPhase,
  plan: { commit: boolean; push: boolean },
): StepRow[] {
  const ids: StepRow["id"][] = [
    ...(plan.commit ? (["commit"] as const) : []),
    ...(plan.push ? (["push"] as const) : []),
  ];
  const at = (id: StepRow["id"]) => ids.indexOf(id);
  return ids.map((id) => {
    const i = at(id);
    switch (phase.kind) {
      case "ready":
        return { id, state: "pending" };
      case "done":
        return { id, state: "done" };
      case "running":
        return {
          id,
          state:
            i < at(phase.step)
              ? "done"
              : i === at(phase.step)
                ? "active"
                : "pending",
        };
      case "failed":
        return i < at(phase.step)
          ? { id, state: "done" }
          : i === at(phase.step)
            ? { id, state: "error", detail: phase.error }
            : { id, state: "pending" };
      default:
        return id === "commit" ? { id, state: "done" } : { id, state: "warn" };
    }
  });
}
