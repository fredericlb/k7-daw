// build.ts — constructors for the documents in ./project, so an app with no song folder can still drive the engine.
// The ONLY runtime module in this package: `.` and `./project` are types and erase to nothing, this one reaches the Rust module.
// The defaults and the names are Rust's (song/src/reads/blank.rs and the generated consts); a ProjectDoc has ~15 required fields.

import { blanks, enginePlugin } from "@studio/k7-wasm/reads";
import type { ClipDoc, PluginDoc, ProjectDoc, TrackDoc } from "./project";

export {
  DEFAULT_METRONOME_LEVEL,
  DSP_FILE,
  DSP_TYPE,
  JSDSP_FILE,
  JSDSP_TYPE,
  METRONOME_MIN,
  MIXER_PLUGIN,
  VOLUME_DB,
} from "@studio/k7-wasm/consts";
export { enginePlugin };

/** A valid, empty project; `recordDir: ""` is a REFUSAL rather than a blank, so pass a real directory to record anything. */
export const emptyDoc = (over: Partial<ProjectDoc> = {}): ProjectDoc => ({
  ...blanks().doc,
  ...over,
});

/** A track. `id` is the one thing nothing can guess — everything else has a sane default. */
export const trackDoc = (
  id: string,
  over: Partial<TrackDoc> = {},
): TrackDoc => ({
  ...blanks().track,
  id,
  name: id,
  ...over,
});

/** A clip. `type` decides which half of the shape matters, so it is required alongside `id`. */
export const clipDoc = (
  over: Partial<ClipDoc> & Pick<ClipDoc, "id" | "type">,
): ClipDoc => ({ ...blanks().clip, ...over });

/** A plugin instance. `id` is the engine's `chain:module` address — nothing else can supply it. */
export const pluginDoc = (
  over: Partial<PluginDoc> & Pick<PluginDoc, "id">,
): PluginDoc => ({ ...blanks().plugin, ...over });
