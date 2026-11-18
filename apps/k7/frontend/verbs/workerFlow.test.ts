// workerFlow.test.ts — init, open, complete and run in the BUILT Worker inside a real Chromium, over HTTP to a fake server.
// What a reader gets wrong: the project is the cap32 capacity project; the fake server is the runtime's own test double, proxied behind the page's origin.

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer, request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { fakeServer } from "../../../../packages/commands/src/runtime/testing/fakeServer.ts";
import { serveHttp } from "../../../../packages/commands/src/runtime/testing/httpServer.ts";
import { Vfs } from "../../../../packages/commands/src/runtime/vfs.ts";
import { buildWorkerFiles, type WorkerFiles } from "./workerBuild.ts";

const installed = existsSync(chromium.executablePath());
let built: WorkerFiles;
let origin = "";
let close: () => Promise<void> = async () => {};

beforeAll(async () => {
  if (!installed) return;
  built = await buildWorkerFiles();
  const disk = new Vfs(true);
  const root = mkdtempSync(join(tmpdir(), "k7-flow-"));
  const { writeCapacity } = await import(
    "../../../../packages/commands/src/runtime/testing/capacity.ts"
  );
  writeCapacity(join(root, "cap.k7"), 8);
  const fake = await serveHttp(fakeServer(disk, root, "cap.k7"));
  const front = createServer((req, res) => {
    const name = (req.url ?? "").slice(1).split("?")[0] ?? "";
    const bytes = built.files.get(name);
    if (bytes) {
      res.setHeader("Content-Type", "text/javascript");
      return void res.end(Buffer.from(bytes));
    }
    const up = request(
      `${fake.base}${req.url}`,
      { method: req.method, headers: req.headers },
      (r) => {
        res.writeHead(r.statusCode ?? 500, r.headers);
        r.pipe(res);
      },
    );
    req.pipe(up);
  });
  await new Promise<void>((ok) => front.listen(0, "127.0.0.1", ok));
  origin = `http://127.0.0.1:${(front.address() as { port: number }).port}`;
  close = async () => {
    front.close();
    await fake.close();
    rmSync(root, { recursive: true, force: true });
  };
}, 120_000);

afterAll(() => close());

describe.skipIf(!installed)("the built Worker's flow in Chromium", () => {
  it("completes track names and runs a verb as the node harness does", async () => {
    const browser = await chromium.launch();
    try {
      const page = await browser.newPage();
      await page.goto(`${origin}/`);
      const out = await page.evaluate(async (entry) => {
        const w = new Worker(`/${entry}`, { type: "module" });
        let n = 0;
        const ask = (m: object) =>
          new Promise<{ ok: boolean; value: unknown; error?: string }>(
            (ok, fail) => {
              const id = ++n;
              w.onerror = (e) => fail(new Error(`worker: ${e.message}`));
              w.onmessage = (e) => e.data.id === id && ok(e.data);
              w.postMessage({ id, ...m });
            },
          );
        await ask({ type: "init", base: location.origin });
        const opened = await ask({ type: "open", at: "cap.k7" });
        const tracks = await ask({
          type: "complete",
          line: "track rename ",
          labelled: true,
        });
        const run = await ask({
          type: "run",
          line: "track rename l0 lane0",
          options: { want: [] },
        });
        w.terminate();
        return { opened, tracks, run };
      }, built.entry);
      expect(out.opened.ok, JSON.stringify(out.opened)).toBe(true);
      expect((out.tracks.value as unknown[]).length).toBe(8);
      const run = out.run.value as { committed: boolean; output: string[] };
      expect(run.committed, JSON.stringify(out.run)).toBe(true);
    } finally {
      await browser.close();
    }
  }, 120_000);
});
