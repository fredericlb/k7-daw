// graphSweep.ts — one small seeded project for the graph gate's sweep: nodes drawn from a pool, random edges, assets on disk.
//
// What a reader gets wrong: it makes projects the golden corpus lacks (assemblies, stacks, positions, tap feeds, strudel lanes), and
// some of them are meant to be refused. Names are ASCII, lower case, never integer-like, and at most 40 characters (decision 412).

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { writeDoc } from "../src/testSupport.ts";

/** A deterministic generator: the same seed draws the same project. */
function mulberry32(a: number): () => number {
  let s = a >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type Doc = Record<string, unknown>;
type Pool = (name: string, r: () => number) => { doc: Doc; role: Role };
type Role = "lane" | "source" | "step" | "sink" | "note";

const AMP = { plugin: { type: "internal", name: "Chorus" } };
const TWO_IN = {
  plugin: { type: "jsdsp" },
  inputs: [
    { name: "a", type: "audio" },
    { name: "b", type: "audio", mandatory: false },
  ],
  outputs: [
    { name: "out", type: "audio" },
    { name: "aux", type: "audio" },
  ],
};

const pick = <T>(r: () => number, xs: readonly T[]): T =>
  xs[Math.floor(r() * xs.length)];

const LANES: Pool[] = [
  (_n, r) => ({
    role: "lane",
    doc: {
      extends: "@meta/track",
      events: [
        { ref: "c1", start: "1:1" },
        ...(r() < 0.5 ? [{ ref: "s1", start: "2:1" }] : []),
      ],
      ...(r() < 0.3 ? { color: "#336699" } : {}),
    },
  }),
  () => ({
    role: "lane",
    doc: { extends: "@meta/track", events: [{ ref: "s1", start: "1:1" }] },
  }),
  () => ({
    role: "lane",
    doc: { extends: "@meta/track", events: [{ ref: "gone", start: "1:1" }] },
  }),
  () => ({ role: "lane", doc: { extends: "@meta/track", disabled: true } }),
  () => ({ role: "lane", doc: { extends: "@meta/picture-track" } }),
  () => ({
    role: "lane",
    doc: { extends: "@meta/strudel-track", strudel: "algo/pat.strudel" },
  }),
];
const SOURCES: Pool[] = [
  () => ({ role: "source", doc: { io: "in", channel: [1] } }),
  () => ({ role: "source", doc: { io: "in", channel: "synth" } }),
  () => ({ role: "source", doc: { io: "in" } }),
  () => ({ role: "source", doc: { io: "camera" } }),
  () => ({ role: "source", doc: { io: "midi", device: "Keys" } }),
  () => ({ role: "source", doc: { io: "file", source: "clips/c1.wav" } }),
];
const STEPS: Pool[] = [
  () => ({ role: "step", doc: AMP }),
  () => ({
    role: "step",
    doc: {
      plugin: { type: "internal", name: "Chorus" },
      mix: 0.5,
      params: { Depth: 1 },
    },
  }),
  () => ({
    role: "step",
    doc: { plugin: { type: "VST3", name: "Nothing Here" } },
  }),
  () => ({
    role: "step",
    doc: { plugin: { type: "not_supported", name: "X" } },
  }),
  () => ({
    role: "step",
    doc: {
      plugin: { type: "offline", name: "Denoise" },
      params: { model: "net.onnx" },
      model_url: "https://example.com/net.onnx",
    },
  }),
  () => ({ role: "step", doc: TWO_IN }),
  () => ({
    role: "step",
    doc: {
      plugin: { type: "internal", name: "Sampler" },
      type: "rack",
      rack: { rack_type: "samples", symbols: { BD: { note: 36, clip: "c1" } } },
    },
  }),
  () => ({
    role: "step",
    doc: {
      plugin: { type: "internal", name: "Sfz" },
      kit: { kick: 36, snare: 38 },
      params: { "Mute 38": 1 },
    },
  }),
  () => ({
    role: "step",
    doc: { extends: "@k7-core/shaders/blur", shader: "soft" },
  }),
  () => ({ role: "step", doc: { extends: "@k7-core/video_transitions/wipe" } }),
  () => ({ role: "step", doc: { extends: "@meta/audio/mixer" } }),
  () => ({
    role: "step",
    doc: { ...AMP, alternatives: [{ name: "wide", params: { Depth: 1 } }] },
  }),
  () => ({
    role: "step",
    doc: {
      ...AMP,
      automations: [
        { param: "Depth", curve: "slow" },
        { param: "Rate", curve: "ghost", trigger: "x" },
      ],
    },
  }),
  () => ({
    role: "step",
    doc: { extends: "@k7-core/audio/surge", base: "warm" },
  }),
  () => ({ role: "step", doc: { ...AMP, channel: [2] } }),
  () => ({ role: "step", doc: { extends: "no-such-ref" } }),
  () => ({
    role: "step",
    doc: {
      plugin: { type: "internal", name: "Amp" },
      params: { model: "gone.nam" },
    },
  }),
];
const NOTES: Pool[] = [
  () => ({ role: "note", doc: { extends: "@meta/doc", content: "a note" } }),
];

const SINKS = [
  "OUT",
  "MIX",
  "RENDER",
  "RENDER#in",
  "RENDER#audio",
  "RENDER#bogus",
  "CUE",
];

/** Writes one project under `proj` (and its libraries under `lib`) from `seed`. */
export function sweepProject(
  proj: string,
  lib: string,
  seed: number,
  index: number,
): void {
  const r = mulberry32(seed);
  const put = (rel: string, text: string) => {
    mkdirSync(dirname(join(proj, rel)), { recursive: true });
    writeFileSync(join(proj, rel), text);
  };
  writeDoc(join(proj, "project.k7x"), {
    project: {
      name: `sweep-${index}`,
      tempo: 90 + Math.floor(r() * 60),
      timeSignature: pick(r, ["4/4", "3/4", "7/8"]),
    },
  });
  put("clips/c1.k7x", 'source("clips/c1.wav")\n');
  put("clips/c1.wav", "RIFF");
  put("sequences/s1.k7x", "notes([t(0).d(1).k(60)])\n");
  put("algo/pat.strudel", "s('bd')\n");
  put("curves/slow.k7x", 'type("lfo")\nshape("sine")\nfrequency(1)\n');
  // Assets vary by project, so a step's `assetChoices` is a fact of the project and not of every step.
  for (const a of ["net.onnx", "amp.nam", "sub/net2.json"])
    if (r() < 0.5) put(`nodes/assets/${a}`, "x");
  // A library of the shapes the pool names: a shader with one whole stage pair and a half one, a transition, a synth with bases.
  writeDoc(join(lib, "k7-core", "modules", "shaders", "blur", "module.k7x"), {
    type: "shader",
  });
  for (const [s, files] of [
    ["soft", ["vertex.glsl", "fragment.glsl"]],
    ["half", ["vertex.glsl"]],
  ] as const)
    for (const f of files) {
      const p = join(
        lib,
        "k7-core",
        "modules",
        "shaders",
        "blur",
        "shaders",
        s,
        f,
      );
      mkdirSync(dirname(p), { recursive: true });
      writeFileSync(p, "x");
    }
  writeDoc(
    join(lib, "k7-core", "modules", "video_transitions", "wipe", "module.k7x"),
    { type: "video_transition" },
  );
  writeDoc(join(lib, "k7-core", "modules", "audio", "surge", "module.k7x"), {
    plugin: { type: "internal", name: "Chorus" },
  });
  for (const b of ["warm.vstpreset", "Pads/Bell Pad.fxp"]) {
    const p = join(lib, "k7-core", "modules", "audio", "surge", "base", b);
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, "x");
  }
  // One project in six is in the chain-file form: its nodes are unnamed, so `place` and `stacks` are drawn.
  if (r() < 1 / 6) {
    chainForm(put, r);
    return;
  }
  const asm = join(lib, "k7-core", "assemblies", "turntable");
  mkdirSync(asm, { recursive: true });
  writeFileSync(
    join(asm, "assembly.k7x"),
    'label("Turntable")\noutputs([name("mix").type("audio").node("level")])\nparams({ Level: { node: "level", param: "Gain" } })\n',
  );
  const nodes: { name: string; role: Role }[] = [];
  const count = 3 + Math.floor(r() * 12);
  // A second generator freezes some plugins, so the main stream draws what it always did.
  const frost = mulberry32(seed ^ 0x6f1e5a);
  for (let n = 0; n < count; n++) {
    const pool = pick(r, [LANES, LANES, SOURCES, STEPS, STEPS, STEPS, NOTES]);
    const name = `n${n}${pick(r, ["a", "b", "c"])}`;
    const { doc, role } = pick(r, pool)(name, r);
    const frozen = role === "step" && !!doc.plugin && frost() < 0.35;
    // A render laid for most of them: a plan stands over a render that is there and not over one that is not.
    if (frozen && frost() < 0.6) put(`renders/${name}.wav`, "RIFF");
    writeDoc(
      join(proj, "nodes", `${name}.k7x`),
      frozen ? { ...doc, frozen: `renders/${name}.wav` } : doc,
    );
    nodes.push({ name, role });
  }
  const modules: Record<string, string[]> = {};
  const all = nodes.map((n) => n.name);
  for (const n of nodes) {
    if (n.role === "sink" || n.role === "note") continue;
    if (n.role === "step" && r() < 0.2) continue;
    const key =
      n.role === "lane" && r() < 0.15
        ? `${n.name}#${pick(r, ["audio", "midi"])}`
        : n.name;
    const dests: string[] = [];
    for (let d = 0; d < 1 + Math.floor(r() * 2); d++) {
      const t = r();
      if (t < 0.3) dests.push(pick(r, SINKS));
      else if (t < 0.4 && nodes.some((x) => x.role === "lane"))
        dests.push(
          pick(
            r,
            nodes.filter((x) => x.role === "lane"),
          ).name,
        );
      else if (t < 0.5) dests.push(`${pick(r, all)}#b`);
      else if (t < 0.55) dests.push("ghost");
      else if (t < 0.6) dests.push(n.name);
      else dests.push(pick(r, all));
    }
    modules[key] = [...new Set(dests)];
  }
  const groups = nodes.filter((n) => n.role === "lane" && r() < 0.2);
  if (groups.length) {
    const tracks = groups
      .map((g) => g.name)
      .concat(r() < 0.3 ? ["nobody"] : [], r() < 0.2 ? ["grp"] : []);
    writeDoc(join(proj, "nodes", "grp.k7x"), {
      extends: "@meta/group-track",
      tracks,
    });
    if (r() < 0.3)
      writeDoc(join(proj, "nodes", "grq.k7x"), {
        extends: "@meta/group-track",
        tracks: groups.map((g) => g.name),
      });
    if (r() < 0.3) modules["@in"] = ["OUT"];
  }
  const graph: Record<string, unknown> = { modules };
  if (r() < 0.3) graph.bypassed = [pick(r, all)];
  if (r() < 0.3)
    graph.positions = Object.fromEntries(
      all
        .slice(0, 1 + Math.floor(r() * 3))
        .map((n) => [
          n,
          { x: Math.floor(r() * 400), y: Math.floor(r() * 300) },
        ]),
    );
  // A rack that taps one sound out to a node some chain runs, drawn as a cable.
  const target = Object.keys(modules).find(
    (k) => !k.includes("#") && !SINKS.includes(k),
  );
  if (target && r() < 0.25) {
    writeDoc(join(proj, "nodes", "tapr.k7x"), {
      plugin: { type: "internal", name: "Sampler" },
      type: "rack",
      rack: { rack_type: "samples", symbols: { BD: { note: 36, clip: "c1" } } },
      tap_outs: { BD: target },
    });
    graph.tap_feeds = [{ node: "tapr", tap: "BD", to: target }];
  }
  writeDoc(join(proj, "graph.k7x"), graph);
  // Boxes: two instances of a template that name one node make two assemblies sharing a member.
  if (r() < 0.25) {
    const members = (): Record<string, string> => ({
      level: pick(r, all),
      fade: r() < 0.15 ? "ghost-node" : pick(r, all),
    });
    const first = members();
    writeDoc(join(proj, "assemblies", "boxa.k7x"), {
      uses: "@k7-core/turntable",
      members: first,
    });
    if (r() < 0.5)
      writeDoc(join(proj, "assemblies", "boxb.k7x"), {
        uses: "@k7-core/turntable",
        members: r() < 0.7 ? { level: first.level } : members(),
      });
    // A box named after a node: one flat key space.
    if (r() < 0.2)
      writeDoc(join(proj, "assemblies", `${pick(r, all)}.k7x`), {
        uses: "@k7-core/turntable",
        members: { level: pick(r, all) },
      });
  }
}

/** The chain-file form: lanes in `tracks/`, one chain with unnamed steps, a side-by-side step and a subchain. */
function chainForm(
  put: (rel: string, text: string) => void,
  r: () => number,
): void {
  const chorus = 'plugin({ type: "internal", name: "Chorus" })';
  put(
    "tracks/a.k7x",
    'extends("@meta/track")\nevents([ref("c1").start("1:1")])\n',
  );
  const steps = ['id("l1").track("a")', `id("p1").${chorus}`];
  if (r() < 0.7)
    steps.push(
      `[id("q1").${chorus}, id("q2").extends("@k7-core/audio/surge")]`,
    );
  const sub = r() < 0.7;
  if (sub) steps.push('{ sub: "s" }');
  steps.push(`to([chain("OUT").level(${pick(r, [0.5, 1])})])`);
  put(
    "chains/main.k7x",
    `start(true)\nsteps([${steps.join(", ")}])\n` +
      (sub
        ? `subchains([{ id: "s", steps: [id("d1").${chorus}, id("d2").${chorus}] }])\n`
        : ""),
  );
  if (r() < 0.5) put("layout.k7x", "positions({ a: { x: 10, y: 20 } })\n");
}
