// serviceWorkspaces.spec.ts — the account's workspaces as the file service answers them: Examples stocked with the
// build's demos, a workspace made with its library pins, the three listings, a project copied between two, the absence
// of any sync route, the bench, and a removal that takes the folder, its capture log and its marker.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import {
  captureDir,
  filesUnder,
  onDisk,
  Service,
  serviceDir,
  wsDir,
} from "../../src/service.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
const other = freshWorkspace();
const quick = freshWorkspace();
const DEMOS = join(import.meta.dirname, "..", "..", "..", "demos");
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.remove(other);
  await svc.remove(quick);
  await svc.dispose();
});

test("Examples is listed, in code-point order with the rest, holding this build's demos, each flagged as one", async () => {
  const names = await svc.names();
  expect(names).toContain("Examples");
  expect(names).toEqual(
    [...names].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0)),
  );
  const demos = readdirSync(DEMOS).filter((n) => n.endsWith(".k7"));
  const listed = (await svc.get("/workspaces/projects")).body.workspaces.find(
    (w: { name: string }) => w.name === "Examples",
  );
  expect(listed.isSingleProject).toBe(false);
  expect(listed.projects.map((p: { path: string }) => p.path)).toEqual(demos);
  for (const p of listed.projects) {
    expect(Object.keys(p)).toEqual([
      "path",
      "app",
      "surfaces",
      "active",
      "editedAt",
      "demo",
    ]);
    expect(p).toMatchObject({ app: "k7", demo: true });
    expect(typeof p.editedAt).toBe("number");
  }
  const row = (await svc.get("/workspaces")).body.workspaces.find(
    (w: { name: string }) => w.name === "Examples",
  );
  expect(row).toEqual({
    name: "Examples",
    isSingleProject: false,
    projectCount: demos.length,
  });
});

test("Examples takes no project of its own", async () => {
  const r = await svc.post(
    "/project/create",
    { shape: "mix", name: "conformance.k7" },
    "Examples",
  );
  expect([r.status, r.body]).toEqual([
    400,
    {
      error:
        "Examples holds the bundled demos and takes no new project — make it in a workspace of your own",
    },
  ]);
});

test("a workspace is made once, by a name that is one folder; a second make answers false", async () => {
  const again = await svc.post("/workspaces", { name: ws });
  expect([again.status, again.body]).toEqual([200, { created: false }]);
  for (const name of ["../x", ".hidden", "a/b", "a\\b"]) {
    const r = await svc.post("/workspaces", { name });
    expect([name, r.status, r.body]).toEqual([
      name,
      400,
      { error: `invalid workspace name: ${name}` },
    ]);
  }
  const empty = await svc.post("/workspaces", { name: "" });
  expect(empty.status).toBe(400);
  const none = await svc.post("/workspaces", {});
  expect(none.status).toBe(400);
  expect(await svc.names()).toContain(ws);
  const row = (await svc.get("/workspaces")).body.workspaces.find(
    (w: { name: string }) => w.name === ws,
  );
  expect(row).toEqual({ name: ws, isSingleProject: false, projectCount: 0 });
});

test("a new workspace holds its library pins alone: each library at its newest version", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  const libs = (await svc.get("/account/libraries")).body.libraries as {
    name: string;
    present: boolean;
    versions?: { version: number }[];
  }[];
  const pins = libs
    .filter((l) => l.present && l.versions?.length)
    .map(
      (l) =>
        `${l.name}(${Math.max(...(l.versions ?? []).map((v) => v.version))})\n`,
    )
    .join("");
  expect(filesUnder(wsDir(ws))).toEqual(["_library-pins.k7x"]);
  expect(onDisk(ws, "_library-pins.k7x")).toBe(pins);
});

test("a name that is no workspace: 404 on a workspace route, 400 on a bad one", async () => {
  const gone = await svc.get("/file?path=x", `${ws}-none`);
  expect([gone.status, gone.body]).toEqual([
    404,
    { error: `unknown workspace: ${ws}-none` },
  ]);
  const bad = await svc.get("/file?path=x", "../x");
  expect([bad.status, bad.body]).toEqual([
    400,
    { error: "invalid workspace name: ../x" },
  ]);
  // Naming a workspace never makes one.
  expect(await svc.names()).not.toContain(`${ws}-none`);
});

test("the home page's listing carries each project's surfaces, its activity and its edit time", async () => {
  await svc.post("/project/create", { shape: "mix", name: "desk.k7" }, ws);
  await svc.post("/project/create", { shape: "video", name: "film.k7" }, ws);
  await svc.open(ws, "film.k7");
  // The listing is held for five seconds: a project made now is listed once that has passed.
  await expect
    .poll(
      async () =>
        (await svc.get("/workspaces/projects")).body.workspaces.find(
          (w: { name: string }) => w.name === ws,
        )?.projects.length,
      { timeout: 15_000 },
    )
    .toBe(2);
  const row = (await svc.get("/workspaces/projects")).body.workspaces.find(
    (w: { name: string }) => w.name === ws,
  );
  expect(row.isSingleProject).toBe(false);
  expect(
    row.projects.map((p: Record<string, unknown>) => ({
      ...p,
      editedAt: typeof p.editedAt,
    })),
  ).toEqual([
    {
      path: "desk.k7",
      app: "k7",
      surfaces: ["turntables"],
      active: false,
      editedAt: "number",
    },
    {
      path: "film.k7",
      app: "k7",
      surfaces: ["timeline", "viewport"],
      active: true,
      editedAt: "number",
    },
  ]);
  const surfaced = (await svc.get("/workspaces/surfaces")).body.projects.filter(
    (p: { workspace: string }) => p.workspace === ws,
  );
  expect(surfaced).toEqual([
    {
      workspace: ws,
      path: "desk.k7",
      app: "k7",
      active: false,
      surfaces: [{ id: "desk", type: "turntables" }],
    },
    {
      workspace: ws,
      path: "film.k7",
      app: "k7",
      active: true,
      surfaces: [
        { id: "timeline", type: "timeline" },
        { id: "viewport", type: "viewport" },
      ],
    },
  ]);
  const served = await svc.get("/engine/served");
  expect(served.body).toEqual({ project: { workspace: ws, path: "film.k7" } });
});

test("a project is copied into another workspace with its suffix, never over one", async () => {
  await svc.create(other);
  const copy = (data: Record<string, string>) =>
    svc.post("/workspaces/copy-project", {
      from: ws,
      path: "desk.k7",
      to: other,
      name: "desk.k7",
      ...data,
    });
  expect((await copy({ to: `${other}-none` })).status).toBe(404);
  expect((await copy({ from: `${ws}-none` })).status).toBe(404);
  const suffix = await copy({ name: "desk" });
  expect([suffix.status, suffix.body]).toEqual([
    400,
    { error: 'a copy of desk.k7 must keep its suffix: "desk" does not' },
  ]);
  const made = await copy({});
  expect([made.status, made.body]).toEqual([200, { project: "desk.k7" }]);
  const twice = await copy({});
  expect([twice.status, twice.body]).toEqual([
    400,
    { error: '"desk.k7" already exists' },
  ]);
  if (filesRoot) {
    const src = join(wsDir(ws), "desk.k7");
    const files = filesUnder(src);
    expect(filesUnder(join(wsDir(other), "desk.k7"))).toEqual(files);
    for (const f of files)
      expect(onDisk(other, `desk.k7/${f}`), f).toBe(onDisk(ws, `desk.k7/${f}`));
  }
});

test("no workspace syncs with a relay: the sync routes are none of the file service's", async () => {
  for (const path of [
    "/sync/log",
    "/sync/project?path=.",
    "/sync/conflicts",
    "/workspaces/sync",
  ]) {
    expect((await svc.get(path, ws)).status, path).toBe(404);
  }
  expect(
    (await svc.post("/workspaces/sync/download", { name: other }, ws)).status,
  ).toBe(404);
});

test("the bench opens one live rig and nothing else", async () => {
  const refused = await svc.post("/workspaces/bench", { project: "x.k7" });
  expect([refused.status, refused.body]).toEqual([
    404,
    {
      error:
        "@banc opens one live rig, and nothing else. A shared MODULE or a CHAIN is edited from a project you already have open — its Libraries tab.",
    },
  ]);
  const library = await svc.post("/workspaces/bench", {
    project: "@k7-core/live.k7",
  });
  expect([library.status, library.body]).toEqual([
    409,
    {
      error: "'@k7-core' is a library, not a chain",
    },
  ]);
  const bench = await svc.post("/workspaces/bench", { project: "live.k7" });
  expect([bench.status, bench.body]).toEqual([
    200,
    { workspace: "Banc", project: "live.k7" },
  ]);
  if (filesRoot)
    expect(existsSync(join(wsDir("Banc"), "live.k7", "project.k7x"))).toBe(
      true,
    );
});

test("a bench opened on a chain holds it, keeps what is edited on a reload and is made fresh by another address", async () => {
  const chain = "@k7-core/live/vocals";
  const opened = await svc.post("/workspaces/bench", {
    project: `${chain}/live.k7`,
  });
  expect([opened.status, opened.body]).toEqual([
    200,
    { workspace: "Banc", project: "live.k7" },
  ]);
  if (!filesRoot) return;
  const rig = join(wsDir("Banc"), "live.k7");
  const source = () =>
    JSON.parse(readFileSync(join(wsDir("Banc"), "bench-source.json"), "utf8"));
  expect(readFileSync(join(rig, "graph.k7x"), "utf8")).toContain(chain);
  expect(source().ref).toBe(chain);
  // A knob turned on the bench survives the same address opened again.
  const fader = join(rig, "nodes", "fader.k7x");
  const turned = `${readFileSync(fader, "utf8")}// turned\n`;
  writeFileSync(fader, turned);
  await svc.post("/workspaces/bench", { project: `${chain}/live.k7` });
  expect(readFileSync(fader, "utf8")).toBe(turned);
  // Another address is another bench.
  await svc.post("/workspaces/bench", { project: "live.k7" });
  expect(readFileSync(fader, "utf8")).not.toContain("turned");
  expect(source().ref).toBeNull();
  const refused = await svc.post("/workspaces/bench", {
    project: "@k7-core/stems/live.k7",
  });
  expect([refused.status, refused.body]).toEqual([
    409,
    {
      error:
        'the chain "@k7-core/stems" cannot run live: "@k7-core/cold/stemsplit" only works on audio already on disk',
    },
  ]);
});

test("the bench saved is the account's chain, only from the bench's own folder, and a saved chain is removed again", async () => {
  const chain = "@k7-core/live/vocals";
  await svc.post("/workspaces/bench", { project: `${chain}/live.k7` });
  test.skip(!filesRoot, NO_FILES_ROOT);
  const rig = join(wsDir("Banc"), "live.k7");
  const saved = await svc.post("/bench/save", { dir: rig, name: "kept-bench" });
  expect([saved.status, saved.body.ref, saved.body.outside]).toEqual([
    200,
    "@account/kept-bench",
    [],
  ]);
  expect(saved.body.steps).toBeGreaterThan(0);
  const elsewhere = await svc.post("/bench/save", {
    dir: wsDir(ws),
    name: "x",
  });
  expect([elsewhere.status, elsewhere.body]).toEqual([
    400,
    {
      error:
        "only the bench saves a chain — open one on the bench from the Frozen chains page",
    },
  ]);
  const bad = await svc.post("/bench/save", { dir: rig, name: "bad name" });
  expect(bad.status).toBe(400);
  const gone = await svc.call("DELETE", "/account/frozen-chains/kept-bench");
  expect([gone.status, gone.body]).toEqual([200, { deleted: true }]);
});

test("a removal refuses a name that is none, and takes the folder, its capture log and its marker", async () => {
  const none = await svc.post("/workspaces/remove", { name: `${other}-none` });
  expect([none.status, none.body]).toEqual([
    400,
    { error: `no such workspace: ${other}-none` },
  ]);
  const bad = await svc.post("/workspaces/remove", { name: "../x" });
  expect([bad.status, bad.body]).toEqual([
    400,
    { error: "invalid workspace name: ../x" },
  ]);
  const root = await svc.root(other);
  await svc.open(other, "desk.k7");
  if (filesRoot) {
    expect(
      readFileSync(
        join(serviceDir(), "workspace", ".active-projects", other),
        "utf8",
      ),
    ).toBe("desk.k7\n");
    await expect.poll(() => existsSync(captureDir(root))).toBe(true);
  }
  const gone = await svc.post("/workspaces/remove", { name: other });
  expect([gone.status, gone.body]).toEqual([200, { ok: true }]);
  expect(await svc.names()).not.toContain(other);
  const after = await svc.get("/workspace", other);
  expect(after.status).toBe(404);
  if (filesRoot) {
    expect(existsSync(wsDir(other))).toBe(false);
    expect(existsSync(captureDir(root))).toBe(false);
  }
});

test("a workspace removed as soon as it is made stays removed, whatever the listings join", async () => {
  await svc.create(quick);
  await svc.post("/project/create", { shape: "live", name: "x.k7" }, quick);
  const gone = await svc.post("/workspaces/remove", { name: quick });
  expect([gone.status, gone.body]).toEqual([200, { ok: true }]);
  // The listings are the folders on the disk: nothing else can bring a removed workspace back.
  for (let round = 0; round < 3; round++) {
    const listed = await svc.get("/workspaces/projects");
    expect(
      listed.body.workspaces.map((w: { name: string }) => w.name),
    ).not.toContain(quick);
    expect(await svc.names()).not.toContain(quick);
    if (filesRoot) expect(existsSync(wsDir(quick))).toBe(false);
    // Time is the subject: "stays removed" is read over listings a few seconds apart.
    await new Promise((r) => setTimeout(r, 2000));
  }
});
