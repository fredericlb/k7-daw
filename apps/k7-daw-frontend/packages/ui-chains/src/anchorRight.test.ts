// The wiring of the pinned mixer: a laid-out row across the ABI and back. The split is tested in the chains crate
// (apps/k7-daw-wasm/chains/src/anchor_right/tests.rs).

import type { PageModuleUI, WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { splitTrailingMixer } from "./anchorRight";
import { moduleRow, nodeSize } from "./graph";

const STRIP_UI: PageModuleUI = {
  cols: 1,
  rows: 7,
  widgets: { v: { type: "fader", col: 0, row: 0, rowSpan: 3, bind: "Volume" } },
};

const graph = (use: string[]): WebGraph => ({
  chains: {
    lead: {
      from: "gt_lead",
      use: use.map((module) =>
        module === "strip"
          ? {
              module,
              ui: STRIP_UI,
              stage: ["@input-volume", "@mix", "@output-volume"],
            }
          : { module },
      ),
      out: [{ to: "MIX", gain: 1 }],
    },
  },
  tracks: ["gt_lead"],
  trackMixers: { gt_lead: "strip" },
  warnings: [],
});

const row = (use: string[]) =>
  moduleRow(graph(use), use, { asDrawn: true, links: true, track: "gt_lead" });

describe("splitTrailingMixer", () => {
  it("lifts the trailing mixer out of a row, bare and sized by its panel", () => {
    const split = splitTrailingMixer(row(["amp", "strip"]));
    expect(split?.row.nodes.map((n) => n.module)).toEqual(["gt_lead", "amp"]);
    expect(split?.mixer.nodes[0]).toMatchObject({
      module: "strip",
      bare: true,
    });
    expect(split?.mixer.nodes[0].w).toBe(nodeSize(STRIP_UI).w);
  });

  it("leaves a row alone when its last card is not the mixer", () => {
    expect(splitTrailingMixer(row(["strip", "amp"]))).toBeNull();
  });
});
