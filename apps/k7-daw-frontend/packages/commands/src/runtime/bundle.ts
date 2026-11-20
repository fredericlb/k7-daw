// bundle.ts — `/verb-bundle` into the Vfs and into the compiler's bundle: the project, the shared texts, the libraries by hash.
// What a reader gets wrong: a library is fetched by its hash once and shared by every project; the bundle never holds one.

import type {
  BundleFile,
  Entry,
  LibraryFiles,
  Remote,
  VerbBundle,
} from "./remote.ts";
import { join } from "./shims/path.ts";
import { Vfs } from "./vfs.ts";

/** What an open project is made of. */
export interface Loaded {
  vfs: Vfs;
  /** The song crate's `SourceBundle`, libraries merged in, for `song.open`. */
  songBundle: Record<string, unknown>;
  rev: string;
  workspaceRoot: string;
  /** The project's folder, as the vfs spells it. */
  project: string;
  preferences: string | null;
  bundle: VerbBundle;
}

/** Libraries held by hash, for the life of the page. */
export class LibraryCache {
  private readonly held = new Map<string, Promise<LibraryFiles>>();

  get(hash: string, make: () => Promise<LibraryFiles>): Promise<LibraryFiles> {
    let p = this.held.get(hash);
    if (!p) {
      p = make();
      this.held.set(hash, p);
      p.catch(() => this.held.delete(hash));
    }
    return p;
  }

  get size(): number {
    return this.held.size;
  }
}

const parentOf = (p: string): string =>
  p.slice(0, Math.max(p.lastIndexOf("/"), 1));

/** The listing of every folder a set of file paths implies, below `root`: library folders have no listing of their own. */
export function listsOf(root: string, rels: string[]): Record<string, Entry[]> {
  const lists = new Map<string, Map<string, "file" | "dir">>();
  const add = (dir: string, name: string, kind: "file" | "dir"): void => {
    const m = lists.get(dir) ?? new Map<string, "file" | "dir">();
    lists.set(dir, m);
    m.set(name, kind);
  };
  lists.set(root, new Map());
  for (const rel of rels) {
    const parts = rel.split("/");
    let dir = root;
    for (let i = 0; i < parts.length; i++) {
      const last = i === parts.length - 1;
      add(dir, parts[i] as string, last ? "file" : "dir");
      dir = `${dir}/${parts[i]}`;
    }
  }
  const out: Record<string, Entry[]> = {};
  for (const [dir, m] of lists)
    out[dir] = [...m]
      .map(([name, kind]) => ({ name, kind }))
      .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return out;
}

function hold(vfs: Vfs, path: string, f: BundleFile, seconds?: number): void {
  if (f.text !== undefined)
    vfs.load(path, {
      text: f.text,
      len: new TextEncoder().encode(f.text).length,
    });
  else vfs.load(path, { len: f.len ?? 0, binary: true, seconds });
}

/** Opens a project: the bundle, its libraries (cached by hash), the Vfs filled, the compiler's bundle merged. */
export async function loadProject(
  remote: Remote,
  at: string,
  cache = new LibraryCache(),
  /** `false` skips the libraries' files: an import reads none of them, and they are megabytes. */
  withLibraries = true,
): Promise<Loaded> {
  const bundle = await remote.bundle(at);
  const wanted = withLibraries ? bundle.libraries : [];
  const libs = await Promise.all(
    wanted.map((l) =>
      cache.get(l.hash, () => remote.library(at, l.name, l.hash)),
    ),
  );
  const vfs = new Vfs();
  const { files, lists, lengths = {} } = bundle.source;
  for (const [dir, entries] of Object.entries(lists))
    vfs.loadList(dir, entries);
  for (const [path, f] of Object.entries(files))
    hold(vfs, path, f, lengths[path]);
  const songFiles: Record<string, BundleFile> = { ...files };
  const songLists: Record<string, Entry[]> = { ...lists };
  const songLengths: Record<string, number> = { ...lengths };
  wanted.forEach((l, i) => {
    const lib = libs[i] as LibraryFiles;
    const rels = Object.keys(lib.files);
    for (const [dir, entries] of Object.entries(listsOf(l.root, rels))) {
      vfs.loadList(dir, entries);
      songLists[dir] = entries;
    }
    for (const rel of rels) {
      const path = `${l.root}/${rel}`;
      const sec = lib.lengths?.[rel] ?? lib.lengths?.[path];
      hold(vfs, path, lib.files[rel] as BundleFile, sec);
      songFiles[path] = lib.files[rel] as BundleFile;
      if (sec !== undefined) songLengths[path] = sec;
    }
  });
  const project = projectPath(bundle.workspaceRoot, at);
  // Only the open project's reads are covered by its token; a chain's and the workspace alone's are each checked.
  vfs.inside = isProjectAt(at) ? [project] : [];
  vfs.world = [bundle.workspaceRoot, ...bundle.libraries.map((l) => l.root)];
  // The account's preferences sit beside the machine's scan file, whose folder the request names.
  const scan = (
    bundle.source.request as { roots?: { scan?: string } } | undefined
  )?.roots?.scan;
  if (scan && bundle.preferences !== null)
    vfs.load(join(scan.slice(0, scan.lastIndexOf("/")), "preferences.k7x"), {
      text: bundle.preferences,
      len: new TextEncoder().encode(bundle.preferences).length,
    });
  return {
    vfs,
    songBundle: {
      ...bundle.source,
      files: songFiles,
      lists: songLists,
      lengths: songLengths,
    },
    rev: bundle.rev,
    workspaceRoot: bundle.workspaceRoot,
    project,
    preferences: bundle.preferences,
    bundle,
  };
}

const LIBRARIES = "_libraries/";

/** Whether `at` names the open project, rather than a library chain or (empty) no project. */
export const isProjectAt = (at: string): boolean =>
  at !== "" && !at.startsWith(LIBRARIES);

/** The vfs path of `at`: a project below the workspace, a chain below its library's virtual root, "" the workspace itself. */
export function projectPath(workspaceRoot: string, at: string): string {
  if (at.startsWith(LIBRARIES)) return `/l/${at.slice(LIBRARIES.length)}`;
  return at === "" ? workspaceRoot : join(workspaceRoot, at);
}

/** The workspace-relative name of a vfs path (a library's file is `_libraries/…`), or null when it is outside both. */
export function relativeTo(workspaceRoot: string, path: string): string | null {
  if (path.startsWith("/l/")) return `${LIBRARIES}${path.slice(3)}`;
  return path.startsWith(`${workspaceRoot}/`)
    ? path.slice(workspaceRoot.length + 1)
    : null;
}

export { parentOf };
