// serviceHost.spec.ts — the host members' routes under /verb-host, as the browser's Worker calls them (3f): the render family, the film's
// probe and sound, a take, a clip prepared in the background, a film transcribed; paths outside refused, the limits.
// Over 500 lines on purpose: one serial account holds every part, and a second file would race it.
// The bed has the engine; it is judged on the native front.

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { expect, test } from "@playwright/test";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../../packages/commands/scripts/bundleRuntime.mjs";
import { base } from "../../src/instance.ts";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import type { SessionUser } from "../../src/session.ts";
import { StandInEngine } from "../../src/standInEngine.ts";
import { freshWorkspace } from "../../src/sync.ts";
import { until } from "../../src/until.ts";

const HOST_USER: SessionUser = {
  sub: "conformance-host",
  name: "Conformance Host",
  email: "conformance-host@st0.app",
  groups: [],
};
const ws = freshWorkspace();
const project = "cold-normalize.k7";
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 300_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect(HOST_USER);
  await svc.create(ws);
  await putTree(svc, ws, join(REPO, "e2e/fixtures", project), project);
  expect((await svc.open(ws, project)).status).toBe(200);
});

let engine: StandInEngine | undefined;

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await engine?.stop(HOST_USER);
  await svc.remove(ws);
  await svc.dispose();
});

const rev = async () => (await svc.get("/revision", ws)).body.rev as string;
const host = (path: string, body: unknown) =>
  svc.post(`/verb-host/${path}`, body, ws);

/** The engine's document with every path of the project spelled as the browser sees it, under `/w`. */
async function browserDoc(): Promise<Record<string, unknown>> {
  const doc = (await svc.get("/engine-doc", ws)).body;
  const own = new RegExp(
    `^(?:[A-Za-z]:)?/.*/${project.replace(".", "\\.")}(/.*)?$`,
  );
  const walk = (v: unknown): unknown => {
    if (typeof v === "string") {
      const m = own.exec(v);
      return m ? `/w/${project}${m[1] ?? ""}` : v;
    }
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === "object")
      return Object.fromEntries(
        Object.entries(v).map(([k, x]) => [k, walk(x)]),
      );
    return v;
  };
  return walk(doc) as Record<string, unknown>;
}

/** Polls a job to its end, as the Worker does each second. */
async function ended(job: string, ms = 120_000) {
  return until(
    async () => (await svc.get(`/verb-host/job?id=${job}`, ws)).body,
    (b) => b.state !== "running",
    ms,
  );
}

test("a doc is rendered into the job's folder, then adopted by one change set, and undo takes it back", async () => {
  const started = await host("render", {
    at: project,
    doc: await browserDoc(),
  });
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  const job = started.body.job as string;
  expect(job).toMatch(/^[0-9a-f]{32}$/);
  const end = await ended(job);
  expect(end.state, JSON.stringify(end)).toBe("done");
  const mix = end.value.files.find((f: { out: string }) => f.out === "mix.wav");
  expect(mix.len).toBeGreaterThan(44);
  expect(mix.seconds).toBeGreaterThan(0);
  expect(JSON.stringify(end)).not.toContain("job-out");
  const before = (await svc.get("/history/entries", ws)).body.length as number;
  // `GET /file` answers a binary NAME before any stat (200, empty), so the tree is what says a file is there.
  const there = async () =>
    JSON.stringify((await svc.get("/tree", ws)).body).includes("rendered.wav");
  expect(await there()).toBe(false);
  const landed = await svc.post(
    "/changes",
    {
      at: project,
      base: await rev(),
      label: "host render",
      ops: [
        { kind: "adopt", job, out: "mix.wav", file: `${project}/rendered.wav` },
      ],
    },
    ws,
  );
  expect(landed.status, JSON.stringify(landed.body)).toBe(200);
  expect(landed.body.changed.map((c: { path: string }) => c.path)).toContain(
    "rendered.wav",
  );
  expect((await svc.get("/history/entries", ws)).body).toHaveLength(before + 1);
  expect(await there()).toBe(true);
  const undone = await svc.post("/undo", { confirm: true }, ws);
  expect(undone.status, JSON.stringify(undone.body)).toBe(200);
  expect(undone.body.applied, JSON.stringify(undone.body)).toBe(true);
  expect(undone.body.files).toEqual([`${project}/rendered.wav`]);
  expect(await there()).toBe(false);
});

test("a doc naming a path outside the folders the person reads is a 403 and starts no job", async () => {
  const doc = (await browserDoc()) as {
    tracks: { clips: { source: string }[] }[];
  };
  const clips = doc.tracks.flatMap((t) => t.clips);
  expect(clips.length).toBeGreaterThan(0);
  for (const bad of ["/etc/passwd", "/w/../../../etc/passwd", "tone.wav"]) {
    const keep = clips[0].source;
    clips[0].source = bad;
    const r = await host("render", { at: project, doc });
    clips[0].source = keep;
    expect(r.status, `${bad}: ${JSON.stringify(r.body)}`).toBe(403);
    expect(JSON.stringify(r.body)).not.toMatch(/job/);
  }
  const base = await host("render", { at: project, doc, base: "/etc" });
  expect(base.status).toBe(403);
  const peak = await host("peak", { file: "/etc/passwd" });
  expect(peak.status).toBe(403);
});

test("a staged output is adopted only from a job that ended with its value", async () => {
  const adopt = async (job: string) =>
    svc.post(
      "/changes",
      {
        at: project,
        base: await rev(),
        label: "forged",
        ops: [{ kind: "adopt", job, out: "mix.wav", file: `${project}/x.wav` }],
      },
      ws,
    );
  expect((await adopt("0123456789abcdef0123456789abcdef")).status).toBe(400);
  expect((await adopt("../../etc")).status).toBe(400);
});

test("a render of another project is a 409 that says which is open", async () => {
  const r = await host("render", { at: "other.k7", doc: await browserDoc() });
  expect([r.status, r.body.open]).toEqual([409, project]);
});

test("a separation with no machine bound is a 503 no-machine, and a bad output is a 400", async () => {
  const body = {
    at: project,
    source: `/w/${project}/clips/tone.wav`,
    model: "/s/models/none.onnx",
    outputs: ["drums"],
  };
  const r = await host("separate", body);
  expect([r.status, r.body.code]).toEqual([503, "no-machine"]);
  const bad = await host("separate", { ...body, outputs: ["../x"] });
  expect(bad.status).toBe(400);
  const stems = await host("playlist-stems", {
    songs: ["a"],
    steps: [{ step: "s", model: "/s/models/none.onnx", outputs: ["drums"] }],
  });
  expect([stems.status, stems.body.code]).toEqual([503, "no-machine"]);
});

test("a body past a route's limit is a 413 before it is read: 64 KiB for most, 8 MiB for a doc", async () => {
  const small = await host("peak", { file: "x".repeat(70 * 1024) });
  expect(small.status).toBe(413);
  // The front answers 413 and closes before the body is sent: the client sees the answer or the reset, by timing.
  const large = await host("render", {
    at: project,
    doc: { pad: "x".repeat(9 << 20) },
  }).catch((e: Error) =>
    /ECONNRESET|EPIPE/.test(e.message) ? null : Promise.reject(e),
  );
  if (large) expect(large.status).toBe(413);
  // A body within its limit is read: a doc of the wrong shape is a 400, not a 413.
  const ok = await host("render", { at: project, doc: { pad: "x" } });
  expect(ok.status).toBe(400);
});

// The Worker's scene for one 160x90 clip, as `sceneFrom` makes it; a full-size film never ends in the Mac's software GL.
const sceneOf = (src: string) => ({
  viewport: [160, 90],
  fps: 10,
  layers: [
    {
      id: "main",
      hidden: false,
      x: 0,
      y: 0,
      w: 160,
      h: 90,
      clips: [{ id: "sample", src, start: 0, length: 1, sourceFrom: 0 }],
    },
  ],
  durationSec: 1,
});

test("a scene the Worker made is rendered into the named folder by the server's browser, and a bad shape is a 400", async () => {
  const sample = join(REPO, "e2e/fixtures/video-lane.k7/clips/sample.mp4");
  expect(
    (await svc.upload(ws, project, "sample.mp4", readFileSync(sample))).status,
  ).toBe(200);
  const src = `media/${encodeURIComponent(`/w/${project}/sample.mp4`)}`;
  const films = mkdtempSync(join(tmpdir(), "k7-films-"));
  const bad = await host("picture", { at: project, into: films, scene: [] });
  expect(bad.status).toBe(400);
  const noFolder = await host("picture", { at: project, scene: sceneOf(src) });
  expect(noFolder.status).toBe(400);
  const started = await host("picture", {
    at: project,
    into: films,
    scene: sceneOf(src),
    format: "webm",
    audio: false,
    settings: { size: { height: 90 }, from: 0, to: 1 },
  });
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  const end = await ended(started.body.job, 240_000);
  expect(end.state, JSON.stringify(end)).toBe("done");
  expect(end.value.path).toBe(join(films, "cold-normalize.webm"));
  expect(readFileSync(end.value.path).subarray(0, 4).toString("hex")).toBe(
    "1a45dfa3",
  );
});

/** A film made by the runner's ffmpeg, or null when it has none. */
function film(sound: boolean): Buffer | null {
  const dir = mkdtempSync(join(tmpdir(), "host-film-"));
  const out = join(dir, "film.mp4");
  const audio = sound ? ["-f", "lavfi", "-i", "sine=duration=1"] : [];
  try {
    execFileSync(
      "ffmpeg",
      [
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=1:size=128x72:rate=10",
        ...audio,
        "-pix_fmt",
        "yuv420p",
        "-shortest",
        out,
      ],
      { stdio: "ignore" },
    );
    return readFileSync(out);
  } catch {
    return null;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("a film is probed, its sound extracted into a job and adopted by one change set; a silent film has none", async () => {
  const withSound = film(true);
  const silent = film(false);
  test.skip(!withSound || !silent, "the runner has no ffmpeg");
  expect(
    (await svc.upload(ws, project, "film.mp4", withSound as Buffer)).status,
  ).toBe(200);
  expect(
    (await svc.upload(ws, project, "mute.mp4", silent as Buffer)).status,
  ).toBe(200);
  const probed = await host("probe", { file: `/w/${project}/film.mp4` });
  expect(probed.status, JSON.stringify(probed.body)).toBe(200);
  expect(probed.body.facts).toMatchObject({
    codec: "h264",
    width: 128,
    height: 72,
    pixFmt: "yuv420p",
  });
  expect(probed.body.facts.rotation).toBeUndefined();
  expect(probed.body.facts.seconds).toBeGreaterThan(0.5);
  const started = await host("extract", { video: `/w/${project}/film.mp4` });
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  const end = await ended(started.body.job);
  expect(end.state, JSON.stringify(end)).toBe("done");
  expect(end.value.facts.channels).toBeGreaterThan(0);
  const [sound] = end.value.files;
  expect(sound.out).toBe("sound.wav");
  expect(sound.seconds).toBeGreaterThan(0.5);
  const landed = await svc.post(
    "/changes",
    {
      at: project,
      base: await rev(),
      label: "host extract",
      ops: [
        {
          kind: "adopt",
          job: started.body.job,
          out: "sound.wav",
          file: `${project}/film.wav`,
        },
      ],
    },
    ws,
  );
  expect(landed.status, JSON.stringify(landed.body)).toBe(200);
  const mute = await host("extract", { video: `/w/${project}/mute.mp4` });
  const none = await ended(mute.body.job);
  expect([none.state, none.value.facts, none.value.files]).toEqual([
    "done",
    null,
    [],
  ]);
  const gone = await host("probe", { file: `/w/${project}/nothing.mp4` });
  expect(gone.status).toBe(404);
  const outside = await host("probe", { file: "/etc/passwd" });
  expect(outside.status).toBe(403);
  const junk = await svc.upload(
    ws,
    project,
    "junk.mp4",
    Buffer.from("not a film"),
  );
  expect(junk.status).toBe(200);
  const bad = await host("probe", { file: `/w/${project}/junk.mp4` });
  expect(bad.status).toBe(500);
  expect(JSON.stringify(bad.body)).not.toContain(tmpdir());
});

// Part 4 (M9): the account's members. Onnx first, while no machine is bound to the account.
test("onnx answers null with no machine bound, and a hostile set is a 400", async () => {
  const none = await host("onnx", {});
  expect([none.status, none.body]).toEqual([200, { providers: null }]);
  const many = await host("onnx", {
    set: Array.from({ length: 1000 }, (_, i) => `p${i}`),
  });
  expect(many.status).toBe(400);
});

/** A call naming the stand-in, as a page that selected it names its engine: the hub binds it to the account. */
const withEngine = (method: string, path: string, data?: unknown) =>
  svc.call(method, path, {
    ws,
    data,
    headers: { "x-studio-engine-id": (engine as StandInEngine).id },
    again: method === "GET",
  });

test("plugins, devices and the engine's ops reach the bed's engine", async () => {
  engine = await StandInEngine.start(HOST_USER);
  engine.scan = {
    frames: [
      { type: "plugin-scan", scanning: false, done: 0, total: 0, version: 1 },
    ],
    plugins: [{ name: "Synth", format: "VST3", manufacturer: "Acme" }],
  };
  engine.devices = [{ name: "Built-in", type: "CoreAudio" }];
  expect((await withEngine("POST", "/open", { path: project })).status).toBe(
    200,
  );
  const plugins = await withEngine("GET", "/verb-host/plugins?filter=VST3");
  expect(plugins.body.plugins?.[0]?.name, JSON.stringify(plugins.body)).toBe(
    "Synth",
  );
  const scanned = await withEngine("POST", "/verb-host/plugins/rescan", {
    force: true,
  });
  expect(scanned.body.plugins).toHaveLength(1);
  const devices = await withEngine("GET", "/verb-host/devices");
  expect(devices.body.devices?.[0]?.name).toBe("Built-in");
  const stereo = await withEngine("POST", "/verb-host/devices/stereo", {
    channel: 1,
    stereo: true,
  });
  expect(stereo.body).toEqual({ heard: true });
  const armed = await withEngine("POST", "/verb-host/engine", {
    op: "arm",
    track: "t1",
  });
  expect([armed.body.armed, armed.body.lines]).toEqual([true, []]);
  const play = await withEngine("POST", "/verb-host/engine", {
    op: "transport",
    action: "play",
  });
  expect(play.body).toEqual({ sent: true });
  const types = engine.heard.map((h) => h.frame.type);
  expect(types).toEqual(
    expect.arrayContaining([
      "list-plugins",
      "rescan-plugins",
      "list-audio-devices",
      "set-input-stereo",
      "arm",
      "transport",
    ]),
  );
  // A scan naming no plugin is one that cannot say: a second stand-in's open leaves the account's catalogue so.
  await engine.stop(HOST_USER);
  engine = await StandInEngine.start(HOST_USER);
  engine.scan = {
    frames: [
      { type: "plugin-scan", scanning: false, done: 0, total: 0, version: 1 },
    ],
    plugins: [],
  };
  await withEngine("POST", "/open", { path: project });
});

test("a bad engine body is a 400", async () => {
  const bad = await host("engine", { op: "reboot" });
  expect(bad.status).toBe(400);
  const stereo = await host("devices/stereo", { channel: 0, stereo: true });
  expect(stereo.status).toBe(400);
});

test("a take is filed only from a record directory the person reads", async () => {
  const none = await host("take/save", { at: project });
  expect(none.status).toBe(400);
  const out = await host("take/save", { at: project, recordDir: "/etc" });
  expect(out.status).toBe(403);
});

/** The clip file a project holds, as `GET /file` reads it. */
const clipText = async (name: string): Promise<string> =>
  (
    await svc.get(
      `/file?${new URLSearchParams({ path: `${project}/clips/${name}.k7x` })}`,
      ws,
    )
  ).body.content as string;

const treeHas = async (name: string): Promise<boolean> =>
  JSON.stringify((await svc.get("/tree", ws)).body).includes(name);

test("a clip is prepared in the background: the optimized copy lands in its file, then the proxy from it, and a second ask while it runs says so", async () => {
  const bytes = film(true);
  test.skip(!bytes, "the runner has no ffmpeg");
  expect(
    (await svc.upload(ws, project, "prep.mov", bytes as Buffer)).status,
  ).toBe(200);
  expect(
    (
      await svc.upload(
        ws,
        `${project}/clips`,
        "prep.k7x",
        Buffer.from('source("prep.mov")\n'),
      )
    ).status,
  ).toBe(200);
  const ask = {
    at: project,
    clip: "prep",
    source: "prep.mov",
    optimized: "transcode",
    proxy: true,
  };
  const [first, second] = await Promise.all([
    host("prepare", ask),
    host("prepare", ask),
  ]);
  expect(
    [first.status, second.status],
    JSON.stringify([first.body, second.body]),
  ).toEqual([200, 200]);
  expect([first.body.started, second.body.started].sort()).toEqual([
    false,
    true,
  ]);
  await until(
    async () => await clipText("prep"),
    (t) => t.includes("optimized") && t.includes("proxy"),
    120_000,
  );
  const text = await clipText("prep");
  expect(text).toContain("prep.optimized.mp4");
  expect(text).toContain("prep.proxy.mp4");
  expect(await treeHas("prep.optimized.mp4")).toBe(true);
  expect(await treeHas("prep.proxy.mp4")).toBe(true);
  expect(await treeHas(".part")).toBe(false);
  // Once it ended, the source is free to be prepared again.
  const again = await host("prepare", { ...ask, optimized: null });
  expect(again.body.started).toBe(true);
  // What it may not touch.
  for (const [source, status] of [
    ["/etc/passwd", 403],
    ["../../../etc/passwd", 403],
    ["nothing.mov", 404],
  ] as const) {
    const r = await host("prepare", { ...ask, source });
    expect(r.status, source).toBe(status);
  }
});

/** Whether a whisper CLI is on the runner's PATH. */
function hasWhisper(): boolean {
  for (const bin of ["whisper", "whisper-cli"]) {
    try {
      execFileSync(bin, ["--help"], { stdio: "ignore" });
      return true;
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "ENOENT") return true;
    }
  }
  return false;
}

test("a film is transcribed by whoever the machine has: its srt or null, and without whisper a job that says how to install one", async () => {
  const bytes = film(true);
  test.skip(!bytes, "the runner has no ffmpeg");
  await svc.upload(ws, project, "words.mp4", bytes as Buffer);
  const none = await host("transcribe", { video: `/w/${project}/nothing.mp4` });
  expect(none.status).toBe(404);
  const outside = await host("transcribe", { video: "/etc/passwd" });
  expect(outside.status).toBe(403);
  const started = await host("transcribe", {
    video: `/w/${project}/words.mp4`,
  });
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  const end = await ended(started.body.job);
  if (hasWhisper() && end.state === "done") {
    expect(["string", "object"]).toContain(typeof end.value.srt);
  } else if (hasWhisper()) {
    // A CLI with no model on the machine is a job that says where to put one.
    expect(end.error).toContain("needs a model");
  } else {
    expect(end.state).toBe("failed");
    expect(end.error).toContain("no whisper on this machine");
  }
  expect(JSON.stringify(end)).not.toContain("job-out");
});

// The end-to-end proof of the /changes token: a freeze commits in one run, and the second takes it back.
test("node freeze runs through the built node runner: the render is adopted by one change set, and a second freeze takes it back", async () => {
  const dist = mkdtempSync(join(tmpdir(), "host-runner-"));
  try {
    await bundleRuntime({ target: "node", outdir: dist });
    const { createNodeRunner } = await import(
      pathToFileURL(join(dist, "node.mjs")).href
    );
    const rt = createNodeRunner({
      base: `${base}/api/files`,
      headers: () => svc.headers(ws),
    });
    const there = async () =>
      JSON.stringify((await svc.get("/tree", ws)).body).includes("norm.wav");
    expect(await there()).toBe(false);
    await rt.open(project);
    const frozen = await rt.run("node freeze norm");
    expect(
      frozen.output.join("\n"),
      JSON.stringify({ ...frozen, output: [...frozen.output] }),
    ).toContain("norm: frozen to renders/norm.wav");
    expect(frozen.committed).toBe(true);
    expect(await there()).toBe(true);
    const thawed = await rt.run("node freeze norm");
    expect(
      thawed.output.join("\n"),
      JSON.stringify({ ...thawed, output: [...thawed.output] }),
    ).toContain("norm: unfrozen");
    expect(await there()).toBe(false);
    rt.close();
  } finally {
    rmSync(dist, { recursive: true, force: true });
  }
});
