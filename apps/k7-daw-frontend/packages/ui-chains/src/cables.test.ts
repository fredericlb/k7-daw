// cables.test.ts — the cable geometry's contracts: the smoothed curve passes through
// its endpoints, the resampling walks the whole arc and the insert midpoint sits on the cable.

import { panelWidgets } from "@studio/graph-model/panel";
import { describe, expect, it } from "vitest";
import {
  autoTarget,
  buildCableGeom,
  cubicAt,
  cubicSegments,
  jackRef,
  liveVideoLanes,
  midByArc,
  smoothPath,
} from "./cables";
import { columnW, type GraphNode, stripWidgetRect } from "./graph";

const line = [
  { x: 0, y: 0 },
  { x: 100, y: 0 },
];
const bend = [
  { x: 0, y: 0 },
  { x: 100, y: 0 },
  { x: 100, y: 100 },
];

describe("cubicSegments", () => {
  it("is empty below two points, degenerate-straight at two", () => {
    expect(cubicSegments([])).toEqual([]);
    expect(cubicSegments([{ x: 1, y: 1 }])).toEqual([]);
    const [c] = cubicSegments(line);
    expect(cubicAt(c, 0)).toEqual({ x: 0, y: 0 });
    expect(cubicAt(c, 1)).toEqual({ x: 100, y: 0 });
  });

  it("passes through every waypoint (Catmull-Rom interpolates)", () => {
    const segs = cubicSegments(bend);
    expect(segs).toHaveLength(2);
    expect(cubicAt(segs[0], 0)).toEqual(bend[0]);
    expect(cubicAt(segs[0], 1)).toEqual(bend[1]);
    expect(cubicAt(segs[1], 1)).toEqual(bend[2]);
  });
});

describe("smoothPath", () => {
  it("draws two points as a straight line, more as cubics", () => {
    expect(smoothPath(line)).toBe("M 0 0 L 100 0");
    expect(smoothPath(bend)).toMatch(/^M 0 0( C [^C]+){2}$/);
    expect(smoothPath([])).toBe("");
  });
});

describe("buildCableGeom", () => {
  it("keeps the base polyline verbatim and measures the arc", () => {
    const g = buildCableGeom(line);
    expect(g.base).toEqual(line);
    expect(g.len).toBeCloseTo(100, 5);
  });

  it("samples are monotonic in arc-length and end at the endpoint", () => {
    const g = buildCableGeom(bend);
    for (let i = 1; i < g.samples.length; i++)
      expect(g.samples[i].s).toBeGreaterThanOrEqual(g.samples[i - 1].s);
    const last = g.samples[g.samples.length - 1];
    expect(last.x).toBeCloseTo(100, 5);
    expect(last.y).toBeCloseTo(100, 5);
    // The bend makes the curve longer than the two chords' minimum straight run.
    expect(g.len).toBeGreaterThanOrEqual(200 - 1e-6);
  });

  it("normals are unit-length", () => {
    for (const p of buildCableGeom(bend).samples)
      expect(Math.hypot(p.nx, p.ny)).toBeCloseTo(1, 5);
  });
});

describe("midByArc", () => {
  it("lands halfway down a straight cable", () => {
    const m = midByArc(buildCableGeom(line));
    expect(m.y).toBeCloseTo(0, 5);
    expect(Math.abs(m.x - 50)).toBeLessThanOrEqual(10); // within one sample step
  });
});

describe("liveVideoLanes", () => {
  const spans = {
    cam: [
      { from: 0, to: 2 },
      { from: 4, to: Number.POSITIVE_INFINITY },
    ],
    titles: [{ from: 1, to: 3 }],
  };

  it("names every lane showing a picture at that second", () => {
    expect(liveVideoLanes(spans, 1.5)).toEqual(["cam", "titles"]);
    expect(liveVideoLanes(spans, 2.5)).toEqual(["titles"]);
    expect(liveVideoLanes(spans, 3.5)).toEqual([]);
    expect(liveVideoLanes(spans, 900)).toEqual(["cam"]);
  });

  // Half-open, like the compositor's activeAt: a cut belongs to the later shot.
  it("ends a span before its `to`", () => {
    expect(liveVideoLanes({ cam: [{ from: 0, to: 2 }] }, 2)).toEqual([]);
    expect(liveVideoLanes({ cam: [{ from: 0, to: 2 }] }, 0)).toEqual(["cam"]);
  });
});

describe("jackRef", () => {
  // A track node's id wears the trk: prefix but its LABEL is the chains name — same for the sink.
  const node = (
    kind: GraphNode["kind"],
    id: string,
    label: string,
    module?: string,
  ) => ({ id, kind, label, module }) as GraphNode;

  it("names a node as the chains spell it, as the chains crate does", () => {
    expect(jackRef(node("module", "m1", "reverb (long)", "reverb"))).toBe(
      "reverb",
    );
    expect(jackRef(node("track", "trk:gtr", "gtr"))).toBe("gtr");
    expect(jackRef(node("sink", "sink", "OUT"))).toBe("OUT");
  });

  it("suffixes the port the jack names, in the file's own spelling", () => {
    expect(jackRef(node("module", "m1", "split", "split"), "b")).toBe(
      "split#b",
    );
  });
});

// An automation cable can be DELETED from the graph, and deleting one is unbinding a param ON A MODULE.
describe("an automation cable", () => {
  const curve = {
    id: "crv:sweep",
    kind: "curve",
    label: "sweep",
    x: 0,
    y: 0,
    w: 60,
    h: 40,
    dim: false,
  } as GraphNode;
  const synth = {
    id: "mod:synth",
    kind: "module",
    label: "synth",
    module: "synth",
    x: 0,
    y: 200,
    w: 100,
    h: 80,
    dim: false,
  } as GraphNode;
  const byId = new Map([curve, synth].map((n) => [n.id, n]));
  const edge = {
    id: "a",
    source: curve.id,
    target: synth.id,
    points: [],
    automation: { param: "Filter Freq" },
  };

  it("knows the module and the param it automates, which is what unbinding it names", () => {
    expect(autoTarget(edge as never, byId)).toMatchObject({
      module: "synth",
      param: "Filter Freq",
    });
  });

  it("stops at the widget's border, on the line from its socket to the widget's centre", () => {
    const knob = { kind: "knob", bind: "Filter Freq", col: 0, row: 0 };
    const withKnob = {
      ...synth,
      ui: { widgets: { k: knob } },
    } as unknown as GraphNode;
    const map = new Map([curve, withKnob].map((n) => [n.id, n]));
    const a = autoTarget(
      {
        ...edge,
        automation: { param: "Filter Freq", widget: { x: 0, y: 0 } },
      } as never,
      map,
    );
    const w = panelWidgets(withKnob.ui).find(
      ([, v]) => v.bind === "Filter Freq",
    )?.[1];
    if (!a || !w) throw new Error("a target on the knob");
    const r = stripWidgetRect(withKnob, w, columnW(withKnob.ui));
    const box = {
      l: synth.x + r.x,
      t: synth.y + r.y,
      r: synth.x + r.x + r.w,
      b: synth.y + r.y + r.h,
    };
    const onEdge =
      Math.abs(a.to.x - box.l) < 1e-6 ||
      Math.abs(a.to.x - box.r) < 1e-6 ||
      Math.abs(a.to.y - box.t) < 1e-6 ||
      Math.abs(a.to.y - box.b) < 1e-6;
    expect(onEdge).toBe(true);
    // …on the side facing the curve, which sits above the module.
    expect(a.to.y).toBeCloseTo(box.t);
  });
});
