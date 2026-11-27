// midiIn.ts — which MIDI device records into a lane, and which instruments a keyboard plays directly (song/src/reads/graph.rs).
// The one thing a reader would get wrong: no node is not "any device", it is no MIDI at all (apps/k7-daw-wasm/song/docs/midiIn.md).

import {
  midiInDeviceFor as deviceFor,
  midiInLanes as lanes,
  type PlayedTarget,
  midiInPlayed as played,
} from "@studio/k7-wasm/reads";
import type { WebGraph } from "./types";

export type { PlayedTarget };

/** The label of the midi-in node feeding `track` ("" any device), or null when none is wired to it. */
export const midiInDeviceFor = (g: WebGraph, track: string): string | null =>
  deviceFor(g, track);

/** Every lane a midi-in node is wired to — the lanes that arm in the BROWSER even before they hold a sequence. */
export const midiInLanes = (g: WebGraph): Set<string> => new Set(lanes(g));

/** Every instrument a midi-in node feeds directly — played live whatever is armed, never recorded (apps/k7-daw-wasm/song/docs/midiIn.md). */
export const midiInPlayed = (g: WebGraph): PlayedTarget[] => played(g);
