// libraryArchive.mjs — the `k7-core` library a desktop or phone bundle carries: the repo's `k7-core/`, copied as it
// stands, manifest included. The library is frozen to this seed; it changes with an app update (`docs/libraries.md`).
// What a reader gets wrong: nothing is fetched, so a stale `k7-core/library.k7x` version ships as it is.

import { cpSync, readFileSync, rmSync } from "node:fs";
import { basename, join } from "node:path";

/** This build's LIBRARY_LEVEL, read off the source that declares it. */
function buildLevel(repo) {
  const src = readFileSync(
    join(
      repo,
      "apps/k7-daw-frontend/packages/song-schema/src/libraryVersions.ts",
    ),
    "utf8",
  );
  const m = /export const LIBRARY_LEVEL = (\d+);/.exec(src);
  if (!m) throw new Error("LIBRARY_LEVEL not found in libraryVersions.ts");
  return Number(m[1]);
}

/** Lay the repo's `k7-core` out at `dest`; throws when this build cannot run what the manifest requires. */
export async function bundleLibrary(repo, dest) {
  const text = readFileSync(join(repo, "k7-core", "library.k7x"), "utf8");
  const version = Number(/^version\((\d+)\)/m.exec(text)?.[1] ?? 1);
  const requires = Number(/^requires\((\d+)\)/m.exec(text)?.[1] ?? 1);
  const level = buildLevel(repo);
  if (requires > level)
    throw new Error(
      `k7-core ${version} requires level ${requires}, this build runs ${level}`,
    );
  rmSync(dest, { recursive: true, force: true });
  cpSync(join(repo, "k7-core"), dest, {
    recursive: true,
    filter: (src) => basename(src) !== ".git",
  });
  console.log(`library: k7-core ${version}, from the repo`);
}
