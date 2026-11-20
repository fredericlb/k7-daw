// buildTools.mjs — what the build scripts share: where output goes, spawning with the Windows shell quirk,
// and Windows' own tar. Shared so build-engine.mjs, package-release.mjs and the desktop app's stage.mjs cannot drift.

import { spawnSync } from "node:child_process";
import { join } from "node:path";

// The one output root of the monorepo — deliverables at its top, nothing per-app about it.
export const distDir = (repo) => join(repo, "dist");

export function run(cmd, args, cwd, extraEnv) {
  console.log(`\n▸ ${cmd} ${args.join(" ")}\n  in ${cwd}`);
  const r = spawnSync(cmd, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, ...extraEnv },
    // cmake and npm are resolved through the shell on Windows (.cmd shims).
    shell: process.platform === "win32",
  });
  if (r.error?.code === "ENOENT") {
    console.error(`\nerror: '${cmd}' not found on PATH.`);
    process.exit(1);
  }
  if (r.status !== 0) {
    console.error(`\nerror: ${cmd} ${args[0]} failed (exit ${r.status}).`);
    process.exit(r.status ?? 1);
  }
}

/** System32's bsdtar, by path: it reads and writes a .zip by itself, and a bare `tar` on Windows may be GNU tar, which cannot. */
export const windowsTar = () =>
  join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
