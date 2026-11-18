// stageDump.ts — the TypeScript's readers run over a staged golden corpus, one JSON per project, for the Rust to compare.
//
//   npx tsx scripts/stageDump.ts <staged dir> <dumps dir>      (from packages/song; stageCorpus.ts made the staged dir)
//
// What a reader gets wrong: the dump holds what the READERS answer (clips, sequences, timelines, the root, the
// markers, the settings, the module tree), not a compile. A family that threw is recorded as threw, since the Rust
// names it a finding. The module half also holds what resolving answers, and nothing is stripped from it. The routing
// half holds the segments, the derived chains and a chain project's nodes; the TypeScript's plugin findings over a
// chain project are respelled by the node's file (decision 102), where it names `modules/undefined/...`.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { isFormatTooNew } from "@studio/song-schema/formatLevel";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";

const [staged, dumps] = process.argv.slice(2);
if (!staged || !dumps)
  throw new Error("usage: stageDump.ts <staged dir> <dumps dir>");
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
// The environment the corpus was staged under, set before the song modules read it.
Object.assign(process.env, manifest.env);
const { checkProjectFormat, loadProjectRoot, loadSettings, readYamlEntities } =
  await import("../src/entities.ts");
const { parseMarkers, parseSettings } = await import("../src/project.ts");
const { loadModules } = await import("../src/moduleTree.ts");
const { isChainProject, loadLinkLevels, readChainProject } = await import(
  "../src/chainProject.ts"
);
const { loadChains, loadVideoTracks } = await import("../src/entities.ts");
const { deriveChains, bypassedOf, busOnlyOf } = await import(
  "../src/chainsGraph.ts"
);
const { applyLinkLevels } = await import("../src/linkLevels.ts");
const { tapFeedsOf, tapPairs } = await import("../src/tapOuts.ts");
const { lanesOf } = await import("../src/trackModule.ts");
const { withChainModuleRefs } = await import("../src/moduleRefs.ts");
const { hostsPlugin, NOT_SUPPORTED_TYPE, OFFLINE_TYPE } = await import(
  "@studio/song-schema/moduleTypes"
);
const { reachesEngine } = await import("../src/compile/plugins.ts");
const { baseModuleRefs, loadModuleRefs } = await import("../src/moduleRefs.ts");
const { resolveModuleMap } = await import("../src/moduleResolve.ts");
const { notSupportedModules, unknownPluginModules } = await import(
  "../src/diagnostics.ts"
);
const { eachLibrarySibling } = await import("../src/moduleSiblings.ts");
const {
  META_MODULES_ROOT,
  accountLibraries,
  accountLibraryHome,
  moduleNamespaceOf,
} = await import("../src/sharedFolders.ts");
const { modelsRoot } = await import("../src/modelCache.ts");
const { workspaceDirOf } = await import("../src/confine.ts");
const { studioHome } = await import("../src/studioHome.ts");

const message = (e: unknown): string =>
  e instanceof Error ? e.message : String(e);

/** One family: its values by id, the files that did not decode, and whether the read threw outright. */
function family(dir: string, sub: string) {
  try {
    const { values, issues } = readYamlEntities(dir, sub);
    const files = [...new Set(issues.map((i) => i.slice(0, i.indexOf(": "))))];
    return { values, files: files.sort(), threw: null as string | null };
  } catch (e) {
    return { values: {}, files: [] as string[], threw: message(e) };
  }
}

/** The root, its markers and the settings, as the compile reads them with validation on. */
function head(dir: string) {
  const out = {
    format_too_new: false,
    root_ok: true,
    root: {} as Record<string, unknown>,
    markers: [] as unknown[],
    settings_ok: true,
    settings: parseSettings({}) as unknown,
  };
  try {
    checkProjectFormat(dir);
    const root = loadProjectRoot(dir, true);
    out.root = root as Record<string, unknown>;
    const [num, den] = String(root.project?.timeSignature ?? "4/4")
      .split("/")
      .map((n) => Number.parseInt(n, 10));
    out.markers = num && den ? parseMarkers(root.markers, num, den) : [];
  } catch (e) {
    if (isFormatTooNew(e)) return { ...out, format_too_new: true };
    out.root_ok = false;
  }
  try {
    out.settings = parseSettings(loadSettings(dir, true));
  } catch {
    out.settings_ok = false;
  }
  return out;
}

/** A path in the Source's spelling: `/` for `\`, and an upper-case drive letter. */
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
    // Every library the project sees, with or without a modules/ folder: a file ref names one all the same.
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

/** A module as JSON, the paths it carries spelled with `/`. */
const plain = (m: unknown): Record<string, unknown> =>
  JSON.parse(JSON.stringify(m));

type Rack = {
  source?: string;
  symbols?: Record<string, { clip?: string }>;
};

const rackSlashes = (r: Rack | undefined): void => {
  if (!r) return;
  if (typeof r.source === "string") r.source = slashes(r.source);
  for (const s of Object.values(r.symbols ?? {}))
    if (typeof s?.clip === "string") s.clip = slashes(s.clip);
};

/** Every path value a module carries spelled with `/`; `ui.script` is text and `assets` names, so neither is touched. */
const asSlashes = (m: Record<string, unknown>): Record<string, unknown> => {
  for (const k of [
    "folderAbs",
    "jsdsp",
    "faust",
    "preset",
    "model",
    "onnx",
    "ir",
  ])
    if (typeof m[k] === "string") m[k] = slashes(m[k] as string);
  rackSlashes(m.rack as Rack | undefined);
  for (const a of (m.alternatives as { rack?: Rack }[] | undefined) ?? [])
    rackSlashes(a?.rack);
  return m;
};

/** Every module of `byRef`, plain. */
const plainAll = (byRef: Record<string, unknown>) =>
  Object.fromEntries(
    Object.entries(byRef).map(([k, m]) => [k, asSlashes(plain(m))]),
  );

/** The module half of a project's dump: its nodes, the vocabulary its roots make, and what the walk found. */
function modulesOf(dir: string) {
  const roots = rootsOf(dir);
  const chain = isChainProject(dir);
  const nodes = { values: {} as unknown, files: [] as string[], chain };
  nodes.values = plainAll(loadModules(dir));
  try {
    loadModules(dir, true);
  } catch (e) {
    const lines = message(e).split("\n").slice(1);
    nodes.files = [
      ...new Set(
        lines
          .map((l) => /^\s*(.*?\.k7x)/i.exec(l)?.[1])
          .filter((f): f is string => !!f),
      ),
    ].sort();
  }
  // The vocabulary as loadModuleRefs makes it: the trees, then every panel and map stamped by ref minus node.
  const issues: { file: string; severity?: string }[] = [];
  const out = loadModuleRefs(dir, undefined, issues as never);
  const siblings = (base: string) => {
    const refs: string[] = [];
    eachLibrarySibling(dir, base, (found) => refs.push(found.ref));
    return [...new Set(refs)].sort();
  };
  return {
    roots,
    nodes,
    vocab: {
      modules: plainAll(out),
      findings: [...new Set(issues.map((i) => i.file))].sort(),
      findings_severity: [
        ...new Set(issues.map((i) => `${i.file}|${i.severity ?? "error"}`)),
      ].sort(),
      base: baseModuleRefs(out as never),
      siblings: { ui: siblings("ui"), controller: siblings("controller") },
    },
    resolved: {
      nodes: plainAll(resolveModuleMap(loadModules(dir), loadModuleRefs(dir))),
      vocab: plainAll(resolveModuleMap(out)),
      // The two plugin findings over the resolved nodes of a node project, by file and severity.
      plugin: chain
        ? chainPluginFindings(dir)
        : [
            ...new Set(
              [...notSupportedModules(dir), ...unknownPluginModules(dir)].map(
                (f) => `${f.file}|${f.severity}`,
              ),
            ),
          ].sort(),
    },
    routing: routingOf(dir, chain),
    strip: [],
  };
}

/** The same two plugin findings over a chain project, each named by its node's own file (decision 102). */
function chainPluginFindings(dir: string): string[] {
  const authored = loadModules(dir);
  const resolved = resolveModuleMap(authored, loadModuleRefs(dir));
  const out = new Set<string>();
  for (const [name, m] of Object.entries(resolved)) {
    const p = (m as { plugin?: { type?: unknown } }).plugin;
    if (!p || typeof p !== "object") continue;
    const own = authored[name] as {
      home?: string;
      chain?: string;
      file?: string;
    };
    const file =
      own?.home === "tracks"
        ? `tracks/${own.file}.k7x`
        : `chains/${own?.chain}.k7x`;
    const type = String(p.type ?? "").toLowerCase();
    if (type === NOT_SUPPORTED_TYPE) out.add(`${file}|warning`);
    else if (
      (m as { kind?: string }).kind === "plugin" &&
      hostsPlugin((m as { category?: string }).category) &&
      type !== OFFLINE_TYPE &&
      !reachesEngine(type)
    )
      out.add(`${file}|error`);
  }
  return [...out].sort();
}

/** A chain project's nodes as the gate compares them. */
function chainNodes(dir: string) {
  const cp = readChainProject(dir);
  return {
    chains: Object.keys(cp.chains),
    hidden: Object.keys(cp.hidden ?? {}),
    nodes: Object.fromEntries(
      Object.entries(cp.nodes).map(([k, n]) => [
        k,
        {
          id: n.id,
          chain: n.chain,
          ...(n.sub ? { sub: n.sub } : {}),
          step: n.step,
          item: n.item,
          ...(n.track ? { track: n.track } : {}),
          ...(n.picture ? { picture: true } : {}),
        },
      ]),
    ),
    issues: cp.issues,
  };
}

/** The routing half: the segments as loadChains reads them, what deriving makes of them, and a chain project's nodes. */
function routingOf(dir: string, chain: boolean) {
  const out: Record<string, unknown> = {
    chain,
    segments: {},
    threw: null as string | null,
    levels: {},
    derived: null as unknown,
    bypassed: [],
    bus_only: [],
    tap_pairs: {},
    lanes: [],
    files: [] as string[],
  };
  let raw: unknown;
  try {
    raw = loadChains(dir, loadProjectRoot(dir, false), false);
  } catch (e) {
    out.threw = message(e);
  }
  out.segments = raw ?? {};
  try {
    loadChains(dir, loadProjectRoot(dir, false), true);
  } catch (e) {
    out.threw = message(e);
  }
  if (chain) {
    const c = chainNodes(dir);
    Object.assign(out, { chains: c.chains, hidden: c.hidden, nodes: c.nodes });
    out.files = [
      ...new Set(
        c.issues.map((i) => `${/^(.*?\.k7x)/i.exec(i)?.[1] ?? i}|error`),
      ),
    ].sort();
  } else if (out.threw) out.files = ["graph.k7x|error"];
  try {
    out.levels = loadLinkLevels(dir);
  } catch {
    out.levels = {};
  }
  let derived: Record<string, unknown> | null = null;
  try {
    const refs = loadModuleRefs(dir);
    const lanes = {
      ...lanesOf(resolveModuleMap(loadModules(dir), refs)),
      ...loadVideoTracks(dir),
    };
    out.lanes = Object.keys(lanes).sort();
    const chains = deriveChains(raw, (n) => n in lanes);
    applyLinkLevels(chains, out.levels as Record<string, number>);
    derived = chains as Record<string, unknown>;
    out.derived = plain(derived);
    out.bypassed = [...bypassedOf(raw)].sort();
    out.bus_only = [...busOnlyOf(raw)].sort();
    out.tap_pairs = Object.fromEntries(
      [...tapPairs(tapFeedsOf(raw)).entries()].map(([n, l]) => [
        n,
        l.map((t) => ({ tap: t.tap, pair: t.pair })),
      ]),
    );
    out.modules_map = plainAll(
      resolveModuleMap(
        withChainModuleRefs(loadModules(dir), refs, derived as never),
        refs,
      ),
    );
  } catch (e) {
    out.derive_threw = message(e);
    // A throw before the derivation is whole is one the Rust must answer with an error finding.
    if (out.derived === null) out.threw ??= message(e);
  }
  return out;
}

/** One project's dump: its three families, its head and, when asked, its module tree. */
function dumpOf(name: string, abs: string, modules = true) {
  return {
    project: name,
    families: {
      clips: family(abs, "clips"),
      sequences: family(abs, "sequences"),
      timelines: family(abs, "timelines"),
    },
    head: head(abs),
    ...(modules ? { modules: modulesOf(abs) } : {}),
  };
}

const write = (name: string, abs: string, modules = true) =>
  writeFileSync(
    join(dumps, `${name.replaceAll("/", "__")}.json`),
    JSON.stringify(dumpOf(name, abs, modules)),
  );

mkdirSync(dumps, { recursive: true });
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs);

// The negative corpus the Rust tests also hold: known-bad files, staged beside the golden ones.
type Content = string | { hex: string };
const negative = JSON.parse(
  readFileSync(
    join(import.meta.dirname, "../rust/tests/all/negative.json"),
    "utf8",
  ),
) as {
  name: string;
  ts: boolean;
  module?: boolean;
  files: Record<string, Content>;
}[];

/** Where a module case's file goes: the project, the workspace beside it, a library, `@meta` or the scan (negative.json). */
function placed(base: string, rel: string): string {
  if (rel.startsWith("../_modules/"))
    return join(base, "ws", rel.slice("../".length));
  if (rel.startsWith("$lib/")) return join(base, "lib", rel.slice(5));
  if (rel.startsWith("$meta/")) return join(base, "meta", rel.slice(6));
  if (rel === "$scan") return join(base, "home", "exts.json");
  if (rel.startsWith("$home/")) return join(base, "home", rel.slice(6));
  return join(base, "ws", "proj", rel);
}

let staged_negative = 0;
for (const c of negative.filter((c) => c.ts)) {
  const base = join(staged, "negative", c.name);
  rmSync(base, { recursive: true, force: true });
  const abs = c.module ? join(base, "ws", "proj") : base;
  mkdirSync(abs, { recursive: true });
  for (const [rel, v] of Object.entries(c.files)) {
    const target = c.module ? placed(base, rel) : join(abs, rel);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(
      target,
      typeof v === "string" ? v : Buffer.from(v.hex, "hex"),
    );
  }
  // A module case reads the libraries and the scan from its own folders, which the reader finds by environment.
  const was = { ...process.env };
  if (c.module) {
    process.env.STUDIO_HOME = join(base, "home");
    process.env.STUDIO_LIBRARIES_HOME = join(base, "lib");
    process.env.STUDIO_MODELS = join(base, "home", "models");
  }
  try {
    write(`negative/${c.name}`, abs, !!c.module);
  } finally {
    for (const k of ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MODELS"]) {
      if (was[k] === undefined) delete process.env[k];
      else process.env[k] = was[k];
    }
  }
  staged_negative++;
}
console.log(
  `${manifest.projects.length} projects and ${staged_negative} negative cases dumped in ${dumps}`,
);
