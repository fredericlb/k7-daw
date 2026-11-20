// libraryForks.ts — the FORKS of a library: the `_modules` and `@account` modules that `extends` one of
// its modules, and which of them would inherit something new on an upgrade, since the module they
// extend differs between the two versions. A fork keeps its own changes whatever happens; this only
// says which ones to listen to again. See ../../../docs/library-versions-plan.md.

import type { LibraryVersion } from "@studio/song-schema/libraryVersions";
import { stub } from "./editStub";

/** One fork: the workspace module, and the library module it extends. */
export interface LibraryFork {
  /** Where it is: `_modules/<path>` or `@account/<path>`. */
  module: string;
  /** What it extends, `@<library>/<path>`. */
  base: string;
}

/** The workspace's modules at `repoDir`, and the account's, that extend a module of `library`. */
export function libraryForks(repoDir: string, library: string): LibraryFork[] {
  return stub("song/libraryForks.libraryForks", repoDir, library);
}

/** A digest of a library module's folder at one version, or null when that version has no such module. */
export function libraryModuleDigest(
  library: string,
  version: LibraryVersion,
  ref: string,
): string | null {
  return stub("song/libraryForks.libraryModuleDigest", library, version, ref);
}

/** The forks whose base differs between `from` and `to` — gone from `to` counts as differing. */
export function forksWhoseBaseMoved(
  repoDir: string,
  library: string,
  from: LibraryVersion,
  to: LibraryVersion,
): LibraryFork[] {
  return stub(
    "song/libraryForks.forksWhoseBaseMoved",
    repoDir,
    library,
    from,
    to,
  );
}
