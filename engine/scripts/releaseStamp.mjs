#!/usr/bin/env node
// releaseStamp.mjs — what a release stamp means: whether CI made it, the channel it follows, the version it ships
// under. `node engine/scripts/releaseStamp.mjs version|channel <stamp>` prints one for a shell. The tray (Rust) and
// the phone (Gradle, Kotlin) keep their own copies, which point here; a change to a rule changes them too.
// An unstamped build (`dev`, empty) follows no channel. See docs/app-downloads.md.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** Where every build is published; the updaters' URLs point there, under the stamp's channel. */
export const DOWNLOADS = "https://k7.rocks/app_downloads";

const RELEASE = /^v\d+\.\d+\.\d+$/;
const SNAPSHOT = /^snapshot-[0-9a-f]{7,40}$/;

/** The two shapes CI makes, and nothing else: `vX.Y.Z`, or `snapshot-` and 7 to 40 lowercase hex digits. */
export const isStamp = (stamp) => RELEASE.test(stamp) || SNAPSHOT.test(stamp);

/** `release` for a tag, `snapshot` for a commit of main, null for a build CI did not stamp. */
export function channelOf(stamp) {
  if (RELEASE.test(stamp)) return "release";
  if (SNAPSHOT.test(stamp)) return "snapshot";
  return null;
}

/** The version a build ships under: the root's, plus the stamp for a snapshot, a prerelease of the release it was cut from. */
export const versionFor = (rootVersion, stamp) =>
  /^snapshot-[0-9a-f]+$/.test(stamp) ? `${rootVersion}-${stamp}` : rootVersion;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [what, stamp = ""] = process.argv.slice(2);
  if (what === "channel") {
    const channel = channelOf(stamp);
    if (!channel) {
      console.error(
        `releaseStamp: '${stamp}' is not vX.Y.Z or snapshot-<commit>`,
      );
      process.exit(1);
    }
    console.log(channel);
  } else if (what === "version") {
    const root = new URL("../../package.json", import.meta.url);
    console.log(
      versionFor(JSON.parse(readFileSync(root, "utf8")).version, stamp),
    );
  } else {
    console.error("usage: releaseStamp.mjs version|channel <stamp>");
    process.exit(2);
  }
}
