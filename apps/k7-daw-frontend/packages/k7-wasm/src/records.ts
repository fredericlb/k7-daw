// records.ts — a set list's lines against the workspace's audio files, answered by the Rust crate (ops/records.rs).
// A reader would get wrong that `files` crosses whole on every call: a null one is a tree not read yet, which holds nothing.

import { call } from "./abi.ts";
import type { PlaylistItem, ResolvedRecord, TreeNode } from "./types/index.ts";

export type { PlaylistItem, ResolvedRecord, TreeNode };

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `records.${op}`, args });

/** A line as the workspace path of its record, its slashes trimmed; null for one that leaves the workspace or names a library (`@`). */
export const recordPathOf = (line: string): string | null =>
  ask("path_of", { line });

/** What a row is called: the file's name without its extension. */
export const recordName = (path: string): string => ask("name", { path });

/** …and the folder holding it, or null for a record at the root. */
export const recordFolder = (path: string): string | null =>
  ask("folder", { path });

/** Does this record match what was typed? Every word must be somewhere in its path. */
export const recordMatches = (path: string, query: string): boolean =>
  ask("matches", { path, query });

/** Every line in the playlist's own order, KEEPING the ones that name nothing: a dropped row is an invisible typo. */
export const resolveRecords = (
  items: readonly PlaylistItem[],
  files: ReadonlySet<string> | null,
): ResolvedRecord[] =>
  ask("resolve", { items, files: files ? [...files] : null });

/** A tree's audio files in tree order; a `stems/` folder's are left out unless asked, being no record. */
export const audioFilesOf = (
  tree: readonly TreeNode[],
  withStems: boolean,
): string[] => ask("audio_files_of", { tree, withStems });
