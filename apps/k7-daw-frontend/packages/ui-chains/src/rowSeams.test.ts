// rowSeams.test.ts — the facade's own half: the glued cables are marked on the row and the row is widened for the tail `+`.
// Where the seams are (gap, stacks, branches, the tail's arrow) is the `chains` crate's: src/tests/seams.rs.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { moduleRow } from "./graph";
import { rowSeams } from "./rowSeams";

/** The room the tail `+` takes past the last card (the `chains` crate's `SEAM_END_ROOM`). */
const SEAM_END_ROOM = 40;

const g: WebGraph = {
  chains: {
    lead: {
      from: "gt_lead",
      use: [{ module: "amp" }, { module: "verb" }],
      out: [{ to: "MIX", gain: 1 }],
    },
  },
  tracks: ["gt_lead"],
  warnings: [],
};

describe("rowSeams", () => {
  it("marks the arrows between touching cards glued, and widens the row for the tail", () => {
    const r = moduleRow(g, ["amp", "verb"], {
      asDrawn: true,
      links: true,
      track: "gt_lead",
      gap: 0,
    });
    const { graph: out, seams } = rowSeams(r, r, g);
    expect(seams.map((s) => s.insert)).toEqual([
      { before: "gt_lead", after: "amp" },
      { before: "amp", after: "verb" },
      { before: "verb", after: "MIX" },
    ]);
    expect(out.edges.every((e) => e.glued)).toBe(true);
    expect(out.width).toBe(r.width + SEAM_END_ROOM);
  });
});
