// bindings.mjs — runs k7-front's bindings binary into the frontend's packages, formats what it wrote and stamps the sources' hash.
// A reader would get wrong that a stale stamp fails `lint:generated`: any edit of the types' Rust sources needs this run again.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { bindingsHash, PACKAGES, TARGETS } from "./bindingsSources.mjs";

const BACKEND = join(import.meta.dirname, "..");
const ROOT = join(BACKEND, "..", "..");
// A target folder is named with --target-dir, never CARGO_TARGET_DIR, which sccache hashes.
const OUT = process.env.K7_BINDINGS_TARGET_DIR;
const dirArgs = OUT ? ["--target-dir", OUT] : [];

const hash = bindingsHash();
execFileSync(
  "cargo",
  [
    "run",
    "-p",
    "k7-front",
    "--features",
    "ts",
    "--bin",
    "bindings",
    ...dirArgs,
    "--",
    PACKAGES,
  ],
  { cwd: BACKEND, stdio: "inherit" },
);
const folders = TARGETS.map((t) => join(PACKAGES, t, "generated"));
// The generated files pass `npm run lint` as written: `check` also sorts ts-rs's imports the way biome orders them.
execFileSync(
  process.execPath,
  [
    join(ROOT, "node_modules", "@biomejs", "biome", "bin", "biome"),
    "check",
    "--write",
    ...folders,
  ],
  { cwd: ROOT, stdio: "inherit" },
);
for (const dir of folders) {
  const index = join(dir, "index.ts");
  const [head, ...rest] = readFileSync(index, "utf8").split("\n");
  writeFileSync(index, [head, `// SOURCES ${hash}`, ...rest].join("\n"));
}
console.log(`bindings: ${TARGETS.join(", ")} written, sources ${hash}`);
