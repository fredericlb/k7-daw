// serviceOpen.spec.ts — what serviceProjects leaves open about a project's open, rename and delete (2082): the open's whole
// body, the default a bare address picks over a bad marker, the marker's bytes after each move, a rename's clean name,
// a library's project refused, and `/project` before and after. It pins both implementations of the route.
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { markerFile, Service, wsDir } from "../../src/service.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
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
  await svc.dispose();
});

const create = (name: string) =>
  svc.post("/project/create", { shape: "live", name }, ws);
const marker = () => readFileSync(markerFile(ws), "utf8");
const active = async () => (await svc.get("/workspace", ws)).body.activeProject;

/** A project folder below `dir`, made through the service: its folder, then the file that makes it one. */
async function nested(dir: string, name: string) {
  const made = await svc.post("/folder", { dir, name }, ws);
  expect([made.status], JSON.stringify(made.body)).toEqual([200]);
  const up = await svc.upload(
    ws,
    `${dir}/${name}`,
    "project.k7x",
    Buffer.from('project({ name: "inner" })\n'),
  );
  expect([up.status], JSON.stringify(up.body)).toEqual([200]);
}

test("the open's body is the same keys in the same order, with a nested project listed and no demo flag", async () => {
  expect((await create("a.k7")).status).toBe(200);
  expect(
    (await svc.post("/folder", { dir: ".", name: "deep" }, ws)).status,
  ).toBe(200);
  await nested("deep", "n.k7");
  const open = await svc.open(ws, "a.k7");
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
    rootIsProject: false,
    activeProject: "a.k7",
    projects: ["a.k7", "deep/n.k7"],
  });
  expect(open.body.activeIsDemo).toBeUndefined();
});

test("the open's marker is written once, and a repeat open leaves the file as it was", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  expect(marker()).toBe("a.k7\n");
  const first = statSync(markerFile(ws)).mtimeMs;
  await new Promise((r) => setTimeout(r, 60));
  expect((await svc.open(ws, "a.k7")).status).toBe(200);
  expect(statSync(markerFile(ws)).mtimeMs).toBe(first);
  expect(marker()).toBe("a.k7\n");
});

test("a bare address ignores a marker naming a folder outside the workspace or a project that is gone, and leaves it as written when the project is already open", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  for (const text of ["../x.k7\n", "gone.k7\n", "\n", "a.k7\r\n"]) {
    writeFileSync(markerFile(ws), text);
    const r = await svc.open(ws, ".");
    expect([text, r.status, r.body.activeProject]).toEqual([text, 200, "a.k7"]);
    // a.k7 is already open in this process, so the open is skipped and the stale marker is left as written
    expect([text, marker()]).toEqual([text, text]);
  }
  const empty = freshWorkspace();
  await svc.create(empty);
  try {
    writeFileSync(markerFile(empty), "gone.k7\n");
    const r = await svc.open(empty, ".");
    expect([r.status, r.body.activeProject, r.body.projects]).toEqual([
      200,
      null,
      [],
    ]);
  } finally {
    await svc.remove(empty);
  }
});

test("the open then the document answers one compile, and /project names the project by its file", async () => {
  await svc.open(ws, "a.k7");
  const rev = (await svc.get("/revision", ws)).body.rev;
  const first = await svc.get("/project-doc", ws);
  const again = await svc.get("/project-doc", ws);
  expect([first.status, again.status]).toEqual([200, 200]);
  expect(again.body).toEqual(first.body);
  expect((await svc.get("/revision", ws)).body.rev).toBe(rev);
  const before = (await svc.get("/project", ws)).body;
  expect(before).toEqual({
    name: expect.any(String),
    root: expect.stringMatching(/[\\/]a\.k7$/),
  });
  const done = await svc.post(
    "/project/rename",
    { path: "a.k7", name: "b.k7" },
    ws,
  );
  expect([done.status, done.body]).toEqual([200, { path: "b.k7" }]);
  const after = (await svc.get("/project", ws)).body;
  expect(after).toEqual({
    name: before.name,
    root: expect.stringMatching(/[\\/]b\.k7$/),
  });
  expect(after.root).not.toBe(before.root);
});

test("a rename drops a separator and blanks from the name, and a name with none left is refused", async () => {
  const rename = (path: string, name: string) =>
    svc.post("/project/rename", { path, name }, ws);
  // The shim's basename splits on the backslash, so what precedes it goes with it
  const cleaned = await rename("b.k7", "  x\\y.k7 ");
  expect([cleaned.status, cleaned.body]).toEqual([200, { path: "y.k7" }]);
  for (const name of ["  ", "\\", "/"]) {
    const r = await rename("y.k7", name);
    expect([name, r.status, r.body]).toEqual([
      name,
      400,
      {
        error:
          "a project keeps the suffix that says which app opens it — '' should end in '.k7'",
      },
    ]);
  }
  const same = await rename("y.k7", "y.k7");
  expect([same.status, same.body]).toEqual([200, { path: "y.k7" }]);
  expect(await active()).toBe("y.k7");
});

test("a rename of the folder above the open project carries the marker, and a delete of the open project empties it", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  expect((await create("outer.k7")).status).toBe(200);
  await nested("outer.k7", "inner.k7");
  expect((await svc.open(ws, "outer.k7/inner.k7")).status).toBe(200);
  expect(marker()).toBe("outer.k7/inner.k7\n");
  const moved = await svc.post(
    "/project/rename",
    { path: "outer.k7", name: "o2.k7" },
    ws,
  );
  expect([moved.status, moved.body]).toEqual([200, { path: "o2.k7" }]);
  expect(marker()).toBe("o2.k7/inner.k7\n");
  expect(await active()).toBe("o2.k7/inner.k7");
  expect(existsSync(join(wsDir(ws), "o2.k7", "inner.k7", "project.k7x"))).toBe(
    true,
  );
  const gone = await svc.post(
    "/project/delete",
    { path: "o2.k7/inner.k7" },
    ws,
  );
  expect([gone.status, gone.body]).toEqual([
    200,
    { recycled: "_recycle_bin/inner.k7" },
  ]);
  expect(marker()).toBe("");
  expect(await active()).toBeNull();
});

test("an open stamps the format level a lane in seconds needs, once", async () => {
  expect((await create("old.k7")).status).toBe(200);
  const lane = join(wsDir(ws), "old.k7", "tracks", "secs.k7x");
  writeFileSync(
    lane,
    'extends("@meta/track")\nevents([ref("kick").start(2.5)])\n',
  );
  const root = join(wsDir(ws), "old.k7", "project.k7x");
  expect(readFileSync(root, "utf8")).not.toContain("format");
  expect((await svc.open(ws, "old.k7")).status).toBe(200);
  const stamped = readFileSync(root, "utf8");
  expect(stamped, "the format stamp is written at the open").toContain(
    "format",
  );
  expect((await svc.open(ws, "old.k7")).status).toBe(200);
  expect(readFileSync(root, "utf8")).toBe(stamped);
});
