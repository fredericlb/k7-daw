// supervisorBuild.mjs — building and finding the native supervisor (apps/k7-daw-engine/k7-supervisor).
// Shared by build-engine.mjs and the dev session, so "which binary is the supervisor" has one answer. See docs/buildEngine.md.

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { run } from "./buildTools.mjs";
import { findBuild } from "./stageEngine.mjs";

/** The supervisor's CMake preset for this host; `$STUDIO_SUPERVISOR_PRESET` for a runner's older toolchain. */
function supervisorPreset() {
  return (
    process.env.STUDIO_SUPERVISOR_PRESET ||
    ({ darwin: "macos-clang", win32: "windows-msvc" }[process.platform] ??
      "dev-gcc")
  );
}

/** Configure (once) and build the supervisor with CMake; exits on failure, like every step here. */
export function buildSupervisor(repo) {
  const dir = join(repo, "apps", "k7-daw-engine", "k7-supervisor");
  const preset = supervisorPreset();
  const out = join(dir, "build", preset);
  // A configure that failed leaves the cache without a build system, which only another configure repairs.
  const configured =
    existsSync(join(out, "CMakeCache.txt")) &&
    readdirSync(out).some(
      (f) =>
        f === "build.ninja" ||
        f === "Makefile" ||
        /\.(sln|slnx|xcodeproj)$/.test(f),
    );
  if (!configured) run("cmake", ["--preset", preset], dir);
  run("cmake", ["--build", "--preset", preset], dir);
}

/** The freshest built supervisor, Debug last — the rule the engine binary and the staged app follow too. */
export function findSupervisor(repo) {
  return findBuild(repo, "k7-supervisor", "k7-supervisor");
}
