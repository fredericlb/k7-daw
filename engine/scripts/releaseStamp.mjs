#!/usr/bin/env node
// releaseStamp.mjs — what a release stamp means: whether it is one CI-style stamp, and the version a build ships
// under. `node engine/scripts/releaseStamp.mjs version <stamp>` prints the version for a shell. The tray (Rust) and
// the phone (Gradle, Kotlin) keep their own copies, which point here; a change to a rule changes them too.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const RELEASE = /^v\d+\.\d+\.\d+$/;
const SNAPSHOT = /^snapshot-[0-9a-f]{7,40}$/;

/** The two shapes a build is stamped with, and nothing else: `vX.Y.Z`, or `snapshot-` and 7 to 40 lowercase hex digits. */
export const isStamp = (stamp) => RELEASE.test(stamp) || SNAPSHOT.test(stamp);

/** The version a build ships under: the root's, plus the stamp for a snapshot, a prerelease of the release it was cut from. */
export const versionFor = (rootVersion, stamp) =>
  /^snapshot-[0-9a-f]+$/.test(stamp) ? `${rootVersion}-${stamp}` : rootVersion;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [what, stamp = ""] = process.argv.slice(2);
  if (what === "version") {
    const root = new URL("../../package.json", import.meta.url);
    console.log(
      versionFor(JSON.parse(readFileSync(root, "utf8")).version, stamp),
    );
  } else {
    console.error("usage: releaseStamp.mjs version <stamp>");
    process.exit(2);
  }
}
