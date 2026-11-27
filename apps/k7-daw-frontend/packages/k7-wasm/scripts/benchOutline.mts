// benchOutline.mts — the editor's tree budget: `k7x.outline` and its nodes against the TypeScript parse it replaced, on the largest library file.
// A reader would get wrong what it times: the whole crossing, JSON both ways and the nodes built, never the Rust parse alone.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { wasmReady } from "../src/abi.ts";
import { parseTree } from "../src/k7x/tree.ts";

const ROOT = join(import.meta.dirname, "..", "..", "..", "..", "..");
const FILE =
  process.argv[2] ?? "k7-core/modules/instruments/synth/surge-xt/ui.k7x";
const text = readFileSync(join(ROOT, FILE), "utf8");

function median(f: () => unknown, runs = 60): number {
  for (let i = 0; i < 10; i++) f();
  const t: number[] = [];
  for (let i = 0; i < runs; i++) {
    const a = performance.now();
    f();
    t.push(performance.now() - a);
  }
  t.sort((x, y) => x - y);
  return t[t.length >> 1];
}

await wasmReady();
const ms = median(() => parseTree(text));
console.log(
  `${FILE}: ${(text.length / 1024).toFixed(0)} KB, k7x.outline + nodes ${ms.toFixed(2)} ms (median)`,
);
