// serviceModules.spec.ts — what a project's builders read and make: the libraries, chains, assemblies, catalogue,
// hints, presets, playlists and references, a module, a clone and a node made, the importers' refusals, the command
// bar's list and completions, the machine's preferences, and the account's chain routes' refusals.
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { putTree, REPO } from "../../src/projectTree.ts";
import { onDisk, Service } from "../../src/service.ts";
import { filesRoot, freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(300_000);
  svc = await Service.connect();
  await svc.create(ws);
  for (const dir of ["jsdsp-fx.k7", "_modules", "_playlists", "_references"])
    await putTree(svc, ws, join(REPO, "e2e/fixtures", dir), dir);
  expect((await svc.open(ws, "jsdsp-fx.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

test("the libraries, chains and assemblies a project reaches, each addressed by a path the service gave", async () => {
  const libs = (await svc.get("/module-libraries", ws)).body.libraries;
  expect(libs.map((l: { ns: string }) => l.ns).slice(0, 2)).toEqual([
    "workspace",
    "k7-core",
  ]);
  expect(libs[0]).toMatchObject({
    ns: "workspace",
    path: "_modules",
    exists: true,
  });
  expect(libs[1].version).toEqual(expect.any(Object));
  const chains = (await svc.get("/library-chains", ws)).body.chains;
  expect(chains.length).toBeGreaterThan(0);
  for (const c of chains) {
    expect(Object.keys(c)).toEqual(["ref", "path", "nodes"]);
    expect(c.path).toMatch(/^_libraries\/k7-core\/chains\/.+\.k7x$/);
  }
  expect((await svc.get("/library-assemblies", ws)).body).toEqual({
    assemblies: [
      {
        ref: "@k7-core/turntable",
        path: "_libraries/k7-core/assemblies/turntable",
        file: "_libraries/k7-core/assemblies/turntable/assembly.k7x",
      },
    ],
  });
  const chain = await svc.get(
    `/file?path=${encodeURIComponent(chains[0].path)}`,
    ws,
  );
  expect(chain.status).toBe(200);
});

test("the catalogue, the hints and the ports name the workspace's modules and the library's", async () => {
  const catalogue = (await svc.get("/module-catalogue", ws)).body as {
    ref: string;
    namespace: string;
  }[];
  const refs = catalogue.map((m) => m.ref);
  expect(refs).toEqual(
    expect.arrayContaining([
      "@workspace/instruments/fourosc",
      "@k7-core/audio/chorus",
    ]),
  );
  for (const m of catalogue.slice(0, 5))
    expect(Object.keys(m)).toEqual(
      expect.arrayContaining([
        "ref",
        "name",
        "namespace",
        "category",
        "kind",
        "nodes",
        "hasUi",
      ]),
    );
  const hints = (await svc.get("/hints", ws)).body;
  expect(hints.baseModules).toEqual(
    expect.arrayContaining(["@workspace/instruments/fourosc"]),
  );
  const base = (await svc.get("/base-modules", ws)).body;
  expect(base.map((b: { ref: string }) => b.ref)).toEqual(hints.baseModules);
  expect((await svc.get("/io-ports", ws)).body).toEqual([
    {
      ref: "@meta/IN",
      name: "@meta/IN",
      namespace: "meta",
      dir: "in",
      channels: [],
      generic: true,
    },
    {
      ref: "@meta/CUE",
      name: "@meta/CUE",
      namespace: "meta",
      dir: "out",
      channels: [],
      generic: true,
    },
    {
      ref: "@meta/OUT",
      name: "@meta/OUT",
      namespace: "meta",
      dir: "out",
      channels: [],
      generic: true,
    },
  ]);
  const consoles = (await svc.get("/consoles", ws)).body.systems;
  expect(consoles).toEqual(
    expect.arrayContaining([
      { ref: "@k7-core/console/console-5", label: "console-5" },
    ]),
  );
});

test("a node's presets are listed by the module it extends, with the scopes a new one may go to", async () => {
  expect((await svc.get("/presets?node=chorus", ws)).body).toEqual({
    ref: "@k7-core/audio/chorus",
    presets: expect.any(Array),
    scopes: ["workspace", "account", "k7-core"],
  });
  const none = await svc.get("/presets?node=nope", ws);
  expect([none.status, none.body]).toEqual([
    400,
    { error: "no node named 'nope'" },
  ]);
  const empty = await svc.get("/presets", ws);
  expect([empty.status, empty.body]).toEqual([400, { error: "node required" }]);
});

test("the playlists and references are the workspace's own folders, listed", async () => {
  expect((await svc.get("/playlists", ws)).body).toEqual({
    playlists: [
      {
        path: "_playlists/e2e.k7x",
        name: "e2e",
        title: "E2E playlist",
        count: 1,
        ok: true,
      },
    ],
  });
  expect((await svc.get("/references", ws)).body).toEqual({
    references: [
      {
        path: "_references/e2e-reference.wav",
        name: "e2e-reference",
        bytes: 9644,
      },
    ],
  });
});

test("a module's panel, engines and bases are read beside its file", async () => {
  const path = "_modules/instruments/fourosc/module.k7x";
  expect((await svc.get(`/module-ui?path=${path}`, ws)).body).toEqual({
    ref: "@workspace/instruments/fourosc",
    path: "_modules/instruments/fourosc/ui.k7x",
    content:
      "// panel for @workspace/instruments/fourosc\ncols(4)\nrows(2)\nwidgets({})\n",
    exists: false,
  });
  expect((await svc.get(`/module-engines?path=${path}`, ws)).body).toEqual({
    engines: [],
  });
  expect((await svc.get(`/module-base?path=${path}`, ws)).body).toEqual({
    bases: [],
  });
  const none = await svc.get("/module-base", ws);
  expect([none.status, none.body]).toEqual([
    400,
    { error: "path is required" },
  ]);
});

test("a module, a clone of it and a node of it are made once each, never over another", async () => {
  const made = await svc.post(
    "/module",
    { root: "_modules", rel: "audio/conf-new" },
    ws,
  );
  expect([made.status, made.body]).toEqual([
    200,
    {
      path: "_modules/audio/conf-new/module.k7x",
      dir: "_modules/audio/conf-new",
    },
  ]);
  const twice = await svc.post(
    "/module",
    { root: "_modules", rel: "audio/conf-new" },
    ws,
  );
  expect([twice.status, twice.body]).toEqual([
    400,
    { error: "_modules/audio/conf-new/module.k7x already exists" },
  ]);
  const dots = await svc.post("/module", { root: "_modules", rel: "../x" }, ws);
  expect([dots.status, dots.body]).toEqual([
    400,
    { error: "'..' is not something a folder can be called" },
  ]);
  const clone = await svc.post(
    "/module/clone",
    { path: "_modules/audio/conf-new", newName: "conf-copy" },
    ws,
  );
  expect([clone.status, clone.body]).toEqual([
    200,
    { path: "_modules/audio/conf-copy/module.k7x" },
  ]);
  expect(
    (
      await svc.post(
        "/module/clone",
        { path: "_modules/audio/conf-new", newName: "conf-copy" },
        ws,
      )
    ).status,
  ).toBe(400);
  const named = await svc.post(
    "/node",
    { path: "_modules/audio/conf-new", node: "mine" },
    ws,
  );
  expect([named.status, named.body]).toEqual([200, { path: "nodes/mine.k7x" }]);
  const auto = await svc.post(
    "/node",
    { path: "_modules/audio/conf-new/module.k7x" },
    ws,
  );
  expect([auto.status, auto.body]).toEqual([
    200,
    { path: "nodes/conf-new_1.k7x" },
  ]);
  const again = await svc.post(
    "/node",
    { path: "_modules/audio/conf-new", node: "mine" },
    ws,
  );
  expect([again.status, again.body]).toEqual([
    400,
    { error: "nodes/mine.k7x already exists — refusing to overwrite" },
  ]);
  if (filesRoot) {
    expect(onDisk(ws, "_modules/audio/conf-new/module.k7x")).toBe(
      "// conf-new\nplugin({ type: null, name: null })\n",
    );
    expect(onDisk(ws, "_modules/audio/conf-copy/module.k7x")).toBe(
      onDisk(ws, "_modules/audio/conf-new/module.k7x"),
    );
    // The node lands in the OPEN project, extending the module.
    expect(onDisk(ws, "jsdsp-fx.k7/nodes/mine.k7x")).toBe(
      '// mine: one use of module "audio/conf-new".\n// It inherits everything that module says — state only what THIS use changes, e.g.\n//   params({ "Filter Freq": 69 })\nextends("@workspace/audio/conf-new")\n',
    );
  }
});

test("a module is made only under a root a library listing answered", async () => {
  const r = await svc.post(
    "/module",
    { root: "elsewhere", rel: "audio/x" },
    ws,
  );
  try {
    expect([r.status, r.body]).toEqual([
      400,
      { error: "'elsewhere' is not a module library of this workspace" },
    ]);
  } finally {
    if (r.status === 200)
      await svc.post("/entry/delete", { path: "elsewhere" }, ws);
  }
});

test("the importers and a library move refuse what they cannot take", async () => {
  // The import is the backend's `POST /import`, a workspace file by its path and a dropped one by its bytes.
  await expect(svc.importPath(ws, "none.dawproject")).rejects.toThrow(
    "no such file: none.dawproject",
  );
  await expect(svc.importPath(ws, "jsdsp-fx.k7/project.k7x")).rejects.toThrow(
    'cannot import "jsdsp-fx.k7/project.k7x": expected one of .dawproject, .als, .zip',
  );
  await expect(
    svc.importFile(ws, new Blob([]), "x.zip", "x.k7"),
  ).rejects.toThrow(/the file is empty/);
  await expect(
    svc.importFile(ws, new Blob(["x"]), "x.als", "x.k7"),
  ).rejects.toThrow(/a dropped project must be one of/);
  await expect(
    svc.importFile(ws, new Blob(["not a zip"]), "x.zip", "x.k7"),
  ).rejects.toThrow(/not a zip/);
  // A refused import leaves nothing in the workspace.
  expect((await svc.get("/file?path=x.k7%2Fproject.k7x", ws)).status).toBe(404);
  const move = await svc.post(
    "/library/move",
    { path: "_modules/audio/conf-new", toDir: "_modules/instruments" },
    ws,
  );
  expect([move.status, move.body]).toEqual([
    400,
    {
      error: "_modules/audio/conf-new is not in one of the account's libraries",
    },
  ]);
});

test("the command bar's list and completions", async () => {
  const list = (await svc.catalogue(ws)) as {
    command: string;
    help: string;
    args: unknown[];
  }[];
  expect(list.map((c) => c.command)).toEqual(
    expect.arrayContaining(["track new", "track rename"]),
  );
  for (const c of list)
    expect(Object.keys(c)).toEqual(
      expect.arrayContaining(["command", "help", "args"]),
    );
  expect(await svc.complete(ws, "track ne")).toEqual(["track new"]);
});

test("the machine's preferences, and vim written back as it was", async () => {
  const prefs = (await svc.get("/preferences", ws)).body;
  expect(Object.keys(prefs)).toEqual([
    "audioDevice",
    "channels",
    "vim",
    "stemsChain",
    "instrumentalChain",
  ]);
  const put = await svc.put("/preferences/vim", { vim: prefs.vim }, ws);
  expect([put.status, put.body]).toEqual([200, { vim: prefs.vim }]);
  expect((await svc.put("/preferences/vim", { vim: "x" }, ws)).status).toBe(
    400,
  );
});

test("a live chain compiles into a slot with the rig around it, and a cold one is refused", async () => {
  const slot = await svc.post("/account/slot", { ref: "@k7-core/live/vocals" });
  expect(slot.status).toBe(200);
  expect(Object.keys(slot.body)).toEqual([
    "id",
    "name",
    "chain",
    "savedAt",
    "protocol",
    "doc",
    "assets",
    "chainPlugins",
  ]);
  expect(slot.body).toMatchObject({
    id: "vocals",
    name: "vocals",
    chain: "@k7-core/live/vocals",
    protocol: 1,
    doc: { name: "slot vocals" },
  });
  expect(slot.body.chainPlugins.length).toBeGreaterThan(0);
  const cold = await svc.post("/account/slot", { ref: "@k7-core/stems" });
  expect([cold.status, cold.body]).toEqual([
    400,
    {
      error:
        '"@k7-core/stems" cannot play live: "@k7-core/cold/stemsplit" only works on audio already on disk',
    },
  ]);
});

test("the account's chains: listed, and every change refused that names what the libraries have not", async () => {
  const live = (await svc.get("/account/live-chains")).body.chains;
  for (const c of live) expect(Object.keys(c)).toEqual(["ref", "unavailable"]);
  const frozen = (await svc.get("/account/frozen-chains")).body.chains;
  expect(frozen.map((c: { ref: string }) => c.ref)).toEqual(
    expect.arrayContaining(["@k7-core/instrumental", "@k7-core/stems"]),
  );
  const cold = (await svc.get("/account/cold-chains")).body;
  expect(Object.keys(cold)).toEqual([
    "chains",
    "stemsChain",
    "instrumentalChain",
  ]);
  for (const [method, path, data, status, error] of [
    [
      "PUT",
      "/account/cold-chains/nope",
      { ref: "" },
      404,
      "no 'nope' chain setting",
    ],
    [
      "PUT",
      "/account/cold-chains/stems",
      { ref: "@nope/x" },
      400,
      "'@nope/x' is not a cold chain of the library",
    ],
    [
      "POST",
      "/account/cold-chains/download",
      { ref: "@nope/x" },
      404,
      "'@nope/x' is not a cold chain of the library",
    ],
    ["GET", "/account/models/nope", undefined, 404, "no such download"],
    [
      "POST",
      "/account/slot",
      { ref: "@nope/x" },
      400,
      'no chain "@nope/x" in the libraries',
    ],
    [
      "POST",
      "/account/frozen-chains/x/copy",
      { from: "@nope/x" },
      400,
      'no chain "@nope/x" in the libraries',
    ],
    [
      "DELETE",
      "/account/frozen-chains/nope",
      undefined,
      404,
      'no chain "nope" of yours',
    ],
  ] as const) {
    const r = await svc.call(method, path, { data });
    expect([path, r.status, r.body]).toEqual([path, status, { error }]);
  }
  // Nothing above changed the machine's settings.
  expect((await svc.get("/account/cold-chains")).body).toEqual(cold);
});
