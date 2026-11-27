// records.ts — a set list's lines against the workspace's audio files: the file each names, whether it
// is there with its four stems, the names a row shows and the drag a row carries. Pure, since k7's
// playlist pane edits the lines and the desk loads one: two copies would drift. See docs/playlists.md.
// What a reader gets wrong: a record's stems are `stems/` beside it, so one record per folder has its own.

import { isAudio } from "@studio/k7-wasm/schema";
import type { PlaylistItem } from "@studio/k7-wasm/types";
import { plainSegments } from "@studio/web-protocol/paths";
import { extOf } from "./audioFiles";
import { STEMS, STEMS_DIR, stemFile } from "./stems";

/** A line as the workspace path of its record, its slashes trimmed; null for one that leaves the workspace or names a library (`@`). */
export function recordPathOf(line: string): string | null {
  const clean = line.trim().replace(/^\/+|\/+$/g, "");
  if (!clean || clean.includes("\\") || clean.startsWith("@")) return null;
  const segs = plainSegments(clean);
  return segs ? segs.join("/") : null;
}

/** A ref the cue sheet may key a record by: a workspace path already in its plain form. */
export const isRecordRef = (v: unknown): v is string =>
  typeof v === "string" && recordPathOf(v) === v;

/** The folder a record's stems are in, workspace-relative with its trailing slash — `stems/` beside it. */
export function recordStemsDir(path: string): string {
  const cut = path.lastIndexOf("/");
  return `${cut < 0 ? "" : path.slice(0, cut + 1)}${STEMS_DIR}/`;
}

/** What a row is called: the file's name without its extension. */
export function recordName(path: string): string {
  const base = path.slice(path.lastIndexOf("/") + 1);
  const ext = extOf(base);
  return ext ? base.slice(0, -ext.length) : base;
}

/** …and the folder holding it, or null for a record at the root. */
export function recordFolder(path: string): string | null {
  const segs = path.split("/");
  return segs.length > 1 ? (segs[segs.length - 2] ?? null) : null;
}

/** Does this record match what was typed? Every word must be somewhere in its path. */
export function recordMatches(path: string, query: string): boolean {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const hay = path.toLowerCase();
  return words.every((w) => hay.includes(w));
}

/** One line, as far as the workspace's files take it. */
export interface ResolvedRecord {
  item: PlaylistItem;
  /** The record's workspace path, or null for a line that names none. */
  path: string | null;
  /** Whether that audio file is in the workspace. */
  present: boolean;
  /** Whether its four stems are beside it. */
  stems: boolean;
}

/** Every line in the playlist's own order, KEEPING the ones that name nothing: a dropped row is an invisible typo. */
export function resolveRecords(
  items: PlaylistItem[],
  files: ReadonlySet<string> | null,
): ResolvedRecord[] {
  return items.map((item) => {
    const path = recordPathOf(item.song);
    const present = !!path && isAudio(path) && !!files?.has(path);
    const dir = path ? recordStemsDir(path) : "";
    const stems =
      present && STEMS.every((s) => files?.has(`${dir}${stemFile(s)}`));
    return { item, path, present, stems };
  });
}

/** The shape of a tree node this reads: a workspace path, a folder or not, and a folder's children. */
export interface TreeNode {
  path: string;
  dir: boolean;
  children?: TreeNode[];
}

/** A tree's audio files in tree order; a `stems/` folder's are left out unless asked, being no record. */
export function audioFilesOf(tree: TreeNode[], withStems: boolean): string[] {
  const out: string[] = [];
  const walk = (nodes: TreeNode[]) => {
    for (const n of nodes) {
      if (n.dir) {
        const name = n.path.slice(n.path.lastIndexOf("/") + 1);
        if (withStems || name !== STEMS_DIR) walk(n.children ?? []);
      } else if (isAudio(n.path)) out.push(n.path);
    }
  };
  walk(tree);
  return out;
}

/** Our own MIME, so a deck accepts a playlist row and refuses every other thing a browser can drop. */
export const RECORD_DRAG_TYPE = "application/x-studio-record";

/** What travels is the record's path: the deck finds it in its own set list. */
export interface RecordDrag {
  record: string;
}

export const packRecordDrag = (d: RecordDrag): string => JSON.stringify(d);

/** …and back, refusing anything that is not one — a drop carries whatever the page it came from wrote. */
export function parseRecordDrag(text: string): RecordDrag | null {
  try {
    const v = JSON.parse(text) as Partial<RecordDrag>;
    return typeof v?.record === "string" && v.record
      ? { record: v.record }
      : null;
  } catch {
    return null;
  }
}
