// editReplay.test.ts — the native replays' plan run through `edit.call` in the BUILT Worker inside a real Chromium.
// What a reader gets wrong: it runs only with K7_EDIT_PLAN set (`npm run test:edit-replay -w @studio/k7-wasm` writes one), like the node replays.

import { existsSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { chromium } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { buildWorkerFiles, type WorkerFiles } from "./workerBuild.ts";

const dir = process.env.K7_EDIT_PLAN;
const planned = !!dir && existsSync(join(dir, "base.json"));
const run = planned && existsSync(chromium.executablePath());
let built: WorkerFiles;
let origin = "";
let close: () => void = () => {};

beforeAll(async () => {
  if (!run) return;
  built = await buildWorkerFiles("testing/editReplayWorker.ts");
  const front = createServer((req, res) => {
    const name = (req.url ?? "").slice(1).split("?")[0] ?? "";
    const bytes = built.files.get(name);
    res.setHeader("Content-Type", bytes ? "text/javascript" : "text/html");
    res.end(bytes ? Buffer.from(bytes) : "<!doctype html>");
  });
  await new Promise<void>((ok) => front.listen(0, "127.0.0.1", ok));
  origin = `http://127.0.0.1:${(front.address() as { port: number }).port}`;
  close = () => front.close();
}, 120_000);

afterAll(() => close());

describe.skipIf(!run)("the edit replay in the built Worker", () => {
  it("answers and changes what the native crate did, number for number", async () => {
    const plan = { blobs: {}, execs: [] } as {
      blobs: Record<string, string>;
      execs: unknown[];
    };
    for (const f of ["base", "time", "lane"]) {
      const part = JSON.parse(
        readFileSync(join(dir as string, `${f}.json`), "utf8"),
      ) as typeof plan;
      Object.assign(plan.blobs, part.blobs);
      plan.execs.push(...part.execs);
    }
    const browser = await chromium.launch();
    try {
      const page = await browser.newPage();
      await page.goto(`${origin}/`);
      const out = await page.evaluate(
        async ({ entry, plan }) => {
          const w = new Worker(`/${entry}`, { type: "module" });
          return new Promise<{
            ok: boolean;
            error?: string;
            tally?: { mismatches: string[] } & Record<string, unknown>;
            native?: Record<string, number>;
          }>((ok) => {
            w.onerror = (e) => ok({ ok: false, error: `worker: ${e.message}` });
            w.onmessage = (e) => ok(e.data);
            w.postMessage({ plan });
          });
        },
        { entry: built.entry, plan },
      );
      expect(out.ok, out.error).toBe(true);
      const t = out.tally as {
        mismatches: string[];
        execs: number;
        ret: number;
        thrown: number;
        unsupported: number;
      };
      expect(t.mismatches.slice(0, 20)).toEqual([]);
      expect({
        execs: t.execs,
        ret: t.ret,
        thrown: t.thrown,
        unsupported: t.unsupported,
      }).toEqual(out.native);
    } finally {
      await browser.close();
    }
  }, 900_000);
});
