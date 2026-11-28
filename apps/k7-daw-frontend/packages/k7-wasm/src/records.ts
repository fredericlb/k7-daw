// records.ts — a set list's lines against the workspace's audio files, answered by the Rust crate (ops/records.rs).
// A reader would get wrong that `files` crosses whole on every call: a null one is a tree not read yet, which holds nothing.

import { call } from "./abi.ts";
import type { PlaylistItem, ResolvedRecord } from "./types/index.ts";

export * from "./ops/records.ts";
export type { PlaylistItem, ResolvedRecord, TreeNode } from "./types/index.ts";

/** Every line in the playlist's own order, KEEPING the ones that name nothing: a dropped row is an invisible typo. */
export const resolveRecords = (
  items: readonly PlaylistItem[],
  files: ReadonlySet<string> | null,
): ResolvedRecord[] =>
  call({
    op: "records.resolve",
    args: { items, files: files ? [...files] : null },
  });
