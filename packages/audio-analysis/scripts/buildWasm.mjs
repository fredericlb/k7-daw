// buildWasm.mjs — builds rust/ for wasm32 and writes it to waveform.wasm, with the hash of its sources in waveform.wasm.sources.
// A reader would get wrong why the module is checked in: the Rust front embeds it (k7-mediatek's build.rs) and builds with no wasm target.

import { execFileSync } from "node:child_process";
import { copyFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { rustSourceHash } from "./rustSources.mjs";

const PKG = join(import.meta.dirname, "..");
const RUST = join(PKG, "rust");
const TARGET = "wasm32-unknown-unknown";

execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "--lib",
    "--target",
    TARGET,
    "--features",
    "decode encode",
  ],
  {
    cwd: RUST,
    stdio: "inherit",
  },
);
const out = join(PKG, "waveform.wasm");
copyFileSync(join(RUST, "target", TARGET, "release", "waveform.wasm"), out);
writeFileSync(`${out}.sources`, `${rustSourceHash()}\n`);
console.log(`${out}: written`);
