// stageDumpGraph.ts — the TypeScript's WebGraph (buildGraphDoc) and graph model (loadGraph) over a staged corpus, one JSON per project.
//
//   npx tsx scripts/stageDumpGraph.ts <staged dir> <dumps dir> [--sweep N --seed 0x...]   (from packages/song)
//
// What a reader gets wrong: a throw is recorded as `threw`, never hidden; `handed` is what the compile gives buildGraphDoc,
// null for a project that does not compile. The sweep writes small seeded projects (mulberry32) the golden corpus lacks.

import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";

const args = process.argv.slice(2);
const flag = (name: string): string | undefined => {
  const at = args.indexOf(name);
  return at < 0 ? undefined : args[at + 1];
};
const [staged, dumps] = args.filter(
  (a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"),
);
if (!staged || !dumps)
  throw new Error(
    "usage: stageDumpGraph.ts <staged dir> <dumps dir> [--sweep N --seed S]",
  );
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
Object.assign(process.env, manifest.env);
delete process.env.DIRT_SAMPLES;
delete process.env.STRUDEL_SAMPLES;
const { loadGraph } = await import("../src/graph.ts");
const { normalizePreferences, PREFS_PATH, savePreferences } = await import(
  "../src/preferences.ts"
);
const { activeFreezePlans } = await import("../src/freeze.ts");
const { compileWithAutoFreezes } = await import("../src/nodeProject.ts");
const { workspaceDirOf } = await import("../src/confine.ts");
const {
  META_MODULES_ROOT,
  accountLibraries,
  accountLibraryHome,
  moduleNamespaceOf,
} = await import("../src/sharedFolders.ts");
const { modelsRoot } = await import("../src/modelCache.ts");
const { studioHome } = await import("../src/studioHome.ts");
const { songRoots } = await import("../src/songRoots.ts");
const { buildGraphDoc, clipExtentsOf, trackColorsOf, trackMixersOf } =
  await import("./dumpHosts/graphDoc.ts");

const message = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);

const slashes = (p: string): string =>
  p
    .replaceAll("\\", "/")
    .replace(/^([a-z]):/, (_, d: string) => `${d.toUpperCase()}:`);

/** The roots as the TypeScript computes them for a project, in the Source's spelling. */
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
    // The folders the host mounts besides: other library versions, and the real path of a linked family (decisions 736, 737).
    also_readable: songRoots(dir).also_readable.map(slashes),
  };
}

/** `ifCompiles`: the value, or null when the project does not compile. */
function ifCompiles<T>(f: () => T): T | null {
  try {
    return f();
  } catch {
    return null;
  }
}

type Handed = {
  extents: unknown;
  mixers: unknown;
  colors: unknown;
  manual: unknown[];
  auto: unknown[];
};

function dumpOf(name: string, dir: string) {
  const roots = rootsOf(dir);
  const compiled = ifCompiles(() =>
    compileWithAutoFreezes(dir, { vsts: "scan" }),
  );
  let model: unknown = null;
  let manual: unknown[] = [];
  try {
    const g = loadGraph(dir);
    model = {
      chains: g.chains,
      tracks: Object.fromEntries(
        Object.entries(g.tracks).map(([k, t]) => [
          k,
          {
            type: t.type,
            ...(t.color ? { color: t.color } : {}),
            mute: t.mute,
            placements: t.placements.map((p) => ({
              ref: p.ref,
              start: p.start,
              kind: p.kind,
            })),
            tracks: t.tracks,
            ...(t.strudel ? { strudel: t.strudel } : {}),
            ...(t.videoSource ? { videoSource: t.videoSource } : {}),
          },
        ]),
      ),
      videoTracks: Object.fromEntries(
        Object.entries(g.videoTracks).map(([k, v]) => [
          k,
          { ...(v.color ? { color: v.color } : {}), mute: v.mute },
        ]),
      ),
      disabledTracks: g.disabledTracks,
      issues: g.issues ?? [],
      warnings: g.warnings,
      modules: Object.fromEntries(
        ["plugin", "io", "doc", "track"].map((kind) => [
          kind,
          Object.entries(g.modules)
            .filter(([, m]) => m.kind === kind)
            .map(([n]) => n)
            .sort(),
        ]),
      ),
    };
    manual = activeFreezePlans(
      g.chains,
      g.modules,
      (n) => !!g.tracks[n],
      (r) => existsSync(resolve(dir, r)),
    ).map((p) => ({
      at: p.at,
      lanes: [...p.lanes],
      modules: [...p.modules],
    }));
  } catch (e) {
    return {
      project: name,
      roots,
      threw: message(e),
      handed: null,
      graph: null,
      model: null,
    };
  }
  const handed: Handed = {
    extents: compiled ? clipExtentsOf(compiled.doc) : null,
    mixers: compiled ? trackMixersOf(compiled.doc) : null,
    colors: compiled ? trackColorsOf(compiled.doc) : null,
    manual,
    auto: (compiled?.freezes ?? []).map((f) => ({
      module: f.module,
      ready: f.ready,
      hosted: f.hosted,
      lanes: [...f.plan.lanes],
      modules: [...f.plan.modules],
    })),
  };
  let graph: unknown = null;
  let threw: string | null = null;
  try {
    graph = buildGraphDoc(
      dir,
      (handed.extents ?? undefined) as never,
      (handed.mixers ?? undefined) as never,
      compiled?.freezes,
      (handed.colors ?? undefined) as never,
    );
  } catch (e) {
    threw = message(e);
  }
  return { project: name, roots, threw, handed, graph, model };
}

/** Dumps one project; a rig is laid as the machine's preferences for the length of the dump. */
function write(name: string, dir: string, rig: unknown, renders?: string) {
  const given = rig && Object.keys(rig as object).length > 0;
  if (given) savePreferences(normalizePreferences(rig));
  try {
    // A case that names ready renders has them made first, so the dump's compile finds them.
    if (renders === "ready")
      for (const f of ifCompiles(() =>
        compileWithAutoFreezes(dir, { vsts: "scan" }),
      )?.freezes ?? []) {
        mkdirSync(dirname(f.render), { recursive: true });
        writeFileSync(f.render, "RIFF");
      }
    writeFileSync(
      join(dumps, `${name.replaceAll("/", "__")}.json`),
      JSON.stringify(dumpOf(name, dir)),
    );
  } finally {
    if (given) rmSync(PREFS_PATH, { force: true });
  }
}

mkdirSync(dumps, { recursive: true });
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs, {});

// The negative corpus: each case laid out as a module case of negative.json (ws/proj), with its own rig.
type Content = string | { hex: string } | { repeat: [string, number] };
const negativeFile = join(
  import.meta.dirname,
  "../rust/tests/all/graph_negative.json",
);
const negative = (
  existsSync(negativeFile) ? JSON.parse(readFileSync(negativeFile, "utf8")) : []
) as {
  name: string;
  ts: boolean;
  rig?: unknown;
  renders?: string;
  files: Record<string, Content>;
}[];

const placed = (base: string, rel: string): string =>
  rel.startsWith("../_modules/")
    ? join(base, "ws", rel.slice("../".length))
    : rel.startsWith("$lib/")
      ? join(base, "lib", rel.slice("$lib/".length))
      : rel.startsWith("$meta/")
        ? join(base, "meta", rel.slice("$meta/".length))
        : rel === "$scan"
          ? join(base, "home", "exts.json")
          : rel.startsWith("$home/")
            ? join(base, "home", rel.slice("$home/".length))
            : join(base, "ws", "proj", rel);

const bytes = (v: Content): string | Buffer =>
  typeof v === "string"
    ? v
    : "hex" in v
      ? Buffer.from(v.hex, "hex")
      : v.repeat[0].repeat(v.repeat[1]);

/** Runs `f` with the environment a case's own folders set, then restores it. */
function inCase<T>(base: string, f: () => T): T {
  const was = { ...process.env };
  process.env.STUDIO_HOME = join(base, "home");
  process.env.STUDIO_LIBRARIES_HOME = join(base, "lib");
  process.env.STUDIO_MODELS = join(base, "home", "models");
  try {
    return f();
  } finally {
    for (const k of ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MODELS"]) {
      if (was[k] === undefined) delete process.env[k];
      else process.env[k] = was[k];
    }
  }
}

let stagedNegative = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "negative", `graph-${c.name}`);
  rmSync(base, { recursive: true, force: true });
  mkdirSync(join(base, "ws", "proj"), { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    const target = placed(base, rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes(v));
  }
  inCase(base, () =>
    write(
      `negative-graph/${c.name}`,
      join(base, "ws", "proj"),
      c.rig,
      c.renders,
    ),
  );
  stagedNegative++;
}

// The sweep: small seeded projects drawn from a pool the golden corpus lacks (docs/rustCompilerGraph.md).
const sweepCount = Number(flag("--sweep") ?? 0);
const seed = Number(flag("--seed") ?? 0x2c0f1a5b) >>> 0;
const { sweepProject } = await import("./graphSweep.ts");
const sweep: string[] = [];
for (let i = 0; i < sweepCount; i++) {
  const base = join(staged, "sweep", String(i));
  rmSync(base, { recursive: true, force: true });
  mkdirSync(join(base, "ws", "proj"), { recursive: true });
  sweepProject(join(base, "ws", "proj"), join(base, "lib"), seed + i * 7919, i);
  // Every other project has its automatic renders made first, so freezes that play and ones that wait are both drawn.
  inCase(base, () =>
    write(
      `sweep/${i}`,
      join(base, "ws", "proj"),
      {},
      i % 2 === 0 ? "ready" : undefined,
    ),
  );
  sweep.push(`sweep/${i}`);
}
writeFileSync(join(dumps, "sweep.json"), JSON.stringify(sweep));
console.log(
  `${manifest.projects.length} projects, ${stagedNegative} negative cases and ${sweep.length} sweep projects dumped in ${dumps}`,
);
