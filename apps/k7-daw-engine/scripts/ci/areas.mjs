// areas.mjs — prints which Rust areas a branch touched since `main`, as words: `backend`, `browser`, both or none.
// The base is K7_AREAS_BASE (the gate sets it), else local main (docs/rust-builds.md).
// What a reader gets wrong: when the base cannot be fetched or read, it prints both; skipping on a doubt is never the answer.
import { execFileSync } from "node:child_process";

const git = (...args) =>
  execFileSync("git", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
const BOTH = "backend browser";
// What makes every area run: the checks' own scripts and the build's configuration.
const ALL = ["apps/k7-daw-engine/scripts/sccache.mjs", "rust-toolchain"];
const BROWSER = [
  "apps/k7-daw-wasm/song/",
  "apps/k7-daw-wasm/song-schema/",
  "apps/k7-daw-wasm/k7-wasm/",
  "apps/k7-daw-wasm/k7x/",
  "apps/k7-daw-wasm/song-edit/",
];
// The backend links the song crate, so a browser change also runs the backend.
const BACKEND = ["apps/k7-daw-backend/", "package-lock.json", ...BROWSER];

function changed() {
  try {
    const base = git("merge-base", "HEAD", process.env.K7_AREAS_BASE ?? "main");
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
