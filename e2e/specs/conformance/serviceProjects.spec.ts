// serviceProjects.spec.ts — a project's life in the file service: each archetype scaffolded byte for byte (ids
// blanked) against scaffolds.json, the names a project may take, opening it and the marker beside the workspaces,
// the default a bare address opens, its plugin needs, a rename the marker follows, a delete into the recycle bin.
// CONFORMANCE_WRITE_SCAFFOLDS=1 rewrites scaffolds.json from the instance: a scaffold changed on purpose.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import {
  blankIds,
  filesUnder,
  markerFile,
  onDiskBytes,
  Service,
  sha,
  wsDir,
} from "../../src/service.ts";
import { deviceBed } from "../../src/session.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
const SHAPES = ["arrangement", "mix", "video", "live", "live-video"] as const;
const MANIFEST = join(import.meta.dirname, "scaffolds.json");
type Scaffold = { files: Record<string, string>; workspace: string[] };
const manifest: Record<string, Scaffold> = JSON.parse(
  readFileSync(MANIFEST, "utf8"),
);
const written: Record<string, Scaffold> = {};
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
  if (process.env.CONFORMANCE_WRITE_SCAFFOLDS)
    writeFileSync(MANIFEST, `${JSON.stringify(written, null, 2)}\n`);
  await svc.remove(ws);
  await svc.dispose();
});

/** A text file's sha256 with its random ids blanked; a binary one's own. */
const digest = (rel: string, bytes: Buffer): string =>
  /\.(mp4|wav|png)$/.test(rel)
    ? sha(bytes)
    : sha(blankIds(bytes.toString("utf8")));

for (const shape of SHAPES)
  test(`the ${shape} archetype is written as scaffolds.json holds it`, async () => {
    const before = filesRoot ? filesUnder(wsDir(ws)) : [];
    const made = await svc.post(
      "/project/create",
      { shape, name: `${shape}.k7` },
      ws,
    );
    expect([made.status, made.body]).toEqual([
      200,
      { project: `${shape}.k7`, missing: [] },
    ]);
    test.skip(!filesRoot, NO_FILES_ROOT);
    const dir = join(wsDir(ws), `${shape}.k7`);
    const files = Object.fromEntries(
      filesUnder(dir).map((rel) => [
        rel,
        digest(rel, readFileSync(join(dir, rel))),
      ]),
    );
    // What the scaffold adds outside its own folder: the workspace's shared modules it needs.
    const outside = filesUnder(wsDir(ws)).filter(
      (f) => !f.startsWith(`${shape}.k7/`) && !before.includes(f),
    );
    written[shape] = { files, workspace: outside };
    if (!process.env.CONFORMANCE_WRITE_SCAFFOLDS)
      expect({ files, workspace: outside }).toEqual(manifest[shape]);
  });

test("a project's text names no folder of this machine, and each id it mints is its own", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  const ids = new Set<string>();
  for (const shape of SHAPES)
    for (const rel of filesUnder(join(wsDir(ws), `${shape}.k7`))) {
      if (/\.(mp4|wav)$/.test(rel)) continue;
      const text = onDiskBytes(ws, `${shape}.k7/${rel}`).toString("utf8");
      expect(text, rel).not.toMatch(/[A-Za-z]:[\\/]|\/home\/|\/Users\//);
      for (const id of text.match(
        /[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/g,
      ) ?? [])
        ids.add(`${shape}/${id}`);
    }
  const bare = [...ids].map((i) => i.split("/")[1]);
  expect(new Set(bare).size).toBe(bare.length);
});

test("a project's name is one folder: a separator or a leading dot is not kept, and a taken name is refused", async () => {
  const create = (name: string, shape = "live") =>
    svc.post("/project/create", { shape, name }, ws);
  for (const [name, landed] of [
    ["../up.k7", "_up.k7"],
    ["a/b.k7", "a_b.k7"],
    [".dot.k7", "dot.k7"],
  ]) {
    const r = await create(name);
    expect([name, r.status, r.body]).toEqual([
      name,
      200,
      { project: landed, missing: [] },
    ]);
    if (filesRoot)
      expect(existsSync(join(wsDir(ws), landed, "project.k7x"))).toBe(true);
  }
  const taken = await create("live.k7");
  expect([taken.status, taken.body]).toEqual([
    400,
    { error: '"live.k7" already exists and is not empty' },
  ]);
  const shape = await create("x.k7", "song");
  expect([shape.status, shape.body]).toEqual([
    400,
    {
      error:
        'Invalid option: expected one of "arrangement"|"mix"|"video"|"live"|"live-video"',
    },
  ]);
  for (const landed of ["_up.k7", "a_b.k7", "dot.k7"])
    expect(
      (await svc.post("/project/delete", { path: landed }, ws)).status,
    ).toBe(200);
});

test("a project made without the .k7 suffix gets it, and the workspace lists it", async () => {
  const r = await svc.post(
    "/project/create",
    { shape: "live", name: "plain" },
    ws,
  );
  try {
    expect([r.status, r.body.project]).toEqual([200, "plain.k7"]);
    expect((await svc.get("/workspace", ws)).body.projects).toContain(
      "plain.k7",
    );
  } finally {
    if (r.status === 200)
      await svc.post("/project/delete", { path: r.body.project }, ws);
  }
});

test("opening a project names it active, remembers it beside the workspaces, and refuses what is none", async () => {
  const before = await svc.get("/workspace", ws);
  expect(before.body).toMatchObject({
    rootIsProject: false,
    activeProject: null,
  });
  expect((await svc.get("/revision", ws)).body.rev).toMatch(/^none:/);
  const open = await svc.open(ws, "mix.k7");
  expect(open.status).toBe(200);
  expect(Object.keys(open.body)).toEqual([
    "root",
    "rootIsProject",
    "activeProject",
    "projects",
    "engineLoad",
    "rev",
  ]);
  expect(open.body).toMatchObject({
    root: await svc.root(ws),
    rootIsProject: false,
    activeProject: "mix.k7",
    projects: SHAPES.map((s) => `${s}.k7`).sort((a, b) => (a < b ? -1 : 1)),
  });
  expect(open.body.rev).toMatch(/^\S+$/);
  // The two-second poll: the workspace, and the engine link's observable parts beside it.
  const poll = (await svc.get("/workspace", ws)).body;
  expect(Object.keys(poll)).toEqual([
    "root",
    "rootIsProject",
    "activeProject",
    "projects",
    "lastTake",
    "engineSync",
    "engineLoad",
    "engineScan",
    "processes",
  ]);
  // A process another file of the lane ended lingers 8 s on the account's list: only a running one would be this open's.
  const running = poll.processes.filter(
    (p: { endedAt?: number }) => p.endedAt === undefined,
  );
  // A device's account keeps the last take of every spec before this one.
  expect([deviceBed() ? null : poll.lastTake, running]).toEqual([null, []]);
  expect((await svc.get("/engine/served")).body).toEqual({
    project: { workspace: ws, path: "mix.k7" },
  });
  expect((await svc.get("/revision", ws)).body).toEqual({ rev: open.body.rev });
  // The name is the one project.k7x declares, the root the folder's.
  expect((await svc.get("/project", ws)).body).toEqual({
    name: "mix",
    root: expect.stringMatching(/[\\/]mix\.k7$/),
  });
  if (filesRoot) expect(readFileSync(markerFile(ws), "utf8")).toBe("mix.k7\n");
  for (const [path, error] of [
    [
      "none.k7",
      "no project 'none.k7' in this workspace — it may have been renamed, removed, or never have been on this machine; go back to the projects page and open it from there",
    ],
    ["../x", "path escapes the workspace: ../x"],
    [
      "live.k7/chains",
      "not a project (the folder must end in .k7): live.k7/chains",
    ],
  ]) {
    const r = await svc.post("/open", { path }, ws);
    expect([path, r.status, r.body]).toEqual([path, 400, { error }]);
  }
  // A refused open leaves the open project as it was.
  expect((await svc.get("/workspace", ws)).body.activeProject).toBe("mix.k7");
});

test("a bare address opens the project last open, and a workspace with none opens as a tree", async () => {
  const dot = await svc.open(ws, ".");
  expect([dot.status, dot.body.activeProject]).toEqual([200, "mix.k7"]);
  const empty = freshWorkspace();
  await svc.create(empty);
  try {
    const none = await svc.open(empty, ".");
    expect([none.status, none.body.activeProject, none.body.projects]).toEqual([
      200,
      null,
      [],
    ]);
  } finally {
    await svc.remove(empty);
  }
});

test("the plugins a project needs are answered without opening it", async () => {
  test.skip(deviceBed(), "a device's engine names its own plugins");
  const mix = await svc.get("/project-plugins?project=mix.k7", ws);
  expect([mix.status, mix.body]).toEqual([
    200,
    {
      plugins: [
        { format: "internal", name: "Equaliser" },
        { format: "internal", name: "studio_turntable" },
      ],
    },
  ]);
  const none = await svc.get("/project-plugins", ws);
  expect([none.status, none.body]).toEqual([
    400,
    { error: "project is required" },
  ]);
  const gone = await svc.get("/project-plugins?project=zz.k7", ws);
  expect([gone.status, gone.body]).toEqual([
    404,
    { error: "not a project: zz.k7" },
  ]);
  const compat = await svc.get("/project-compat?project=mix.k7", ws);
  expect(compat.status).toBe(200);
  expect(Object.keys(compat.body)).toEqual(["compat"]);
  expect((await svc.get("/project-compat", ws)).status).toBe(400);
});

test("a rename keeps the suffix, never clobbers, and the open project's marker follows it", async () => {
  const rename = (path: string, name: string) =>
    svc.post("/project/rename", { path, name }, ws);
  const suffix = await rename("mix.k7", "desk");
  expect([suffix.status, suffix.body]).toEqual([
    400,
    {
      error:
        "a project keeps the suffix that says which app opens it — 'desk' should end in '.k7'",
    },
  ]);
  const taken = await rename("mix.k7", "live.k7");
  expect([taken.status, taken.body]).toEqual([
    400,
    { error: "already exists: live.k7" },
  ]);
  const done = await rename("mix.k7", "desk.k7");
  expect([done.status, done.body]).toEqual([200, { path: "desk.k7" }]);
  const info = (await svc.get("/workspace", ws)).body;
  expect(info.activeProject).toBe("desk.k7");
  expect(info.projects).toContain("desk.k7");
  expect(info.projects).not.toContain("mix.k7");
  if (filesRoot) {
    expect(readFileSync(markerFile(ws), "utf8")).toBe("desk.k7\n");
    expect(existsSync(join(wsDir(ws), "mix.k7"))).toBe(false);
    expect(existsSync(join(wsDir(ws), "desk.k7", "project.k7x"))).toBe(true);
  }
});

test("a delete recycles the folder, ends the open project and empties its marker", async () => {
  const del = await svc.post("/project/delete", { path: "desk.k7" }, ws);
  expect([del.status, del.body]).toEqual([
    200,
    { recycled: "_recycle_bin/desk.k7" },
  ]);
  const info = (await svc.get("/workspace", ws)).body;
  expect([info.activeProject, info.projects.includes("desk.k7")]).toEqual([
    null,
    false,
  ]);
  expect((await svc.get("/engine/served")).body).toEqual({ project: null });
  if (filesRoot) {
    expect(readFileSync(markerFile(ws), "utf8")).toBe("");
    expect(
      existsSync(join(wsDir(ws), "_recycle_bin", "desk.k7", "project.k7x")),
    ).toBe(true);
    expect(existsSync(join(wsDir(ws), "desk.k7"))).toBe(false);
  }
  const again = await svc.post("/project/delete", { path: "desk.k7" }, ws);
  expect(again.status).toBe(400);
  const bin = await svc.post(
    "/project/delete",
    { path: "_recycle_bin/desk.k7" },
    ws,
  );
  expect(bin.status).toBe(400);
});
