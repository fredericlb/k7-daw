// playlistVocabulary.ts — what may be written in a playlist file, from the audio files the workspace
// actually holds. Its own adapter and not `schemaVocabulary`, which answers for a k7 PROJECT
// and rightly says nothing about a playlist. Pure. See ../docs/completion.md.

import { recordPathOf } from "@studio/k7-wasm/records";
import { STEMS_DIR } from "@studio/k7-wasm/stems";
import type { YamlVocabulary } from "@studio/ui-editor/yaml/completions";

/** The two fields `parsePlaylist` reads at the top level, and the one an item carries. */
const FILE_FIELDS = ["title", "items"];
const ITEM_FIELDS = ["song"];

/** The records a line may name: every audio file but a `stems/` folder's, sorted. */
const recordsOf = (files: ReadonlySet<string>): string[] =>
  [...files]
    .filter(
      (f) =>
        recordPathOf(f) === f && !f.split("/").slice(0, -1).includes(STEMS_DIR),
    )
    .sort();

/** The vocabulary for a playlist, over the workspace's audio files — null on the first frame, and offering nothing then is honest. */
export function playlistVocabulary(
  files: ReadonlySet<string> | null,
): YamlVocabulary {
  const refs = files ? recordsOf(files) : [];
  return {
    fieldsAt: (path) => {
      if (path.length === 0) return FILE_FIELDS;
      // A `- ` under `items:` may be a mapping, so this is offered beside the refs: until the colon is typed the line is genuinely both.
      return path.length === 1 && path[0] === "items" ? ITEM_FIELDS : [];
    },
    valuesAt: (path, field) => {
      // The bare form, `- <ref>`: the value belongs to the key that introduced the list.
      if (path.length === 0 && field === "items") return refs;
      if (path.length !== 1 || path[0] !== "items") return [];
      return field === "song" ? refs : [];
    },
  };
}
