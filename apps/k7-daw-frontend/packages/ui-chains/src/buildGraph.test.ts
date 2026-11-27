// buildGraph.test.ts — the facade's own halves: the call reaches the build, the folder lists are ordered for people, a card is sized to its open tab.
// The build's rules (ids, cables, signals, sums, lanes, curves, boxes, clips) are tested in the `chains` crate (apps/k7-daw-wasm/chains/src/tests).

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { buildGraph, withOpenTabs } from "./graph";

const graph = (
  chains: WebGraph["chains"],
  tracks: string[] = [],
): WebGraph => ({
  chains,
  tracks,
  warnings: [],
});

describe("buildGraph", () => {
  it("wires track -> module -> sink for a one-module chain", () => {
    const { nodes, edges } = buildGraph(
      graph(
        {
          lead: {
            from: "melody",
            use: [{ module: "synth" }],
            out: [{ to: "OUT", gain: 1 }],
          },
        },
        ["melody"],
      ),
    );
    const byId = Object.fromEntries(nodes.map((n) => [n.id, n]));
    expect(byId["trk:melody"]?.kind).toBe("track");
    expect(byId["mod:synth"]).toMatchObject({ kind: "module", chain: "lead" });
    expect(byId["snk:OUT"]?.kind).toBe("sink");
    expect(edges.map((e) => `${e.source}->${e.target}`).sort()).toEqual([
      "mod:synth->snk:OUT",
      "trk:melody->mod:synth",
    ]);
  });

  // The folder arrives in code points, capitals first; each row keeps its place in it as its value.
  it("draws a folder's names for people, each keeping its value", () => {
    const widget = { type: "dropdown" as const, col: 0, row: 0, bind: "@base" };
    const { nodes } = buildGraph(
      graph(
        {
          preview: {
            from: "in",
            use: [
              {
                module: "fuzz",
                bases: ["FM Bass", "WT Bass", "fingered"],
                ui: { cols: 2, rows: 1, widgets: { b: widget } },
              },
            ],
            out: [{ to: "OUT", gain: 1 }],
          },
        },
        ["in"],
      ),
    );
    const w = nodes.find((n) => n.kind === "module")?.ui?.widgets.b;
    expect(w?.options?.map((o) => [o.label, o.value])).toEqual([
      ["none", 0],
      ["fingered", 3],
      ["FM Bass", 1],
      ["WT Bass", 2],
    ]);
  });
});

describe("withOpenTabs", () => {
  const nodes = [
    { id: "a", y: 5, h: 100, tabH: [100, 60] },
    { id: "b", y: 9, h: 40 },
  ];

  it("sizes a card to its open tab, its top edge where it was", () => {
    const [a, b] = withOpenTabs(nodes, { a: 1 });
    expect(a).toMatchObject({ y: 5, h: 60 });
    expect(b).toBe(nodes[1]);
  });

  it("returns the same nodes when nothing is open past the first page", () => {
    expect(withOpenTabs(nodes, {})).toBe(nodes);
  });
});
