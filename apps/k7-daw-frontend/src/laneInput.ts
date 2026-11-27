// laneInput.ts — the input feeding a lane, audio, MIDI or a camera, off the graph document (song/src/reads/graph.rs).
// A reader would get wrong that it walks UPSTREAM, as the server's `runCarriesInput` does: an input behind a preamp's chain still counts.

import { type LaneInput, laneInput as read } from "@studio/k7-wasm/reads";
import type { WebGraph } from "./types";

export type { LaneInput };

/** The first live input (`monitor` or `device`) feeding `track`, directly or through other chains, or null. */
export const laneInput = (g: WebGraph, track: string): LaneInput | null =>
  read(g, track);

/** Does an input feed `track` at all — what refuses a second `track input`. */
export const laneHasInput = (g: WebGraph, track: string): boolean =>
  laneInput(g, track) !== null;
