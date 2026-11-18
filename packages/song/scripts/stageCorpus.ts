// stageCorpus.ts — stages the golden corpus's inputs into `<out>/` and keeps them, for the stage gates and the golden smoke:
// the demos, the e2e fixtures, one project per k7-core module with a lane per engine, and the five archetypes' frozen scaffolds.
// What a reader gets wrong: the archetypes are COMMITTED (fixtures/shapes), not scaffolded, so a scaffold edit changes no gate.
// Run: `npx tsx scripts/stageCorpus.ts <out>` from packages/song; a staged tree serves one run, a compile repairs its files.

import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  realpathSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { projectApp } from "@studio/web-protocol/projects";
import { loadModuleRefs } from "../src/loader.ts";

/** The four groups of inputs, each a workspace of its own, named by the folder its fixtures go to. */
const SOURCES = ["demos", "e2e", "k7-core", "shapes"] as const;
type Source = (typeof SOURCES)[number];

/** A staged tree: where each workspace sits, and the folders whose paths the output must not carry. */
interface StagedCorpus {
  tmp: string;
  workspaces: Record<Source, string>;
  /** Absolute folder → the token that replaces it, longest first. */
  tokens: [string, string][];
}

/** The ambient state a compile reads, pinned inside the tree so no machine's own leaks in. */
function pinEnvironment(tmp: string): void {
  process.env.STUDIO_HOME = join(tmp, "home");
  process.env.STUDIO_LIBRARIES_HOME = join(tmp, "libs");
  process.env.STUDIO_MEDIATEK = join(tmp, "no-mediatek");
  mkdirSync(join(tmp, "home"), { recursive: true });
}

/** Families that are data the compile only names, joined by junction; the rest is copied, since a compile writes caches there. */
const LINKED_FAMILIES = new Set(["samples", "models", "irs", "wavetables"]);

// The caches and VCS noise a checkout accumulates, never part of a fixture.
const copyFilter = (s: string): boolean => {
  const base = s.split(/[/\\]/).pop() ?? "";
  return base !== "node_modules" && !base.startsWith(".studio-");
};

/** k7-core as a library: a real folder, so the library walk sees a directory, its data families joined by junction. */
function stageLibrary(repo: string, libs: string): void {
  const src = join(repo, "k7-core");
  const dest = join(libs, "k7-core");
  mkdirSync(dest, { recursive: true });
  for (const e of readdirSync(src, { withFileTypes: true }))
    if (e.isDirectory() && LINKED_FAMILIES.has(e.name))
      symlinkSync(join(src, e.name), join(dest, e.name), "junction");
    else
      cpSync(join(src, e.name), join(dest, e.name), {
        recursive: true,
        filter: copyFilter,
      });
}

const write = (path: string, text: string): void => {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, text);
};

const NOTES =
  "notes([t(0).d(1).k(60).v(0.8), t(1).d(1).k(64), t(2).d(2).k(67)])\n";
const SYNTH = "@k7-core/instruments/k7-4osc";

const NOTE_LANE =
  'extends("@meta/track")\nevents([ref("melody").start("1:1:1")])\n';
const CLIP_LANE =
  'extends("@meta/track")\nevents([ref("kick").start("1:1:1")])\n';
const extendsLine = (ref: string): string =>
  `extends(${JSON.stringify(ref)})\n`;

/** One module's project: its graph's segments, its node files, and what its lanes play. */
interface Staging {
  segments: Record<string, string[]>;
  nodes: Record<string, string>;
  /** The lanes play an audio clip (`clips/kick`) rather than the note sequence. */
  clip: boolean;
}

/** Where one variant `fx<s>` of module `ref` plays: after a clip lane, or a note lane when it takes notes, a synth after it when it gives notes. */
function placeVariant(st: Staging, ref: string, s: string): void {
  const kind = ref.split("/")[1];
  const fx = `fx${s}`;
  if (kind === "shaders") {
    st.segments[`pic${s}`] = [fx];
    st.segments[fx] = ["RENDER"];
    st.nodes[`pic${s}`] = 'extends("@meta/picture-track")\n';
  } else if (kind === "instruments" || kind === "transformers") {
    st.clip = false;
    st.segments[`melody${s}`] = [fx];
    st.nodes[`melody${s}`] = NOTE_LANE;
    if (kind === "instruments") st.segments[fx] = ["MIX"];
    else {
      st.segments[fx] = [`synth${s}`];
      st.segments[`synth${s}`] = ["MIX"];
      st.nodes[`synth${s}`] = extendsLine(SYNTH);
    }
  } else if (ref.endsWith("/buss")) {
    // A console's buss sits between the mix and the output, as airwindows.k7 places it.
    st.segments[`drums${s}`] = ["MIX"];
    st.segments.MIX = [...(st.segments.MIX ?? []), fx];
    st.segments[fx] = ["OUT"];
    st.nodes[`drums${s}`] = CLIP_LANE;
  } else {
    st.segments[`drums${s}`] = [fx];
    st.segments[fx] = ["MIX"];
    st.nodes[`drums${s}`] = CLIP_LANE;
  }
}

/** Every k7-core module a project can name, with the engines it offers, read the way a project reads them. */
function k7CoreModules(tmp: string): { ref: string; engines: string[] }[] {
  const probe = join(tmp, "probe.k7");
  write(join(probe, "project.k7x"), 'project({ name: "probe", tempo: 120 })\n');
  write(join(probe, "graph.k7x"), 'modules({ MIX: ["OUT"] })\n');
  const refs = loadModuleRefs(probe);
  return Object.keys(refs)
    .filter((k) => k.startsWith("@k7-core/") && !k.includes("/engines/"))
    .sort()
    .map((ref) => ({
      ref,
      engines: Object.keys(refs[ref].engineRefs ?? {}).sort(),
    }));
}

/** One project per k7-core module, a lane per engine; the default engine is the lane whose node has no `engine()` line. */
function stageK7Core(repo: string, tmp: string, ws: string): void {
  const kick = join(
    repo,
    "e2e",
    "fixtures",
    "clip-lane.k7",
    "clips",
    "kick.wav",
  );
  for (const { ref, engines } of k7CoreModules(tmp)) {
    const name = ref.slice("@k7-core/".length).replaceAll("/", ".");
    const dir = join(ws, `${name}.k7`);
    const video = ref.split("/")[1] === "shaders";
    const st: Staging = { segments: {}, nodes: {}, clip: !video };
    for (const engine of [null, ...engines]) {
      const s = engine ? `-${engine}` : "";
      placeVariant(st, ref, s);
      st.nodes[`fx${s}`] =
        `${extendsLine(ref)}${engine ? `engine(${JSON.stringify(engine)})\n` : ""}`;
    }
    st.segments.MIX ??= ["OUT"];
    if (video) st.segments.RENDER = ["VIDEO_OUT"];
    write(
      join(dir, "project.k7x"),
      `project({ name: ${JSON.stringify(name)}, tempo: 120, sampleRate: 48000, timeSignature: "4/4"${video ? ", viewport: [1280, 720, 30]" : ""} })\n`,
    );
    write(join(dir, "graph.k7x"), `modules(${JSON.stringify(st.segments)})\n`);
    if (st.clip) {
      write(join(dir, "clips", "kick.k7x"), 'source("clips/kick.wav")\n');
      copyFileSync(kick, join(dir, "clips", "kick.wav"));
    } else if (!video) write(join(dir, "sequences", "melody.k7x"), NOTES);
    for (const [n, text] of Object.entries(st.nodes))
      write(join(dir, "nodes", `${n}.k7x`), text);
    write(
      join(dir, "surfaces", "timeline.k7x"),
      'type("timeline")\nlayout("split")\n',
    );
    if (video)
      write(
        join(dir, "surfaces", "viewport.k7x"),
        'type("viewport")\nlayout("floating")\n',
      );
  }
}

const here = import.meta.dirname;
const repo = resolve(here, "../../..");

/** Build the whole tree under a fresh temporary folder; the caller removes `tmp` when done. */
function stageTree(): StagedCorpus {
  const tmp = realpathSync(mkdtempSync(join(tmpdir(), "k7-golden-")));
  pinEnvironment(tmp);
  const libs = join(tmp, "libs");
  stageLibrary(repo, libs);
  const workspaces = Object.fromEntries(
    SOURCES.map((s) => [s, join(tmp, "ws", s)]),
  ) as Record<Source, string>;
  cpSync(join(repo, "demos"), workspaces.demos, {
    recursive: true,
    filter: copyFilter,
  });
  cpSync(join(repo, "e2e", "fixtures"), workspaces.e2e, {
    recursive: true,
    filter: copyFilter,
  });
  mkdirSync(workspaces["k7-core"], { recursive: true });
  stageK7Core(repo, tmp, workspaces["k7-core"]);
  cpSync(join(here, "../fixtures/shapes"), workspaces.shapes, {
    recursive: true,
    filter: copyFilter,
  });
  const k7core = realpathSync(join(repo, "k7-core"));
  const tokens: [string, string][] = [
    [k7core, "<libs>/k7-core"],
    [join(libs, "k7-core"), "<libs>/k7-core"],
    [join(tmp, "ws"), "<ws>"],
    [join(tmp, "home"), "<home>"],
    [join(tmp, "no-mediatek"), "<mediatek>"],
    [tmp, "<tmp>"],
    [realpathSync(repo), "<repo>"],
  ];
  return {
    tmp,
    workspaces,
    tokens: tokens.sort((a, b) => b[0].length - a[0].length),
  };
}

const out = resolve(process.argv[2] ?? "");
if (!process.argv[2]) throw new Error("usage: stageCorpus.ts <out>");
mkdirSync(out, { recursive: true });
for (const k of ["TMPDIR", "TEMP", "TMP"]) process.env[k] = out;
const staged = stageTree();

const isProject = (name: string): boolean => projectApp(name) !== null;
const projects = SOURCES.flatMap((source) => {
  const root = staged.workspaces[source];
  return readdirSync(root, { withFileTypes: true })
    .filter((e) => e.isDirectory() && isProject(e.name))
    .map((e) => e.name)
    .sort()
    .map((p) => ({
      abs: join(root, p),
      project: `${source}/${p}`,
      fixture: `${source}/${p.replace(/\.k7$/, "")}.json`,
    }));
});

const env = Object.fromEntries(
  ["STUDIO_HOME", "STUDIO_LIBRARIES_HOME", "STUDIO_MEDIATEK"].map((k) => [
    k,
    process.env[k] ?? "",
  ]),
);
writeFileSync(
  join(out, "manifest.json"),
  `${JSON.stringify({ tmp: staged.tmp, tokens: staged.tokens, env, projects }, null, 1)}\n`,
);
console.log(`${projects.length} projects staged in ${staged.tmp}`);
