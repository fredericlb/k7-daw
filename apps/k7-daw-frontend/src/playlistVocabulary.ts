// playlistVocabulary.ts — what may be written in a playlist file, from the audio files the workspace holds, as the editor's vocabulary.
// A reader would get wrong where the rule is: which fields and refs a position offers is Rust's (`@studio/k7-wasm/edit`).

import {
  playlistFieldsAt,
  playlistRefs,
  playlistValuesAt,
} from "@studio/k7-wasm/edit";
import type { YamlVocabulary } from "@studio/ui-editor/yaml/completions";

/** The vocabulary for a playlist, over the workspace's audio files: null on the first frame, and offering nothing then is honest. */
export function playlistVocabulary(
  files: ReadonlySet<string> | null,
): YamlVocabulary {
  const refs = files ? playlistRefs([...files]) : [];
  return {
    fieldsAt: (path) => playlistFieldsAt(path),
    valuesAt: (path, field) => playlistValuesAt(refs, path, field),
  };
}
