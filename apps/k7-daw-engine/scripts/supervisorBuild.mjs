// supervisorBuild.mjs — building and finding the machine's supervisor, the Rust crate apps/k7-daw-backend/packages/k7-supervisor,
// with ONNX Runtime copied beside the binary, where it loads it from. Shared by build-engine.mjs and the dev session, so
// "which binary is the supervisor" has one answer. See docs/buildEngine.md.

import { copyFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { run } from "./buildTools.mjs";
import { ortRuntime } from "./onnxRuntime.mjs";
import { sccacheEnv } from "./sccache.mjs";

const exe = (n) => (process.platform === "win32" ? `${n}.exe` : n);

/** The cargo target the supervisor is built in: `$K7_SUPERVISOR_TARGET_DIR`, else one per checkout outside it. */
export function supervisorTargetDir(repo) {
  return (
    process.env.K7_SUPERVISOR_TARGET_DIR ||
    join(homedir(), ".cache", `k7-supervisor-${basename(repo)}`)
  );
}

/** Build the release supervisor and lay ONNX Runtime beside it; exits on failure, like every step here. */
export async function buildSupervisor(repo) {
  const target = supervisorTargetDir(repo);
  // A wrapper already set (an agent's, the dev session's) wins over the dev cache's.
  const env = process.env.RUSTC_WRAPPER === undefined ? sccacheEnv("dev") : {};
  run(
    "cargo",
    [
      "build",
      "--release",
      "--locked",
      "-p",
      "k7-supervisor",
      "--bin",
      "k7-supervisor",
      "--target-dir",
      target,
    ],
    join(repo, "apps", "k7-daw-backend"),
    env,
  );
  const ort = await ortRuntime();
  // Not fatal: the supervisor runs, and says it cannot cut stems.
  if (!ort.length)
    console.warn(
      `warning: ONNX Runtime ships no library for ${process.platform}-${process.arch} — no stem separation`,
    );
  for (const f of ort) copyFileSync(f, join(target, "release", basename(f)));
}

/** The built supervisor, or null. */
export function findSupervisor(repo) {
  const path = join(supervisorTargetDir(repo), "release", exe("k7-supervisor"));
  return existsSync(path) ? path : null;
}
