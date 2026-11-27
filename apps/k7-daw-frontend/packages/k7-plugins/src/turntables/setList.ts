// setList.ts — the PLAYLIST the surface names, resolved against the workspace's audio files. Two
// failures are kept apart on purpose: a set list that will not parse, and a line naming no file
// the workspace holds. See ../../docs/setList.md.

import {
  recordFolder,
  recordName,
  recordPathOf,
  resolveRecords,
} from "@studio/k7-wasm/records";
import { parsePlaylist } from "@studio/k7-wasm/schema";
import type { RecordDrag } from "@studio/song-schema/records";
import { m } from "../paraglide/messages.js";

/** One line of the set list: what the playlist named, and whether the workspace holds it. */
export interface SetEntry {
  /** As written in the playlist, its slashes trimmed. */
  ref: string;
  /** The record's workspace path, else the ref: what names a row and its cues. */
  id: string;
  /** What to show: the file's name without its extension. */
  name: string;
  /** The folder holding it, shown as the record's artist; null at the root. */
  artist: string | null;
  /** The record's workspace path when the file is there, else null: nothing to play. */
  file: string | null;
  /** Whether its four stems are beside it. */
  stems: boolean;
}

export interface SetList {
  entries: SetEntry[];
  /** What the playlist said that could not be read, one line per problem. Empty when it read. */
  issues: string[];
  /** The surface names NO playlist — distinct from an empty one and from a missing one (docs/setList.md). */
  absent: boolean;
}

/** A string as the browser SEARCHES it: lower case, accents folded — a DJ types `alizee`. */
function fold(s: string): string {
  // The combining-marks block as escapes: the literal characters are invisible in an editor.
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/** The set's songs as the browser shows them: every term anywhere, sorted by ARTIST — docs/songBrowser.md. */
export function browseSongs(entries: SetEntry[], query: string): SetEntry[] {
  const terms = fold(query).split(/\s+/).filter(Boolean);
  const hit = (s: SetEntry) => {
    const hay = fold(`${s.artist ?? ""} ${s.name}`);
    return terms.every((t) => hay.includes(t));
  };
  const cmp = (a: string, b: string) =>
    a.localeCompare(b, "fr", { sensitivity: "base", numeric: true });
  return entries
    .filter(hit)
    .sort((a, b) => cmp(a.artist ?? "", b.artist ?? "") || cmp(a.name, b.name));
}

/** The set list of a desk whose surface names no playlist — shared, so three call sites cannot drift. */
export const NO_PLAYLIST: SetList = { entries: [], issues: [], absent: true };

/** The row a dragged playlist line means, or undefined when this set list does not carry it. */
export function entryForDrag(
  entries: readonly SetEntry[],
  drag: RecordDrag,
): SetEntry | undefined {
  const want = recordPathOf(drag.record) ?? drag.record;
  return entries.find((e) => e.id === want);
}

/** Resolve a playlist against the workspace's files — every line kept, duplicates dropped. See docs/setList.md. */
export function resolveSet(
  text: string | null,
  files: ReadonlySet<string> | null,
): SetList {
  if (text === null)
    return {
      entries: [],
      absent: false,
      issues: [m["set.notFound"]()],
    };

  const parsed = parsePlaylist(text);
  if (parsed === null)
    return {
      entries: [],
      absent: false,
      issues: [m["set.unreadable"]()],
    };

  const seen = new Set<string>();
  const out: SetEntry[] = [];
  for (const r of resolveRecords(parsed.items, files)) {
    const ref = r.item.song.trim().replace(/^\/+|\/+$/g, "");
    const id = r.path ?? ref;
    if (seen.has(id)) continue;
    seen.add(id);
    out.push({
      ref,
      id,
      name: recordName(id),
      artist: r.path ? recordFolder(r.path) : null,
      file: r.present ? r.path : null,
      stems: r.stems,
    });
  }
  return { entries: out, absent: false, issues: [] };
}
