// workerHarness.ts — the built Worker bundle run in a worker_threads thread, with a page-side client over it.
// What a reader gets wrong: the bundle runs untouched; only a `self` is lent to it, made of the thread's parent port.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Worker } from "node:worker_threads";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../../scripts/bundleRuntime.mjs";
import { client } from "../protocol.ts";

let build:
  | Promise<{ dist: string; built: { files: { path: string }[] } }>
  | undefined;

/** Builds the Worker into the first `dist` asked, once per process, and starts a thread over it; `stop()` ends it. */
export async function startWorker(dist: string) {
  const started =
    build ??
    bundleRuntime({ target: "worker", outdir: dist }).then(
      (b: { files: { path: string }[] }) => ({ dist, built: b }),
    );
  build = started;
  const { dist: shared, built } = await started;
  dist = shared;
  mkdirSync(dist, { recursive: true });
  const entry = join(dist, "harness.mjs");
  writeFileSync(
    entry,
    `import { parentPort } from "node:worker_threads";
globalThis.self = { postMessage: (m) => parentPort.postMessage(m), addEventListener: (_t, f) => parentPort.on("message", (d) => f({ data: d })) };
await import("./worker.js");
`,
  );
  const w = new Worker(entry);
  const page = client({
    postMessage: (m) => w.postMessage(m),
    addEventListener: (_t, f) => w.on("message", (d) => f({ data: d })),
  });
  return { page, built, stop: () => w.terminate() };
}
