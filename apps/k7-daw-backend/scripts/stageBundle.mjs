// stageBundle.mjs — what a device's front reads beside itself, laid out in one folder: the built page at its repo
// path and the `k7-core` library archive it seeds (`$STUDIO_LIBRARY_SEED`). The code root is `stageCodeRoot.mjs`'s.
// Usage: node stageBundle.mjs <outDir> [--no-frontend], after the frontend's build; see ../docs/standalone.md.

import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bundleLibrary } from "./libraryArchive.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const out = process.argv[2];
if (!out) throw new Error("usage: stageBundle.mjs <outDir> [--no-frontend]");
// The dev tray's page comes from Vite (K7_DEV_FRONTEND), so its bundle needs no built frontend.
const noFrontend = process.argv.includes("--no-frontend");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

if (!noFrontend) {
  const dist = "apps/k7-daw-frontend/dist";
  if (!existsSync(join(repo, dist)))
    throw new Error(`${dist} is missing — build the frontend first`);
  cpSync(join(repo, dist), join(out, dist), { recursive: true });
}

await bundleLibrary(repo, join(out, "k7-core"));

writeFileSync(join(out, "STAMP"), `${new Date().toISOString()}\n`);
console.log(`bundle staged in ${out}`);
