// replayPlan.ts — the plan files the native replays wrote, found and loaded for the replay (`replayRun.ts`) to run.
// A reader would get wrong where the replay runs: here it reads the disk; the Worker's copy is posted the plan.

import { createRequire } from "node:module";
import type { Plan } from "./replayRun.ts";

export * from "./replayRun.ts";

// The real fs: a test under the Vfs aliases `node:fs`, and the plan lives on the disk.
const real = createRequire(import.meta.url)(
  "node:fs",
) as typeof import("node:fs");
const pathm = createRequire(import.meta.url)(
  "node:path",
) as typeof import("node:path");

const REPO = real.realpathSync(
  pathm.join(import.meta.dirname, "..", "..", "..", "..", ".."),
);

/** The folder of plan files the native replays wrote (`npm run test:edit-replay` makes one); none, no replay. */
export function planDir(): string | undefined {
  const given = process.env.K7_EDIT_PLAN;
  return given && real.existsSync(pathm.join(given, "base.json"))
    ? given
    : undefined;
}

export function loadPlan(): Plan {
  const dir = planDir() as string;
  const plan: Plan = { blobs: {}, execs: [] };
  for (const f of ["base", "time", "lane"]) {
    const part = JSON.parse(
      real.readFileSync(pathm.join(dir, `${f}.json`), "utf8"),
    ) as Plan;
    Object.assign(plan.blobs, part.blobs);
    plan.execs.push(...part.execs);
  }
  return plan;
}

export { REPO };
