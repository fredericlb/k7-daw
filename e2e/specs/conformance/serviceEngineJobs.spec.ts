// serviceEngineJobs.spec.ts — what the file service asks of its engine beyond playing, as the engine hears it: a thaw that sends the
// project again, two plugin queries answered by their ids in the reverse order, a refused window, an audition's frame after the
// document, a take armed on an event and filed into it. Both settings of `engine-jobs` must agree; judged at the bed (K5).
// The snapshot route needs a plugin scan the suite has none of: `tests_routes.rs` holds it.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { clickWav } from "../../src/instance.ts";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import { E2E_USER } from "../../src/session.ts";
import type { SessionUser } from "../../src/sessionCookie.ts";
import { type Frame, StandInEngine } from "../../src/standInEngine.ts";
import { freshWorkspace } from "../../src/sync.ts";
import { until } from "../../src/until.ts";

/** An account of its own: the thawed list and the queries' waiters are its link's memory. */
const JOBS_USER: SessionUser = {
  sub: "conformance-engine-jobs",
  name: "Conformance Engine Jobs",
  email: "conformance-engine-jobs@st0.app",
  groups: [],
};
const ws = freshWorkspace();
const project = "clip-lane.k7";
const session = `k7:${(E2E_USER.email ?? "").replace(/[^a-zA-Z0-9]/g, "_")}`;
let svc: Service;
let engine: StandInEngine;
test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect(JOBS_USER);
  await svc.create(ws);
  await putTree(svc, ws, join(REPO, "e2e/fixtures", project), project);
  engine = await StandInEngine.start(JOBS_USER);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await engine?.stop(JOBS_USER);
  await svc.remove(ws);
  await svc.dispose();
});

const call = (method: string, path: string, data?: unknown) =>
  svc.call(method, path, {
    ws,
    data,
    headers: { "x-studio-engine-id": engine.id },
    again: method === "GET",
  });

/** A verb run by the runtime's node build, the stand-in bound as `call` binds it. */
const verb = (line: string) =>
  svc.verb(ws, line, { headers: { "x-studio-engine-id": engine.id } });

const frames = (type: string): Frame[] =>
  engine.of(session).filter((f) => f.type === type);

/** The questions of one kind the engine heard, on whatever session asked them. */
const asked = (type: string): Frame[] =>
  engine.heard.filter((h) => h.frame.type === type).map((h) => h.frame);

test("an open is held until the engine has the project", async () => {
  const opened = await call("POST", "/open", { path: project });
  expect(opened.status).toBe(200);
  await until(
    () => call("GET", "/workspace"),
    (r) => r.status === 200 && r.body.engineLoad === null,
    60_000,
  );
  expect(frames("project").length).toBeGreaterThan(0);
});

test("a thaw sends the open project to the engine again", async () => {
  const before = frames("project").length;
  const r = await call("POST", "/auto-freeze/thaw", {
    module: "drums",
    on: true,
  });
  expect([r.status, r.body]).toEqual([200, { ok: true }]);
  await expect
    .poll(() => frames("project").length, { timeout: 30_000 })
    .toBeGreaterThan(before);
  await call("POST", "/auto-freeze/thaw", { module: "drums", on: false });
});

test("two plugin queries at once are each answered by their own id, in any order", async () => {
  const a = call("GET", "/plugin-params?format=VST3&id=plug-a");
  const b = call("GET", "/plugin-params?format=VST3&id=plug-b");
  await expect
    .poll(() => asked("list-plugin-params").length, { timeout: 15_000 })
    .toBeGreaterThanOrEqual(2);
  const answer = (id: string, name: string): Frame => ({
    type: "plugin-param-list",
    format: "VST3",
    id,
    found: true,
    params: [{ name }],
  });
  // B's answer comes first: A must not take it.
  engine.broadcast(answer("plug-b", "from-b"));
  engine.broadcast(answer("plug-a", "from-a"));
  const [ra, rb] = await Promise.all([a, b]);
  expect([ra.status, ra.body]).toEqual([
    200,
    { found: true, params: [{ name: "from-a" }] },
  ]);
  expect([rb.status, rb.body]).toEqual([
    200,
    { found: true, params: [{ name: "from-b" }] },
  ]);
});

test("a window that did not open is a 502 naming the plugin", async () => {
  const r = call("GET", "/plugin-native-ui?format=VST3&id=plug-w");
  await expect
    .poll(() => asked("show-native-plugin-ui-of").length, { timeout: 15_000 })
    .toBeGreaterThan(0);
  engine.broadcast({
    type: "native-plugin-ui-opened",
    format: "VST3",
    id: "plug-w",
    ok: false,
  });
  const done = await r;
  expect([done.status, done.body]).toEqual([
    502,
    { error: "the engine could not open a window for plug-w — see its log" },
  ]);
});

test("an audition's frame reaches the engine after the document that was in flight", async () => {
  const before = engine.of(session).length;
  const sent = await verb("project tempo 99");
  expect(sent.status).toBe(200);
  const r = await call("POST", "/preview", {
    kind: "source",
    source: "clips/kick.wav",
  });
  expect([r.status, r.body]).toEqual([200, { ok: true, start: 0, length: 0 }]);
  await expect
    .poll(
      () =>
        engine
          .of(session)
          .slice(before)
          .some((f) => f.type === "preview"),
      { timeout: 15_000 },
    )
    .toBe(true);
  const after = engine.of(session).slice(before);
  const doc = after.findIndex(
    (f) => f.type === "project" && f.doc.tempo === 99,
  );
  const preview = after.findIndex((f) => f.type === "preview");
  expect(doc, JSON.stringify(after.map((f) => f.type))).toBeGreaterThanOrEqual(
    0,
  );
  expect(preview).toBeGreaterThan(doc);
  expect(after[preview].file).toMatch(/kick\.wav$/);
});

test("a take armed on an event is filed into it, whichever runtime recorded the arm", async () => {
  const armed = await verb("event take arm drums 1:1:1 1:1:1 3:1:1");
  // The HTTP body is `{ output, changed }`; only the actor's own op adds `refused`.
  expect(armed.status, JSON.stringify(armed.body)).toBe(200);
  expect(armed.body.output.join("\n")).toMatch(
    /takes go into the event at 1:1:1/,
  );
  const recordDir = frames("project").at(-1)?.doc.recordDir as string;
  expect(recordDir).toBeTruthy();
  mkdirSync(recordDir, { recursive: true });
  const file = join(recordDir, "take-jobs.wav");
  writeFileSync(file, clickWav(1));
  engine.say(session, {
    type: "recorded",
    track: "drums",
    file,
    start: 0,
    length: 1,
  });
  await expect
    .poll(() => svc.text(ws, `${project}/nodes/drums.k7x`), {
      timeout: 30_000,
    })
    .toMatch(/\.takes\(\[ref\(/);
});
