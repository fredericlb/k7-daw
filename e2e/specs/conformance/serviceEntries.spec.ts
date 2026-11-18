// serviceEntries.spec.ts — what serviceFiles does not pin of the routes that change a workspace's entries: what a
// rename, a move and a delete refuse (the active project, the root, _modules, the libraries), a library module
// renamed with its refs repointed, a library entry recycled into the library's own bin, a case-only rename, a YAML name.
// Run with the native entries on and off: every answer is the same, and the disk is read back under the files root.
import { existsSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { adminGroup } from "../../src/instance.ts";
import { onDisk, SERVICE_USER, Service, wsDir } from "../../src/service.ts";
import { deviceBed } from "../../src/session.ts";
import { filesRoot, freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
// The admin works in a workspace of its own: one made by an admin pins each library's draft, the only version written.
const wsAdmin = `${ws}-a`;
const tag = ws.slice(-6);
let svc: Service;
let admin: Service | null = null;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  if (adminGroup)
    admin = await Service.connect({ ...SERVICE_USER, groups: [adminGroup] });
  await svc.create(ws);
  await admin?.create(wsAdmin);
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
  if (admin) await svc.remove(wsAdmin);
  await svc.dispose();
  await admin?.dispose();
  // The library's bin keeps what the admin recycled: the shared draft is left as the run found it.
  const draft = draftOf("k7-core");
  if (draft) {
    rmSync(join(draft, "_recycle_bin", "hall"), {
      recursive: true,
      force: true,
    });
    rmSync(join(draft, "_recycle_bin", `ent${tag}b`), {
      recursive: true,
      force: true,
    });
    rmSync(join(draft, "_recycle_bin", `ent${tag}m`), {
      recursive: true,
      force: true,
    });
    rmSync(join(draft, "modules", "audio", `ent${tag}b`), {
      recursive: true,
      force: true,
    });
  }
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

test("the libraries are listed by the admins: their folder and each library's root are never moved from a workspace", async () => {
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
          `^can't ${verb} '[^']*': the libraries are listed by the admins, not moved from a workspace$`,
        ),
      );
    }
  }
});

test("an account that is not the library's admin changes nothing in it: rename, move in, delete, upload", async () => {
  test.skip(
    deviceBed(),
    "a device's one account is no admin and has no stranger: tests_changes.rs holds the refusal",
  );
  const chains = (await svc.get("/library-chains", ws)).body.chains as {
    path: string;
  }[];
  expect(chains.length).toBeGreaterThan(0);
  const entry = chains[0].path;
  const admins = (verb: string) =>
    new RegExp(
      `^can't ${verb} '[^']*': only the origin's admins edit a library`,
    );
  refused(
    await svc.post("/rename", { path: entry, name: "z.k7x" }, ws),
    400,
    admins("rename"),
  );
  refused(
    await svc.post("/entry/delete", { path: entry }, ws),
    400,
    admins("delete"),
  );
  refused(
    await svc.post(
      "/move",
      { path: "plain.txt", toDir: "_libraries/k7-core" },
      ws,
    ),
    400,
    admins("move into"),
  );
  refused(
    await svc.post("/folder", { dir: "_libraries/k7-core", name: "x" }, ws),
    400,
    admins("create a folder in"),
  );
  refused(
    await svc.upload(ws, "_libraries/k7-core", "x.bin", Buffer.from("x")),
    400,
    admins("add a file to"),
  );
  expect(
    (await svc.get(`/file?path=${encodeURIComponent(entry)}`, ws)).status,
  ).toBe(200);
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

/** The library's draft folder on disk, where an admin's edits land. */
function draftOf(name: string): string | null {
  if (!filesRoot) return null;
  const home = join(filesRoot, "libraries", name);
  return existsSync(join(home, "draft")) ? join(home, "draft") : home;
}

test("an admin renames a library module and its folder, and every ref in the songs follows", async () => {
  test.skip(!admin, "env.yaml names no AUTH_ADMIN_GROUPS");
  const adm = admin as Service;
  const cat = `ent${tag}`;
  const lib = "_libraries/k7-core/modules/audio";
  for (const [path, content] of [
    [`${lib}/${cat}/verb/module.k7x`, 'extends("@engine/reverb")\n'],
    [
      `${lib}/${cat}/verb/engines/plate/module.k7x`,
      'extends("@engine/reverb")\n',
    ],
    [
      "proj.k7/nodes/e.k7x",
      `extends("@k7-core/audio/${cat}/verb")\n// @k7-core/audio/${cat}x/verb\n`,
    ],
  ]) {
    const w = await adm.put("/file", { path, content }, wsAdmin);
    expect([path, w.status, w.body]).toEqual([path, 200, { ok: true }]);
  }
  const engine = await adm.post(
    "/rename",
    { path: `${lib}/${cat}/verb/engines/plate`, name: "room" },
    wsAdmin,
  );
  refused(engine, 400, /^an engine is picked by its name/);

  const module = await adm.post(
    "/rename",
    { path: `${lib}/${cat}/verb`, name: "hall" },
    wsAdmin,
  );
  expect([module.status, module.body]).toEqual([
    200,
    { path: `${lib}/${cat}/hall`, repointed: ["proj.k7/nodes/e.k7x"] },
  ]);
  expect(await adm.text(wsAdmin, "proj.k7/nodes/e.k7x")).toBe(
    `extends("@k7-core/audio/${cat}/hall")\n// @k7-core/audio/${cat}x/verb\n`,
  );

  const folder = await adm.post(
    "/rename",
    { path: `${lib}/${cat}`, name: `${cat}b` },
    wsAdmin,
  );
  expect([folder.status, folder.body]).toEqual([
    200,
    { path: `${lib}/${cat}b`, repointed: ["proj.k7/nodes/e.k7x"] },
  ]);
  expect(await adm.text(wsAdmin, "proj.k7/nodes/e.k7x")).toBe(
    `extends("@k7-core/audio/${cat}b/hall")\n// @k7-core/audio/${cat}x/verb\n`,
  );
  const draft = draftOf("k7-core");
  if (draft)
    expect(
      existsSync(
        join(draft, "modules", "audio", `${cat}b`, "hall", "module.k7x"),
      ),
    ).toBe(true);
});

test("an admin moves a library module into a category of its library, its refs follow, and another library is refused", async () => {
  test.skip(!admin, "env.yaml names no AUTH_ADMIN_GROUPS");
  const adm = admin as Service;
  const cat = `ent${tag}m`;
  const cat2 = `ent${tag}f`;
  const lib = "_libraries/k7-core/modules/audio";
  try {
    for (const [path, content] of [
      [`${lib}/${cat}/verb/module.k7x`, 'extends("@engine/reverb")\n'],
      [`${lib}/${cat2}/old/module.k7x`, 'extends("@engine/reverb")\n'],
      ["proj.k7/nodes/m.k7x", `extends("@k7-core/audio/${cat}/verb")\n`],
    ]) {
      const w = await adm.put("/file", { path, content }, wsAdmin);
      expect([path, w.status, w.body]).toEqual([path, 200, { ok: true }]);
    }
    const moved = await adm.post(
      "/library/move",
      { path: `${lib}/${cat}/verb`, toDir: `${lib}/${cat2}` },
      wsAdmin,
    );
    expect([moved.status, moved.body]).toEqual([
      200,
      {
        path: `${lib}/${cat2}/verb`,
        from: `@k7-core/audio/${cat}/verb`,
        to: `@k7-core/audio/${cat2}/verb`,
        repointed: ["proj.k7/nodes/m.k7x"],
      },
    ]);
    expect(await adm.text(wsAdmin, "proj.k7/nodes/m.k7x")).toBe(
      `extends("@k7-core/audio/${cat2}/verb")\n`,
    );
    const other = await adm.post(
      "/library/move",
      {
        path: `${lib}/${cat2}/verb`,
        toDir: "_libraries/account/modules/audio",
      },
      wsAdmin,
    );
    refused(
      other,
      400,
      "a module stays in its library — this one is k7-core's",
    );
    const draft = draftOf("k7-core");
    if (draft)
      expect(
        existsSync(join(draft, "modules", "audio", cat2, "verb", "module.k7x")),
      ).toBe(true);
  } finally {
    // The folders go even when a check above failed.
    await adm.post("/entry/delete", { path: `${lib}/${cat}` }, wsAdmin);
    await adm.post("/entry/delete", { path: `${lib}/${cat2}` }, wsAdmin);
  }
});

test("an admin's delete lands in the library's own bin, and an upload into the library lands whole", async () => {
  test.skip(!admin, "env.yaml names no AUTH_ADMIN_GROUPS");
  const adm = admin as Service;
  const cat = `ent${tag}b`;
  const lib = "_libraries/k7-core/modules/audio";
  const gone = await adm.post(
    "/entry/delete",
    { path: `${lib}/${cat}/hall` },
    wsAdmin,
  );
  expect([gone.status, gone.body]).toEqual([
    200,
    { recycled: "_libraries/k7-core/_recycle_bin/hall" },
  ]);
  const up = await adm.upload(
    wsAdmin,
    `${lib}/${cat}`,
    "ir.bin",
    Buffer.from("ir"),
  );
  expect([up.status, up.body]).toEqual([200, { path: `${lib}/${cat}/ir.bin` }]);
  const draft = draftOf("k7-core");
  if (draft) {
    expect(existsSync(join(draft, "_recycle_bin", "hall", "module.k7x"))).toBe(
      true,
    );
    expect(existsSync(join(wsDir(wsAdmin), "_recycle_bin"))).toBe(false);
    expect(
      readdirSync(join(draft, "modules", "audio", cat)).filter(
        (f) => f !== "ir.bin",
      ),
    ).toEqual([]);
  }
  // The suite's own edits leave the shared library as it was.
  await adm.post("/entry/delete", { path: `${lib}/${cat}` }, wsAdmin);
});
