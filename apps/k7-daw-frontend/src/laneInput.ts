// laneInput.ts — the input feeding a lane, audio, MIDI or a camera, off the graph document: what the
// per-track view's title bar shows and writes. Walks UPSTREAM from the lane, as the server's
// `runCarriesInput` does, so an input reaching it through a preamp's chain still counts.

import type { WebGraph } from "./types";

/** A live input node, as its file states it: the module to write, and the fields the title bar shows. */
export interface LaneInput {
  module: string;
  kind: "audio" | "midi" | "camera";
  channel?: number[];
  monitor?: "off" | "auto" | "on";
  device?: string;
}

/** The first live input (`monitor` or `device`) feeding `track`, directly or through other chains, or null. */
export function laneInput(g: WebGraph, track: string): LaneInput | null {
  const seen = new Set<string>();
  const todo = [track];
  while (todo.length) {
    const target = todo.pop() as string;
    if (seen.has(target)) continue;
    seen.add(target);
    for (const c of Object.values(g.chains)) {
      if (!c.out?.some((o) => o.to.split("#")[0] === target)) continue;
      const s = c.use.find(
        (st) => st.monitor !== undefined || st.device !== undefined,
      );
      if (s)
        return {
          module: s.module,
          kind:
            s.device !== undefined
              ? "midi"
              : s.camera !== undefined
                ? "camera"
                : "audio",
          channel: s.channel,
          monitor: s.monitor,
          device: s.device,
        };
      const head = c.from ?? c.use[0]?.module;
      if (head) todo.push(head);
    }
  }
  return null;
}

/** Does an input feed `track` at all — what refuses a second `track input`. */
export function laneHasInput(g: WebGraph, track: string): boolean {
  return laneInput(g, track) !== null;
}
