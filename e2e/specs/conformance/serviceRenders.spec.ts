// serviceRenders.spec.ts — what the file service renders through the engine binary, hands on and fetches: a cold pass
// measured and baked (`clip process`), a take kept in the project's record directory, a node's model downloaded.
// The bed has the engine and ffmpeg.
import { randomBytes } from "node:crypto";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { clickWav } from "../../src/instance.ts";
import { putTree, REPO } from "../../src/projectTree.ts";
import { SERVICE_USER, Service, wsDir } from "../../src/service.ts";
import {
  accountDir,
  filesRoot,
  freshWorkspace,
  NO_FILES_ROOT,
} from "../../src/sync.ts";

const ws = freshWorkspace();
const project = "cold-normalize.k7";
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 300_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  await putTree(svc, ws, join(REPO, "e2e/fixtures", project), project);
  expect((await svc.open(ws, project)).status).toBe(200);
});

test.afterAll(async () => {
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const verb = (line: string) => svc.verb(ws, line, { want: [] });

/** The loudest bucket of a clip's whole waveform, in dBFS. */
async function peakDb(clip: string): Promise<number> {
  const r = await svc.get(`/waveform?clip=${clip}&buckets=64&whole=1`, ws);
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  return 20 * Math.log10(Math.max(...(r.body.peak as number[])));
}

test("clip process measures the clip's true peak, renders it through the chain and points the clip at the render", async () => {
  // The fixture's tone is a sine at −12 dBFS, and the chain normalises to −6.
  expect(await peakDb("tone")).toBeCloseTo(-12, 0);
  const r = await verb("clip process tone drums");
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  expect(r.body.refused).toBeFalsy();
  expect(r.body.output.join("\n")).toMatch(/clip tone now plays it/);
  const doc = (await svc.get("/engine-doc", ws)).body;
  const source = doc.tracks.find((t: { id: string }) => t.id === "drums")
    ?.clips[0]?.source as string;
  expect(source).toMatch(/tone_drums_[^\\/]*\.wav$/);
  expect(await peakDb("tone")).toBeCloseTo(-6, 0);
  // The step is the journal's: an undo points the clip back at its source.
  const back = await svc.post("/undo", { confirm: true }, ws);
  expect(back.status, JSON.stringify(back.body)).toBe(200);
  expect(await peakDb("tone")).toBeCloseTo(-12, 0);
});

test("a file saved while a verb renders is kept, the journal holding the verb's one step", async () => {
  const before = (await svc.get("/history/entries", ws)).body.length as number;
  const running = verb("clip process tone drums");
  await new Promise((ok) => setTimeout(ok, 300));
  const text = await svc.text(ws, `${project}/project.k7x`);
  const saved = await svc.put(
    "/file",
    {
      path: `${project}/project.k7x`,
      content: text.replace("tempo: 120", "tempo: 121"),
    },
    ws,
  );
  expect(saved.status, JSON.stringify(saved.body)).toBe(200);
  const done = await running;
  expect(done.status, JSON.stringify(done.body)).toBe(200);
  const labels = (await svc.get("/history/entries", ws)).body
    .slice(before)
    .map((e: { label: string }) => e.label);
  // The Worker's set rests on the project it read: a save inside the render makes it stale, and a verb that called the
  // host is told to run again rather than land half. Either the verb's one step, or nothing of it: never a lost save.
  const moved = /the project moved/.test(done.body.output.join("\n"));
  expect(
    labels.filter((l: string) => l === "clip process tone drums"),
  ).toHaveLength(moved ? 0 : 1);
  expect(await svc.text(ws, `${project}/project.k7x`)).toContain("tempo: 121");
});

test("a take is kept in the project's record directory, where the engine left it", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  // The take as the engine leaves it in the project's record directory.
  const take = `take-${randomBytes(4).toString("hex")}.wav`;
  writeFileSync(join(wsDir(ws), project, "clips", take), clickWav(1));
  const r = await verb(`project record save ${take}`);
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  expect(r.body.refused, JSON.stringify(r.body.output)).toBeFalsy();
  const [line] = r.body.output as string[];
  expect(line).toBe(`saved ${take}`);
  expect(existsSync(join(wsDir(ws), project, "clips", take))).toBe(true);
  // A name that is no file of the record directory is refused before anything is opened.
  const refused = await verb("project record save ..");
  expect(refused.body.refused).toBe(true);
  expect(refused.body.output.join("\n")).toMatch(
    /could not save the take: .*is not one of this project's takes/,
  );
});

test("a model a node names is downloaded from its model_url into the machine's cache, through a redirect", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  // Past what an answer is read as text: the body is written to the file as it arrives.
  const bytes = randomBytes(6 << 20);
  const file = `conformance-${randomBytes(4).toString("hex")}.onnx`;
  const server = createServer((req, res) => {
    if (req.url === "/moved") res.writeHead(302, { location: "/model.onnx" });
    else if (req.url === "/model.onnx")
      res.writeHead(200, {
        "content-type": "application/octet-stream",
        "content-length": bytes.length,
      });
    else res.writeHead(404);
    res.end(req.url === "/model.onnx" ? bytes : undefined);
  });
  await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
  const url = `http://127.0.0.1:${(server.address() as { port: number }).port}/moved`;
  const cached = join(accountDir(SERVICE_USER), "models", file);
  try {
    for (const [path, content] of [
      [
        `${project}/graph.k7x`,
        'modules({ drums: ["norm"], norm: ["MIX"], fetcher: ["MIX"], MIX: ["OUT"] })\n',
      ],
      [
        `${project}/nodes/fetcher.k7x`,
        `cold(true)\nplugin({ type: "offline", name: "Fetcher" })\nparams({ onnx: "${file}" })\nmodel_url("${url}")\n`,
      ],
    ])
      expect((await svc.put("/file", { path, content }, ws)).status).toBe(200);
    const r = await svc.post("/models/download", { node: "fetcher" }, ws);
    expect(r.status, JSON.stringify(r.body)).toBe(200);
    expect(r.body.job).toMatchObject({ file, url, error: null });
    const id = r.body.job.id as string;
    await expect
      .poll(async () => (await svc.get(`/account/models/${id}`)).body.done, {
        timeout: 60_000,
      })
      .toBe(true);
    expect((await svc.get(`/account/models/${id}`)).body).toEqual({
      id,
      file,
      url,
      received: bytes.length,
      total: bytes.length,
      done: true,
      error: null,
    });
    expect(readFileSync(cached).equals(bytes)).toBe(true);
    // The node's model is on the machine now: nothing to download.
    const again = await svc.post("/models/download", { node: "fetcher" }, ws);
    expect([again.status, again.body]).toEqual([200, { job: null }]);
    const gone = await svc.get("/account/models/no-such-job");
    expect([gone.status, gone.body]).toEqual([
      404,
      { error: "no such download" },
    ]);
  } finally {
    rmSync(cached, { force: true });
    await new Promise((ok) => server.close(ok));
  }
});
