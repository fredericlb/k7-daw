// diskBundle.ts — what `GET /verb-bundle` would answer for a project on this disk, in the server's virtual roots, for the runtime's tests.
// What a reader gets wrong: every path is virtual (`/w`, `/l/<name>`, `/k7`, `/s`, `/b/<n>`, `/x/<n>`), spelled as k7-file-service's verb_paths.rs does.

import { createHash } from "node:crypto";
import { lstatSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { type SongRoots, songRig, songRoots } from "@studio/song/songRoots";
import { wavSeconds } from "@studio/take-render/wavLength";
import type { BundleFile, Entry, LibraryFiles, VerbBundle } from "../remote.ts";

const BINARY =
  /\.(wav|mp3|flac|ogg|aif|aiff|png|jpg|jpeg|mp4|mov|webm|bin|onnx)$/i;
const SHARED = ["_modules", "_playlists", "_library-pins.k7x"];
// The checkout holding apps/k7-daw-wasm/verbs/commands.yaml: this file is five folders below it.
const CODE_ROOT = resolve(import.meta.dirname, "../../../../../../..");

interface Walk {
  files: Record<string, BundleFile>;
  lists: Record<string, Entry[]>;
}

/** A real path's virtual spelling and back: the longest real root names a path. */
export interface Spelling {
  virt(real: string): string;
  real(virt: string): string | null;
}

const within = (root: string, p: string): boolean =>
  p === root || p.startsWith(`${root}/`);

function walk(path: string, into: Walk): void {
  let st: ReturnType<typeof lstatSync>;
  try {
    st = lstatSync(path);
  } catch {
    return;
  }
  if (st.isFile()) {
    into.files[path] = fileOf(path, st.size);
    return;
  }
  if (!st.isDirectory()) return;
  const entries: Entry[] = [];
  for (const name of readdirSync(path).sort()) {
    const child = join(path, name);
    const cs = lstatSync(child);
    entries.push({ name, kind: cs.isDirectory() ? "dir" : "file" });
    walk(child, into);
  }
  into.lists[path] = entries;
}

/** A folder's names and lengths, never its bytes: the models folder, as verb_bundle.rs's `add_names` lists it. */
function walkNames(path: string, into: Walk): void {
  let st: ReturnType<typeof statSync>;
  try {
    st = statSync(path);
  } catch {
    return;
  }
  if (st.isFile()) {
    into.files[path] = { len: st.size, binary: true };
    return;
  }
  if (!st.isDirectory()) return;
  const entries: Entry[] = [];
  for (const name of readdirSync(path).sort()) {
    if (name.startsWith(".")) continue;
    const isDir = statSync(join(path, name)).isDirectory();
    entries.push({ name, kind: isDir ? "dir" : "file" });
    walkNames(join(path, name), into);
  }
  into.lists[path] = entries;
}

/** The seconds of each wav the walk found, by real path: what the server's bundle states beside the bytes it does not send. */
function wavLengths(into: Walk): Record<string, number> {
  const out: Record<string, number> = {};
  for (const path of Object.keys(into.files)) {
    const s = /\.wav$/i.test(path) ? wavSeconds(path) : null;
    if (s !== null) out[path] = s;
  }
  return out;
}

function fileOf(path: string, len: number): BundleFile {
  // An empty file is UTF-8 to the compile, never a binary.
  if (BINARY.test(path) && len > 0) return { len, binary: true };
  return { text: readFileSync(path, "utf8") };
}

/** The spelling of the roots `roots` names for the project at `dir`, as verb_paths.rs builds them. */
export function spellingFrom(dir: string, roots: SongRoots): Spelling {
  const pairs: [string, string][] = [];
  const add = (real: string, virt: string): void => {
    const r = resolve(real);
    if (!pairs.some(([x]) => x === r)) pairs.push([r, virt]);
  };
  add(dirname(dir), "/w");
  for (const l of roots.libraries) add(l.root, `/l/${l.name}`);
  add(roots.meta, "/l/@meta");
  add(CODE_ROOT, "/k7");
  add(dirname(roots.scan), "/s");
  for (const [i, d] of roots.sample_banks.entries()) add(d, `/b/${i}`);
  const rest = [
    ...roots.also_readable,
    roots.confine,
    roots.models,
    roots.renders,
    roots.scan,
  ];
  for (const [i, d] of rest.entries())
    if (!pairs.some(([r]) => within(r, resolve(d)))) add(d, `/x/${i}`);
  pairs.sort((a, b) => b[0].length - a[0].length);
  const byVirt = [...pairs].sort((a, b) => b[1].length - a[1].length);
  return {
    virt(real) {
      const p = resolve(real);
      const hit = pairs.find(([r]) => within(r, p));
      if (hit) return hit[1] + p.slice(hit[0].length);
      return `/x/${createHash("sha256").update(p).digest("hex").slice(0, 12)}`;
    },
    real(virt) {
      if (virt.split("/").includes("..")) return null;
      const hit = byVirt.find(
        ([, v]) => virt === v || virt.startsWith(`${v}/`),
      );
      return hit ? hit[0] + virt.slice(hit[1].length) : null;
    },
  };
}

/** The spelling of the project at `project`, from its own song roots. */
export function spellingOf(project: string): Spelling {
  const dir = resolve(project);
  return spellingFrom(dir, songRoots(dir));
}

/** The roots as the bundle shows them: every folder and file in virtual spelling. */
function spelledRoots(roots: SongRoots, sp: Spelling): SongRoots {
  const m = (p: string): string => sp.virt(p);
  return {
    ...roots,
    project: m(roots.project),
    workspace: m(roots.workspace),
    confine: m(roots.confine),
    libraries: roots.libraries.map((l) => ({ ...l, root: m(l.root) })),
    meta: m(roots.meta),
    scan: m(roots.scan),
    renders: m(roots.renders),
    models: m(roots.models),
    sample_banks: roots.sample_banks.map(m),
    also_readable: roots.also_readable.map(m),
  };
}

function spelledKeys<T>(
  obj: Record<string, T>,
  sp: Spelling,
): Record<string, T> {
  return Object.fromEntries(
    Object.entries(obj).map(([p, v]) => [sp.virt(p), v]),
  );
}

/** The bundle of `project`, its libraries named by a hash of their names (the double does not hash bytes). */
export function diskBundle(project: string, rev: string): VerbBundle {
  const dir = resolve(project);
  const root = dirname(dir);
  const real = songRoots(dir);
  const sp = spellingFrom(dir, real);
  const into: Walk = { files: {}, lists: {} };
  walk(dir, into);
  for (const name of SHARED) walk(join(root, name), into);
  walkNames(real.models, into);
  const libraries = [
    ...real.libraries.map((l) => ({ name: l.name, root: l.root })),
    { name: "@meta", root: real.meta },
  ].map((l) => ({
    name: l.name,
    root: sp.virt(l.root),
    hash: `h-${l.name}`,
    files: 0,
    bytes: 0,
  }));
  const mounts = [
    real.project,
    join(root, "_modules"),
    real.confine,
    real.renders,
    real.models,
    real.meta,
  ]
    .concat(
      real.libraries.map((l) => l.root),
      real.also_readable,
    )
    .filter((m, i, all) => all.indexOf(m) === i)
    .map((m) => sp.virt(m));
  return {
    format: 1,
    at: dir.slice(root.length + 1),
    rev,
    source: {
      format: 1,
      mounts,
      files: spelledKeys(into.files, sp),
      lists: spelledKeys(into.lists, sp),
      lengths: spelledKeys(wavLengths(into), sp),
      request: {
        roots: spelledRoots(real, sp),
        rig: songRig(),
        vsts: "none",
        thawed: [],
      },
    },
    workspaceRoot: sp.virt(root),
    preferences: null,
    libraries,
  };
}

/** One library's files, by their path below its real root. */
export function diskLibrary(name: string, rootDir: string): LibraryFiles {
  const into: Walk = { files: {}, lists: {} };
  try {
    if (statSync(rootDir).isDirectory()) walk(rootDir, into);
  } catch {
    // A library that is not there is an empty one.
  }
  const files: Record<string, BundleFile> = {};
  for (const [abs, f] of Object.entries(into.files))
    files[abs.slice(rootDir.length + 1)] = f;
  return { name, hash: `h-${name}`, files };
}
