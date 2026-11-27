// masterRun.test.ts — the master run set aside at the bottom right of the laid-out graph.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { buildGraph, layout } from "./graph";

const graph = (
  chains: WebGraph["chains"],
  tracks: string[] = [],
): WebGraph => ({
  chains,
  tracks,
  warnings: [],
});

// The MASTER RUN stands fully right of the rest — a PREFERENCE, so unlike the push below inputs a pin outranks it.
describe("the master run goes to the bottom right", () => {
  // The two lanes meet at the SUM, and the master chain leaves it — `MIX -> OUT`, as a set is wired.
  const twoTracks = graph(
    {
      a: {
        from: "vox",
        use: [{ module: "comp" }],
        out: [{ to: "MIX", gain: 1 }],
      },
      b: {
        from: "gtr",
        use: [{ module: "eq" }],
        out: [{ to: "MIX", gain: 1 }],
      },
      MIX: { from: "MIX", use: [], out: [{ to: "OUT", gain: 1 }] },
    },
    ["vox", "gtr"],
  );

  /** The rightmost edge of everything that is NOT on the master run. */
  const restRight = (laid: ReturnType<typeof layout>, run: Set<string>) =>
    Math.max(...laid.nodes.filter((n) => !run.has(n.id)).map((n) => n.x + n.w));

  /** The sum and what it feeds, by node id. */
  const runOf = (laid: ReturnType<typeof layout>) => {
    const sum = laid.nodes.find((n) => n.kind === "sink" && n.label === "MIX");
    const ids = new Set(sum ? [sum.id] : []);
    for (let grew = true; grew; ) {
      grew = false;
      for (const e of laid.edges)
        if (ids.has(e.source) && !ids.has(e.target)) {
          ids.add(e.target);
          grew = true;
        }
    }
    return ids;
  };

  it("puts the whole run past everything else", () => {
    const { nodes, edges } = buildGraph(twoTracks);
    const laid = layout(nodes, edges);
    const run = runOf(laid);
    expect(run.size).toBeGreaterThan(1); // the sum AND the output it feeds
    const edge = restRight(laid, run);
    for (const n of laid.nodes.filter((x) => run.has(x.id)))
      expect(n.x, n.label).toBeGreaterThanOrEqual(edge);
  });

  it("stands on the TRACK row, level with the lanes it sums, and right of every one of them", () => {
    const { nodes, edges } = buildGraph(twoTracks);
    const laid = layout(nodes, edges);
    const run = runOf(laid);
    const sum = laid.nodes.find((n) => n.kind === "sink" && n.label === "MIX");
    const lanes = laid.nodes.filter((n) => n.kind === "track");
    expect(sum?.y).toBe(Math.min(...lanes.map((n) => n.y)));
    // …and LAST on that row: right of everything that is not its own run.
    const right = Math.max(
      ...laid.nodes.filter((n) => !run.has(n.id)).map((n) => n.x + n.w),
    );
    expect(sum?.x).toBeGreaterThan(right);
  });

  it("makes the MIX -> OUT cable short instead of page-tall", () => {
    const { nodes, edges } = buildGraph(twoTracks);
    const laid = layout(nodes, edges);
    const sum = laid.nodes.find((n) => n.kind === "sink" && n.label === "MIX");
    const out = laid.nodes.find((n) => n.kind === "sink" && n.label === "OUT");
    const wire = (out?.y ?? 0) - ((sum?.y ?? 0) + (sum?.h ?? 0));
    // One rank apart, not the height of the graph — which is what this whole rule is for.
    expect(wire).toBeLessThanOrEqual(120);
    expect(wire).toBeGreaterThanOrEqual(0);
  });

  it("a PIN still wins: where the column sits is a preference, not a correctness rule", () => {
    const { nodes, edges } = buildGraph(twoTracks);
    const laid = layout(nodes, edges, { MIX: { x: 0, y: 0 } });
    const sum = laid.nodes.find((n) => n.kind === "sink" && n.label === "MIX");
    expect(sum?.x).toBe(0);
  });
});
