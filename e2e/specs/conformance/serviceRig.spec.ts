// serviceRig.spec.ts — a studio whose preferences name channels and a cue output: the ports and the
// engine's document answer with them, and with the file gone they answer with the empty rig again.
// It runs in the service lane, one file at a time on the suite's account, and puts back the preferences file it found; it skips without a files root.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { Service, serviceDir } from "../../src/service.ts";
import { deviceBed } from "../../src/session.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
const PREFERENCES =
  'channels({ in: [ch([3]).label("guitar")], out: [ch([3, 4]).label("cue")] })\ncueOutput("cue")\n';
const FILES: Record<string, string> = {
  "p.k7/project.k7x": 'project({ name: "rig", tempo: 120 })\n',
  "p.k7/nodes/gtr.k7x": 'extends("@meta/track")\n',
  "p.k7/nodes/drums.k7x": 'extends("@meta/track")\n',
  "p.k7/nodes/amp.k7x": 'extends("@engine/compressor")\n',
  "p.k7/graph.k7x": 'modules({ gtr: ["amp"], drums: ["amp"], amp: ["CUE"] })\n',
  "_modules/io/IN/module.k7x": 'io("in")\n',
  "_modules/io/IN/gtr.k7x": 'channel("guitar")\n',
};
let svc: Service;
let before: string | null = null;
const preferences = () => join(serviceDir(), "preferences.k7x");

function readText(path: string): string | null {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return null;
  }
}

type Port = { ref: string; channels: number[]; label?: string };
type Track = { id: string; outputChannels?: number[]; mute: boolean };

async function heard() {
  const ports = (await svc.get("/io-ports", ws)).body as Port[];
  const engine = (await svc.get("/engine-doc", ws)).body as { tracks: Track[] };
  return {
    guitar: ports.find((p) => p.ref === "@workspace/io/IN:gtr"),
    amp: engine.tracks.find((t) => t.id === "amp"),
  };
}

// A device's engine names its own modules: the rig's `@engine/compressor` is a server's catalogue.
test.skip(deviceBed(), "a device sends the rig to its real engine");
test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  test.setTimeout(300_000);
  mkdirSync(serviceDir(), { recursive: true });
  before = readText(preferences());
  svc = await Service.connect();
  await svc.create(ws);
  for (const [path, content] of Object.entries(FILES))
    expect((await svc.put("/file", { path, content }, ws)).status).toBe(200);
  expect((await svc.open(ws, "p.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.skip(!filesRoot, NO_FILES_ROOT);
  test.setTimeout(180_000);
  if (before === null) rmSync(preferences(), { force: true });
  else writeFileSync(preferences(), before);
  await svc.remove(ws);
  await svc.dispose();
});

test("the empty rig names no channel and mutes the cued run", async () => {
  const { guitar, amp } = await heard();
  expect(guitar).toMatchObject({ channels: [] });
  expect(amp).toMatchObject({ mute: true });
  expect(amp?.outputChannels ?? []).toEqual([]);
});

test("the preferences' channels and cue output reach the ports and the engine's document", async () => {
  writeFileSync(preferences(), PREFERENCES);
  const { guitar, amp } = await heard();
  expect(guitar).toMatchObject({ channels: [3], label: "guitar" });
  expect(amp).toMatchObject({ outputChannels: [3, 4], mute: false });
  const doc = await svc.get("/project-doc", ws);
  expect(doc.status).toBe(200);
  expect((doc.body.tracks as { id: string }[]).map((t) => t.id)).toContain(
    "gtr",
  );
});

test("with the preferences file gone the rig is empty again", async () => {
  rmSync(preferences());
  const { guitar, amp } = await heard();
  expect(guitar).toMatchObject({ channels: [] });
  expect(amp).toMatchObject({ mute: true });
});
