// chains.test.ts — the facade's own glue: maps in and out of the ABI, the slimmed graph, the spread over the page's nodes.
// A reader would get wrong that the rules are tested here: they are the Rust tests of the `chains` crate.

import type { PageWidget, WebGraph } from "@studio/graph-model";
import { beforeAll, describe, expect, it } from "vitest";
import { wasmReady } from "./abi.ts";
import {
  computeFlow,
  curvesUnderLeft,
  resolveMapping,
  seedFlows,
  seedsEq,
  subgraphForTrack,
  unmappedControls,
  widgetDenorm,
  widgetNorm,
  withoutCurves,
} from "./chains.ts";

beforeAll(async () => {
  await wasmReady();
});

const graph = (chains: unknown): WebGraph =>
  ({ chains, tracks: [], warnings: [] }) as unknown as WebGraph;

describe("the widget maths", () => {
  it("answers NaN, as the page always read it, for a range it cannot travel", () => {
    const w = { type: "knob", col: 0, row: 0, min: 0.5, max: -1, curve: "log" };
    expect(widgetNorm(w as PageWidget, -3)).toBeNaN();
    expect(widgetDenorm(w as PageWidget, 0.5)).toBeNaN();
  });
});

describe("the flow", () => {
  it("round-trips the seeds and the cables through maps", () => {
    const seeds = seedFlows({
      melody: { db: -40, notes: { load: 4, peak: 4 } },
    });
    expect(seeds.get("melody")).toEqual({ audio: 0.5, midi: 1, video: 0 });
    const flow = computeFlow(
      [{ id: "a", source: "trk:melody", target: "m", signal: "midi" }],
      seeds,
    );
    expect(flow.get("a")).toBe(1);
    expect(
      computeFlow([{ id: "a", source: "x", target: "y" }], new Map()),
    ).toEqual(new Map());
  });

  it("compares two seed maps by what they hold", () => {
    const a = new Map([["m", { audio: 0.5, midi: 0, video: 0 }]]);
    expect(seedsEq(a, new Map(a))).toBe(true);
    expect(
      seedsEq(a, new Map([["m", { audio: 0.6, midi: 0, video: 0 }]])),
    ).toBe(false);
  });
});

describe("a lane's slice", () => {
  it("keeps the colours and curves and drops the `from` of a chain fed by another lane", () => {
    const g = {
      ...graph({
        a: { from: "vox", use: [{ module: "x" }], out: [{ to: "b", gain: 1 }] },
        b: { from: "mic", use: [{ module: "y" }], out: [] },
      }),
      trackColors: { vox: "#fff" },
      curves: { sweep: { type: "custom" } },
    } as unknown as WebGraph;
    const sub = subgraphForTrack(g, "vox");
    expect(Object.keys(sub.chains)).toEqual(["a", "b"]);
    expect(sub.chains.a.from).toBe("vox");
    expect(sub.chains.b.from).toBeUndefined();
    expect(sub.tracks).toEqual(["vox"]);
    expect(sub.curves).toEqual({ sweep: { type: "custom" } });
  });
});

describe("a surface's bindings", () => {
  const g = graph({
    bus: {
      use: [
        {
          module: "eqA",
          pluginId: "bus:eqA",
          ui: {
            widgets: { other: { bind: "Mid", min: 3 } },
            tabs: [{ name: "t", widgets: { lo: { bind: "Low", min: -20 } } }],
          },
        },
      ],
    },
  });
  const controls = [{ id: "a.one" }, { id: "a.two" }];

  it("hands back the caller's own control objects and the widget of a tab", () => {
    const { rows, problems } = resolveMapping(
      { "a.one": { module: "eqA", param: "Low" } },
      g,
      { controls, noun: "x", plugless: "track" },
    );
    expect(problems).toEqual([]);
    expect(rows[0].control).toBe(controls[0]);
    expect(rows[0].target.widget).toMatchObject({ min: -20 });
  });

  it("answers the controls a mapping leaves unwired, as the caller's objects", () => {
    const mapping = { "a.one": { module: "m", param: "p" }, "a.two": null };
    expect(unmappedControls(controls, mapping)).toEqual([controls[1]]);
  });
});

describe("the curve cards", () => {
  const nodes = [
    { id: "c", kind: "module", x: 2000, y: 900, w: 200, h: 120, label: "c" },
    { id: "crv", kind: "curve", x: 0, y: 0, w: 200, h: 120, label: "crv" },
  ];

  it("spreads the new place over the page's node and keeps the rest of it", () => {
    const [, crv] = curvesUnderLeft(nodes, new Map([["crv", "c"]]));
    expect(crv).toMatchObject({ x: 1776, y: 924, label: "crv" });
  });

  it("drops the curve cards and their cables, and hands back the same graph when there is none", () => {
    const edges = [
      { source: "c", target: "x" },
      { source: "crv", target: "c" },
    ];
    expect(withoutCurves({ nodes, edges })).toEqual({
      nodes: [nodes[0]],
      edges: [edges[0]],
    });
    const plain = { nodes: [nodes[0]], edges: [edges[0]] };
    expect(withoutCurves(plain)).toBe(plain);
  });
});
