// stage.mjs — lay out src-tauri/resources for `tauri build` (and `tauri dev`): the front's bundle (the built page and
// the library it seeds), standalone.json, the code root, the engine and the supervisor.
// The engine comes from the repo's builds, or `$STUDIO_PREBUILT_BIN` in CI (apps/k7-daw-engine/scripts/docs/buildEngine.md).

import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { stageCodeRoot } from "../../k7-daw-backend/scripts/stageCodeRoot.mjs";
import { run } from "../../k7-daw-engine/scripts/buildTools.mjs";
import { stageEngine } from "../../k7-daw-engine/scripts/stageEngine.mjs";
import { signStaged } from "./macSign.mjs";

const app = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(app, "..", "..");
const out = join(app, "src-tauri", "resources");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

console.log("· frontend");
run("npm", ["exec", "--workspace", "k7-frontend", "--", "vite", "build"], repo);

console.log("· page and library");
run(
  "node",
  [
    join(repo, "apps", "k7-daw-backend", "scripts", "stageBundle.mjs"),
    join(out, "backend"),
  ],
  repo,
);
copyFileSync(
  join(repo, "apps", "k7-daw-backend", "standalone.json"),
  join(out, "standalone.json"),
);

// The files the Rust front's actor reads at its code root, which the desktop app names to it as K7_CODE_ROOT (docs/app.md).
const code = stageCodeRoot(join(out, "code"));
console.log(`· the front's code root, ${(code / 1024).toFixed(0)} KB`);

console.log("· engine and supervisor");
stageEngine({ repo, bin: join(out, "bin") });

// Notarization refuses any unsigned Mach-O inside the bundle, and Tauri signs only the main binary (docs/macos.md).
if (process.platform === "darwin") {
  console.log(`· signed ${signStaged(out)} binaries`);
}

console.log(`staged in ${out}`);
