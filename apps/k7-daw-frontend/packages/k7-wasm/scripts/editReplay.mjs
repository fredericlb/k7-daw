// editReplay.mjs — the native replays write their screened calls, then the module re-runs them through `edit.call`.
// A reader would get wrong why it is not part of `npm test`: it compiles the song-edit replays first, which takes minutes cold.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PKG = join(import.meta.dirname, "..");
const dir = mkdtempSync(join(tmpdir(), "k7-edit-plan-"));
const env = { ...process.env, K7_EDIT_PLAN: dir };
const run = (cmd, args, cwd) =>
  execFileSync(cmd, args, { cwd, env, stdio: "inherit" });
run(
  "cargo",
  [
    "test",
    "--release",
    "--test",
    "replay",
    "--test",
    "replay_lane",
    // A target folder in the tree would be committed by a `git add -A`.
    ...(process.argv.includes("--target-dir")
      ? []
      : ["--target-dir", join(tmpdir(), "k7-edit-replay-target")]),
    ...process.argv.slice(2),
  ],
  join(PKG, "..", "..", "..", "k7-daw-wasm", "song-edit"),
);
run("npx", ["vitest", "run", "src/edit.replay.test.ts"], PKG);
