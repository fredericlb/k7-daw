// anchorRight.ts — a module row's trailing MIXER, lifted out so the host pins it to the pane's right edge: the split is Rust's (the chains crate).
// The row keeps every other card and cable; the mixer becomes a graph of its own, one BARE card at the row's height.
// The one thing a reader would get wrong: the cable INTO the mixer is dropped, not redrawn (docs/graphBuild.md).

import { splitTrailingMixer as split } from "@studio/k7-wasm/nodes";
import type { LaidOutGraph } from "./graph";

/** The row without its last card, and that card alone — or null when the row does not END on a lane's mixer. */
export const splitTrailingMixer = (
  row: LaidOutGraph,
): { row: LaidOutGraph; mixer: LaidOutGraph } | null =>
  split(row as never) as { row: LaidOutGraph; mixer: LaidOutGraph } | null;
