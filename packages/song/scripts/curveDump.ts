// curveDump.ts — the TypeScript's half of the curves gate: loadCurves, the runs, the firings and the bakes, dumped for Rust.
//
//   npx tsx scripts/curveDump.ts <staged dir> <dumps dir>      (from packages/song; stageCorpus.ts made the staged dir)
//
// What a reader gets wrong: the sweep and the wired cases are GENERATED here from a fixed seed, so a run is repeatable.
// It writes <dumps>/curves/{corpus.json, negative/, sweep/, sweep.json, wired/, wired.json} (docs/rustCompilerCurves.md).

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const [staged, dumps] = process.argv.slice(2);
if (!staged || !dumps)
  throw new Error("usage: curveDump.ts <staged dir> <dumps dir>");
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
// The environment the corpus was staged under, set before the song modules read it.
Object.assign(process.env, manifest.env);
const { bakeAdsr, bakeCurve, loadCurves } = await import("../src/curves.ts");
const { curveRuns, envelopeFires } = await import("../src/curveConditions.ts");
const { readYamlEntities } = await import("../src/entities.ts");
const { bakeAutomations } = await import("../src/compile/plugins.ts");

const out = join(dumps, "curves");
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "negative"), { recursive: true });
const message = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);
const write = (rel: string, value: unknown) =>
  writeFileSync(join(out, rel), JSON.stringify(value));

/** One project folder: its curves as loadCurves answers, or why it threw, and the files the schema refused. */
function dumpOf(abs: string) {
  let curves: Record<string, unknown> = {};
  let threw: string | null = null;
  try {
    curves = loadCurves(abs);
  } catch (e) {
    threw = message(e);
  }
  let files: string[] = [];
  try {
    const { issues } = readYamlEntities(abs, "curves");
    files = [...new Set(issues.map((i) => i.slice(0, i.indexOf(": "))))].sort();
  } catch {
    // A syntax error throws out of the read: the dump records `threw` above.
  }
  return { curves, threw, files };
}

const corpus = (manifest.projects as { abs: string; project: string }[]).map(
  (p) => ({ project: p.project, ...dumpOf(p.abs) }),
);
write("corpus.json", corpus);

// The negative corpus the Rust tests also hold: the cases the TypeScript can read, staged beside the golden ones.
type Content = string | { hex: string };
const negative = JSON.parse(
  readFileSync(
    join(import.meta.dirname, "../rust/tests/all/negative_curves.json"),
    "utf8",
  ),
) as { name: string; ts: boolean; files: Record<string, Content> }[];
let negatives = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "curves-negative", c.name);
  rmSync(base, { recursive: true, force: true });
  mkdirSync(base, { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    mkdirSync(dirname(join(base, rel)), { recursive: true });
    writeFileSync(
      join(base, rel),
      typeof v === "string" ? v : Buffer.from(v.hex, "hex"),
    );
  }
  write(`negative/${c.name}.json`, { project: c.name, ...dumpOf(base) });
  negatives++;
}

// --- the seeded sweep ---------------------------------------------------------

const SEED = 0x2e7f0a11;
const SIGS: [number, number][] = [
  [4, 4],
  [3, 4],
  [6, 8],
  [7, 8],
  [5, 2],
  [1, 1],
  [12, 16],
  [13, 16],
  [3, 2],
  [1, 64],
  [1, 4096],
];
// Past 342 beats an oscillator's grid is widened to its 2,048 points; 1e6 is where the grid is always the cap.
const SONGS = [0, 3, 8, 16, 33.5, 64, 100, 200, 400, 1000, 5000, 1e5, 1e6];
const SWEEP_CURVES = 3200;

function mulberry32(seed: number): () => number {
  let a = seed | 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rand = mulberry32(SEED);
const pick = <T>(xs: readonly T[]): T => xs[Math.floor(rand() * xs.length)];
const int = (lo: number, hi: number) => lo + Math.floor(rand() * (hi - lo + 1));
const chance = (p: number) => rand() < p;
/** A multiple of an eighth, so a sum of them is exact and both sides see the same beat. */
const eighths = (lo: number, hi: number) => int(lo * 8, hi * 8) / 8;

/** A chain of `n` fields, each from `lo`: the deep notation reads past four fields, each a quarter of the one before. */
const chain = (n: number, lo: number) =>
  Array.from({ length: n }, () => int(lo, lo + 2)).join(":");
const position = () => {
  switch (
    pick(["plain", "plain", "plain", "ticks", "zero", "four", "deep", "fine"])
  ) {
    case "ticks":
      return `${int(1, 9)}:${int(1, 4)}:${int(1, 2)}.${int(1, 900)}`;
    // Below the first beat: a field of 0 is a beat before the origin.
    case "zero":
      return pick([
        `0:${int(1, 4)}:1`,
        `${int(1, 3)}:0:${int(1, 2)}`,
        "0:1:1",
        "1:1:0",
      ]);
    case "four":
      return chain(4, 1);
    // Five to thirteen fields: the finest differences are far under a microbeat.
    case "deep":
      return chain(int(5, 13), 1);
    case "fine":
      return `${int(1, 4)}:1:1:1:1:1:${int(1, 9)}:${int(1, 9)}`;
    default:
      return `${int(1, 9)}:${int(1, 4)}:${int(1, 2)}`;
  }
};
const DURATIONS = ["0", "0:2", "1:0", "2:2", "0:1:2", "3:0"];
const EVENTS = ["Note On", "noteOn", "note-on", "Note Off", "Pitch"];

function conditionText(): string {
  const re = chance(0.5) ? `.retrigger(${chance(0.7)})` : "";
  switch (pick(["start", "end", "between", "event", "event"])) {
    case "start":
      return `type("start").at("${position()}")${re}`;
    case "end":
      return `type("end").at("${position()}")${re}`;
    case "between":
      return `type("between").start("${position()}").end("${position()}")${re}`;
    default:
      return `type("event").event("${pick(EVENTS)}")${re}`;
  }
}

function pointsText(onoff: boolean): string {
  const n = int(1, 6);
  const pts = Array.from({ length: n }, () => {
    const v = onoff ? int(0, 1) : pick([0, 0.25, 0.5, 1, 0.8, 1.5, -0.5]);
    return `time("${position()}").value(${v})`;
  });
  return `points([${pts.join(", ")}])`;
}

/** One curve file: any kind, any shape, polarity, amplitude, cut, duration, one to four conditions. */
function curveText(): string {
  const kind = pick(["lfo", "custom", "onoff", "adsr"]);
  const lines = [`type("${kind}")`];
  const shaped = kind !== "onoff";
  if (shaped && chance(0.5))
    lines.push(`polarity("${pick(["unipolar", "bipolar"])}")`);
  if (shaped && chance(0.4))
    lines.push(`amplitude(${pick([0.25, 0.5, 0.75, 1])})`);
  if (shaped && chance(0.4))
    lines.push(`cut("${pick(["none", "positive", "negative"])}")`);
  if (kind === "lfo") {
    if (chance(0.6))
      lines.push(`shape("${pick(["sine", "triangle", "saw", "square"])}")`);
    if (chance(0.5))
      lines.push(`frequency(${pick([0.25, 0.5, 1, 1.5, 2, 4])})`);
    if (chance(0.4)) lines.push(`phase(${pick([0, 0.25, 0.5, 0.75])})`);
    if (chance(0.4)) lines.push(`duration("${pick(DURATIONS)}")`);
  } else if (kind === "adsr") {
    for (const k of ["attack", "decay", "sustain", "release"])
      if (chance(0.8))
        lines.push(`${k}(${pick([0, 0, 50, 200, 500, 1000, 2500])})`);
    if (chance(0.6)) lines.push(`level(${pick([0, 0.25, 0.5, 0.8, 1])})`);
  } else lines.push(pointsText(kind === "onoff"));
  if (chance(0.55))
    lines.push(
      `conditions([${Array.from({ length: int(1, 4) }, conditionText).join(", ")}])`,
    );
  return `${lines.join("\n")}\n`;
}

const triggers = (song: number): number[] => {
  const top = Math.max(song * 1.3, 20);
  // Off the eighth grid, and a microbeat apart: the sort and the merge see ties and near ties.
  const off = chance(0.4);
  const list = Array.from({ length: int(1, 12) }, () =>
    off ? rand() * top : eighths(0, top),
  );
  if (off && chance(0.5)) list.push(list[0] + pick([1e-7, 4e-7, 1e-6, 2e-6]));
  switch (pick(["empty", "sorted", "unsorted", "duplicated", "past"])) {
    case "empty":
      return [];
    case "sorted":
      return list.sort((a, b) => a - b);
    case "duplicated":
      return [...list, ...list].sort((a, b) => a - b);
    case "past":
      return [...list.map((b) => b + song + 50)].sort((a, b) => a - b);
    default:
      return list;
  }
};

const sweepDir = join(out, "sweep");
mkdirSync(join(sweepDir, "curves"), { recursive: true });
for (let i = 0; i < SWEEP_CURVES; i++)
  writeFileSync(join(sweepDir, "curves", `s${i}.k7x`), curveText());
const swept = loadCurves(sweepDir);

// biome-ignore lint/suspicious/noExplicitAny: the fakes stand in for types this script does not import.
type Anything = any;
const finite = (baked: { points: [number, number][] }, id: string) => {
  for (const p of baked.points)
    if (!(Number.isFinite(p[0]) && Number.isFinite(p[1])))
      throw new Error(
        `curve ${id}: the TypeScript baked a point that is not finite`,
      );
};

const entries = [];
for (let i = 0; i < SWEEP_CURVES; i++) {
  const id = `s${i}`;
  const curve = swept[id] as Anything;
  const [num, den] = pick(SIGS);
  const tempo = chance(0.5) ? int(60, 240) : 60 + rand() * 180;
  const songBeats = pick(SONGS);
  const trig = triggers(songBeats);
  const bar = num * (4 / den);
  const end = Math.max(songBeats, bar * 4);
  const adsr = curve.type === "adsr";
  const runs = adsr ? null : curveRuns(curve.conditions, end, trig, num, den);
  const fires = adsr ? envelopeFires(curve.conditions, trig, num, den) : null;
  const baked = adsr
    ? bakeAdsr(curve, fires as Anything, tempo)
    : bakeCurve(curve, num, den, songBeats, trig);
  finite(baked, id);
  entries.push({
    id,
    sig: [num, den],
    tempo,
    song_beats: songBeats,
    spec: curve,
    triggers: trig,
    runs,
    fires,
    baked,
  });
}
write("sweep.json", { seed: SEED, entries });

// --- the wired cases: the real bakeAutomations over fake chains and tracks ------------

const wiredDir = join(out, "wired");
mkdirSync(join(wiredDir, "curves"), { recursive: true });
const WIRED_CURVES = 60;
const wiredIds: string[] = [];
for (let i = 0; i < WIRED_CURVES; i++) {
  const id = `s${i * Math.floor(SWEEP_CURVES / WIRED_CURVES)}`;
  wiredIds.push(id);
  writeFileSync(
    join(wiredDir, "curves", `${id}.k7x`),
    readFileSync(join(sweepDir, "curves", `${id}.k7x`)),
  );
}

function clipText() {
  const looped = chance(0.4);
  // A looped clip of length 0 never ends in the TypeScript (decision 211), so it is left to the Rust tests.
  const length = looped ? pick([4, 8, 16]) : pick([0, 4, 8, 16]);
  const loopStart = looped ? eighths(0, 2) : 0;
  const loopEnd = looped ? loopStart + eighths(0.5, 4) : 0;
  const notes = Array.from({ length: int(0, 8) }, () => ({
    time: eighths(0, 12),
  }));
  return {
    type: "midi",
    start: eighths(0, 24),
    length,
    loopStart,
    loopEnd,
    notes,
  };
}

const wired = [];
const WIRED_CASES = 220;
for (let n = 0; n < WIRED_CASES; n++) {
  const [num, den] = pick(SIGS);
  const tempo = chance(0.5) ? int(60, 240) : 60 + rand() * 180;
  const lanes = int(1, 3);
  const tracks = Array.from({ length: lanes }, (_, j) => ({
    id: `t${j}`,
    clips: [
      ...Array.from({ length: int(0, 3) }, clipText),
      ...(chance(0.2)
        ? [{ type: "audio", start: 0, length: 8, notes: [] }]
        : []),
    ],
    automation: [] as unknown[],
  }));
  const chains = Object.fromEntries(
    tracks.map((t, j) => [
      `chain${j}`,
      { owner: t.id, use: [{ module: `m${j}a` }, { module: `m${j}b` }] },
    ]),
  );
  const modules = tracks.flatMap((_, j) => [`m${j}a`, `m${j}b`]);
  const bindings = Array.from({ length: int(1, 4) }, (_, k) => {
    const owner = chance(0.05) ? "ghost" : `t${int(0, lanes - 1)}`;
    return {
      owner,
      pluginId: `chain:${owner}:${k}`,
      param: `p${k}`,
      curve: chance(0.05) ? "missing" : pick(wiredIds),
      trigger: chance(0.2) ? "nobody" : pick(modules),
    };
  });
  const input = JSON.parse(JSON.stringify({ tracks, chains, bindings }));
  bakeAutomations(
    wiredDir,
    { chains } as Anything,
    { ownerOf: (_name: string, c: Anything) => c.owner } as Anything,
    bindings,
    tracks as Anything,
    num,
    den,
    tempo,
  );
  for (const t of tracks)
    finite(
      { points: t.automation.flatMap((a: Anything) => a.points) },
      `wired ${n}`,
    );
  wired.push({
    sig: [num, den],
    tempo,
    ...input,
    out: Object.fromEntries(tracks.map((t) => [t.id, t.automation])),
  });
}
write("wired.json", { cases: wired });

console.log(
  `${manifest.projects.length} projects, ${negatives} negative cases, ${SWEEP_CURVES} swept curves and ${WIRED_CASES} wired cases dumped in ${out}`,
);
