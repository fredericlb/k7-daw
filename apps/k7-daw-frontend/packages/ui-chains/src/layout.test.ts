// layout.test.ts — the facade's own half: the places are put back on the page's cards, with the cables' points and a box's push.
// The layout's rules (rows, the track line, pins, memory, the receiver below its feed, boxes) are the `chains` crate's: src/tests/layout_rules.rs.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { buildGraph, layout } from "./graph";

const graph: WebGraph = {
  chains: {
    long: {
      from: "vox",
      use: [{ module: "comp" }, { module: "eq" }],
      out: [{ to: "OUT", gain: 1 }],
    },
  },
  tracks: ["vox"],
  warnings: [],
};

describe("layout", () => {
  it("reports zero extent for an empty layout, never -Infinity", () => {
    const { nodes, width, height } = layout([], []);
    expect([nodes.length, width, height]).toEqual([0, 0, 0]);
  });

  it("puts a pinned module where its pin says, on the page's own card, with its cables' points", () => {
    const { nodes, edges } = buildGraph(graph);
    const laid = layout(nodes, edges, { comp: { x: 640, y: 480 } });
    const comp = laid.nodes.find((n) => n.module === "comp");
    expect(comp).toMatchObject({ x: 640, y: 480, id: "mod:comp" });
    expect(laid.edges).toHaveLength(edges.length);
    for (const e of laid.edges)
      expect(e.points.length).toBeGreaterThanOrEqual(2);
  });

  it("carries a box's members and its inner cables back onto the cards", () => {
    const boxed: WebGraph = {
      chains: {
        gtr: {
          from: "gtr",
          use: [{ module: "trimA" }, { module: "fadeA" }],
          out: [{ to: "OUT", gain: 1 }],
        },
      },
      tracks: ["gtr"],
      assemblies: {
        turntableA: {
          uses: "@k7-core/turntable",
          label: "Turntable",
          members: ["trimA", "fadeA"],
          inputs: [{ name: "in", type: "audio", node: "trimA" }],
          outputs: [{ name: "mix", type: "audio", node: "fadeA" }],
        },
      },
      warnings: [],
    };
    const { nodes, edges } = buildGraph(boxed, {}, ["turntableA"]);
    const pins = { gtr: { x: 0, y: 0 }, OUT: { x: 40, y: 900 } };
    const laid = layout(nodes, edges, pins);
    expect(laid.nodes.filter((n) => n.assembly === "turntableA")).toHaveLength(
      2,
    );
    expect(laid.edges.filter((e) => e.assembly === "turntableA")).toHaveLength(
      1,
    );
  });
});
