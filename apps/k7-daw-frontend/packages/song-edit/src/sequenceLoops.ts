// sequenceLoops.ts — after a sequence is DOUBLED, halve the loop count of every placement of it,
// so what was four passes of one bar is two passes of two and the arrangement does not move.
// Across every lane, every take and every alternative — the per-file writer is @studio/song-schema's
// halveSequenceLoops. See docs/sequenceLoops.md.

import { withWriteHook } from "@studio/k7-wasm/edit";
import { stub } from "@studio/song/editStub";
import type { KeptLoop } from "@studio/song-schema/yaml/trackFile";

/** What the halving reached, and what it deliberately did not — what the command reports. */
export interface LoopHalving {
  /** Lane files rewritten, project-relative, in the order they were written. */
  files: string[];
  halved: number;
  /** One entry per placement left as it was, named by its lane and its start. */
  kept: (KeptLoop & { lane: string })[];
}

/** Halve the loop of every placement of `ref` across the project's lanes. */
export function halveLoopsOf(
  dir: string,
  ref: string,
  /** Called with a lane's path BEFORE it is overwritten, so a host's undo has the old bytes. */
  snapshot?: (path: string) => void,
): LoopHalving {
  return withWriteHook(snapshot, () =>
    stub("song-edit/sequenceLoops.halveLoopsOf", dir, ref, undefined),
  );
}
