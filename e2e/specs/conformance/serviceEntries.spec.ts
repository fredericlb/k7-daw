// serviceEntries.spec.ts — what serviceFiles does not pin of the routes that change a workspace's entries: what a
// rename, a move and a delete refuse (the active project, the root, _modules, the libraries), a case-only rename, a YAML name.
// Run with the native entries on and off: every answer is the same, and the disk is read back under the files root.
import { existsSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { onDisk, Service, wsDir } from "../../src/service.ts";
import { filesRoot, freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
const _tag = ws.slice(-6);
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  const put = (path: string, content: string) =>
    svc.put("/file", { path, content }, ws);
  for (const [path, content] of [
    ["proj.k7/project.k7x", 'project({ name: "proj" })\n'],
    ["plain.txt", "plain\n"],
    ["_modules/shared/note.txt", "shared\n"],
  ])
    expect((await put(path, content)).status).toBe(200);
  expect((await svc.open(ws, "proj.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const refused = (
  r: { status: number; body: { error?: string } },
  status: number,
  error: string | RegExp,
) => {
  expect(r.status, JSON.stringify(r.body)).toBe(status);
  if (typeof error === "string") expect(r.body).toEqual({ error });
  else expect(r.body.error).toMatch(error);
};

test("the active project's folder and the workspace root are never renamed, moved or recycled", async () => {
  for (const [verb, call] of [
    ["rename", () => svc.post("/rename", { path: "proj.k7", name: "z" }, ws)],
    [
      "move",
      () => svc.post("/move", { path: "proj.k7", toDir: "_modules" }, ws),
    ],
    ["delete", () => svc.post("/entry/delete", { path: "proj.k7" }, ws)],
  ] as const) {
    refused(
      await call(),
      400,
      `can't ${verb === "delete" ? "delete" : verb} the active project's folder — stop it first, then ${verb === "delete" ? "delete" : verb} it`,
    );
  }
  refused(
    await svc.post("/rename", { path: ".", name: "z" }, ws),
    400,
    "can't rename the workspace root",
  );
  refused(
    await svc.post("/entry/delete", { path: "." }, ws),
    400,
    "can't delete the workspace root",
  );
  if (filesRoot) expect(onDisk(ws, "proj.k7/project.k7x")).toContain("proj");
});

test("_modules is edited in place: its folder is never renamed, moved or recycled, nor anything made in it", async () => {
  const shared = (verb: string) =>
    new RegExp(`^can't ${verb} '_modules': a shared module is edited in place`);
  refused(
    await svc.post("/rename", { path: "_modules", name: "m" }, ws),
    400,
    shared("rename"),
  );
  refused(
    await svc.post("/move", { path: "_modules", toDir: "proj.k7" }, ws),
    400,
    shared("move"),
  );
  refused(
    await svc.post("/entry/delete", { path: "_modules" }, ws),
    400,
    shared("delete"),
  );
  refused(
    await svc.post("/move", { path: "plain.txt", toDir: "_modules" }, ws),
    400,
    /^can't move into '_modules': a shared module is edited in place/,
  );
  refused(
    await svc.post("/folder", { dir: "_modules", name: "x" }, ws),
    400,
    /^can't create a folder in '_modules\/x': a shared module is edited in place/,
  );
  if (filesRoot)
    expect(existsSync(join(wsDir(ws), "_modules", "shared", "note.txt"))).toBe(
      true,
    );
});

test("the libraries are listed: their folder and each library's root are never moved from a workspace", async () => {
  for (const path of ["_libraries", "_libraries/k7-core"]) {
    for (const [verb, call] of [
      ["rename", () => svc.post("/rename", { path, name: "z" }, ws)],
      ["move", () => svc.post("/move", { path, toDir: "proj.k7" }, ws)],
      ["delete", () => svc.post("/entry/delete", { path }, ws)],
    ] as const) {
      const r = (await call()) as { status: number; body: { error?: string } };
      expect([path, verb, r.status]).toEqual([path, verb, 400]);
      expect(r.body.error).toMatch(
        new RegExp(
          `^can't ${verb} '[^']*': the libraries are listed, not moved from a workspace$`,
        ),
      );
    }
  }
});

test("a rename that changes the case alone is what the server's disk says, and a YAML name is refused", async () => {
  expect(
    (await svc.put("/file", { path: "Case.txt", content: "c\n" }, ws)).status,
  ).toBe(200);
  // The probe: a disk that folds case finds Case.txt under another spelling.
  const folds = (await svc.get("/file?path=case.txt", ws)).status === 200;
  const r = await svc.post(
    "/rename",
    { path: "Case.txt", name: "case.txt" },
    ws,
  );
  if (folds) refused(r, 400, "already exists: case.txt");
  else expect([r.status, r.body]).toEqual([200, { path: "case.txt" }]);
  const yaml = await svc.post(
    "/rename",
    { path: "plain.txt", name: "b.YML" },
    ws,
  );
  refused(yaml, 400, "b.YML: a workspace file is .k7x, not YAML");
  const upload = await svc.upload(ws, ".", "up.yaml", Buffer.from("x"));
  refused(upload, 400, "up.yaml: a workspace file is .k7x, not YAML");
  if (filesRoot) {
    expect(existsSync(join(wsDir(ws), "up.yaml"))).toBe(false);
    expect(onDisk(ws, "plain.txt")).toBe("plain\n");
  }
});
