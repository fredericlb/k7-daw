// areas.mjs — prints which Rust areas a branch touched since `main`, as words: `backend`, `browser`, both or none.
// The base is K7_AREAS_BASE (the gate sets it to local main), else origin's main (docs/rust-builds.md).
// What a reader gets wrong: when the base cannot be fetched or read, it prints both; skipping on a doubt is never the answer.
import { execFileSync } from "node:child_process";

const git = (...args) =>
  execFileSync("git", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
const BOTH = "backend browser";
// What makes every area run: the checks' own scripts and the build's configuration.
const ALL = [
  ".github/workflows/",
  "engine/scripts/ci/",
  "engine/scripts/sccache.mjs",
  "rust-toolchain",
];
const BROWSER = [
  "packages/song/rust/",
  "packages/song-schema/rust/",
  "packages/k7-wasm/rust/",
  "packages/k7x/rust/",
  "packages/song-edit/rust/",
];
// The backend links the song crate, so a browser change also runs the backend.
const BACKEND = ["apps/k7/backend-rs/", "package-lock.json", ...BROWSER];

function changed() {
  try {
    // K7_AREAS_BASE names a local ref instead, to try the rule on a machine.
    let ref = process.env.K7_AREAS_BASE;
    if (!ref) {
      ref = "refs/remotes/origin/main";
      git("fetch", "--quiet", "origin", `+refs/heads/main:${ref}`);
    }
    const base = git("merge-base", "HEAD", ref);
    // Against the working tree, so a local run counts what is not committed yet; a clean checkout is the same either way.
    return git("diff", "--name-only", base).split("\n").filter(Boolean);
  } catch {
    return null;
  }
}

const files = changed();
if (files === null) {
  console.log(BOTH);
} else {
  const hit = (prefixes) =>
    files.some((f) => prefixes.some((p) => f === p || f.startsWith(p)));
  if (hit(ALL)) console.log(BOTH);
  else
    console.log(
      [hit(BACKEND) && "backend", hit(BROWSER) && "browser"]
        .filter(Boolean)
        .join(" "),
    );
}
