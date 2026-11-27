// setList.ts — the PLAYLIST the surface names, as the song browser lists it: the resolution is Rust's, the words and the sort are the page's.
// A reader would get wrong that two failures stay apart on purpose: a set list that will not parse, and a line naming no file the workspace holds.

import {
  type SetList as RawSetList,
  resolveSet as resolve,
  type SetEntry,
  type SetIssue,
} from "@studio/k7-wasm/desk";
import { recordPathOf } from "@studio/k7-wasm/records";
import type { RecordDrag } from "@studio/ui/record-drag";
import { m } from "../paraglide/messages.js";

export type { SetEntry };

export interface SetList extends Omit<RawSetList, "issues"> {
  /** What the playlist said that could not be read, one line per problem. Empty when it read. */
  issues: string[];
}

const SAYING: Record<SetIssue, () => string> = {
  notFound: () => m["set.notFound"](),
  unreadable: () => m["set.unreadable"](),
};

/** The set list of a desk whose surface names no playlist — shared, so three call sites cannot drift. */
export const NO_PLAYLIST: SetList = { entries: [], issues: [], absent: true };

/** Resolve a playlist against the workspace's files — every line kept, duplicates dropped. See docs/setList.md. */
export function resolveSet(
  text: string | null,
  files: ReadonlySet<string> | null,
): SetList {
  const set = resolve(text, files);
  return { ...set, issues: set.issues.map((i) => SAYING[i]()) };
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

/** The row a dragged playlist line means, or undefined when this set list does not carry it. */
export function entryForDrag(
  entries: readonly SetEntry[],
  drag: RecordDrag,
): SetEntry | undefined {
  const want = recordPathOf(drag.record) ?? drag.record;
  return entries.find((e) => e.id === want);
}
