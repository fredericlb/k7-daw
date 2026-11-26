// workerLoad.test.ts — the built Worker loads and answers in a real Chromium, from files served as a production build serves them.
// What a reader gets wrong: it skips when Playwright's Chromium is not installed; the e2e suite's install step provides it.

import { existsSync } from "node:fs";
import { createServer } from "node:http";
import { chromium } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { buildWorkerFiles, type WorkerFiles } from "./workerBuild.ts";

const installed = existsSync(chromium.executablePath());
let built: WorkerFiles;
let server: ReturnType<typeof createServer>;
let origin = "";

beforeAll(async () => {
  if (!installed) return;
  built = await buildWorkerFiles();
  server = createServer((req, res) => {
    const name = (req.url ?? "").slice(1).split("?")[0] ?? "";
    const bytes = built.files.get(name);
    if (!bytes) {
      res.setHeader("Content-Type", "text/html");
      res.end("<!doctype html><title>t</title>");
      return;
    }
    res.setHeader("Content-Type", "text/javascript");
    res.end(Buffer.from(bytes));
  });
  await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
  origin = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
}, 120_000);

afterAll(() => server?.close());

describe.skipIf(!installed)("the built Worker in Chromium", () => {
  it("loads with no node builtin left and answers its first call", async () => {
    // Nothing in the files names a node: specifier the browser cannot resolve.
    for (const [name, bytes] of built.files)
      expect(new TextDecoder().decode(bytes), name).not.toMatch(/from"node:/);
    const browser = await chromium.launch();
    try {
      const page = await browser.newPage();
      await page.goto(`${origin}/`);
      const result = await page.evaluate(async (entry) => {
        const w = new Worker(`/${entry}`, { type: "module" });
        const ask = (m: object) =>
          new Promise<{ ok: boolean; value: unknown; error?: string }>(
            (ok, fail) => {
              w.onerror = (e) =>
                fail(
                  new Error(
                    `worker: ${e.message} at ${e.filename}:${e.lineno}:${e.colno}`,
                  ),
                );
              w.onmessage = (e) => ok(e.data);
              w.postMessage(m);
            },
          );
        const init = await ask({ id: 1, type: "init", base: "/api" });
        const r = await ask({ id: 2, type: "videoDoc" });
        w.terminate();
        return { init: init.ok, view: r.error };
      }, built.entry);
      expect(result.init).toBe(true);
      expect(result.view).toMatch(/no project/);
    } finally {
      await browser.close();
    }
  }, 60_000);
});
