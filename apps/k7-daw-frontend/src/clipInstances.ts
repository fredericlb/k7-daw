// clipInstances.ts — which materials the song places more than once, and where each one FIRST plays (song/src/reads/clip.rs).
// A reader would get wrong `materialOf`: the clipboard and the ghost badge both name a material by it, so it is one spelling here.

import { type Instances, materialInstances } from "@studio/k7-wasm/reads";
import type { WebClip } from "./types";

export type { Instances };

/** How a placement names its MATERIAL — one spelling, so the clipboard and the ghost badge cannot disagree about which events are copies. */
export const materialOf = (c: WebClip): string => c.name || c.id;

export { materialInstances };

/** Is this the placement `inst` calls the first — the one that wears no ghost? */
export const isFirstPlacement = (
  inst: Instances | undefined,
  trackId: string,
  start: number,
): boolean =>
  !!inst && inst.track === trackId && Math.abs(inst.start - start) < 1e-6;
