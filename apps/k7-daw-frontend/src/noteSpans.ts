// noteSpans.ts — when each lane's notes are SOUNDING, in seconds, so a MIDI cable can ripple with what it carries (song/src/reads/spans.rs).
// A reader would get wrong the tempo: one song tempo converts beats to seconds, and a tempo map would make this wrong as it would videoSpans.

import { noteSpans as read } from "@studio/k7-wasm/reads";
import type { LaneNotes } from "@studio/ui-chains/noteLoad";
import type { WebProject } from "./types";

/** Every lane's notes and the busiest instant each reaches, by lane id; a lane with no notes is left OUT — the seed builder walks these keys on every meter frame. */
export const noteSpans = (
  doc: WebProject | null | undefined,
): Record<string, LaneNotes> => (doc ? read(doc.tracks, doc.tempo) : {});
