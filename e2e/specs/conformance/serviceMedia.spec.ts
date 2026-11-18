// serviceMedia.spec.ts — what the file service answers about sound: a clip's waveform and its `{file}.cache` sidecar
// (format, fingerprint, a sensitivity kept), the prewarm's state, an audition's window, the project's resources, a
// mixdown rendered and streamed, the plugin and engine routes' refusals, and the Logs panel as this service reads it.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { clickWav, fingerprint } from "../../src/instance.ts";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service, wsDir } from "../../src/service.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
const beat = clickWav(4);
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  await putTree(
    svc,
    ws,
    join(REPO, "e2e/fixtures/clip-lane.k7"),
    "clip-lane.k7",
  );
  // A clip of its own: an 8 kHz click every half second, something a waveform has numbers for.
  const up = await svc.call(
    "POST",
    "/upload?dir=clip-lane.k7/clips&name=beat.wav",
    {
      ws,
      data: beat,
      headers: { "content-type": "application/octet-stream" },
    },
  );
  expect(up.status).toBe(200);
  expect(
    (
      await svc.put(
        "/file",
        {
          path: "clip-lane.k7/clips/beat.k7x",
          content: 'source("clips/beat.wav")\n',
        },
        ws,
      )
    ).status,
  ).toBe(200);
});

test.afterAll(async () => {
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const NUM = "\\d+\\.\\d{4}";
const LIST = `\\[(?:${NUM}(?:,${NUM})*)?\\]`;
/** A sidecar as its writer prints it: fixed keys, fixed decimals, the 4 s click's own header. */
const SIDECAR = new RegExp(
  `^\\{"v":4,"fingerprint":"${fingerprint(beat)}","sampleRate":8000,"channels":1,"frames":32000,"seconds":4\\.000000` +
    `,"peak":${LIST},"rms":${LIST},"low":${LIST},"mid":${LIST},"high":${LIST},"bpm":(\\d+\\.\\d{2})` +
    `,"sensitivity":(\\d+\\.\\d{4}),"transients":\\[(?:\\d+\\.\\d{6}(?:,\\d+\\.\\d{6})*)?\\]\\}$`,
);
const sidecar = () =>
  readFileSync(
    join(wsDir(ws), "clip-lane.k7", "clips", "beat.wav.cache"),
    "utf8",
  );

test("without an open project, what needs one says so", async () => {
  const none = "no active project — select a project in the workspace first";
  for (const [method, path, data] of [
    ["GET", "/export", undefined],
    ["POST", "/preview", { kind: "source", source: "clips/beat.wav" }],
  ] as const) {
    const r = await svc.call(method, path, { ws, data });
    expect([path, r.status, r.body]).toEqual([path, 400, { error: none }]);
  }
  expect((await svc.get("/presets?node=x", ws)).body).toEqual({
    error: "no active project",
  });
});

test("a clip's waveform is answered in buckets, its sidecar written beside the audio", async () => {
  expect((await svc.open(ws, "clip-lane.k7")).status).toBe(200);
  const r = await svc.get("/waveform?clip=beat&buckets=16&whole=1", ws);
  expect(r.status).toBe(200);
  expect(Object.keys(r.body)).toEqual([
    "rms",
    "peak",
    "low",
    "mid",
    "high",
    "seconds",
    "intervalMs",
    "startBeat",
    "clipBeats",
    "grid",
    "warpMarks",
    "sourceMap",
    "transients",
    "detected",
    "onsetEdits",
  ]);
  for (const k of ["rms", "peak", "low", "mid", "high"])
    expect(r.body[k], k).toHaveLength(16);
  expect(Math.max(...r.body.peak)).toBeGreaterThan(0);
  expect(r.body).toMatchObject({
    seconds: 4,
    intervalMs: 250,
    startBeat: 0,
    sourceMap: { inPoint: 0, stretch: 1, zeroBeat: 0 },
  });
  // A click every half second: eight onsets in four seconds.
  expect(r.body.transients.length).toBeGreaterThanOrEqual(7);
  test.skip(!filesRoot, NO_FILES_ROOT);
  const [, bpm, sensitivity] = SIDECAR.exec(sidecar()) ?? [];
  expect(sensitivity).toBe("2.0000");
  expect(Math.abs(Number(bpm) - 120)).toBeLessThan(2);
});

test("a sensitivity asked for is bounded, written into the sidecar, and kept by the next ask", async () => {
  const at = async (q: string) =>
    (await svc.get(`/waveform?clip=beat&buckets=16&whole=1${q}`, ws)).status;
  expect(await at("&sensitivity=3")).toBe(200);
  if (filesRoot) expect(SIDECAR.exec(sidecar())?.[2]).toBe("3.0000");
  expect(await at("")).toBe(200);
  if (filesRoot) expect(SIDECAR.exec(sidecar())?.[2]).toBe("3.0000");
  expect(await at("&sensitivity=100")).toBe(200);
  if (filesRoot) expect(SIDECAR.exec(sidecar())?.[2]).toBe("8.0000");
  expect(await at("&sensitivity=0")).toBe(200);
  if (filesRoot) expect(SIDECAR.exec(sidecar())?.[2]).toBe("0.2500");
});

test("a waveform names its clip: none, or one the project has not, is refused", async () => {
  const none = await svc.get("/waveform", ws);
  expect([none.status, none.body]).toEqual([400, { error: "clip required" }]);
  const unknown = await svc.get("/waveform?clip=nope", ws);
  expect([unknown.status, unknown.body]).toEqual([
    400,
    { error: "no clip 'nope'" },
  ]);
});

test("the prewarm's state is three fields, and a cancel answers it", async () => {
  for (const [method, path] of [
    ["GET", "/waveforms/status"],
    ["POST", "/waveforms/prewarm"],
    ["POST", "/waveforms/cancel"],
  ] as const) {
    const r = await svc.call(method, path, { ws });
    expect(r.status).toBe(200);
    expect(Object.keys(r.body)).toEqual(["warming", "done", "total"]);
  }
  await expect
    .poll(async () => (await svc.get("/waveforms/status", ws)).body.warming, {
      timeout: 30_000,
    })
    .toBe(false);
});

/** Two new clips (never the fixture's kick or snare), a decode each; /upload refuses to overwrite (409). */
async function staleClips(names: [string, string]): Promise<void> {
  for (const [name, seconds] of [
    [names[0], 5],
    [names[1], 6],
  ] as const) {
    const up = await svc.call(
      "POST",
      `/upload?dir=clip-lane.k7/clips&name=${name}`,
      {
        ws,
        data: clickWav(seconds),
        headers: { "content-type": "application/octet-stream" },
      },
    );
    expect(up.status).toBe(200);
  }
}

const settled = async () => {
  const s = (await svc.get("/waveforms/status", ws)).body;
  return !s.warming && s.total >= 1 && s.done === s.total;
};

test("a prewarm reaches done equal to total, a waveform is answered while it runs, and a cancel lets a clip be renamed", async () => {
  expect((await svc.open(ws, "clip-lane.k7")).status).toBe(200);
  // Only clips the song places count (beat is on no lane), so the stale one is kick: its sidecar is recycled for the prewarm to rebuild.
  const gone = await svc.post(
    "/entry/delete",
    { path: "clip-lane.k7/clips/kick.wav.cache" },
    ws,
  );
  expect(gone.status).toBe(200);
  await staleClips(["pad.wav", "clap.wav"]);
  await svc.post("/waveforms/prewarm", {}, ws);
  await expect
    .poll(async () => (await svc.get("/waveforms/status", ws)).body.total, {
      timeout: 30_000,
    })
    .toBeGreaterThanOrEqual(1);
  const drawn = await svc.get("/waveform?clip=kick&buckets=16&whole=1", ws);
  expect(drawn.status).toBe(200);
  await expect.poll(settled, { timeout: 30_000 }).toBe(true);
  // The open project's folder is refused by the guard; a clip a decode held is what a cancel frees (new names: an upload never overwrites).
  await staleClips(["hat.wav", "tom.wav"]);
  await svc.post("/waveforms/prewarm", {}, ws);
  const cancelled = await svc.post("/waveforms/cancel", {}, ws);
  expect(cancelled.body).toEqual({ warming: false, done: 0, total: 0 });
  const moved = await svc.post(
    "/rename",
    { path: "clip-lane.k7/clips/tom.wav", name: "tom2.wav" },
    ws,
  );
  expect([moved.status, moved.body]).toEqual([200, expect.anything()]);
  const back = await svc.post(
    "/rename",
    { path: "clip-lane.k7/clips/tom2.wav", name: "tom.wav" },
    ws,
  );
  expect(back.status).toBe(200);
  test.skip(!filesRoot, NO_FILES_ROOT);
  // The sidecar is the same bytes whichever implementation wrote it.
  expect(sidecar()).toMatch(SIDECAR);
});

test("an audition hands back the window it sent, and refuses audio it cannot find", async () => {
  const ask = (body: object) => svc.post("/preview", body, ws);
  expect((await ask({ kind: "clip", clip: "beat" })).body).toEqual({
    ok: true,
    start: 0,
    length: 0,
  });
  expect(
    (await ask({ kind: "source", source: "clips/beat.wav" })).body,
  ).toEqual({ ok: true, start: 0, length: 0 });
  expect(
    (
      await ask({
        kind: "slice",
        source: "clips/beat.wav",
        from: 0.5,
        to: 1.25,
      })
    ).body,
  ).toEqual({
    ok: true,
    start: 0.5,
    length: 0.75,
  });
  const missing = await ask({ kind: "source", source: "x.wav" });
  expect([missing.status, missing.body]).toEqual([
    400,
    { error: "no audio for 'x.wav'" },
  ]);
  const bad = await ask({ kind: "bogus" });
  expect([bad.status, bad.body]).toEqual([
    400,
    {
      error:
        "Invalid discriminator value. Expected 'source' | 'clip' | 'slice'",
    },
  ]);
});

test("the project's resources are its clips' audio, each with its source and size", async () => {
  const r = await svc.get("/resources", ws);
  expect(r.body.resources).toEqual([
    {
      id: "beat",
      kind: "audio",
      source: "clip-lane.k7/clips/beat.wav",
      bytes: beat.length,
    },
    {
      id: "kick",
      kind: "audio",
      source: "clip-lane.k7/clips/kick.wav",
      bytes: 9644,
    },
    {
      id: "kick-window",
      kind: "audio",
      source: "clip-lane.k7/clips/kick.wav",
      bytes: 9644,
    },
    {
      id: "snare",
      kind: "audio",
      source: "clip-lane.k7/clips/snare.wav",
      bytes: 9644,
    },
  ]);
});

test("a mixdown is rendered and streamed as an attachment, WAV or MP3; a timeline id is one name", async () => {
  const wav = await svc.request.get("/api/files/export", {
    headers: svc.headers(ws),
  });
  expect(wav.status()).toBe(200);
  expect(wav.headers()["content-type"]).toBe("audio/wav");
  expect(wav.headers()["content-disposition"]).toMatch(
    /^attachment; filename=".+\.wav"; filename\*=UTF-8''.+\.wav$/,
  );
  const bytes = await wav.body();
  expect([
    bytes.subarray(0, 4).toString("latin1"),
    bytes.subarray(8, 12).toString("latin1"),
  ]).toEqual(["RIFF", "WAVE"]);
  const mp3 = await svc.request.get("/api/files/export?format=mp3", {
    headers: svc.headers(ws),
  });
  expect([mp3.status(), mp3.headers()["content-type"]]).toEqual([
    200,
    "audio/mpeg",
  ]);
  expect((await mp3.body()).subarray(0, 3).toString("latin1")).toBe("ID3");
  const bad = await svc.get("/export?timeline=a/b", ws);
  expect([bad.status, bad.body]).toEqual([
    400,
    { error: "'a/b' names no timeline" },
  ]);
});

test("the plugin routes refuse what names no scanned plugin, and say when no engine answers", async () => {
  const none = await svc.get("/plugin-params", ws);
  expect([none.status, none.body]).toEqual([
    400,
    { error: "format and id, or ref, are required" },
  ]);
  const unknown = { error: 'no scanned plugin for "@engine/x"' };
  for (const path of [
    "/plugin-params?ref=@engine/x",
    "/plugin-native-ui?ref=@engine/x",
  ]) {
    const r = await svc.get(path, ws);
    expect([path, r.status, r.body]).toEqual([path, 404, unknown]);
  }
  const snap = await svc.post(
    "/module-base/snapshot",
    { path: "x/module.k7x", name: "a", ref: "@engine/x" },
    ws,
  );
  expect([snap.status, snap.body]).toEqual([404, unknown]);
  const silent = await svc.get("/plugin-params?format=vst3&id=Nope", ws);
  expect([silent.status, silent.body]).toEqual([
    503,
    { error: "the engine did not answer — is it running?" },
  ]);
});

test("the engine's own routes answer ok, and an unload ends what the account serves", async () => {
  for (const [path, body] of [
    ["/engine/project", { doc: {} }],
    ["/engine/sync/retry", {}],
    ["/auto-freeze/thaw", { module: "x", on: true }],
  ] as const)
    expect((await svc.post(path, body, ws)).body).toEqual({ ok: true });
  const model = await svc.post("/models/download", { node: "nope" }, ws);
  expect([model.status, model.body]).toEqual([
    404,
    { error: "no node 'nope' in this project" },
  ]);
  expect((await svc.get("/engine/served")).body).toEqual({
    project: { workspace: ws, path: "clip-lane.k7" },
  });
  expect((await svc.post("/engine/unload", {}, ws)).body).toEqual({ ok: true });
  expect((await svc.get("/engine/served")).body).toEqual({ project: null });
});

test("the Logs panel: the engine machine's lines by byte, the service's own by number, a verb's among them", async () => {
  const r = await svc.get("/logs?tail=5", ws);
  expect(r.status).toBe(200);
  expect(Object.keys(r.body)).toEqual(["offset", "lines", "service"]);
  expect(Object.keys(r.body.service)).toEqual(["seq", "lines"]);
  expect(r.body.service.lines.length).toBeLessThanOrEqual(5);
  for (const line of r.body.service.lines) {
    expect(Object.keys(line)).toEqual(["seq", "t", "level", "text"]);
    expect(line.seq).toBeLessThanOrEqual(r.body.service.seq);
  }
  const after = await svc.get(`/logs?after=${r.body.service.seq}`, ws);
  expect(after.body.service.seq).toBeGreaterThanOrEqual(r.body.service.seq);
  // A verb runs in the Worker now, so the service says no `[cmd]` line of it; the open project still moves.
  expect((await svc.open(ws, "clip-lane.k7")).status).toBe(200);
  const ran = await svc.verb(ws, "track new logs_probe", { want: [] });
  expect(ran.status).toBe(200);
  expect(ran.body.rev).toBeDefined();
});
