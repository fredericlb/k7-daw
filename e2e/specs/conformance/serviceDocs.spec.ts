// serviceDocs.spec.ts — what a project compiles to: the web view, the picture, the engine's document, its surfaces
// and errors, the graph a verb hands back or does not, a dsp module that does not decode, an unresolved module.
// The project is e2e/fixtures' jsdsp-fx.k7 with the fixtures' `_modules`, in a workspace of the suite's.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service, wsDir } from "../../src/service.ts";
import { filesRoot, freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
// With the native compiler answering (`CONFORMANCE_SONG_NATIVE=on`) its sentences replace the TypeScript's, which this file otherwise pins (decision 763).
const native = process.env.CONFORMANCE_SONG_NATIVE === "on";
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(300_000);
  svc = await Service.connect();
  await svc.create(ws);
  for (const dir of ["jsdsp-fx.k7", "_modules"])
    await putTree(svc, ws, join(REPO, "e2e/fixtures", dir), dir);
  expect((await svc.open(ws, "jsdsp-fx.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

test("a project's documents: the web view, the picture, the engine's, its surfaces and a clean compile's errors", async () => {
  const doc = (await svc.get("/project-doc", ws)).body;
  expect(doc).toMatchObject({
    name: "jsdsp-fx",
    tempo: 120,
    timeSigNumerator: 4,
    timeSigDenominator: 4,
  });
  expect(doc.tracks.map((t: { id: string }) => t.id)).toEqual(["melody"]);
  // The picture is the Worker's answer, not a route's: the node runner is the same runtime.
  expect(await svc.picture(ws)).toEqual({
    doc: {
      name: "jsdsp-fx.k7",
      viewport: [0, 0],
      fps: 0,
      duration: 0,
      markers: [],
      videoTracks: [],
      framePlugins: [],
      sound: null,
      transitions: {},
      stretches: {},
    },
    errors: { errors: [], diagnostics: [] },
  });
  expect((await svc.get("/surfaces", ws)).body).toEqual({
    surfaces: [
      {
        id: "timeline",
        file: "surfaces/timeline.k7x",
        surface: { type: "timeline", layout: "split" },
      },
    ],
    diagnostics: [],
  });
  expect((await svc.get("/project-errors", ws)).body).toEqual({
    errors: [],
    diagnostics: [],
  });
  const engine = await svc.get("/engine-doc", ws);
  expect(engine.status).toBe(200);
  expect(engine.body.tracks.map((t: { id: string }) => t.id)).toContain(
    "melody",
  );
});

test("a verb hands back the graph it was asked for, and no graph when it was asked for none", async () => {
  const graph = (await svc.get("/graph", ws)).body;
  expect([graph.tracks, Object.keys(graph.chains)]).toEqual([
    ["melody"],
    ["melody", "MIX"],
  ]);
  const withGraph = await svc.verb(ws, "track new conf_a");
  expect(withGraph.status).toBe(200);
  expect(withGraph.body.rev).not.toBe(withGraph.body.revBefore);
  expect(
    [...(withGraph.body.graph as { tracks: string[] }).tracks].sort(),
  ).toEqual(["conf_a", "melody"]);
  // The graph a verb returns is the one /graph answers after it.
  expect(withGraph.body.graph).toEqual((await svc.get("/graph", ws)).body);
  const none = await svc.verb(ws, "track new conf_b", { want: [] });
  expect(none.body.graph).toBeUndefined();
  expect((await svc.get("/revision", ws)).body).toEqual({ rev: none.body.rev });
  const unknown = await svc.verb(ws, "nothing at all", { want: [] });
  expect(unknown.body.output).toEqual(["unknown command: nothing at all"]);
  expect(unknown.body.refused).toBe(true);
  expect(unknown.body.changed).toEqual([]);
  const graphOut = await svc.get("/graph?at=../x", ws);
  expect([graphOut.status, graphOut.body.error]).toEqual([
    400,
    "path escapes the workspace: ../x",
  ]);
});

test("a dsp module whose dsp.k7x does not decode compiles to a diagnostic naming its line, and the song still compiles", async () => {
  await svc.put(
    "/file",
    {
      path: "_modules/audio/conf-broken/module.k7x",
      content: 'plugin({ type: "dsp", name: "Broken" })\n',
    },
    ws,
  );
  await svc.put(
    "/file",
    {
      path: "_modules/audio/conf-broken/dsp.k7x",
      content: 'params({})\ngraph([\n  output(amul("dyr", 1)),\n])\n',
    },
    ws,
  );
  await svc.put(
    "/file",
    {
      path: "jsdsp-fx.k7/nodes/wah.k7x",
      content: 'extends("@workspace/audio/conf-broken")\n',
    },
    ws,
  );
  const errors = (await svc.get("/project-errors", ws)).body;
  expect(errors.errors).toEqual([]);
  const said = JSON.stringify(errors.diagnostics);
  expect(said).toContain("line 3: ");
  expect(said).toContain("names nothing");
  expect(
    errors.diagnostics.some(
      (d: { severity?: string }) => d.severity === "error",
    ),
  ).toBe(true);
  expect((await svc.get("/engine-doc", ws)).status).toBe(200);
  // Nothing is written beside the module: a decode has no cache.
  if (filesRoot)
    expect(
      existsSync(
        join(wsDir(ws), "_modules", "audio", "conf-broken", "jsdsp.json"),
      ),
    ).toBe(false);
});

test("a project that will not compile answers the compiler's error, and its graph draws the missing module", async () => {
  await svc.put(
    "/file",
    {
      path: "jsdsp-fx.k7/nodes/lead.k7x",
      content: 'extends("@workspace/instruments/none")\n',
    },
    ws,
  );
  const error =
    "unresolved reference(s):\n  - module 'lead': extends unknown module '@workspace/instruments/none'";
  for (const path of ["/engine-doc", "/project-doc"]) {
    const r = await svc.get(path, ws);
    if (native) {
      // The sentence is the native compiler's own: the status and the file it names are what stays.
      expect([path, r.status, typeof r.body.error]).toEqual([
        path,
        400,
        "string",
      ]);
      expect(r.body.error).toContain("nodes/lead.k7x");
    } else expect([path, r.status, r.body]).toEqual([path, 400, { error }]);
  }
  const listed = (await svc.get("/project-errors", ws)).body;
  if (native) {
    expect(listed.errors.length).toBeGreaterThan(0);
    expect(JSON.stringify(listed.diagnostics)).toContain("nodes/lead.k7x");
  } else expect(listed).toEqual({ errors: [error], diagnostics: [] });
  const graph = (await svc.get("/graph", ws)).body;
  expect(graph.chains.melody.use[0]).toMatchObject({
    module: "lead",
    uses: "@workspace/instruments/none",
    stub: true,
    hasNativePluginUI: false,
  });
  await svc.put(
    "/file",
    {
      path: "jsdsp-fx.k7/nodes/lead.k7x",
      content: 'extends("@workspace/instruments/fourosc")\n',
    },
    ws,
  );
  expect((await svc.get("/engine-doc", ws)).status).toBe(200);
});

test("the compile routes name the compiler only when the native one answers", async () => {
  const sources = /export const SOURCES = "([0-9a-f]{16})"/.exec(
    readFileSync(
      join(
        REPO,
        "apps/k7-daw-frontend/packages/k7-wasm/src/k7Wasm.generated.ts",
      ),
      "utf8",
    ).slice(0, 2000),
  )?.[1];
  for (const path of [
    "/project-doc",
    "/engine-doc",
    "/project-errors",
    "/graph",
    "/surfaces",
    "/io-ports",
    "/module-catalogue",
    "/base-modules",
    "/hints",
  ]) {
    const header = (await svc.get(path, ws)).headers["x-k7-compiler"];
    expect([path, header]).toEqual([path, native ? sources : undefined]);
  }
});
