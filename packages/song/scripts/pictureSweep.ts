// pictureSweep.ts — seeded projects with picture lanes, sounds and timelines, for the stage gate of the pictures.
//
// A reader would get wrong that this is random: the seed is fixed, so a failure names a project that replays.
// It writes no ':' timecode on a shot and no number that overflows: those are the Rust's own departures (505, 510).

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { writeDoc, writeLane } from "../src/testSupport";

export const SWEEP_SEED = 0x2e7f1b33;
export const SWEEP_SIZE = 600;

type Rand = () => number;
type Obj = Record<string, unknown>;

/** mulberry32: a 32-bit generator small enough to read, and the same on every machine. */
function prng(seed: number): Rand {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const TEMPOS = [60, 90, 120, 123, 137.3, 174];
const COARSE = [0, 0.5, 1, 1.5, 2, 4];
const SPEEDS = [0.5, 2, 1.5, 0, -1, 1];

const pick = <T>(r: Rand, of: readonly T[]): T =>
  of[Math.floor(r() * of.length)] as T;
const chance = (r: Rand, p: number): boolean => r() < p;
const between = (r: Rand, lo: number, hi: number): number =>
  lo + Math.floor(r() * (hi - lo + 1));
/** A time in seconds: on the coarse grid, so ties and touching spans occur, or a random float. */
const seconds = (r: Rand): number =>
  chance(r, 0.6) ? pick(r, COARSE) : Math.round(r() * 8000) / 1000;

/** A fade as a number or a map, some as long as the whole shot. */
function fade(r: Rand, whole: number): unknown {
  const over =
    chance(r, 0.25) && whole > 0 ? whole : pick(r, [0.25, 0.5, 1, 2]);
  return chance(r, 0.5) ? over : { over, with: "wipe" };
}

/** One shot of a picture lane. */
function shot(r: Rand, refs: string[], start: number): Obj {
  if (chance(r, 0.08)) return { start };
  const out: Obj = { ref: pick(r, refs), start };
  const length = pick(r, [undefined, 0, 1, 2, 4, seconds(r)]);
  if (length !== undefined) out.length = length;
  const span = typeof length === "number" && length > 0 ? length : 1;
  if (chance(r, 0.25)) out.fadeIn = fade(r, span);
  if (chance(r, 0.25)) out.fadeOut = fade(r, span);
  const speed = chance(r, 0.25) ? pick(r, SPEEDS) : undefined;
  if (speed !== undefined) out.speed = speed;
  if (chance(r, 0.1)) out.silent = true;
  if (chance(r, 0.2)) out.pitch = "keep";
  if (chance(r, 0.3)) out.from = pick(r, [0, 0.5, 1, "2.5", seconds(r)]);
  // A shot running backwards needs the source behind it: the schema refuses one that runs out.
  if (speed !== undefined && speed < 0 && span > 0 && length)
    out.from = span * -speed + pick(r, [0, 0.5, 2.5]);
  return out;
}

/** A lane's shots, mostly in order of their start. */
function shots(r: Rand, refs: string[], n: number): Obj[] {
  const starts = Array.from({ length: n }, () => seconds(r));
  if (chance(r, 0.7)) starts.sort((a, b) => a - b);
  return starts.map((s) => shot(r, refs, s));
}

/** Three shots whose last two end their fades at one instant, so the one last in the file names the crossfade. */
function tie(r: Rand, ref: string): Obj[] {
  const x = pick(r, [0, 1, 2.5]);
  return [
    { ref, start: x, length: 6 },
    { ref, start: x + 4, length: 2, fadeIn: 1 },
    { ref, start: x + 3, length: 3, fadeIn: 2 },
  ];
}

/** One project of the sweep, written under `dir`. */
function writeOne(r: Rand, dir: string): void {
  mkdirSync(dir, { recursive: true });
  const tempo = pick(r, TEMPOS);
  writeDoc(join(dir, "project.k7x"), { project: { name: "sweep", tempo } });
  // Clips: some with a sound on disk, some with none, one naming a sound that is not there.
  const refs: string[] = [];
  const clips = between(r, 1, 4);
  for (let i = 0; i < clips; i++) {
    const id = `v${i}`;
    refs.push(id);
    const sounds = chance(r, 0.8);
    writeDoc(join(dir, "clips", `${id}.k7x`), {
      source: `clips/${id}.mp4`,
      ...(sounds ? { sound: `clips/${id}.wav` } : {}),
    });
    writeFileSync(join(dir, "clips", `${id}.mp4`), "");
    if (sounds && !chance(r, 0.04))
      writeFileSync(join(dir, "clips", `${id}.wav`), "");
  }
  writeDoc(join(dir, "clips", "au.k7x"), { source: "clips/au.wav" });
  writeFileSync(join(dir, "clips", "au.wav"), "");
  if (chance(r, 0.1)) refs.push("au");
  if (chance(r, 0.05)) refs.push("nothing");
  const pictures = between(r, 1, 3);
  const names = Array.from({ length: pictures }, (_, i) => `pic${i}`);
  names.forEach((name, i) => {
    const own = chance(r, 0.35) ? between(r, 0, 3) : between(r, 1, 40);
    writeLane(dir, name, "picture-track", {
      events: [
        ...shots(r, refs, own),
        ...(chance(r, 0.3) ? tie(r, pick(r, refs)) : []),
      ],
      ...(chance(r, 0.1) ? { disabled: true } : {}),
      ...(i === 0 ? { frame: [0, 0, 640, 360] } : {}),
    });
  });
  // Lanes naming a picture lane: most well, one naming nothing, one with events beside.
  const named = between(r, 0, 2);
  for (let i = 0; i < named; i++) {
    const kind = r();
    writeLane(dir, `snd${i}`, "track", {
      video_source: kind < 0.1 ? "nowhere" : pick(r, names),
      ...(kind > 0.9 ? { events: [{ ref: refs[0], start: "1:1" }] } : {}),
      ...(chance(r, 0.1) ? { disabled: true } : {}),
    });
  }
  // The timelines: a later one may be placed in an earlier one, never the reverse.
  const timelines = between(r, 0, 3);
  const tids = Array.from({ length: timelines }, (_, i) => `tl${i}`);
  tids.forEach((id, i) => {
    const inner = tids.slice(i + 1);
    const events = inner
      .filter(() => chance(r, 0.5))
      .map((t) => ({
        ref: t,
        start: pick(r, ["1:1", "1:3", 1, 2]),
        ...(chance(r, 0.1) ? { disabled: true } : {}),
      }));
    const lane = pick(r, [...names, "ghost"]);
    const other = names.filter((n) => n !== lane);
    writeDoc(join(dir, "timelines", `${id}.k7x`), {
      length: pick(r, ["1:0", "2:0", "4:0", "1:2"]),
      lanes: [
        { lane: "x", events, ...(chance(r, 0.1) ? { disabled: true } : {}) },
      ],
      pictures: [
        {
          lane,
          events: shots(r, refs, between(r, 1, 6)),
          ...(chance(r, 0.1) ? { disabled: true } : {}),
        },
        ...(chance(r, 0.3) && other.length
          ? [{ lane: pick(r, other), events: shots(r, refs, between(r, 1, 4)) }]
          : []),
      ],
    });
  });
  // Placed on sound lanes, some twice.
  const hosts = between(r, 1, 2);
  for (let i = 0; i < hosts && tids.length; i++)
    writeLane(dir, `host${i}`, "track", {
      events: tids
        .filter(() => chance(r, 0.7))
        .map((t) => ({
          ref: t,
          start: pick(r, ["1:1", "2:1", "3:1", 0, 3]),
          ...(chance(r, 0.1) ? { disabled: true } : {}),
        })),
    });
  // A picture lane's #audio wired for some.
  const wired = names.filter(() => chance(r, 0.3));
  if (wired.length)
    writeDoc(join(dir, "graph.k7x"), {
      modules: {
        ...Object.fromEntries(wired.map((n) => [`${n}#audio`, ["MIX"]])),
        MIX: ["OUT"],
      },
    });
}

/** Writes `count` projects under `root`, each at `<root>/sweep-<n>/ws/proj`; returns their folder names. */
export function writeSweep(
  root: string,
  count: number,
  seed: number,
): string[] {
  const r = prng(seed);
  const names: string[] = [];
  for (let n = 0; n < count; n++) {
    const name = `sweep-${String(n).padStart(4, "0")}`;
    writeOne(r, join(root, name, "ws", "proj"));
    names.push(name);
  }
  return names;
}
