// stageDumpSweeps.ts — the fields lattice and the two sweeps of the views' gate: dumped beside the project dumps.
//
//   run by stageDumpViews.ts, with its arguments: <staged dir> <dumps dir> [--sweep-clips 1500 --sweep-wav 2000 --seed 0x2d4a0e77]
//
// What a reader gets wrong: the sweeps are drawn from a seed, so a difference reproduces; the lattice is every kind over
// every path of up to three segments of a pool, which is where the editor's completion lists can drift.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  fieldsAt,
  fileKindFor,
  valuesAt,
} from "@studio/song-schema/yaml/fields";

const argv = process.argv.slice(2);
const dumps = argv[1];
const flag = (name: string, fallback: number): number => {
  const at = argv.indexOf(name);
  return at < 0 ? fallback : Number(argv[at + 1]);
};
const SWEEP_CLIPS = flag("--sweep-clips", 1500);
const SWEEP_WAV = flag("--sweep-wav", 2000);
const SEED = flag("--seed", 0x2d4a0e77);
const { toWebTracks } = await import("./dumpHosts/webTracks.ts");
const { wavSeconds } = await import(
  new URL("../../take-render/src/wavLength.ts", import.meta.url).href
);
const slashes = (p: string): string =>
  p
    .replaceAll("\\", "/")
    .replace(/^([a-z]):/, (_, d: string) => `${d.toUpperCase()}:`);

// The fields lattice: every kind over every path of up to three segments of a pool, and the named deeper paths.
const KINDS = [
  "module",
  "chains",
  "chain",
  "clip",
  "sequence",
  "curve",
  "project",
] as const;
const POOL =
  "project warp notes points events fadeIn fadeOut alternatives 0 ui widgets w tabs rack symbols s steps to subchains plugin params x".split(
    " ",
  );
const NAMED = [
  "ui/tabs/0/widgets/w",
  "steps/0/to",
  "subchains/0/steps/0",
  "subchains/s/steps/ui/widgets/w",
  "alternatives/0/name",
  "steps/ui/widgets/w",
  "subchains/steps/to",
  "subchains/steps/events/fadeIn",
];
const FIELD_POOL =
  "type io shape monitor curve source rack_type polarity start x".split(" ");
const paths: string[][] = [[]];
for (const a of POOL) {
  paths.push([a]);
  for (const b of POOL) {
    paths.push([a, b]);
    for (const c of POOL) paths.push([a, b, c]);
  }
}
for (const n of NAMED) paths.push(n.split("/"));
const fields: Record<string, string[]> = {};
const values: Record<string, string[]> = {};
for (const k of KINDS) {
  for (const p of paths) {
    const f = fieldsAt(k, p);
    if (f.length) fields[`${k}|${p.join("/")}`] = f;
  }
  for (const p of [
    [],
    ...POOL.map((s) => [s]),
    ...NAMED.map((n) => n.split("/")),
  ])
    for (const field of FIELD_POOL) {
      const v = valuesAt(k, p, field);
      if (v.length) values[`${k}|${p.join("/")}|${field}`] = v;
    }
}
const FOLDERS = [
  "modules",
  "nodes",
  "tracks",
  "chains",
  "clips",
  "sequences",
  "curves",
  "Modules",
  "CLIPS",
  "Chains",
];
const TAILS = [
  "x.k7x",
  "graph.k7x",
  "chains.k7x",
  "project.k7x",
  "x.yaml",
  "x",
  "X.K7X",
  "Graph.K7x",
  "a/graph.k7x",
  ".k7x",
  "PROJECT.K7X",
];
const lead = ["", "a/", "/", "a/b/", "..\\", "x\\"];
const files: Record<string, string | null> = {};
let q = 0;
for (const l of lead)
  for (const f of FOLDERS)
    for (const t of TAILS) {
      if (q++ % 3 !== 0) continue;
      const p = `${l}${f}/${t}`;
      files[p] = fileKindFor(p) ?? null;
    }
for (const p of [
  "chains.k7x",
  "project.k7x",
  "Chains.K7X",
  "dir\\chains.k7x",
  "",
  "xchains.k7x",
  "a/project.k7x",
  "clips",
  "curves/",
])
  files[p] = fileKindFor(p) ?? null;
writeFileSync(
  join(dumps, "fields.json"),
  JSON.stringify({
    pool: POOL,
    named: NAMED,
    fieldPool: FIELD_POOL,
    fields,
    values,
    files,
  }),
);

// mulberry32, from the seed.
let state = SEED >>> 0;
const rand = (): number => {
  state = (state + 0x6d2b79f5) >>> 0;
  let t = state;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = <T>(xs: T[]): T => xs[Math.floor(rand() * xs.length)];
const chance = (p: number): boolean => rand() < p;
const int = (lo: number, hi: number): number =>
  lo + Math.floor(rand() * (hi - lo + 1));

/** A RIFF/WAVE image whose pieces the sweep varies; `parts` is the chunk list after `WAVE`. */
function chunk(id: string, body: Buffer, size = body.length): Buffer {
  const h = Buffer.alloc(8);
  h.write(id, 0, "ascii");
  h.writeUInt32LE(size >>> 0, 4);
  const pad =
    body.length & 1 && size === body.length ? Buffer.alloc(1) : Buffer.alloc(0);
  return Buffer.concat([h, body, pad]);
}
function fmt(
  tag: number,
  align: number,
  rate: number,
  extensible = false,
): Buffer {
  const b = Buffer.alloc(extensible ? 40 : 16);
  b.writeUInt16LE(tag, 0);
  b.writeUInt16LE(2, 2);
  b.writeUInt32LE(rate, 4);
  b.writeUInt32LE(rate * align, 8);
  b.writeUInt16LE(align, 12);
  b.writeUInt16LE(24, 14);
  if (extensible) b.writeUInt16LE(pick([1, 3, 2]), 24);
  return b;
}
function wavFile(seconds: number): Buffer {
  const rate = 48000;
  const frame = 6;
  const data = Math.round(rate * seconds) * frame;
  return Buffer.concat([
    Buffer.from("RIFF\0\0\0\0WAVE", "binary"),
    chunk("fmt ", fmt(1, frame, rate)),
    chunk("data", Buffer.alloc(data)),
  ]);
}

const wavDir = join(dumps, "sweep", "wav");
mkdirSync(wavDir, { recursive: true });
const wavExpected: Record<string, number | null> = {};
for (let i = 0; i < SWEEP_WAV; i++) {
  const kind = int(0, 9);
  let bytes: Buffer;
  const tag = pick([1, 3, 1, 3, 0xfffe, 2, 0x55]);
  const align = pick([6, 4, 8, 3, 0]);
  const rate = pick([48000, 44100, 96000, 0]);
  const dataLen = int(0, 40) * 48;
  const fmtBody = fmt(tag, align, rate, tag === 0xfffe);
  const head = Buffer.from("RIFF\0\0\0\0WAVE", "binary");
  const data = (declared: number | null) =>
    chunk("data", Buffer.alloc(dataLen), declared ?? dataLen);
  switch (kind) {
    case 0:
      bytes = Buffer.concat([head, chunk("fmt ", fmtBody), data(null)]);
      break;
    case 1:
      bytes = Buffer.concat([
        head,
        chunk("LIST", Buffer.alloc(int(0, 9))),
        chunk("fmt ", fmtBody),
        data(null),
      ]);
      break;
    case 2:
      bytes = Buffer.concat([head, chunk("fmt ", fmtBody), data(0)]);
      break;
    case 3:
      bytes = Buffer.concat([
        head,
        chunk("fmt ", fmtBody),
        data(dataLen + int(1, 100000)),
      ]);
      break;
    case 4:
      bytes = Buffer.concat([
        head,
        chunk("fmt ", fmtBody.subarray(0, int(0, 15))),
      ]);
      break;
    case 5:
      bytes = Buffer.concat([head, chunk("fmt ", fmtBody)]);
      break;
    case 6:
      bytes = Buffer.concat([head, data(null), chunk("fmt ", fmtBody)]);
      break;
    case 7:
      bytes = Buffer.from(
        Array.from({ length: int(0, 60) }, () => int(0, 255)),
      );
      break;
    case 8:
      bytes = Buffer.concat([
        head.subarray(0, int(0, 12)),
        chunk("fmt ", fmtBody),
        data(null),
      ]);
      break;
    default:
      bytes = wavFile(int(0, 40) / 10);
      break;
  }
  const file = join(wavDir, `w${i}.wav`);
  writeFileSync(file, bytes);
  wavExpected[`w${i}.wav`] = wavSeconds(file);
}
writeFileSync(join(dumps, "wav.json"), JSON.stringify(wavExpected));

// The clips sweep: lists of ClipDocs written beside real WAV files, and what toWebTracks makes of each.
const audioDir = join(dumps, "sweep", "audio");
mkdirSync(audioDir, { recursive: true });
const sources: string[] = [];
for (let i = 0; i < 12; i++) {
  const file = join(audioDir, `a${i}.wav`);
  writeFileSync(file, wavFile(int(1, 90) / 10));
  sources.push(slashes(file));
}
const notWav = slashes(join(audioDir, "notes.txt"));
writeFileSync(notWav, "not a wav");
const missing = slashes(join(audioDir, "gone.wav"));
const base = (id: string, over: Record<string, unknown> = {}) => ({
  id,
  type: "audio",
  name: id.split("@")[0],
  source: "",
  start: 0,
  length: 0,
  offset: 0,
  fadeIn: 0,
  fadeOut: 0,
  loopStart: 0,
  loopEnd: 0,
  notes: [],
  ...over,
});
let serial = 0;
const audioClip = (id: string, start: number): Record<string, unknown> => {
  const over: Record<string, unknown> = {
    start,
    source: pick([...sources, ...sources, notWav, missing]),
    length: chance(0.4) ? int(1, 8) : 0,
  };
  if (chance(0.3)) over.sourceFrom = int(0, 120) / 10;
  if (chance(0.2)) over.sourceTo = int(1, 120) / 10;
  if (chance(0.3)) over.offset = int(0, 20) / 10;
  if (chance(0.3)) over.fadeIn = int(1, 8) / 4;
  if (chance(0.3)) over.fadeOut = int(1, 8) / 4;
  if (chance(0.15)) over.warp = { algorithm: "elastique", markers: [[0, 0]] };
  if (chance(0.1)) over.warpOff = true;
  if (chance(0.1)) over.disabled = true;
  if (chance(0.1)) over.loopEnd = int(1, 4);
  if (chance(0.1)) over.event = { from: int(0, 5), gain: 1 };
  return base(id, over);
};
const midiClip = (id: string, start: number): Record<string, unknown> =>
  base(id, {
    type: "midi",
    start,
    length: chance(0.5) ? int(1, 8) : 0,
    loopEnd: chance(0.2) ? 2 : 0,
    notes: Array.from({ length: int(0, 5) }, (_, n) => ({
      time: n * 0.5,
      duration: 0.5,
      key: int(36, 84),
      vel: chance(0.4) ? 1 : int(1, 99) / 100,
      channel: 0,
      rel: 0,
      ...(chance(0.2) ? { repeat: int(2, 4) } : {}),
      ...(chance(0.2) ? { skip: int(2, 4) } : {}),
    })),
  });
const leaf = (id: string, start: number) =>
  chance(0.7) ? audioClip(id, start) : midiClip(id, start);
function laneOf(): Record<string, unknown>[] {
  const out: Record<string, unknown>[] = [];
  const items = int(3, 12);
  for (let i = 0; i < items; i++) {
    const kind = int(0, 9);
    const at = int(0, 64);
    const id = `c${serial++}@${at}`;
    if (kind <= 4) {
      const c = leaf(chance(0.1) ? `${id}__frozen` : id, at);
      if (chance(0.1)) c.crossfade = { in: true };
      out.push(c);
    } else if (kind <= 7) {
      const of = `ev${serial++}@${at}`;
      const pieces = Array.from({ length: int(1, 5) }, (_, n) =>
        leaf(`${of}#${n}`, at + n * 2 + (chance(0.3) ? -3 : 0)),
      );
      const ordered = chance(0.5) ? pieces : pieces.reverse();
      for (const p of ordered)
        out.push({
          ...p,
          comp: { of, take: `t${int(0, 3)}` },
          ...(chance(0.2) ? { crossfade: { out: true } } : {}),
        });
    } else {
      const of = `tl${serial++}@${at}`;
      const nest = {
        of,
        ref: `tl${int(0, 3)}`,
        start: at,
        length: int(1, 16),
        ...(chance(0.4)
          ? { event: chance(0.5) ? { disabled: true } : { gain: 2 } }
          : {}),
      };
      const inner = int(1, 4);
      const allMidi = chance(0.3);
      const cOf = `${of}>g@0`;
      for (let n = 0; n < inner; n++) {
        const c = allMidi
          ? midiClip(`${of}>m${n}@0`, at + n)
          : leaf(`${of}>l${n}@0`, at + n);
        out.push(
          chance(0.3)
            ? { ...c, id: `${cOf}${n}`, comp: { of: cOf, take: `t${n}` }, nest }
            : { ...c, nest },
        );
      }
    }
  }
  return out;
}
const lists: { tempo: number | null; clips: unknown[]; web: unknown[] }[] = [];
for (let i = 0; i < SWEEP_CLIPS; i++) {
  const tempo = chance(0.1) ? null : pick([60, 90, 110, 120, 140]);
  const clips = laneOf();
  const track = { id: "t", name: "t", clips, plugins: [] };
  const web = toWebTracks([track], tempo ?? undefined)[0].clips;
  lists.push({ tempo, clips, web: JSON.parse(JSON.stringify(web)) });
}
writeFileSync(join(dumps, "clips.json"), JSON.stringify(lists));
console.log(
  `${Object.keys(fields).length} lattice entries, ${SWEEP_WAV} wavs and ${SWEEP_CLIPS} clip lists dumped in ${dumps}`,
);
