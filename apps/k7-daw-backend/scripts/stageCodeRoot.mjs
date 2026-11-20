// stageCodeRoot.mjs — the Rust front's code root, laid out where a host ships it: the files the native pieces read
// at its root, each at its repo path.
// Usage: node stageCodeRoot.mjs <outDir>. A host names the folder to the front as K7_CODE_ROOT (docs/service.md).

import { cpSync, existsSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repo = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);

/** What the native pieces read at the root; switch.rs names what a code root must hold. */
export const CODE_ROOT = [
  "apps/k7-daw-frontend/packages/song/meta",
  "apps/k7-daw-frontend/packages/song/samples/video/sample.mp4",
  "apps/k7-daw-frontend/packages/commands/commands.yaml",
  "demos",
];

/** Bytes under `path`, a file or a folder. */
function sizeOf(path) {
  const s = statSync(path);
  if (!s.isDirectory()) return s.size;
  return readdirSync(path).reduce((n, e) => n + sizeOf(join(path, e)), 0);
}

/** The code root copied fresh into `out`; its size in bytes. */
export function stageCodeRoot(out) {
  rmSync(out, { recursive: true, force: true });
  for (const rel of CODE_ROOT) {
    const from = join(repo, rel);
    if (!existsSync(from))
      throw new Error(`${rel} is missing from the checkout`);
    cpSync(from, join(out, rel), { recursive: true });
  }
  return sizeOf(out);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const out = process.argv[2];
  if (!out) throw new Error("usage: stageCodeRoot.mjs <outDir>");
  const bytes = stageCodeRoot(out);
  console.log(`code root staged in ${out}: ${(bytes / 1024).toFixed(0)} KB`);
}
