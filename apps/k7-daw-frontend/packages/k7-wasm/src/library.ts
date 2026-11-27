// library.ts — the LIBRARIES tab's rows and the rules over a library path: song-edit's library_tree, one sync call each.
// A reader would get wrong that the rows are sorted here: every list goes in already in people's order (the page's collator) and keeps it.

import { call } from "./abi.ts";
import type {
  LibraryAssemblyRow,
  LibraryChainRow,
  LibraryLevel,
} from "./types/index.ts";

export type { LibraryAssemblyRow, LibraryChainRow, LibraryLevel };

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `library.${op}`, args });

/** The libraries as file rows (the page types them `FileNode`): a level or a repository with its families. */
export const libraryRows = (
  levels: readonly LibraryLevel[],
  chains: readonly LibraryChainRow[],
  assemblies: readonly LibraryAssemblyRow[],
): unknown[] => ask("tree", { levels, chains, assemblies });

/** Every path under a READ-ONLY level, plus the level's own row. */
export const readOnlyPathList = (levels: readonly LibraryLevel[]): string[] =>
  ask("readOnly", { levels });

/** The index of the level a path is the root of or lies under, null under a read-only level or none. */
export const rootIndexOf = (
  levels: readonly LibraryLevel[],
  path: string,
): number | null => ask("rootOf", { levels, path });

/** The family folder (`samples`, `irs`, `wavetables`, `models`) a row is or lies in, null elsewhere. */
export const fileFolderOf = (path: string): string | null =>
  ask("fileFolder", { path });

/** The ref a chain writes for one of a SHARED module's files, null when the path names none. */
export const libraryModuleRef = (file: string): string | null =>
  ask("moduleRef", { path: file });

/** Is this workspace path inside one of the ACCOUNT's libraries? */
export const isLibraryPath = (path: string): boolean => ask("isPath", { path });

/** WHICH library a `_libraries/<name>/…` path is in, or null. */
export const libraryOfPath = (path: string): string | null =>
  ask("ofPath", { path });
