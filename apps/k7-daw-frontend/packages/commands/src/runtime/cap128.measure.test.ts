// cap128.measure.test.ts — the plan's measures on the capacity project: bundle size, open, a verb, the runtime's bytes.
// A reader would get wrong that it asserts nothing about speed: it prints figures, and runs only with K7_MEASURE=1.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { capacityFiles, writeCapacity } from "./testing/capacity.ts";
import { fakeServer } from "./testing/fakeServer.ts";
import { serveHttp } from "./testing/httpServer.ts";
import { startWorker } from "./testing/workerHarness.ts";
import { Vfs } from "./vfs.ts";

const on = process.env.K7_MEASURE === "1";
let root: string;
let http: Awaited<ReturnType<typeof serveHttp>>;
let server: ReturnType<typeof fakeServer>;

beforeAll(async () => {
  if (!on) return;
  root = mkdtempSync(join(tmpdir(), "cap128-"));
  writeCapacity(join(root, "cap.k7"));
  server = fakeServer(new Vfs(true), root, "cap.k7");
  http = await serveHttp(server);
}, 120_000);

afterAll(async () => {
  if (!on) return;
  await http.close();
  rmSync(root, { recursive: true, force: true });
});

describe.skipIf(!on)("cap128 through the Worker", () => {
  it("measures the bundle, the open and a verb", async () => {
    const out = mkdtempSync(join(tmpdir(), "cap128-out-"));
    const t0 = performance.now();
    const w = await startWorker(out);
    try {
      await w.page.init(http.base);
      const t1 = performance.now();
      await w.page.open("cap.k7");
      const t2 = performance.now();
      const first = await w.page.run("track rename l0 lane0");
      const t3 = performance.now();
      const second = await w.page.run("track rename lane0 lane00");
      const t4 = performance.now();
      const text = JSON.stringify(await server.bundle("cap.k7"));
      const files = capacityFiles().length;
      const gz = gzipSync(text).length;
      const total = w.built.files.reduce(
        (n: number, f: { gzip: number }) => n + f.gzip,
        0,
      );
      console.log(
        JSON.stringify({
          project_files: files,
          bundle_json_bytes: http.bytes.bundle,
          bundle_gzip_bytes: gz,
          worker_gzip_bytes_total: total,
          build_ms: Math.round(t1 - t0),
          open_ms: Math.round(t2 - t1),
          verb_first_ms: Math.round(t3 - t2),
          verb_second_ms: Math.round(t4 - t3),
          first_output: first.output,
          second_output: second.output,
          runs: [first.runs, second.runs],
        }),
      );
      expect(first.committed).toBe(true);
    } finally {
      await w.stop();
      rmSync(out, { recursive: true, force: true });
    }
  }, 600_000);
});
