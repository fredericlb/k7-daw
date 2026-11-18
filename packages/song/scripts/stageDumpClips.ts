// stageDumpClips.ts — the TypeScript's placements over a staged corpus, one JSON per project, for the Rust to compare.
//
//   npx tsx scripts/stageDumpClips.ts <staged dir> <dumps dir>   (from packages/song; stageCorpus.ts made the staged dir)
//
// What a reader gets wrong: a lane's clips are what nodeProject.ts hands the engine (expanded, placed, crossfades marked), and
// a lane the TypeScript threw on is recorded as threw, since the Rust names it a finding. It folds into stageDump.ts at 2h.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";
import { parseDuration, startBeats } from "@studio/song-schema/timecode";

const [staged, dumps] = process.argv.slice(2);
if (!staged || !dumps)
  throw new Error("usage: stageDumpClips.ts <staged dir> <dumps dir>");
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
Object.assign(process.env, manifest.env);
// The machine's two sample folders are the host's to name; the corpus names none.
delete process.env.DIRT_SAMPLES;
delete process.env.STRUDEL_SAMPLES;
const { loadProjectRoot, readYamlEntities } = await import(
  "../src/entities.ts"
);
const { isChainProject } = await import("../src/chainProject.ts");
const { loadModules } = await import("../src/moduleTree.ts");
const { loadModuleRefs } = await import("../src/moduleRefs.ts");
const { resolveModuleMap } = await import("../src/moduleResolve.ts");
const { lanesOf } = await import("../src/trackModule.ts");
const { projectRoots, workspaceDirOf } = await import("../src/confine.ts");
const { placementToClip } = await import("../src/compile/clips.ts");
const { compClips, markCrossfades } = await import("../src/compClips.ts");
const { expandEvents, placeWithin, timelineDocs, timelineLength } =
  await import("../src/compile/timelines.ts");
const { patternPlugins, patternShares, sampleBanks, isPatternLane } =
  await import("../src/enginePattern.ts");
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

/** Every clip's `source` spelled with `/`, whatever it sits in. */
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

/** One project's placements, as nodeProject.ts makes them lane by lane. */
function dumpOf(name: string, dir: string) {
  if (isChainProject(dir)) return { project: name, chain: true };
  const root = loadProjectRoot(dir, true);
  const proj = root.project ?? {};
  const tempo = Number(proj.tempo ?? 120);
  const [num, den] = String(proj.timeSignature ?? "4/4")
    .split("/")
    .map((n) => Number.parseInt(n, 10));
  const roots = projectRoots(dir);
  const localModules = loadModules(dir, true);
  const moduleRefs = loadModuleRefs(dir);
  const lanesMap = lanesOf(resolveModuleMap(localModules, moduleRefs));
  const clipsMap = readYamlEntities(dir, "clips").values;
  const seqMap = readYamlEntities(dir, "sequences").values;
  const timelinesMap = readYamlEntities(dir, "timelines").values;
  // nodeProject.ts' plain, placed and timelineCtx, copied.
  const plain = (p: never) =>
    placementToClip(dir, p, clipsMap, seqMap, num, den, tempo, roots);
  const placed = (l: { p: never; within?: never }) => {
    const own = compClips(
      l.p,
      plain(l.p),
      (ref, start) => plain({ ref, start } as never),
      num,
      den,
      tempo,
    );
    return l.within ? placeWithin(own, l.within) : own;
  };
  const ctx = {
    timelines: timelinesMap,
    pos: (tc: unknown) => startBeats(tc, num, den, tempo),
    dur: (tc: unknown) =>
      tc != null && tc !== "" ? parseDuration(String(tc), num, den) : 0,
    bar: (num * 4) / den,
    beatsPerSec: tempo / 60,
    clipOf: plain,
  };
  const threw: Record<string, string> = {};
  const lanes: Record<string, unknown> = {};
  for (const [lane, t] of Object.entries(lanesMap)) {
    if (t.type === "group" || t.disabled) continue;
    try {
      lanes[lane] = clipSlashes(
        markCrossfades(
          expandEvents(t.events, ctx as never).flatMap(placed as never),
        ),
      );
    } catch (e) {
      threw[lane] = message(e);
    }
  }
  const lengths: Record<string, number> = {};
  const lengthsThrew: Record<string, string> = {};
  for (const id of Object.keys(timelinesMap)) {
    try {
      lengths[id] = timelineLength(id, { ...ctx, lengths: undefined } as never);
    } catch (e) {
      lengthsThrew[id] = message(e);
    }
  }
  let timelines: Record<string, unknown> = {};
  let timelinesThrew: string | null = null;
  try {
    const docs = timelineDocs(
      { ...ctx, lengths: undefined } as never,
      placed as never,
    );
    timelines = Object.fromEntries(docs.map((d) => [d.id, clipSlashes(d)]));
  } catch (e) {
    timelinesThrew = message(e);
  }
  const shares = patternShares(dir, lanesMap);
  const banks = sampleBanks(dir).map(slashes);
  const patterns: Record<string, unknown> = {};
  for (const [lane, t] of Object.entries(lanesMap)) {
    const share = shares.get(lane);
    if (!isPatternLane(t) || !share) continue;
    const both = (halves: { id: string; port?: string }[]) =>
      Object.fromEntries(patternPlugins(dir, lane, t, halves, share, banks));
    patterns[lane] = {
      single: both([{ id: lane }]),
      split: both([
        { id: `${lane}#midi`, port: "midi" },
        { id: `${lane}#audio`, port: "audio" },
      ]),
    };
  }
  return {
    project: name,
    chain: false,
    roots: rootsOf(dir),
    threw,
    lanes,
    lengths,
    lengths_threw: lengthsThrew,
    timelines,
    timelines_threw: timelinesThrew,
    shares: Object.fromEntries(shares),
    banks,
    patterns,
  };
}

const write = (name: string, dir: string) =>
  writeFileSync(
    join(dumps, `${name.replaceAll("/", "__")}.json`),
    JSON.stringify(dumpOf(name, dir)),
  );

mkdirSync(dumps, { recursive: true });
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs);

// The negative corpus: each case laid out as a module case of negative.json (ws/proj), the TypeScript's answers beside the Rust's.
type Content = string | { hex: string } | { repeat: [string, number] };
const negative = JSON.parse(
  readFileSync(
    join(import.meta.dirname, "../rust/tests/all/clips_negative.json"),
    "utf8",
  ),
) as { name: string; ts: boolean; files: Record<string, Content> }[];

/** Where a case's file goes: the project, or the workspace beside it (negative.json's `../_modules/`). */
const placed = (base: string, rel: string): string =>
  rel.startsWith("../_modules/")
    ? join(base, "ws", rel.slice("../".length))
    : join(base, "ws", "proj", rel);

const bytes = (v: Content): string | Buffer =>
  typeof v === "string"
    ? v
    : "hex" in v
      ? Buffer.from(v.hex, "hex")
      : v.repeat[0].repeat(v.repeat[1]);

let stagedNegative = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "negative", `clips-${c.name}`);
  rmSync(base, { recursive: true, force: true });
  mkdirSync(join(base, "ws", "proj"), { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    const target = placed(base, rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes(v));
  }
  // A case reads its libraries and its scan from its own folders, which the reader finds by environment.
  const was = { ...process.env };
  process.env.STUDIO_HOME = join(base, "home");
  process.env.STUDIO_LIBRARIES_HOME = join(base, "lib");
  process.env.STUDIO_MODELS = join(base, "home", "models");
  try {
    write(`negative-clips/${c.name}`, join(base, "ws", "proj"));
  } finally {
    for (const k of ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MODELS"]) {
      if (was[k] === undefined) delete process.env[k];
      else process.env[k] = was[k];
    }
  }
  stagedNegative++;
}
console.log(
  `${manifest.projects.length} projects and ${stagedNegative} negative cases dumped in ${dumps}`,
);
