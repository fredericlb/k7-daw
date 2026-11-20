// run-supervisor-tests.mjs — the native supervisor's C++ suite (apps/k7-daw-engine/k7-supervisor), run after run-tests.mjs by `npm test`.
// What a reader gets wrong: like run-tests.mjs it SKIPS an unconfigured tree rather than configure one; the suite itself takes under a second.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { cappedBuildEnv, takeBuildLock } from "./buildLock.mjs";

const PRESET =
  process.env.STUDIO_SUPERVISOR_PRESET ||
  { win32: "windows-msvc", darwin: "macos-clang" }[process.platform] ||
  "dev-gcc";
const buildDir = join(import.meta.dirname, "..", "..", "k7-supervisor", "build", PRESET);

if (!existsSync(join(buildDir, "CMakeCache.txt"))) {
  console.log(`\n  SKIPPED — apps/k7-daw-engine/k7-supervisor has no build/${PRESET}: cmake --preset ${PRESET}\n`);
  process.exit(0);
}

const env = cappedBuildEnv();
const run = (cmd, args) => {
  const r = spawnSync(cmd, args, { stdio: "inherit", env });
  if (r.error) {
    console.error(`\n  ${cmd} could not be run: ${r.error.message}\n`);
    return 1;
  }
  return r.status ?? 1;
};

takeBuildLock(buildDir, "run-supervisor-tests.mjs");
const built = run("cmake", ["--build", buildDir, "--target", "k7_supervisor_tests"]);
process.exit(built !== 0 ? built : run("ctest", ["--test-dir", buildDir, "-C", "RelWithDebInfo", "--output-on-failure"]));
