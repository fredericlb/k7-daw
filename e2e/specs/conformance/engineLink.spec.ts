// engineLink.spec.ts — the account's engine link as the page reads it, whichever implementation holds it: the poll's keys and
// their load through an open, the served project before and after an unload, a document a browser built, a failed copy and its retry, and a load held by the plugin scan.
// Run against the suite's stand-in engine; it never reads a header that names the link.
import { readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import type { SessionUser } from "@studio/oidc";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import { acct } from "../../src/session.ts";
import { type Frame, StandInEngine } from "../../src/standInEngine.ts";
import { accountDir, filesRoot, freshWorkspace } from "../../src/sync.ts";
import { until } from "../../src/until.ts";

/** An account of its own: a served project is its session's memory, which other specs read empty. */
const LINK_USER: SessionUser = {
  sub: "conformance-link",
  name: "Conformance Link",
  email: "conformance-link@st0.app",
  groups: [],
};
const ws = freshWorkspace();
const project = "clip-lane.k7";
const session = `k7:${(acct(LINK_USER).email ?? "").replace(/[^a-zA-Z0-9]/g, "_")}`;
let svc: Service;
let engine: StandInEngine;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect(LINK_USER);
  await svc.create(ws);
  await putTree(svc, ws, join(REPO, "e2e/fixtures", project), project);
  engine = await StandInEngine.start(LINK_USER);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await engine?.stop(LINK_USER);
  await svc.remove(ws);
  if (filesRoot)
    rmSync(join(accountDir(LINK_USER), ".history"), {
      recursive: true,
      force: true,
    });
  await svc.dispose();
});

const call = (method: string, path: string, data?: unknown) =>
  svc.call(method, path, {
    ws,
    data,
    headers: { "x-studio-engine-id": engine.id },
    again: method === "GET",
  });

const frames = (type: string): Frame[] =>
  engine.of(session).filter((f) => f.type === type);

async function poll(done: (b: Frame) => boolean, ms = 60_000): Promise<Frame> {
  const r = await until(
    () => call("GET", "/workspace"),
    (r) => r.status === 200 && done(r.body),
    ms,
  );
  expect(r.status).toBe(200);
  if (!done(r.body)) throw new Error(`/workspace: ${JSON.stringify(r.body)}`);
  return r.body;
}

test("the poll carries the engine's keys, and its load ends when the project is playable", async () => {
  const opened = await call("POST", "/open", { path: project });
  expect(opened.status).toBe(200);
  expect(["scanning", "copying", "applying", "warming"]).toContain(
    opened.body.engineLoad,
  );
  const body = await poll((b) => b.engineLoad === null);
  // The keys of the poll, in the order the page reads them: the workspace's, then the engine's.
  const keys = Object.keys(body);
  const tail = [
    "lastTake",
    "engineSync",
    "engineLoad",
    "engineScan",
    "processes",
  ];
  expect(keys.slice(-tail.length)).toEqual(tail);
  expect(body.engineSync).toBeNull();
  expect(body.engineScan).toBeNull();
  expect(Array.isArray(body.processes)).toBe(true);
});

test("the served project is the open one, and nothing once it is unloaded", async () => {
  expect((await svc.get("/engine/served")).body).toEqual({
    project: { workspace: ws, path: project },
  });
  const out = await call("POST", "/engine/unload");
  expect(out.status).toBe(200);
  expect(out.body).toEqual({ ok: true });
  expect((await svc.get("/engine/served")).body).toEqual({ project: null });
  await expect
    .poll(() => engine.of(session).some((f) => f.type === "drop-session"), {
      timeout: 15_000,
    })
    .toBe(true);
});

test("a document a browser built is what the engine plays until a reload", async () => {
  await call("POST", "/open", { path: project });
  await poll((b) => b.engineLoad === null);
  const mine = { ...frames("project").at(-1)?.doc, tempo: 88 };
  const sent = await call("POST", "/engine/project", { doc: mine });
  expect(sent.status).toBe(200);
  expect(sent.body).toEqual({ ok: true });
  await expect
    .poll(() => frames("project").at(-1)?.doc.tempo, { timeout: 30_000 })
    .toBe(88);
  // A body without a document is the page's mistake, answered as the schema words it.
  const bad = await call("POST", "/engine/project", {});
  expect(bad.status).toBe(400);
  // The copy's retry forgets the browser's document: the project's own comes back.
  const retried = await call("POST", "/engine/sync/retry");
  expect(retried.status).toBe(200);
  await expect
    .poll(() => frames("project").at(-1)?.doc.tempo, { timeout: 30_000 })
    .not.toBe(88);
});

test("a failed copy is shown by the poll, sends nothing, and the retry copies again", async () => {
  test.skip(process.platform === "win32", "the cache is swapped by rename");
  const before = frames("project").length;
  engine.breakCache();
  try {
    // A changed clip is the one file the machine has to be sent again.
    const kick = readFileSync(
      join(REPO, "e2e/fixtures", project, "clips/kick.wav"),
    );
    // An upload refuses to overwrite (409): the old clip goes first.
    const gone = await svc.post(
      "/entry/delete",
      { path: `${project}/clips/kick.wav` },
      ws,
    );
    expect(gone.status).toBe(200);
    const up = await svc.upload(
      ws,
      `${project}/clips`,
      "kick.wav",
      Buffer.concat([kick, Buffer.alloc(2)]),
    );
    expect(up.status).toBe(200);
    expect((await call("POST", "/engine/sync/retry")).status).toBe(200);
    const body = await poll((b) => (b.engineSync?.failed ?? 0) > 0);
    expect(body.engineSync.failures.length).toBeGreaterThan(0);
    expect(frames("project").length).toBe(before);
  } finally {
    engine.mendCache();
  }
  expect((await call("POST", "/engine/sync/retry")).status).toBe(200);
  await poll((b) => b.engineSync === null && b.engineLoad === null);
  // The frame crosses a socket after the poll's last word on the copy.
  await expect
    .poll(() => frames("project").length, { timeout: 30_000 })
    .toBeGreaterThan(before);
});

test("a load of a project naming an external plugin waits for the scan, then the engine plays it", async () => {
  const other = "scan-gate.k7";
  const files: [string, string][] = [
    [
      "project.k7x",
      'project({ name: "scan-gate", tempo: 120, sampleRate: 48000, timeSignature: "4/4", video: false, layers: [] })\nmarkers({})\n',
    ],
    ["graph.k7x", 'modules({ lead: ["MIX"], MIX: ["OUT"] })\n'],
    ["nodes/lead.k7x", 'plugin({ type: "vst3", name: "Serum" })\n'],
  ];
  for (const [path, content] of files)
    expect(
      (await svc.put("/file", { path: `${other}/${path}`, content }, ws))
        .status,
    ).toBe(200);
  const before = frames("project").length;
  engine.scan = {
    frames: [
      {
        type: "plugin-scan",
        scanning: true,
        done: 1,
        total: 2,
        current: "x",
        version: 1,
      },
    ],
    plugins: [{ name: "Surge" }],
  };
  try {
    // Only a load holds for the scan: a switch on a bound engine goes at once, so the engine starts empty.
    expect((await call("POST", "/engine/unload")).status).toBe(200);
    expect((await call("POST", "/open", { path: other })).status).toBe(200);
    // The engine's first word on the scan comes after the load is held, so the poll waits for it too.
    const held = await poll(
      (b) => b.engineLoad === "scanning" && b.engineScan?.done === 1,
    );
    expect(held.engineScan).toMatchObject({ done: 1, total: 2, current: "x" });
    expect((await svc.get("/engine/served")).body).toEqual({
      project: { workspace: ws, path: other },
    });
    expect(frames("project").length).toBe(before);
    engine.broadcast({ type: "plugins", plugins: [{ name: "Surge" }] });
    engine.broadcast({
      type: "plugin-scan",
      scanning: false,
      done: 2,
      total: 2,
      version: 2,
    });
    const done = await poll((b) => b.engineLoad === null);
    expect(done.engineScan).toBeNull();
    await expect
      .poll(() => frames("project").length, { timeout: 30_000 })
      .toBeGreaterThan(before);
  } finally {
    engine.scan = null;
  }
  // Back to the first project: it is the served one again, and plays.
  await call("POST", "/open", { path: project });
  await poll((b) => b.engineLoad === null);
  expect((await svc.get("/engine/served")).body).toEqual({
    project: { workspace: ws, path: project },
  });
});
