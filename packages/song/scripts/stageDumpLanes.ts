// stageDumpLanes.ts — the TypeScript's lanes, owners, live inputs, reference check, assemblies and console over a staged corpus.
//
//   npx tsx scripts/stageDumpLanes.ts <staged dir> <dumps dir>   (from packages/song; stageCorpus.ts made the staged dir)
//
// What a reader gets wrong: a throw is recorded, never hidden, and its lines are parsed by anchored patterns into fault keys;
// a line that matches none FAILS the dump, so a change of the TypeScript's wording cannot pass quietly. No Rust reads a message.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";

const [staged, dumps] = process.argv.slice(2);
if (!staged || !dumps)
  throw new Error("usage: stageDumpLanes.ts <staged dir> <dumps dir>");
const manifest = JSON.parse(
  readFileSync(join(staged, "manifest.json"), "utf8"),
);
Object.assign(process.env, manifest.env);
delete process.env.DIRT_SAMPLES;
delete process.env.STRUDEL_SAMPLES;
const { loadCompileInputs, checkReferences, liveInputsOf } = await import(
  "../src/compile/inputs.ts"
);
const { chainOwnership } = await import("../src/compile/ownership.ts");
const { laneOf, pictureLanesOf } = await import("../src/trackModule.ts");
const { resolveModuleMap } = await import("../src/moduleResolve.ts");
const { normalizePreferences } = await import("../src/preferences.ts");
const { resolveAssemblies, assemblyIssues } = await import(
  "../src/assemblies.ts"
);
const { consoleSystems, withConsole } = await import("../src/console.ts");
const { emptyDoc, trackDoc } = await import("@studio/engine-protocol/build");
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

/** One lane (or picture lane) as laneOf writes it, its events reduced to their refs: the Rust compares the events apart. */
const laneJson = (lane: ReturnType<typeof laneOf>) =>
  JSON.parse(
    JSON.stringify({
      ...lane,
      events: (lane?.events ?? []).map((e) => String(e?.ref ?? "")),
    }),
  );

/** A throw's `  - ` lines. */
const bullets = (msg: string): string[] =>
  msg
    .split("\n")
    .slice(1)
    .map((l) => l.replace(/^\s*- /, ""));

/** `track 'L'` or `timeline 'T' lane 'L'` as a holder key. */
const where = (w: string): string => {
  const t = /^track '(.*)'$/.exec(w);
  if (t) return `track:${t[1]}`;
  const l = /^timeline '(.*)' lane '(.*)'$/.exec(w);
  if (l) return `timeline:${l[1]}:${l[2]}`;
  throw new Error(`a location no pattern knows: ${w}`);
};

/** A rule of the table: an anchored pattern and the key it gives, from its captures. */
type Rule = [RegExp, (m: string[]) => string];

/** Lines that name no location. */
const BARE: Rule[] = [
  [
    /^module '(.*)': extends '(.*)', which /,
    (m) => `extends_broken|module:${m[1]}|${m[2]}`,
  ],
  [
    /^module '(.*)': extends unknown module '(.*)'$/,
    (m) => `extends_unknown|module:${m[1]}|${m[2]}`,
  ],
  [
    /^timeline failed to load — (.*?)(?:: |$)/,
    (m) => `timeline_broken|file:${m[1]}|`,
  ],
  [
    /^track '(.*)': strudel pattern '(.*)' file missing$/,
    (m) => `strudel_missing|track:${m[1]}|${m[2]}`,
  ],
  [
    /^chain '(.*)': from '(.*)' is a group/,
    (m) => `group_source|chain:${m[1]}|${m[2]}`,
  ],
  [
    /^chain '(.*)' step (\d+) \(module '(.*)'\): feeds \d+ input\(s\) but \d+ mandatory/,
    (m) => `too_few|chain:${m[1]}:${m[2]}|${m[3]}`,
  ],
  [
    /^chain '(.*)' step (\d+) \(module '(.*)'\): feeds \d+ input\(s\) but the module has only/,
    (m) => `too_many|chain:${m[1]}:${m[2]}|${m[3]}`,
  ],
  [
    /^chain '(.*)' step (\d+) \(module '.*'\): port '(.*)' expects \w+ but source '.*' is \w+$/,
    (m) => `port_type|chain:${m[1]}:${m[2]}|${m[3]}`,
  ],
  [
    /^node '(.*)': tap '(.*)' is routed to '.*', which is not a subchain/,
    (m) => `tap_unrouted|node:${m[1]}|${m[2]}`,
  ],
  [
    /^project\.console '.*': no (\w+) module there$/,
    (m) => `console_absent|console|${m[1]}`,
  ],
];

/** Lines that follow a location (`<W>: …`), the location as a holder key. */
const LOCATED: Rule[] = [
  [
    /^timeline '(.*?)' is placed inside itself/,
    (m) => `timeline_cycle|@|${m[1]}`,
  ],
  [/^timeline '(.*?)' is placed with /, (m) => `timeline_fixed|@|${m[1]}`],
  [/^clip '(.*?)' failed to load/, (m) => `clip_broken|@|${m[1]}`],
  [/^sequence '(.*?)' failed to load/, (m) => `sequence_broken|@|${m[1]}`],
  [/^clip\/sequence '(.*?)' not found$/, (m) => `ref_missing|@|${m[1]}`],
  [
    /^audio clip '(.*?)' names a file outside/,
    (m) => `audio_outside|@|${m[1]}`,
  ],
  [/^audio clip '(.*?)' file missing/, (m) => `audio_missing|@|${m[1]}`],
  [/^sequence '(.*?)' is placed with a gain/, (m) => `sequence_gain|@|${m[1]}`],
  [/^clip '(.*?)' is placed with loop: /, (m) => `loop_no_to|@|${m[1]}`],
];

const tried = (rules: Rule[], text: string): string | null => {
  for (const [re, build] of rules) {
    const m = re.exec(text);
    if (m) return build(m);
  }
  return null;
};

/** One throw line as `kind|holder|target`; a line no pattern matches fails the dump. */
function faultOf(line: string): string {
  const bare = tried(BARE, line);
  if (bare) return bare;
  const at = /^(track '.*'|timeline '.*' lane '.*'): (.*)$/.exec(line);
  const key = at ? tried(LOCATED, at[2]) : null;
  if (at && key) return key.replace("@", where(at[1]));
  throw new Error(`a fault line no pattern knows: ${line}`);
}

/** The faults a throwing call gave, each line parsed. */
const faultsOf = (msg: string): string[] => bullets(msg).map(faultOf);

/** A path in the Source's spelling: `/` for `\\`, and an upper-case drive letter. */
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

/** One project's lanes, owners, live inputs, checks, assemblies and console. */
function dumpOf(name: string, dir: string, rig: unknown) {
  let inp: ReturnType<typeof loadCompileInputs>;
  try {
    inp = loadCompileInputs(dir);
  } catch (e) {
    return { project: name, roots: rootsOf(dir), threw: message(e) };
  }
  inp.prefs = normalizePreferences(rig ?? {});
  const faults: string[] = [];
  let wiringRan = true;
  try {
    checkReferences(dir, inp);
  } catch (e) {
    const msg = message(e);
    faults.push(...faultsOf(msg));
    // The wiring check runs only once the reference set is empty.
    wiringRan = msg.startsWith("module input wiring invalid");
  }
  let own: ReturnType<typeof chainOwnership> | null = null;
  let ownThrew: string | null = null;
  try {
    own = chainOwnership(inp);
  } catch (e) {
    ownThrew = message(e);
    faults.push(
      `tap_unrouted|node:${/^node '(.*?)': tap '(.*?)'/.exec(ownThrew)?.[1]}|${/^node '(.*?)': tap '(.*?)'/.exec(ownThrew)?.[2]}`,
    );
  }
  const lanes = Object.fromEntries(
    Object.entries(inp.lanesMap).map(([k, v]) => [k, laneJson(v)]),
  );
  const pictures = Object.fromEntries(
    Object.entries(
      pictureLanesOf(resolveModuleMap(inp.localModules, inp.moduleRefs)),
    ).map(([k, v]) => [k, laneJson(v)]),
  );
  const videoTracks = Object.fromEntries(
    Object.entries(inp.videoLanes).map(([k, v]) => [
      k,
      JSON.parse(
        JSON.stringify({
          ...v,
          events: (v.events ?? []).map((e) => String(e?.ref ?? "")),
        }),
      ),
    ]),
  );
  const out: Record<string, unknown> = {
    project: name,
    roots: rootsOf(dir),
    threw: null,
    lanes,
    pictures,
    video_tracks: videoTracks,
  };
  if (own) {
    const chains = Object.entries(inp.chains);
    const steps = chains.flatMap(([, c]) => c.use.map((s) => s.module));
    const ids = [
      ...new Set([
        ...chains.map(([n]) => n),
        ...Object.keys(inp.lanesMap),
        ...steps,
        "~absent~",
      ]),
    ];
    Object.assign(out, {
      owners: Object.fromEntries(
        chains.map(([n, c]) => [n, own.ownerOf(n, c)]),
      ),
      bus: [...own.busChains],
      input: [...own.inputChains],
      tap: [...own.tapChains].map(([k, v]) => [k, v]),
      cued: chains.filter(([, c]) => own.isCued(c)).map(([n]) => n),
      cue_pair: own.cuePair,
      side: Object.fromEntries(ids.map((id) => [id, own.sideInputTrack(id)])),
      live: liveInputsOf(inp, own),
    });
  }
  const assemblies = resolveAssemblies(dir);
  const issues = [
    ...new Set(assemblyIssues(dir).map((d) => `${d.file}|${d.severity}`)),
  ].sort();
  let consoleOut: Record<string, unknown> = {
    systems: consoleSystems(inp.moduleRefs),
    channel: null,
    buss: null,
    fault: null,
  };
  const system = inp.root.project?.console as string | undefined;
  try {
    const doc = withConsole(
      emptyDoc({ tracks: [trackDoc("x", {})] }),
      system,
      inp.moduleRefs,
    );
    if (system)
      consoleOut = {
        ...consoleOut,
        channel: doc.tracks[0].plugins[0],
        buss: doc.masterPlugins[0],
      };
  } catch (e) {
    const key = faultsOf(message(e))[0];
    faults.push(key);
    consoleOut.fault = key;
  }
  return {
    ...out,
    faults: [...new Set(faults)].sort(),
    wiring_ran: wiringRan,
    own_threw: ownThrew,
    assemblies,
    assembly_issues: issues,
    console: consoleOut,
  };
}

const write = (name: string, dir: string, rig: unknown) =>
  writeFileSync(
    join(dumps, `${name.replaceAll("/", "__")}.json`),
    JSON.stringify(dumpOf(name, dir, rig)),
  );

mkdirSync(dumps, { recursive: true });
for (const p of manifest.projects as { abs: string; project: string }[])
  write(p.project, p.abs, {});

// The negative corpus: each case laid out as a module case of negative.json (ws/proj), with its own rig.
type Content = string | { hex: string } | { repeat: [string, number] };
const negative = JSON.parse(
  readFileSync(
    join(import.meta.dirname, "../rust/tests/all/lanes_negative.json"),
    "utf8",
  ),
) as {
  name: string;
  ts: boolean;
  rig?: unknown;
  files: Record<string, Content>;
}[];

/** Where a case's file goes under its base, as `support::placed` places it. */
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
  const base = join(staged, "negative", `lanes-${c.name}`);
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
    write(`negative-lanes/${c.name}`, join(base, "ws", "proj"), c.rig);
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
