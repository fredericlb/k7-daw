// stageDumpPictures.ts — the TypeScript's picture sounds and placed shots over a staged corpus, the negative cases and the sweep.
//
//   npx tsx scripts/stageDumpPictures.ts <staged dir> <dumps dir>   (from packages/song; stageCorpus.ts made the staged dir)
//
// What a reader gets wrong: it runs nodeProject.ts lines 218 to 244 as written and parses `laneErrs` by anchored
// patterns; a line none of them knows FAILS the dump, so a change of the TypeScript's wording cannot pass quietly.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";
import { parseDuration, startBeats } from "@studio/song-schema/timecode";

const [staged, dumps] = process.argv.slice(2);
if (!staged || !dumps)
  throw new Error("usage: stageDumpPictures.ts <staged dir> <dumps dir>");
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
Object.assign(process.env, manifest.env);
delete process.env.DIRT_SAMPLES;
delete process.env.STRUDEL_SAMPLES;
const { loadCompileInputs } = await import("../src/compile/inputs.ts");
const { loadVideoTracks } = await import("../src/entities.ts");
const { placementToClip } = await import("../src/compile/clips.ts");
const { timelineShots, withTimelineShots } = await import(
  "../src/timelinePictures.ts"
);
const { pictureLaneSounds, videoSoundLanes } = await import(
  "../src/videoSound.ts"
);
const { SWEEP_SEED, SWEEP_SIZE, writeSweep } = await import(
  "./pictureSweep.ts"
);
const { workspaceDirOf } = await import("../src/confine.ts");
const {
  META_MODULES_ROOT,
  accountLibraries,
  accountLibraryHome,
  moduleNamespaceOf,
} = await import("../src/sharedFolders.ts");
const { modelsRoot } = await import("../src/modelCache.ts");
const { studioHome } = await import("../src/studioHome.ts");

const message = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);

/** A path in the Source's spelling: `/` for `\`, and an upper-case drive letter. */
const slashes = (p: string): string =>
  p
    .replaceAll("\\", "/")
    .replace(/^([a-z]):/, (_, d: string) => `${d.toUpperCase()}:`);

/** The roots as the TypeScript computes them for a project (stageDump.ts' rootsOf), in the Source's spelling. */
function rootsOf(dir: string) {
  const shared = moduleNamespaceOf(resolve(dir, "../_modules"));
  const account = { name: ACCOUNT_LIBRARY, root: accountLibraryHome() };
  return {
    project: slashes(dir),
    workspace: slashes(dirname(dir)),
    ...(shared === "workspace" ? {} : { workspace_namespace: shared }),
    confine: slashes(workspaceDirOf(dir)),
    libraries: [account, ...accountLibraries(dir)].map((l) => ({
      name: l.name,
      root: slashes(l.root),
      editable: false,
    })),
    meta: slashes(META_MODULES_ROOT),
    scan: slashes(join(studioHome(), "exts.json")),
    models: slashes(modelsRoot()),
    renders: slashes(join(studioHome(), AUTO_RENDERS_HOME_DIR)),
  };
}

/** Every `source` of a clip spelled with `/`, whatever it sits in. */
function clipSlashes<T>(v: T): T {
  const clips = JSON.parse(JSON.stringify(v));
  const walk = (x: unknown): void => {
    if (Array.isArray(x)) x.forEach(walk);
    else if (x && typeof x === "object") {
      const o = x as Record<string, unknown>;
      if (typeof o.source === "string") o.source = slashes(o.source);
      Object.values(o).forEach(walk);
    }
  };
  walk(clips);
  return clips;
}

/** A rule of the table: an anchored pattern and the key it gives, from its captures. */
type Rule = [RegExp, (m: string[]) => string];

const RULES: Rule[] = [
  [
    /^lane '(.*)': video_source '(.*)' and events: both /,
    (m) => `video_source_both|lane:${m[1]}|${m[2]}`,
  ],
  [
    /^lane '(.*)': video_source names '(.*)', which is not a picture lane /,
    (m) => `video_source_absent|lane:${m[1]}|${m[2]}`,
  ],
  [
    /^(picture lane|lane) '(.*?)': shot '(.*?)' sounds .*, which is not there /,
    (m) =>
      `sound_missing|${m[1] === "lane" ? "lane" : "picture"}:${m[2]}|${m[3]}`,
  ],
];

/** One `laneErrs` line as `kind|holder|target`; a line no pattern matches fails the dump. */
function faultOf(line: string): string {
  for (const [re, build] of RULES) {
    const m = re.exec(line);
    if (m) return build(m);
  }
  throw new Error(`a fault line no pattern knows: ${line}`);
}

/** The placed shots the walk answered, reduced to what the Rust compares. */
function placedJson(placed: Map<string, Record<string, unknown>[]>) {
  return Object.fromEntries(
    [...placed].map(([lane, shots]) => [
      lane,
      shots.map((s) => ({
        ref: String(s.ref ?? ""),
        start: s.start,
        length: s.length,
        timeline: s.timeline,
      })),
    ]),
  );
}

/** One project's picture sounds: nodeProject.ts lines 218 to 244, copied. */
function dumpOf(name: string, dir: string) {
  let inp: ReturnType<typeof loadCompileInputs>;
  try {
    inp = loadCompileInputs(dir);
  } catch (e) {
    return { project: name, roots: rootsOf(dir), threw: message(e) };
  }
  try {
    const { root, roots, lanesMap, chains, clipsMap, seqMap, timelinesMap } =
      inp;
    const proj = root.project ?? {};
    const tempo = Number(proj.tempo ?? 120);
    const [num, den] = String(proj.timeSignature ?? "4/4")
      .split("/")
      .map((n) => Number.parseInt(n, 10));
    if (!num || !den)
      throw new Error(
        `timeSignature must look like '4/4', got '${proj.timeSignature}'`,
      );
    const timelineCtx = {
      timelines: timelinesMap,
      pos: (tc: unknown) => startBeats(tc, num, den, tempo),
      dur: (tc: unknown) =>
        tc != null && tc !== "" ? parseDuration(String(tc), num, den) : 0,
      bar: (num * 4) / den,
      beatsPerSec: tempo / 60,
      clipOf: (p: never) =>
        placementToClip(dir, p, clipsMap, seqMap, num, den, tempo, roots),
    };
    const laneErrs: string[] = [];
    const placedShots = new Map();
    for (const t of Object.values(lanesMap))
      if (t.type !== "group" && !t.disabled)
        timelineShots(t.events, timelineCtx as never, placedShots);
    const pictures = withTimelineShots(loadVideoTracks(dir), placedShots);
    const videoSounds = videoSoundLanes(
      dir,
      lanesMap,
      pictures,
      clipsMap,
      tempo,
      laneErrs,
    );
    const audioChainsOf = (lane: string) =>
      Object.entries(chains).filter(
        ([, c]) => c.from === lane && c.fromPort === "audio",
      );
    const pictureSounds = pictureLaneSounds(
      dir,
      lanesMap,
      pictures,
      clipsMap,
      tempo,
      laneErrs,
      (lane) => audioChainsOf(lane).length > 0,
    );
    return {
      project: name,
      roots: rootsOf(dir),
      threw: null,
      video_sounds: clipSlashes(Object.fromEntries(videoSounds)),
      picture_sounds: clipSlashes(Object.fromEntries(pictureSounds)),
      placed: placedJson(placedShots),
      faults: [...new Set(laneErrs.map(faultOf))].sort(),
    };
  } catch (e) {
    if (message(e).startsWith("a fault line no pattern knows")) throw e;
    return { project: name, roots: rootsOf(dir), threw: message(e) };
  }
}

const write = (name: string, dir: string) =>
  writeFileSync(
    join(dumps, `${name.replaceAll("/", "__")}.json`),
    JSON.stringify(dumpOf(name, dir)),
  );

/** Runs `f` with the three folders a case reads its libraries, its scan and its models from set to its own. */
function inCase(base: string, f: () => void): void {
  const was = { ...process.env };
  process.env.STUDIO_HOME = join(base, "home");
  process.env.STUDIO_LIBRARIES_HOME = join(base, "lib");
  process.env.STUDIO_MODELS = join(base, "home", "models");
  try {
    f();
  } finally {
    for (const k of ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MODELS"]) {
      if (was[k] === undefined) delete process.env[k];
      else process.env[k] = was[k];
    }
  }
}

mkdirSync(dumps, { recursive: true });
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs);

// The negative corpus: each case laid out as a module case of negative.json (ws/proj).
type Content = string | { hex: string } | { repeat: [string, number] };
const negative = JSON.parse(
  readFileSync(
    join(import.meta.dirname, "../rust/tests/all/pictures_negative.json"),
    "utf8",
  ),
) as { name: string; ts: boolean; files: Record<string, Content> }[];

const bytes = (v: Content): string | Buffer =>
  typeof v === "string"
    ? v
    : "hex" in v
      ? Buffer.from(v.hex, "hex")
      : v.repeat[0].repeat(v.repeat[1]);

let stagedNegative = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "negative", `pictures-${c.name}`);
  rmSync(base, { recursive: true, force: true });
  mkdirSync(join(base, "ws", "proj"), { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    const target = rel.startsWith("../_modules/")
      ? join(base, "ws", rel.slice("../".length))
      : join(base, "ws", "proj", rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes(v));
  }
  inCase(base, () =>
    write(`negative-pictures/${c.name}`, join(base, "ws", "proj")),
  );
  stagedNegative++;
}

// The sweep: seeded projects the generator writes, each dumped under its own folders.
const sweepRoot = join(staged, "sweep");
rmSync(sweepRoot, { recursive: true, force: true });
const names = writeSweep(sweepRoot, SWEEP_SIZE, SWEEP_SEED);
for (const n of names)
  inCase(join(sweepRoot, n), () =>
    write(`sweep/${n}`, join(sweepRoot, n, "ws", "proj")),
  );
writeFileSync(join(dumps, "sweep.json"), JSON.stringify(names));
console.log(
  `${manifest.projects.length} projects, ${stagedNegative} negative cases and ${names.length} sweep projects dumped in ${dumps}`,
);
