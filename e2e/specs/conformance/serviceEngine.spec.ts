// serviceEngine.spec.ts — the file service's engine link as an engine hears it: an open held until the engine holds
// the project after a silent dry run, a write and a verb sent again with no dry run, a verb's transport, a take turned
// into a clip, a node frozen through the engine binary, and the project let go on delete. The engine is the suite's
// own stand-in behind the reference tunnel agent (`src/standInEngine.ts`), claimed by an account of its own.
import { existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import type { SessionUser } from "@studio/oidc";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import { E2E_USER } from "../../src/session.ts";
import { type Frame, StandInEngine } from "../../src/standInEngine.ts";
import {
  accountDir,
  filesRoot,
  freshWorkspace,
  NO_FILES_ROOT,
} from "../../src/sync.ts";
import { until } from "../../src/until.ts";

/** An account of its own: a take and a served project are its session's memory, which other specs read empty. */
const ENGINE_USER: SessionUser = {
  sub: "conformance-engine",
  name: "Conformance Engine",
  email: "conformance-engine@st0.app",
  groups: [],
};
const ws = freshWorkspace();
const wsDir = (name: string): string =>
  join(accountDir(ENGINE_USER), "workspace", name);
const project = "clip-lane.k7";
/** The engine session of the account's projects: `k7:` and its email with every other character a `_`. */
const session = `k7:${(E2E_USER.email ?? "").replace(/[^a-zA-Z0-9]/g, "_")}`;
let svc: Service;
let engine: StandInEngine;
test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect(ENGINE_USER);
  await svc.create(ws);
  await putTree(svc, ws, join(REPO, "e2e/fixtures", project), project);
  engine = await StandInEngine.start(ENGINE_USER);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await engine?.stop(ENGINE_USER);
  await svc.remove(ws);
  // The account is this spec's alone: its projects' journals go with the workspace.
  if (filesRoot)
    rmSync(join(accountDir(ENGINE_USER), ".history"), {
      recursive: true,
      force: true,
    });
  await svc.dispose();
});

/** A call naming the stand-in, as a page that selected it names its engine: the hub binds it to the account. */
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

/** The session's frames of `type`, oldest first. */
const frames = (type: string): Frame[] =>
  engine.of(session).filter((f) => f.type === type);

/** Polls `/workspace` until `done` holds of its answer, which it returns. */
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

test("an open holds Play until the engine holds the project, then a silent dry run lets go", async () => {
  const opened = await call("POST", "/open", { path: project });
  expect(opened.status).toBe(200);
  expect(opened.body.activeProject).toBe(project);
  // The load this open began is in its answer, so the page holds Play from now.
  expect(["scanning", "copying", "applying", "warming"]).toContain(
    opened.body.engineLoad,
  );
  await poll((b) => b.engineLoad === null);
  const docs = frames("project").map((f) => f.doc);
  expect(docs.length).toBeGreaterThanOrEqual(3);
  const level = docs[0].masterVolume;
  expect(level).not.toBe(-100);
  expect(docs[0].name).toBe("clip-lane");
  // The project reaches the engine's machine first: every clip it names, at a path under the agent's cache.
  expect(docs[0].tracks[0].clips[0].source).toMatch(/kick\.wav$/);
  // The dry run: asked whether the engine holds it, the same document silenced, rolled half a second, stopped, rewound.
  const kinds = engine
    .of(session)
    .map((f) =>
      f.type === "project"
        ? `project ${f.doc.masterVolume === -100 ? "silent" : "level"}`
        : f.type === "transport"
          ? `transport ${f.action}`
          : f.type === "record-enable"
            ? `record ${f.enabled}`
            : f.type,
    )
    .filter((k) => /^(project|transport|record|list-sessions)/.test(k));
  // Each step found after the one before it: an earlier project's flush may come first in the session.
  const steps = [
    "project level",
    "list-sessions",
    "project silent",
    "record false",
    "transport play",
    "transport stop",
    "transport rewind",
  ];
  let from = 0;
  for (const k of steps) {
    const i = kinds.indexOf(k, from);
    expect(i, `${k} in ${kinds.join(", ")}`).toBeGreaterThanOrEqual(from);
    from = i + 1;
  }
  // …and the project at its own level once more, last.
  expect(kinds.lastIndexOf("project level"), kinds.join(", ")).toBeGreaterThan(
    kinds.lastIndexOf("transport rewind"),
  );
  const served = await svc.get("/engine/served");
  expect(served.body).toEqual({ project: { workspace: ws, path: project } });
});

test("a write inside the open project is sent again, with no dry run", async () => {
  const before = engine.of(session).length;
  const text = await svc.text(ws, `${project}/project.k7x`);
  const wrote = await call("PUT", "/file", {
    path: `${project}/project.k7x`,
    content: text.replace("tempo: 120", "tempo: 97"),
  });
  expect(wrote.status).toBe(200);
  await expect
    .poll(() => frames("project").at(-1)?.doc.tempo, { timeout: 30_000 })
    .toBe(97);
  // Time is the subject: a dry run that is NOT sent shows only as a window with no transport in it.
  await new Promise((ok) => setTimeout(ok, 1500));
  const after = engine.of(session).slice(before);
  expect(after.filter((f) => f.type === "transport")).toEqual([]);
  expect((await poll(() => true)).engineLoad).toBeNull();
});

test("a verb's documents and its transport reach the engine in the verb's order", async () => {
  const before = engine.of(session).length;
  const tempo = await verb("project tempo 101");
  expect(tempo.status).toBe(200);
  const play = await verb("transport play");
  expect(play.status).toBe(200);
  await expect
    .poll(
      () =>
        engine
          .of(session)
          .slice(before)
          .some((f) => f.type === "transport" && f.action === "play"),
      {
        timeout: 15_000,
      },
    )
    .toBe(true);
  const after = engine.of(session).slice(before);
  const doc = after.findIndex(
    (f) => f.type === "project" && f.doc.tempo === 101,
  );
  const roll = after.findIndex(
    (f) => f.type === "transport" && f.action === "play",
  );
  expect(doc, JSON.stringify(after.map((f) => f.type))).toBeGreaterThanOrEqual(
    0,
  );
  expect(roll).toBeGreaterThan(doc);
  await verb("transport stop");
});

test("a node frozen is rendered by the engine binary, the transport stopped first", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  const before = engine.of(session).length;
  const r = await verb("node freeze drums");
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  expect(r.body.output.join("\n")).toMatch(
    /drums: frozen to renders\/drums\.wav/,
  );
  expect(existsSync(join(wsDir(ws), project, "renders", "drums.wav"))).toBe(
    true,
  );
  // The lane sent again frozen, its authored clips kept beside for the browser to draw.
  const frozenAt = () =>
    engine
      .of(session)
      .slice(before)
      .findIndex(
        (f) =>
          f.type === "project" &&
          f.doc.tracks.some((t: Frame) => t.id === "drums" && t.frozenMaterial),
      );
  await expect.poll(frozenAt, { timeout: 30_000 }).toBeGreaterThanOrEqual(0);
  const after = engine.of(session).slice(before);
  const stop = after.findIndex(
    (f) => f.type === "transport" && f.action === "stop",
  );
  expect(stop, JSON.stringify(after.map((f) => f.type))).toBeGreaterThanOrEqual(
    0,
  );
  expect(frozenAt()).toBeGreaterThan(stop);
});

test("deleting the open project stops and lets go of the engine's session", async () => {
  const before = engine.of(session).length;
  const r = await call("POST", "/project/delete", { path: project });
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  const after = engine
    .of(session)
    .slice(before)
    .map((f) => (f.type === "transport" ? `transport ${f.action}` : f.type));
  expect(after).toContain("transport stop");
  expect(after).toContain("flush");
  expect(
    engine.heard
      .slice(-20)
      .some(
        (h) => h.frame.type === "drop-session" && h.frame.session === session,
      ),
  ).toBe(true);
  expect((await svc.get("/engine/served")).body).toEqual({ project: null });
});
