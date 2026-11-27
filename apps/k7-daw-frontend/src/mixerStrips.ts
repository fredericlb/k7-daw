// mixerStrips.ts — which modules the Mixer panel draws, and in what order (song/src/reads/clip.rs).
// A reader would get wrong that a shared bus strip is ONE card: modules are listed once, in TRACK order.

import { mixerModules as read } from "@studio/k7-wasm/reads";
import type { WebProject } from "./types";

/** The mixer modules to draw, in TRACK order — a track with no mixer contributes nothing, and a shared bus strip is ONE card. */
export const mixerModules = (proj: WebProject | null): string[] =>
  proj ? read(proj.tracks) : [];
