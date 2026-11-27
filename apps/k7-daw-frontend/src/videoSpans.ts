// videoSpans.ts — when each video lane is putting a picture on screen, in seconds, so the graph's cables ripple for the frame (song/src/reads/spans.rs).
// A reader would get wrong that an open end crosses as null: JSON has no infinity, so the page reads it back as one.

import { videoSpans as read } from "@studio/k7-wasm/reads";
import type { LaneSpan } from "@studio/ui-chains/cables";
import type { WebVideoProject } from "@studio/web-protocol";

/** The whole picture's spans, by lane id — a lane with nothing to show is left OUT, the renderer walking these keys on every frame. */
export function videoSpans(
  doc: WebVideoProject | null | undefined,
): Record<string, LaneSpan[]> {
  const out: Record<string, LaneSpan[]> = {};
  if (!doc) return out;
  for (const [id, spans] of Object.entries(read(doc.videoTracks ?? [])))
    out[id] = spans.map((s) => ({
      from: s.from,
      to: s.to ?? Number.POSITIVE_INFINITY,
    }));
  return out;
}
