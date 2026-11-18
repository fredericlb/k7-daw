// serviceGolden.spec.ts — the corpus as a smoke test (packages/song/docs/rustCompilerHost.md, 900 to 910): each archetype,
// demo and e2e fixture compiled by the running service, rendered through the engine, and the sound judged. It reads no
// committed fixture. What a reader gets wrong: "sounds" is a table per project, asserted both ways, so one that starts
// to sound fails as surely as one that stops; the fixtures carry no table, only the floors of the last test.
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import {
  engineBinary,
  type Rendered,
  renderDoc,
  sounds,
} from "../../src/engineRender.ts";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import { freshWorkspace } from "../../src/sync.ts";

const native = process.env.CONFORMANCE_SONG_NATIVE === "on";
const shapes = freshWorkspace();
const demos = freshWorkspace();
const ws = freshWorkspace();
const dirs = (at: string) =>
  readdirSync(join(REPO, at)).filter((n) => n.endsWith(".k7"));
const DEMOS = dirs("demos");
const FIXTURES = dirs("e2e/fixtures");
let svc: Service;

/** What a project's render must give (902): a mix that sounds, a silent one, or the engine's own refusal. */
type Sound = "sounds" | "silent" | "nothing audible";
const TABLE: Record<string, Sound> = {
  "arrangement.k7": "sounds",
  "video.k7": "sounds",
  "mix.k7": "silent",
  "live.k7": "nothing audible",
  "live-video.k7": "nothing audible",
  "crossfade-demo.k7": "sounds",
  "rack-demo.k7": "sounds",
  "synth-demo.k7": "sounds",
  "video-demo.k7": "sounds",
  "node-demo.k7": "sounds",
};
const SHAPES = ["arrangement", "mix", "video", "live", "live-video"];

/** The two fixtures that must fail, and the file each failure names (907): the sequence file, the second one at the node under `on`. */
const BROKEN: Record<string, string> = {
  "broken-sequence.k7": "sequences/melody.k7x",
  "broken-unresolved.k7": native ? "nodes/ghost.k7x" : "ghost",
};

// The counts of 905, kept across the serial file and held by its last test.
const seen = { compiled: 0, rendered: 0, fixtureSounding: 0, tableSounding: 0 };
const printed: string[] = [];

/** Floors of staged work and of the effect, never of a field; `tableSounding` is exact, the table's own count. */
const FLOOR = {
  compiled: 41,
  rendered: 39,
  fixtureSounding: 12,
  tableSounding: 7,
};

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(300_000);
  svc = await Service.connect();
  for (const w of [shapes, demos, ws]) await svc.create(w);
  for (const shape of SHAPES) {
    const made = await svc.post(
      "/project/create",
      { shape, name: `${shape}.k7` },
      shapes,
    );
    expect([made.status, made.body]).toEqual([
      200,
      { project: `${shape}.k7`, missing: [] },
    ]);
  }
  for (const demo of DEMOS) {
    // A demo is copied out of Examples, never opened there: an open repairs it, and Examples is the account's.
    const copied = await svc.post("/workspaces/copy-project", {
      from: "Examples",
      path: demo,
      to: demos,
      name: demo,
    });
    expect([copied.status, copied.body.project]).toEqual([200, demo]);
  }
  // Apart from the archetypes: the fixtures' own `_modules` would stand in for the scaffold's.
  await putTree(svc, ws, join(REPO, "e2e/fixtures"));
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  for (const w of [shapes, demos, ws]) await svc.remove(w);
  await svc.dispose();
});

/** Open `project` and ask the three compile routes; the engine document is returned as the service answered it. */
async function compiled(inWs: string, project: string) {
  const opened = await svc.open(inWs, project);
  expect(opened.status, JSON.stringify(opened.body)).toBe(200);
  const engine = await svc.get("/engine-doc", inWs);
  const errors = await svc.get("/project-errors", inWs);
  const graph = await svc.get("/graph", inWs);
  expect(errors.status, `${project} errors`).toBe(200);
  return { engine, errors, graph };
}

const line = (project: string, r: Rendered) =>
  `${project.padEnd(24)} exit ${r.code} ${r.seconds.toFixed(1).padStart(5)} s peak ${r.peak === null ? "none" : r.peak.toFixed(3)}`;

test("the engine binary is found, and a missing one fails with a sentence, never a skip (908)", () => {
  expect(() => engineBinary()).not.toThrow();
  expect(() => engineBinary("")).toThrow(/engine binary not found at ''/);
  expect(() => engineBinary(join(REPO, "no", "such", "k7-runtime"))).toThrow(
    /engine binary not found/,
  );
});

test("the render helper hears sound and silence: synth-demo sounds, and with every track's clips and plugins emptied it does not (906)", async () => {
  const { engine } = await compiled(demos, "synth-demo.k7");
  expect(engine.status).toBe(200);
  const heard = await renderDoc(engine.body);
  expect(heard.code, heard.said).toBe(0);
  expect(sounds(heard), line("synth-demo", heard)).toBe(true);
  // Clips alone leave the pattern generators sounding (peak 0.16): a track's plugins are its sources too.
  const emptied = {
    ...engine.body,
    tracks: engine.body.tracks.map((t: object) => ({
      ...t,
      clips: [],
      plugins: [],
    })),
  };
  const silenced = await renderDoc(emptied);
  expect(sounds(silenced), line("synth-demo, emptied", silenced)).toBe(false);
});

for (const [group, ws_, names] of [
  ["archetype", () => shapes, SHAPES.map((s) => `${s}.k7`)],
  ["demo", () => demos, DEMOS],
] as const)
  for (const project of names)
    test(`the ${group} ${project} compiles and ${TABLE[project] ?? "has a row in the table"} (900, 902)`, async () => {
      const want = TABLE[project];
      expect(want, `${project} needs a row in TABLE`).toBeDefined();
      const { engine, graph } = await compiled(ws_(), project);
      expect(
        [engine.status, graph.status],
        JSON.stringify(engine.body).slice(0, 300),
      ).toEqual([200, 200]);
      seen.compiled++;
      const r = await renderDoc(engine.body);
      printed.push(line(project, r));
      if (want === "nothing audible") {
        expect(r.code, r.said).toBe(1);
        expect(r.said).toMatch(/nothing audible/);
        expect(r.mix).toBe(false);
        return;
      }
      expect([r.code, r.mix], r.said).toEqual([0, true]);
      seen.rendered++;
      expect(sounds(r), `${line(project, r)}, the table says ${want}`).toBe(
        want === "sounds",
      );
      if (want === "sounds") seen.tableSounding++;
    });

for (const fixture of FIXTURES)
  test(`the fixture ${fixture} ${fixture in BROKEN ? "fails naming its file" : "compiles and renders"} (904, 907)`, async () => {
    const { engine, errors } = await compiled(ws, fixture);
    if (fixture in BROKEN) {
      expect(engine.status).toBe(400);
      expect(
        engine.body.error,
        `${fixture}: ${JSON.stringify(engine.body)}`,
      ).toContain(BROKEN[fixture]);
      const listed = [
        ...(errors.body.errors ?? []),
        ...(errors.body.diagnostics ?? []),
      ];
      expect(listed.length, JSON.stringify(errors.body)).toBeGreaterThan(0);
      return;
    }
    expect(
      engine.status,
      `${fixture}: ${JSON.stringify(engine.body).slice(0, 300)}`,
    ).toBe(200);
    seen.compiled++;
    const r = await renderDoc(engine.body);
    printed.push(line(fixture, r));
    expect([r.code, r.mix], `${fixture}: ${r.said}`).toEqual([0, true]);
    seen.rendered++;
    if (sounds(r)) seen.fixtureSounding++;
  });

test("the counts floor: what was compiled, rendered and heard (905)", () => {
  console.log(`${printed.join("\n")}\ncounts ${JSON.stringify(seen)}`);
  expect(seen.compiled).toBeGreaterThanOrEqual(FLOOR.compiled);
  expect(seen.rendered).toBeGreaterThanOrEqual(FLOOR.rendered);
  expect(seen.fixtureSounding).toBeGreaterThanOrEqual(FLOOR.fixtureSounding);
  expect(seen.tableSounding).toBe(FLOOR.tableSounding);
});
