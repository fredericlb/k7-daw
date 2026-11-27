// rowSeams.ts — where a GLUED module row offers its `+`: on each seam between two columns a cable joins, once past the last column, and a branch.
// The seams are the `chains` crate's; this file marks the glued cables and widens the row, and holds the one number the button is drawn by.

import type { WebGraph } from "@studio/graph-model";
import { type SeamsAnswer, seamsOp } from "@studio/k7-wasm/graph";
import type { LaidOutGraph } from "./graphNode";

export type RowSeam = SeamsAnswer["seams"][number];

/** Where on a seam its branch button sits, as a share of its height from the top. */
export const SEAM_BRANCH_AT = 0.8;

/** The seams of `row`, its tail arrow read off `full` (the row before a card was lifted off its end), and the row with its glued cables marked. */
export function rowSeams(
  full: LaidOutGraph,
  row: LaidOutGraph,
  graph: WebGraph,
  branch = false,
): { graph: LaidOutGraph; seams: RowSeam[] } {
  const { seams, glued, width } = seamsOp(full, row, graph, branch);
  const gluedIds = new Set(glued);
  return {
    graph: {
      ...row,
      width,
      // A cable between two touching cards would be a loop drawn backwards over the seam.
      edges: row.edges.map((e) =>
        gluedIds.has(e.id) ? { ...e, glued: true } : e,
      ),
    },
    seams,
  };
}
