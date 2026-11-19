#!/usr/bin/env node
// dev-front.mjs — the dev session's Rust front (the Tiltfile's `k7-front`): on 8789, given env.yaml's `k7-front`
// section (apps/k7/backend-rs/docs/front.md, docs/dev.md).

import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { loadServiceEnv } from "@studio/env";
import { sccacheEnv } from "../../../../engine/scripts/sccache.mjs";

const backendRs = join(dirname(fileURLToPath(import.meta.url)), "..");
const service = loadServiceEnv("k7-front");
const env = {
  ...process.env,
  ...service,
  // A second session on the machine, the macOS agent's e2e bed, names other ports (engine/scripts/ci/e2e-macos.sh).
  K7_FRONT_LISTEN: process.env.K7_FRONT_LISTEN || "127.0.0.1:8789",
  // The dev session's own, which the e2e specs behind the internal seam name too.
  K7_FRONT_SECRET: "k7-dev-front",
  // Vite proxies every page request, and its X-Forwarded-* name the page's origin.
  K7_FRONT_TRUST_HOPS: "1",
  // The registry, the relay and the tunnel hub answered here (docs/dev.md).
  K7_ENGINE_CORE: "front",
  // The file hub, its relay and pairing answered here (docs/dev.md).
  K7_FILES: "front",
  // rustc through sccache when the machine has it: a new worktree's first build then reuses the dependencies (docs/rust-builds.md).
  ...sccacheEnv("dev"),
};

// K7_FRONT_TARGET_DIR keeps a bed's build out of the tree (e2e/docs/e2e.md).
const targetDir = process.env.K7_FRONT_TARGET_DIR
  ? ["--target-dir", process.env.K7_FRONT_TARGET_DIR]
  : [];
const cargo = (args) =>
  spawn("cargo", [...args, ...targetDir], {
    cwd: backendRs,
    env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
const child = cargo(["run", "--release", "-p", "k7-front"]);
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => child.kill(signal));
/** What an exit code says on Windows, where a crash leaves no line of its own (apps/k7/backend-rs/docs/front.md). */
const WINDOWS_CODES = {
  3: "abort() from C",
  3221225477: "an access violation",
  3221225725: "a stack overflow",
  3221226505: "a fail-fast: a Rust abort or a stack buffer overrun",
  3221225786: "Ctrl-C",
  3221226356: "a corrupted heap",
};
function describeExit(code, signal) {
  if (code === null) return `killed by ${signal ?? "a signal"}`;
  const hex = `0x${(code >>> 0).toString(16).padStart(8, "0")}`;
  const why = WINDOWS_CODES[code >>> 0];
  return `exit code ${code} (${hex})${why ? `: ${why}` : ""}`;
}
// Said whatever the code: a front gone with nothing in its log is explained here.
child.on("exit", (c, signal) => {
  console.log(`k7-front exited, ${describeExit(c, signal)}`);
  process.exit(c ?? 1);
});
