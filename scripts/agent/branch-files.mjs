// branch-files.mjs — the files a branch's own commits touched since its base, and the files that change every backend check.
// Shared by affected-crates.mjs and affected-tests.mjs so the two cannot disagree on the range.
// What a reader gets wrong: a merge of main brings nothing in (checked on main already); only a merge's own resolution counts.
import { execFileSync } from "node:child_process";

export const run = (cmd, args, cwd, env = {}) =>
  execFileSync(cmd, args, {
    cwd,
    env: { ...process.env, ...env },
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
    maxBuffer: 1 << 28,
  });

// Files that change every crate's build or check: the workspace's manifests and settings, the toolchain, the Rust checks' own scripts.
export const WIDE = [
  "apps/k7-daw-backend/Cargo.toml",
  "apps/k7-daw-backend/Cargo.lock",
  "apps/k7-daw-backend/.config/",
  "rust-toolchain",
  "apps/k7-daw-engine/scripts/sccache.mjs",
  "scripts/agent/mac-checks.sh",
  "scripts/agent/rerun-failed.sh",
  "scripts/agent/affected-crates.mjs",
  "scripts/agent/affected-tests.mjs",
  "scripts/agent/branch-files.mjs",
  "scripts/agent/gate.sh",
];

export const MANIFESTS = [
  "apps/k7-daw-backend/Cargo.toml",
  "apps/k7-daw-backend/Cargo.lock",
];

// The base is K7_CRATES_BASE (gate.sh --milestone passes the last milestone that passed on this branch), else main's merge-base.
export function branchBase(root) {
  return (
    process.env.K7_CRATES_BASE ||
    run("git", ["merge-base", "HEAD", "main"], root).trim()
  );
}

export function branchFiles(root, base = branchBase(root)) {
  const own = (args) => run("git", args, root).split("\n");
  return [
    ...own([
      "log",
      "--no-merges",
      "--format=",
      "--name-only",
      `${base}..HEAD`,
      "--not",
      "main",
    ]),
    // A merge's own resolution: the files where it differs from every parent.
    ...own([
      "log",
      "--merges",
      "--format=",
      "--name-only",
      "-c",
      `${base}..HEAD`,
    ]),
    ...own(["diff", "--name-only", "HEAD"]),
    ...own(["ls-files", "--others", "--exclude-standard"]),
  ].filter(Boolean);
}

// A lock or workspace manifest change that comes with a crate's own manifest belongs to that crate; alone, it is a bump.
export function wideFiles(files) {
  const crateManifest = files.some(
    (f) =>
      f.startsWith("apps/k7-daw-backend/packages/") &&
      f.endsWith("/Cargo.toml"),
  );
  return files.filter(
    (f) =>
      WIDE.some((w) => f === w || f.startsWith(w)) &&
      !(crateManifest && MANIFESTS.includes(f)),
  );
}
