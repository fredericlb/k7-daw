// verbsWorker.measure.spec.ts — the verbs' Worker in a real Chromium on three sizes of project.
// What a reader gets wrong: it asserts only that the Worker answers; the figures are printed (and kept as annotations) for the Z2 report.
/// <reference lib="dom" />
import { expect, test } from "@playwright/test";
import { capacityFiles } from "../../../apps/k7-daw-frontend/packages/commands/src/runtime/testing/capacity.ts";
import { base as origin } from "../../src/instance.ts";
import { Service } from "../../src/service.ts";
import { freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;
const sizes = [
  { name: "synth", lanes: 0 },
  { name: "cap32", lanes: 32 },
  { name: "cap128", lanes: 128 },
];
const LINES = [
  "track rename ",
  "track rename l0 ",
  "node new ",
  "node param l0 ",
  "project ",
  "track ",
];
const REPEAT = 5;

test.describe.configure({ mode: "serial", timeout: 600_000 });
test.skip(!process.env.K7_MEASURE, "a measure: run it with K7_MEASURE=1");
test.skip(
  process.env.E2E_TARGET === "android",
  "the Worker is measured in Chromium",
);

test.beforeAll(async () => {
  test.setTimeout(600_000);
  svc = await Service.connect();
  await svc.create(ws);
  for (const s of sizes) {
    const project = `${s.name}.k7`;
    if (!s.lanes) {
      const r = await svc.post(
        "/project/create",
        { shape: "live", name: project },
        ws,
      );
      expect(r.status).toBe(200);
      continue;
    }
    const files = capacityFiles(s.lanes);
    for (let i = 0; i < files.length; i += 24)
      await Promise.all(
        files
          .slice(i, i + 24)
          .map(([rel, content]) =>
            svc.put("/file", { path: `${project}/${rel}`, content }, ws),
          ),
      );
  }
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const p50 = (xs: number[]): number =>
  [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;

for (const s of sizes) {
  test(`${s.name}: the Worker`, async ({ browser }) => {
    const project = `${s.name}.k7`;
    expect((await svc.open(ws, project)).status).toBe(200);
    const page = await browser.newPage();
    try {
      // The Worker's file is served by the page's own origin, so its fetches are same-origin.
      const res = await page.goto(`${origin}/verbs-worker/worker.js`);
      expect(res?.status(), "the dev server serves the Worker").toBe(200);
      const headers = svc.headers(ws);
      const lines = s.lanes
        ? [
            "project tempo 121",
            "project tempo 122",
            "track rename l0 lane0",
            "track rename lane0 l0",
          ]
        : ["project tempo 121", "project tempo 122", "project tempo 123"];
      const out = await page.evaluate(
        async ({ base, headers, lines, project, completions, repeat }) => {
          const w = new Worker("/verbs-worker/worker.js", { type: "module" });
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
          const time = async <T>(f: () => Promise<T>): Promise<[number, T]> => {
            const t = performance.now();
            const v = await f();
            return [performance.now() - t, v];
          };
          const head = { ...headers, "content-type": "application/json" };
          const [initMs] = await time(() =>
            ask({ type: "init", base: `${base}/api/files`, headers }),
          );
          const [catMs] = await time(() => ask({ type: "catalogue" }));
          const [openMs, opened] = await time(() =>
            ask({ type: "open", at: project }),
          );
          const worker: number[] = [];
          const runs: number[] = [];
          const notes: string[] = [];
          for (const line of lines) {
            const [wms, r] = await time(() =>
              ask({ type: "run", line, options: { want: [] } }),
            );
            const v = r.value as
              | { committed?: boolean; runs?: number; output?: string[] }
              | undefined;
            notes.push(
              `${line}: ${r.ok ? `${v?.committed ? "committed" : "no commit"}, ${v?.runs} run(s) ${(v?.output ?? []).join(" | ")}` : r.error}`,
            );
            worker.push(wms);
            runs.push(v?.runs ?? 0);
            // Does the project's token hold still after the commit? An empty set at that base says.
            const after = (r.value as { after?: string } | undefined)?.after;
            if (after && notes.length < 3) {
              for (const wait of [0, 1000]) {
                await new Promise((ok) => setTimeout(ok, wait));
                const probe = await fetch(`${base}/api/files/changes`, {
                  method: "POST",
                  headers: head,
                  body: JSON.stringify({
                    at: project,
                    base: after,
                    label: "probe",
                    ops: [],
                  }),
                });
                notes.push(
                  `after ${line} +${wait}ms: base ${after} -> ${probe.status} ${(await probe.text()).slice(0, 300)}`,
                );
              }
            }
          }
          const completeW: Record<string, number[]> = {};
          const counts: Record<string, number[]> = {};
          for (const line of completions) {
            completeW[line] = [];
            for (let i = 0; i < repeat; i++) {
              const [a, ra] = await time(() =>
                ask({ type: "complete", line, labelled: true }),
              );
              completeW[line]?.push(a);
              if (i === 0)
                counts[line] = [
                  (ra.value as unknown[] | undefined)?.length ?? -1,
                ];
            }
          }
          w.terminate();
          return {
            initMs,
            catMs,
            openMs,
            openOk: opened.ok,
            worker,
            runs,
            completeW,
            counts,
            notes,
          };
        },
        {
          base: origin,
          headers,
          lines,
          project,
          completions: LINES,
          repeat: REPEAT,
        },
      );
      expect(out.openOk).toBe(true);
      const figures = {
        project: s.name,
        init_ms: Math.round(out.initMs),
        catalogue_ms: Math.round(out.catMs),
        open_ms: Math.round(out.openMs),
        verbs_worker_ms: out.worker.map(Math.round),
        worker_runs: out.runs,
        complete_p50_ms: Object.fromEntries(
          Object.keys(out.completeW).map((line) => [
            line,
            {
              worker: Math.round(p50(out.completeW[line] ?? [])),
              items: out.counts[line]?.[0],
            },
          ]),
        ),
        notes: out.notes,
      };
      // The Worker lists its completions (the server's /complete is gone, so there is no twin to count against).
      for (const [line, c] of Object.entries(out.counts))
        expect(c[0], `completion of "${line}"`).toBeGreaterThanOrEqual(0);
      console.log(`Z2-M6 ${JSON.stringify(figures)}`);
      test.info().annotations.push({
        type: "z2-m6",
        description: JSON.stringify(figures),
      });
    } finally {
      await page.close();
    }
  });
}
