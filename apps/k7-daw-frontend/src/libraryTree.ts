// libraryTree.ts — the LIBRARIES tab: one row per module level, its modules under it, and each library repository as one row with its families.
// The rows and the path rules are Rust's (song-edit's library_tree); what stays here is the collator order every list is sent in.
// Why the tab exists: apps/k7-daw-frontend/docs/sharedModules.md. The levels come from the file service (`GET /module-libraries`).

import type { LIBRARY_FILE_FAMILIES } from "@studio/k7-wasm/consts";
import {
  fileFolderOf as folderOf,
  libraryRows,
  readOnlyPathList,
  rootIndexOf,
} from "@studio/k7-wasm/library";
import { isDocFile } from "@studio/k7-wasm/schema";
import type { FileNode, ModuleLibrary } from "@studio/web-protocol";
import { byPeople, byPeoplePath, sortNodes } from "@studio/web-protocol/tree";

export {
  isLibraryPath,
  libraryModuleRef,
  libraryOfPath,
} from "@studio/k7-wasm/library";

/** The family folders a library holds, as the file params name them. */
type LibraryFileFamily = keyof typeof LIBRARY_FILE_FAMILIES;

/** A library chain, as the tab lists it: `ref` is `@<library>/<path under chains/>`, `path` its file or a folder chain's folder. */
export interface LibraryChainRow {
  ref: string;
  path: string;
  /** A FOLDER chain's node files; a chain file holds its nodes, so none. */
  nodes?: string[];
}

/** A chain kept as ONE file — the format; a folder chain is one a library clone has not migrated yet. */
export const isChainFile = isDocFile;

/** An assembly template of the library: `path` its folder, `file` the `assembly.k7x` the builder opens. */
export interface LibraryAssemblyRow {
  ref: string;
  path: string;
  file: string;
}

/** A file folder's rows for people, folders first: the service answers in code points (../docs/listOrder.md). */
const forPeople = (nodes: FileNode[] | undefined): FileNode[] | undefined =>
  nodes && sortNodes(nodes, { foldersFirst: true });

/** The libraries as tree rows, every list sent in the order people read it (Rust keeps that order). */
export function libraryTree(
  libraries: ModuleLibrary[],
  // The libraries' CHAINS, by `@<library>/…` ref — each filed under its own library's row.
  chains: readonly LibraryChainRow[] = [],
  // …and their ASSEMBLY templates, the third family — one leaf each, the assembly builder being what opens one.
  assemblies: readonly LibraryAssemblyRow[] = [],
): FileNode[] {
  const levels = libraries.map((l) => ({
    ...l,
    nodes: forPeople(l.nodes) ?? [],
    samples: forPeople(l.samples),
    irs: forPeople(l.irs),
    wavetables: forPeople(l.wavetables),
    models: forPeople(l.models),
  }));
  const sorted = [...chains]
    .sort((a, b) => byPeoplePath(a.ref, b.ref))
    .map((c) => ({ ...c, nodes: c.nodes && [...c.nodes].sort(byPeople) }));
  const templates = [...assemblies].sort((a, b) => byPeoplePath(a.ref, b.ref));
  return libraryRows(levels, sorted, templates) as FileNode[];
}

/** Which of a library's FILE folders this row is or lies in — `samples/`, `irs/`, `wavetables/`, `models/` — the only places a drop goes. */
export const fileFolderOf = (path: string): LibraryFileFamily | null =>
  folderOf(path) as LibraryFileFamily | null;

/** Every path under a READ-ONLY level, plus the level's own row — `@engine` and only it, a scanned plugin having no folder. */
export const readOnlyPaths = (libraries: ModuleLibrary[]): Set<string> =>
  new Set(readOnlyPathList(libraries.filter((l) => l.readOnly)));

/** The library ROOT a row belongs to, or null under a read-only level — what 'New folder…' and 'New module…' resolve against. */
export function rootOf(
  libraries: ModuleLibrary[],
  path: string,
): ModuleLibrary | null {
  // Only where each level is travels: the rows under it do not decide a root.
  const levels = libraries.map(({ ns, path: p, readOnly }) => ({
    ns,
    path: p,
    readOnly,
    nodes: [],
  }));
  const at = rootIndexOf(levels, path);
  return at === null ? null : libraries[at];
}
