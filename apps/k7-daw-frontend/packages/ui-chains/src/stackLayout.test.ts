// stackLayout.test.ts — the facade's own half: the places are put on the built cards, every cable is kept, a side cable runs outside its column.
// The arrangement itself (columns, bars, the sum's column, the sockets a fork keeps) is the `chains` crate's: src/tests/stacks.rs.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { buildGraph, cablePath } from "./graph";
import { stackLayout } from "./stackLayout";

// The synth feeds two chains, a delay and a reverb, both of which land in its lane's column.
const fork = {
  chains: {
    lead: {
      from: "melody",
      use: [{ module: "synth" }],
      out: [
        { to: "wet", gain: 1 },
        { to: "space", gain: 1 },
      ],
    },
    wet: { use: [{ module: "dly" }], out: [{ to: "MIX", gain: 1 }] },
    space: { use: [{ module: "rev" }], out: [{ to: "MIX", gain: 1 }] },
  },
  tracks: ["melody"],
  warnings: [],
} as unknown as WebGraph;

describe("the stack view", () => {
  it("keeps every cable and hangs the side one out of the right edge, outside the column", () => {
    const built = buildGraph(fork);
    const l = stackLayout(built.nodes, built.edges, fork, ["melody"]);
    expect(l.edges.length).toBe(built.edges.length);
    const e = l.edges.find((x) => x.side !== undefined);
    const src = l.nodes.find((n) => n.id === e?.source);
    const tgt = l.nodes.find((n) => n.id === e?.target);
    const path = e && cablePath(e, src, tgt);
    expect(path?.[0].x).toBe((src?.x ?? 0) + (src?.w ?? 0));
    expect(path?.[path.length - 1].x).toBe((tgt?.x ?? 0) + (tgt?.w ?? 0));
    expect(Math.max(...(path ?? []).map((p) => p.x))).toBeGreaterThan(
      (src?.x ?? 0) + (src?.w ?? 0),
    );
  });
});
