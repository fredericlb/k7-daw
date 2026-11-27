// moduleRow.test.ts — the facade's own halves: the page's option names, the ranks and spacing applied to the cards, a cable met on the sides.
// The row's rules (order, links, scale, curves, lane, inputs) and the two passes are the `chains` crate's: src/tests/rows.rs, src/tests/rank.rs.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import {
  alignModuleRank,
  buildGraph,
  jackPoint,
  layout,
  moduleRow,
  spaceModulesEvenly,
} from "./graph";

const graph = (
  chains: WebGraph["chains"],
  tracks: string[] = [],
): WebGraph => ({
  chains,
  tracks,
  warnings: [],
});

const lane = graph(
  {
    vox: {
      from: "vox",
      use: [{ module: "comp" }, { module: "eq" }],
      out: [{ to: "OUT", gain: 1 }],
    },
  },
  ["vox"],
);

describe("moduleRow", () => {
  it("places the modules left to right in the order asked for", () => {
    const row = moduleRow(lane, ["eq", "comp"]);
    expect(row.nodes.map((n) => n.module)).toEqual(["eq", "comp"]);
  });

  it("takes the machine's lists under `devices`, so a MIDI device's dropdown offers them", () => {
    const keys = graph(
      {
        vox: {
          from: "vox",
          use: [{ module: "keys", device: "" }, { module: "comp" }],
          out: [{ to: "OUT", gain: 1 }],
        },
      },
      ["vox"],
    );
    const row = moduleRow(keys, ["keys", "comp"], {
      asDrawn: true,
      devices: { midiDevices: ["KeyStep 37"] },
    });
    const card = row.nodes.find((n) => n.module === "keys");
    expect(card?.ui?.widgets.device?.options?.map((o) => o.label)).toEqual([
      "any",
      "KeyStep 37",
    ]);
  });

  it("meets its cables on the sides, which the page's socket points follow", () => {
    const row = moduleRow(lane, ["comp", "eq"], { links: true, gap: -1 });
    const [a, b] = row.nodes;
    const out = jackPoint(a, "out", 0);
    const into = jackPoint(b, "in", 0);
    expect(out.x).toBeGreaterThan(a.x + a.w);
    expect(into.x).toBeLessThan(b.x);
    expect(out.y).toBeCloseTo(a.y + a.h / 2);
    expect(into.y).toBeCloseTo(b.y + b.h / 2);
  });
});

describe("the rank and spacing passes", () => {
  const two = graph(
    {
      a: {
        from: "vox",
        use: [{ module: "comp" }, { module: "mx-vox" }],
        out: [{ to: "OUT", gain: 1 }],
      },
      b: {
        from: "bass",
        use: [{ module: "mx-bass" }],
        out: [{ to: "OUT", gain: 1 }],
      },
    },
    ["vox", "bass"],
  );
  const row = ["mx-vox", "mx-bass"];

  it("hands back the same nodes when nothing needs aligning, and the changed ranks otherwise", () => {
    const { nodes } = buildGraph(two);
    expect(alignModuleRank(nodes, ["mx-vox"])).toBe(nodes);
    const rank = (ns: typeof nodes, m: string) =>
      ns.find((n) => n.module === m)?.rank;
    const out = alignModuleRank(nodes, row);
    expect(rank(out, "mx-bass")).toBe(rank(nodes, "mx-vox"));
  });

  it("hands back the same graph when nothing needs spacing, and moves the cards otherwise", () => {
    const { nodes, edges } = buildGraph(two);
    const laid = layout(alignModuleRank(nodes, row), edges);
    expect(spaceModulesEvenly(laid, ["mx-vox"])).toBe(laid);
    const after = spaceModulesEvenly(laid, row);
    const [a, b] = row.map((m) => after.nodes.find((n) => n.module === m));
    expect((b?.x ?? 0) - ((a?.x ?? 0) + (a?.w ?? 0))).toBe(24);
    expect(after.edges).toBe(laid.edges);
  });
});
