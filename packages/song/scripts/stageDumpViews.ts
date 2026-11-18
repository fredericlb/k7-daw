// stageDumpViews.ts — the TypeScript's views over a staged corpus, plus the sweeps and the fields lattice, for the Rust to compare.
//
//   npx tsx scripts/stageDumpViews.ts <staged dir> <dumps dir> [--sweep-clips 1500 --sweep-wav 2000 --seed 0x2d4a0e77]
//
// What a reader gets wrong: a call that threw is recorded as threw, since the Rust names it a finding; the dump holds the
// catalogue, the base modules, the surfaces and their issues, the I/O ports, the hints, the web project and the error list
// of a compile, the lattice and the two sweeps. The compile reads a copy of the staged tree.

import {
  cpSync,
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

const argv = process.argv.slice(2);
const [staged, dumps] = argv;
if (!staged || !dumps)
  throw new Error("usage: stageDumpViews.ts <staged dir> <dumps dir> [flags]");

const manifestOf = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
// The compile builds jsdsp graphs into the tree it reads, so it reads a COPY and the Rust reads that copy.
const copy = join(dumps, "tree", "golden");
rmSync(join(dumps, "tree"), { recursive: true, force: true });
mkdirSync(join(dumps, "tree"), { recursive: true });
cpSync(manifestOf.tmp, copy, { recursive: true, verbatimSymlinks: true });
const quoted = (s: string) => JSON.stringify(s).slice(1, -1);
const manifest = JSON.parse(
  JSON.stringify(manifestOf).replaceAll(quoted(manifestOf.tmp), quoted(copy)),
);
Object.assign(process.env, manifest.env);
delete process.env.DIRT_SAMPLES;
delete process.env.STRUDEL_SAMPLES;
const { moduleCatalogue } = await import("../src/moduleCatalogue.ts");
const { listBaseModuleRefs } = await import("../src/moduleRefs.ts");
const { projectSurfaces, surfaceIssues } = await import("../src/surfaces.ts");
const { projectHints } = await import("../src/hints.ts");
const { ioPorts } = await import("../src/ioPorts.ts");
const { scannedPluginNames } = await import("../src/engineCatalogue.ts");
const { normalizePreferences, PREFS_PATH, savePreferences } = await import(
  "../src/preferences.ts"
);
const { compileWithAutoFreezes } = await import("../src/nodeProject.ts");
const { withOutputRecorder } = await import("../src/outputRecorder.ts");
const { workspaceDirOf } = await import("../src/confine.ts");
const {
  accountLibraries,
  accountLibraryHome,
  META_MODULES_ROOT,
  moduleNamespaceOf,
} = await import("../src/sharedFolders.ts");
const { loadProjectWithFreezes } = await import("../src/project.ts");
const { webProjectOf } = await import("./dumpHosts/webProject.ts");
const { projectErrorsIn } = await import("./dumpHosts/workspaceDocs.ts");
const { modelsRoot } = await import("../src/modelCache.ts");
const { studioHome } = await import("../src/studioHome.ts");
const { songRoots } = await import("../src/songRoots.ts");

const message = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);
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
    // The folders the host mounts besides: other library versions, and the real path of a linked family (decisions 736, 737).
    also_readable: songRoots(dir).also_readable.map(slashes),
  };
}

const attempt = <T>(f: () => T): { ok: T } | { threw: string } => {
  try {
    return { ok: f() };
  } catch (e) {
    return { threw: message(e) };
  }
};

/** The document the file service compiles, with the VSTs the case asks for. */
function compileDoc(dir: string, vstsNone: boolean) {
  if (!vstsNone) return loadProjectWithFreezes(dir).doc;
  const { doc } = compileWithAutoFreezes(dir, { vsts: "none" });
  return withOutputRecorder({ ...doc, recordDir: join(dir, "clips") }, dir);
}

function dumpOf(
  name: string,
  dir: string,
  rig: unknown = {},
  vstsNone = false,
) {
  const cat = attempt(() =>
    moduleCatalogue(dir).map((m) => ({
      ...m,
      ...(m.path ? { path: slashes(m.path) } : {}),
    })),
  );
  const base = attempt(() => listBaseModuleRefs(dir));
  const surfaces = attempt(() => projectSurfaces(dir));
  // The rig stands for the machine's preferences, which projectHints reads itself and a case cannot hand it.
  const prefs = normalizePreferences(rig ?? {});
  // The route's own answer: the hints, with the two keys the file service fills.
  const hints = attempt(() => ({
    ...projectHints(dir),
    channels: prefs.channels.in.map((c) => c.label).sort(),
    pluginNames: scannedPluginNames(),
    projectRoot: slashes(dir),
  }));
  const issues = attempt(() => surfaceIssues(dir));
  // One compile serves both: the web project from its document, the errors around the same document or the same throw.
  let doc: ReturnType<typeof compileDoc> | undefined;
  let refusal: unknown;
  try {
    doc = compileDoc(dir, vstsNone);
  } catch (e) {
    refusal = e;
  }
  const web = attempt(() => {
    if (!doc) throw refusal;
    return webProjectOf(doc);
  });
  const errors = attempt(() =>
    projectErrorsIn(dir, () => {
      if (!doc) throw refusal;
      return doc;
    }),
  );
  const io = attempt(() => ioPorts(dir, prefs));
  const out = <T>(r: { ok: T } | { threw: string }) =>
    "ok" in r ? JSON.parse(JSON.stringify(r.ok)) : null;
  const threw = (r: { threw: string } | object) =>
    "threw" in r ? r.threw : null;
  return {
    project: name,
    roots: rootsOf(dir),
    catalogue: out(cat),
    catalogue_threw: threw(cat),
    base: out(base),
    base_threw: threw(base),
    surfaces: out(surfaces),
    surfaces_threw: threw(surfaces),
    hints: out(hints),
    hints_threw: threw(hints),
    issues: out(issues),
    issues_threw: threw(issues),
    io: out(io),
    io_threw: threw(io),
    // The TypeScript's compile threw: `web` is null, and the errors carry what it said.
    compile_threw: doc ? null : message(refusal),
    web: doc ? out(web) : null,
    web_threw: doc ? threw(web) : null,
    errors: out(errors),
    errors_threw: threw(errors),
  };
}

mkdirSync(dumps, { recursive: true });
const write = (name: string, dir: string, rig: unknown, vstsNone = false) =>
  writeFileSync(
    join(dumps, `${name.replaceAll("/", "__")}.json`),
    JSON.stringify(dumpOf(name, dir, rig, vstsNone)),
  );
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs, {});

// The negative corpus: each case laid out as a module case (ws/proj), with its own rig, as stageDumpLanes.ts does.
type Content = string | { hex: string } | { repeat: [string, number] };
type NegativeCase = {
  name: string;
  ts: boolean;
  rig?: unknown;
  vsts_none?: boolean;
  renders?: string[];
  files: Record<string, Content>;
};
const corpus = (file: string): NegativeCase[] =>
  JSON.parse(
    readFileSync(join(import.meta.dirname, "../rust/tests/all", file), "utf8"),
  );
// …and the document corpus's cases that need no render, which the Rust reads as `doc-…` (views_gate/cases.rs).
const negative = [
  ...corpus("views_negative.json"),
  ...corpus("doc_negative.json")
    .filter((c) => c.ts && !c.renders)
    .map((c) => ({ ...c, name: `doc-${c.name}` })),
];
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
let stagedNegative = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "negative", `views-${c.name}`);
  rmSync(base, { recursive: true, force: true });
  mkdirSync(join(base, "ws", "proj"), { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    const target = placed(base, rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes(v));
  }
  const was = { ...process.env };
  process.env.STUDIO_HOME = join(base, "home");
  process.env.STUDIO_LIBRARIES_HOME = join(base, "lib");
  process.env.STUDIO_MODELS = join(base, "home", "models");
  try {
    // The rig is the machine's preferences, which the compile reads itself.
    if (c.rig && Object.keys(c.rig as object).length)
      savePreferences(normalizePreferences(c.rig));
    else if (existsSync(PREFS_PATH)) rmSync(PREFS_PATH);
    write(
      `negative-views/${c.name}`,
      join(base, "ws", "proj"),
      c.rig,
      c.vsts_none ?? false,
    );
  } finally {
    for (const k of ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MODELS"]) {
      if (was[k] === undefined) delete process.env[k];
      else process.env[k] = was[k];
    }
  }
  stagedNegative++;
}

// The lattice and the two sweeps are in their own script, which reads the same arguments.
await import("./stageDumpSweeps.ts");
console.log(
  `${manifest.projects.length} projects and ${stagedNegative} negative cases dumped in ${dumps}`,
);
