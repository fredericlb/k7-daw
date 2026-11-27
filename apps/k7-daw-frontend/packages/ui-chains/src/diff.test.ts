// diff.test.ts — the build, the rows, the seams and the stacks against the TypeScript twins they replace, on the golden graphs and random ones.
// The twins are a frozen copy under __old__/; this file and that folder go when the parity has been shown (docs/rust-ts-inventory.md, M2a).

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { WebGraph } from "@studio/graph-model";
import { afterAll, describe, expect, it } from "vitest";
import { foldAssemblies as oldFold } from "./__old__/assembly";
import { buildGraph as oldBuild } from "./__old__/buildGraph";
import {
  alignModuleRank as oldAlign,
  moduleRow as oldRow,
  spaceModulesEvenly as oldSpace,
} from "./__old__/moduleRow";
import { rowExit as oldExit, rowSeams as oldSeams } from "./__old__/rowSeams";
import { stackLayout as oldStack } from "./__old__/stackLayout";
import { buildGraph } from "./buildGraph";
import { alignModuleRank, moduleRow, spaceModulesEvenly } from "./moduleRow";
import { rowSeams } from "./rowSeams";
import { stackLayout } from "./stackLayout";

const plain = <T>(x: T): T => JSON.parse(JSON.stringify(x ?? null));

const golden = join(__dirname, "../../../../k7-daw-backend/fixtures/golden");
const fixtures: [string, WebGraph][] = [];
for (const dir of readdirSync(golden)) {
  for (const f of readdirSync(join(golden, dir)).filter((n) =>
    n.endsWith(".json"),
  )) {
    const graph = JSON.parse(readFileSync(join(golden, dir, f), "utf8")).graph;
    if (graph?.chains) fixtures.push([`${dir}/${f}`, graph]);
  }
}

function rng(seed: number) {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type R = () => number;
const pick = <T>(r: R, xs: readonly T[]): T => xs[Math.floor(r() * xs.length)];
const some = (r: R, p: number) => r() < p;
const int = (r: R, lo: number, hi: number) =>
  lo + Math.floor(r() * (hi - lo + 1));

const TYPES = [
  "knob",
  "button",
  "fader",
  "dropdown",
  "group",
  "lcdscreen",
  "waveform",
  "meter",
  "tap",
  "number",
];

function widget(r: R, cols: number, rows: number) {
  const type = pick(r, TYPES);
  const w: Record<string, unknown> = {
    type,
    col: int(r, 0, cols - 1),
    row: int(r, 0, rows - 1),
  };
  if (some(r, 0.3)) w.colSpan = int(r, 1, 3);
  if (some(r, 0.3)) w.rowSpan = int(r, 1, 3);
  if (some(r, 0.7))
    w.bind = pick(r, ["Gain", "Freq", "@base", "@shader", "@mute", "Mix", "Q"]);
  if (some(r, 0.5)) w.label = pick(r, ["a", "b", "gain", "freq"]);
  if (type === "dropdown" || some(r, 0.1))
    w.options = [
      { label: "x", value: 0 },
      { label: "y", value: 1 },
    ];
  return w;
}

function panel(r: R) {
  const cols = int(r, 1, 7);
  const rows = int(r, 1, 9);
  const widgets = (): Record<string, unknown> => {
    const out: Record<string, unknown> = {};
    for (let i = 0; i < int(r, 0, 7); i++) out[`w${i}`] = widget(r, cols, rows);
    return out;
  };
  const ui: Record<string, unknown> = { cols, rows, widgets: widgets() };
  if (some(r, 0.4))
    ui.tabs = Array.from({ length: int(r, 1, 4) }, (_, i) => ({
      name: `t${i}`,
      widgets: widgets(),
    }));
  if (some(r, 0.3)) {
    const vcols = int(r, 1, 4);
    ui.vertical = {
      cols: vcols,
      rows: int(r, 1, 12),
      widgets: { v: widget(r, vcols, 4) },
    };
  }
  return ui;
}

function genGraph(seed: number): WebGraph {
  const r = rng(seed);
  const tracks = Array.from({ length: int(r, 1, 4) }, (_, i) => `t${i}`);
  const videoTracks = some(r, 0.4) ? ["v0"] : [];
  const lanes = [...tracks, ...videoTracks];
  const mods = Array.from({ length: int(r, 2, 8) }, (_, i) => `m${i}`);
  const chainNames = Array.from({ length: int(r, 1, 5) }, (_, i) => `c${i}`);
  const curves: Record<string, unknown> = {};
  for (let i = 0; i < int(r, 0, 4); i++)
    curves[`k${i}`] = {
      type: pick(r, ["lfo", "adsr", "custom", "onoff"]),
      shape: pick(r, ["sine", "saw"]),
      spec: some(r, 0.8)
        ? {
            amplitude: pick(r, [0.5, 1, 2]),
            polarity: pick(r, ["unipolar", "bipolar"]),
            cut: pick(r, ["none", "positive", "negative"]),
            ...(some(r, 0.3) ? { conditions: [{ type: "event" }] } : {}),
          }
        : undefined,
    };
  const curveIds = Object.keys(curves);
  const chains: Record<string, unknown> = {};
  const sig = () => pick(r, ["audio", "midi", "video", undefined]);
  for (const name of chainNames) {
    const use = Array.from({ length: int(r, 0, 4) }, () => {
      const s: Record<string, unknown> = {
        module: pick(r, mods),
        stub: false,
        hasNativePluginUI: false,
        params: {},
      };
      if (some(r, 0.2)) s.enabled = false;
      const e = sig();
      if (e) s.emits = e;
      if (some(r, 0.6)) s.ui = panel(r);
      if (some(r, 0.15)) s.strip = true;
      if (some(r, 0.15)) s.monitor = pick(r, ["off", "auto", "on"]);
      if (some(r, 0.08)) s.camera = pick(r, ["", "Lens"]);
      if (some(r, 0.08)) s.device = pick(r, ["", "Keys"]);
      if (some(r, 0.1))
        s.rack = Array.from({ length: int(r, 0, 14) }, (_, i) => ({
          symbol: `S${i}`,
          note: int(r, 30, 80),
        }));
      if (some(r, 0.05)) s.rackKit = true;
      if (s.monitor) s.channel = pick(r, [[1], [2], [1, 2], [3, 4]]);
      if (some(r, 0.2)) s.bases = ["b2", "b10", "b1"];
      if (some(r, 0.2)) s.shaders = ["Z", "a"];
      if (some(r, 0.2)) s.category = pick(r, ["audio", "shaders", "midi"]);
      if (some(r, 0.3))
        s.inputs = [
          { name: "a", type: "audio" },
          { name: "b", type: "audio", mandatory: false },
        ];
      if (some(r, 0.3))
        s.outputs = [
          { name: "out", type: pick(r, ["audio", "midi"]) },
          { name: "wet", type: "audio" },
        ];
      if (some(r, 0.2)) s.events = ["note-on", "x"];
      if (some(r, 0.1)) s.pluginId = "p1";
      if (some(r, 0.1)) s.stage = ["a"];
      if (curveIds.length && some(r, 0.4))
        s.automations = [
          {
            param: "Gain",
            curve: pick(r, curveIds),
            ...(some(r, 0.3) ? { trigger: pick(r, mods) } : {}),
          },
        ];
      return s;
    });
    const target = () =>
      pick(r, [
        ...lanes,
        ...chainNames,
        ...mods,
        "OUT",
        "MIX",
        "RENDER",
        "VIDEO_OUT",
        `${pick(r, mods)}#b`,
        "ghost",
      ]);
    chains[name] = {
      ...(some(r, 0.8)
        ? { from: pick(r, [...lanes, ...chainNames, "MIX", "RENDER"]) }
        : {}),
      ...(some(r, 0.2) ? { fromPort: pick(r, ["midi", "audio"]) } : {}),
      use,
      out: Array.from({ length: int(r, 0, 3) }, () => ({
        to: target(),
        gain: pick(r, [1, 1, 0.5, 0.333]),
        ...(some(r, 0.2) ? { fromPort: "wet" } : {}),
      })),
    };
  }
  const g: Record<string, unknown> = {
    chains,
    tracks,
    videoTracks,
    trackColors: some(r, 0.5) ? { [tracks[0]]: "#abc" } : {},
    trackSignals: some(r, 0.5)
      ? { [tracks[0]]: pick(r, ["audio", "midi"]) }
      : {},
    modules: some(r, 0.4)
      ? {
          [pick(r, mods)]: {
            content: "a note\nwith lines and a very long line ".repeat(
              int(r, 1, 5),
            ),
          },
        }
      : {},
    warnings: [],
  };
  if (curveIds.length) g.curves = curves;
  if (some(r, 0.5))
    g.trackMixers = {
      [tracks[0]]: pick(r, mods),
      ...(tracks[1] ? { [tracks[1]]: pick(r, mods) } : {}),
    };
  if (some(r, 0.5))
    g.trackClips = {
      [tracks[0]]: Array.from({ length: int(r, 1, 9) }, (_, i) => ({
        ref: `c${i}`,
        start: i * 4,
        tc: `${i + 1}:1:1`,
        kind: pick(r, ["audio", "midi"]),
        ...(some(r, 0.7) ? { len: int(r, 1, 8) } : {}),
      })),
    };
  if (some(r, 0.3))
    g.trackAlternatives = {
      [tracks[0]]: { alternatives: ["a", "b"], alternative: "a" },
    };
  if (some(r, 0.3))
    g.frozen = { [pick(r, [...mods, ...tracks])]: pick(r, ["at", "inside"]) };
  if (some(r, 0.2)) g.autoFrozen = { [pick(r, mods)]: "vst" };
  if (some(r, 0.3))
    g.tapFeeds = [
      { node: pick(r, mods), tap: "BD", to: pick(r, [...mods, ...chainNames]) },
    ];
  if (some(r, 0.3)) g.stacks = [[pick(r, mods), pick(r, mods)]];
  if (some(r, 0.4))
    g.assemblies = {
      A: {
        uses: "tpl",
        label: "Box A",
        members: [pick(r, mods), pick(r, mods), pick(r, tracks)],
        inputs: [
          {
            name: "in",
            type: "audio",
            node: pick(r, mods),
            ...(some(r, 0.3) ? { port: "a" } : {}),
          },
        ],
        outputs: [{ name: "out", type: "audio", node: pick(r, mods) }],
        ...(some(r, 0.5)
          ? {
              params: {
                gain: { node: pick(r, mods), param: "Gain" },
                mix: { node: pick(r, mods), param: "Mix", pluginId: "p" },
              },
            }
          : {}),
        ...(some(r, 0.2) ? { missing: true } : {}),
      },
    };
  return g as unknown as WebGraph;
}

const OPTION_SETS = [
  {},
  { drawMainOut: true },
  { drawMainOut: false },
  { drawMainOut: true, mainOutVolume: -3, aspect: 1 },
  {
    aspect: 0,
    inputDevices: [
      { name: "Card", channels: [1] },
      { name: "Pair", channels: [1, 2] },
    ],
    cameras: ["Lens A", "Lens B"],
    midiDevices: ["Keys"],
  },
  { layout: "vertical" as const, fit: { axis: "rows" as const, max: 3 } },
  { fit: { axis: "cols" as const, max: 2 } },
  { layout: "vertical" as const },
  { fit: { axis: "rows" as const, max: 4 } },
];

const graphs: [string, WebGraph][] = [
  ...fixtures,
  ...Array.from({ length: 220 }, (_, i): [string, WebGraph] => [
    `random ${i}`,
    genGraph(1000 + i),
  ]),
];

const stat = {
  nodes: 0,
  edges: 0,
  ui: 0,
  folded: 0,
  tabH: 0,
  tabW: 0,
  clips: 0,
  rows: 0,
  rowNodes: 0,
  seams: 0,
  sides: 0,
  fits: 0,
};
afterAll(() => console.log(JSON.stringify(stat)));

describe("buildGraph against its twin", () => {
  it("has graphs to compare", () => {
    expect(fixtures.length).toBeGreaterThan(100);
  });
  for (const [name, graph] of graphs) {
    it(name, () => {
      OPTION_SETS.forEach((opts, i) => {
        const a = plain(oldBuild(graph, opts));
        const b = plain(buildGraph(graph, opts));
        expect(b, `${name} options #${i}`).toEqual(a);
        stat.nodes += b.nodes.length;
        stat.edges += b.edges.length;
        stat.ui += b.nodes.filter((n: { ui?: unknown }) => n.ui).length;
        stat.tabH += b.nodes.filter((n: { tabH?: unknown }) => n.tabH).length;
        stat.tabW += b.nodes.filter((n: { tabW?: unknown }) => n.tabW).length;
        stat.clips += b.nodes.filter(
          (n: { clips?: unknown }) => n.clips,
        ).length;
      });
      const open = new Set(Object.keys(graph.assemblies ?? {}).slice(0, 1));
      for (const opened of [new Set<string>(), open]) {
        const oldFolded = plain(oldFold(graph, oldBuild(graph, {}), opened));
        const newFolded = plain(buildGraph(graph, {}, [...opened]));
        expect(newFolded, `${name} fold`).toEqual(oldFolded);
        stat.folded += newFolded.nodes.filter(
          (n: { kind: string }) => n.kind === "assembly",
        ).length;
      }
    });
  }
});

describe("the arrangements against their twins", () => {
  const reachable = (g: WebGraph) => {
    const out: string[] = [];
    for (const c of Object.values(g.chains))
      for (const s of c.use) out.push(s.module);
    return [...new Set(out)];
  };
  for (const [name, graph] of graphs.slice(0, 260)) {
    it(`rows and stacks of ${name}`, () => {
      const r = rng(name.length * 7 + 3);
      const mods = reachable(graph);
      if (!mods.length) return;
      const picked = mods.filter(() => r() < 0.7);
      const track = some(r, 0.5) ? pick(r, graph.tracks) : undefined;
      const row = {
        links: some(r, 0.6),
        asDrawn: some(r, 0.4),
        stack: some(r, 0.4),
        curves: some(r, 0.4),
        widthScale: pick(r, [1, 1, 1.25]),
        gap: pick(r, [undefined, 0, -1, 10]),
        track,
        inputs: track ? mods.filter(() => r() < 0.3) : undefined,
        lead: some(r, 0.3) ? { id: "lead", label: "in", sub: "x" } : undefined,
        layout: some(r, 0.3) ? ("vertical" as const) : undefined,
        fit: some(r, 0.3) ? { axis: "rows" as const, max: 3 } : undefined,
        tabOf: some(r, 0.3) ? { [`mod:${mods[0]}`]: 1 } : undefined,
        devices: some(r, 0.3)
          ? { cameras: ["L"], midiDevices: ["K"] }
          : undefined,
      };
      const a = plain(oldRow(graph, picked, row));
      const b = plain(moduleRow(graph, picked, row));
      expect(b).toEqual(a);
      stat.rows++;
      stat.rowNodes += b.nodes.length;

      const built = oldBuild(graph, {});
      const aligned = plain(oldAlign(built.nodes, picked.slice(0, 3)));
      expect(
        plain(alignModuleRank(plain(built.nodes), picked.slice(0, 3))),
      ).toEqual(aligned);

      const laid = plain(oldRow(graph, picked, { links: true, asDrawn: true }));
      const space = pick(r, [picked.slice(0, 3), picked]);
      expect(plain(spaceModulesEvenly(laid, space))).toEqual(
        plain(oldSpace(laid, space)),
      );

      const order = graph.tracks.slice().reverse();
      const aStack = plain(oldStack(built.nodes, built.edges, graph, order));
      const bStack = plain(
        stackLayout(plain(built.nodes), plain(built.edges), graph, order),
      );
      expect(bStack).toEqual(aStack);
      stat.sides += bStack.edges.filter(
        (e: { side?: number }) => e.side !== undefined,
      ).length;

      // The seams of the glued row, tail and branches both.
      const glued = plain(
        oldRow(graph, picked, {
          links: true,
          asDrawn: true,
          gap: -1,
          stack: true,
          track,
        }),
      );
      for (const branch of [false, true]) {
        const shown = glued;
        const aOut = plain(
          oldSeams(shown, oldExit(glued, shown, graph), branch),
        );
        const bOut = plain(rowSeams(glued, shown, graph, branch));
        expect(bOut).toEqual(aOut);
        stat.seams += bOut.seams.length;
      }
    });
  }
});
