// playlist.ts — what a playlist's text says it as a k7x edit: a stub, and adding, dropping or reordering a
// line. Reading one and naming its path are the Rust crate's (`parsePlaylist`, `playlistPathOf` in
// `@studio/k7-wasm/schema`). No `node:fs`, so the file service and the browser go by the same rules.
// See docs/playlists.md.

import type { K7Scope } from "@studio/k7-wasm/k7x";
import { stringify } from "@studio/k7-wasm/k7x";
import type { PlaylistItem } from "@studio/k7-wasm/types";

export type { Playlist, PlaylistItem } from "@studio/k7-wasm/types";

/** What a playlist file that does not exist yet says — a stub that PARSES, see docs/playlistFile.md. */
export function playlistStub(name: string): string {
  return stringify({ title: name, items: [] }, { header: [name] });
}

/** An item as it is written: the bare song when it names no variant, which is what anyone types. */
const itemValue = (item: PlaylistItem): unknown =>
  item.variant ? { song: item.song, variant: item.variant } : item.song;

/** One more line on the end of `items`, made when missing; false when `items` is something other than a list. */
export function appendPlaylistItem(doc: K7Scope, item: PlaylistItem): boolean {
  const items = doc.getIn(["items"]);
  if (items !== undefined && !Array.isArray(items)) return false;
  doc.addIn(["items"], itemValue(item));
  return true;
}

/** Drop one line by its 0-based position; false when `items` is no list or holds no such line. */
export function removePlaylistItem(doc: K7Scope, index: number): boolean {
  const items = doc.getIn(["items"]);
  if (!Array.isArray(items) || index < 0 || index >= items.length) return false;
  return doc.deleteIn(["items", index]);
}

/** Move one line to another 0-based position, its comments with it; false when either position is not a line. */
export function movePlaylistItem(
  doc: K7Scope,
  from: number,
  to: number,
): boolean {
  const items = doc.getIn(["items"]);
  if (!Array.isArray(items)) return false;
  const ok = (i: number) => Number.isInteger(i) && i >= 0 && i < items.length;
  if (!ok(from) || !ok(to)) return false;
  doc.moveIn(["items"], from, to);
  return true;
}
