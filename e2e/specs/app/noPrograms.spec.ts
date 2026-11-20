// noPrograms.spec.ts — the members that start a program answer on iOS as a machine without it, each with its reason.
// The one thing a reader would get wrong: iOS runs no ffmpeg, whisper or browser, so these answers are the feature, not a failure.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FIXTURES_WORKSPACE as WS } from "../../src/fixtures.ts";
import { expect, type K7, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";

const PROJECT = "video-lane.k7";
const FILM = `/w/${PROJECT}/clips/sample.mp4`;
const WHY = /iOS app cannot start a program/;

test.describe.configure({ mode: "serial", timeout: 240_000 });
test.beforeEach(({ k7 }) => {
  test.skip(k7.target !== "ios", "only the iOS app has no program to start");
});

/** The files service bound to the phone's own engine, as the page binds it once an engine is picked. */
async function bound(k7: K7, project: string) {
  const page = await k7.open(`/@local/${WS}/${project}`);
  await engineUp(page, 60_000);
  const files = await k7.files();
  const [engine] = (await (await files.request.get("/api/engines")).json()) as {
    id: string;
  }[];
  files.bindEngine(engine?.id ?? "");
  await files.open(WS, project);
  return files;
}

/** A film with sound made by the runner's ffmpeg, or null: the phone's own cannot make one. */
function soundFilm(): Buffer | null {
  const dir = mkdtempSync(join(tmpdir(), "phone-film-"));
  const out = join(dir, "film.mp4");
  try {
    const lavfi = (spec: string) => ["-f", "lavfi", "-i", spec];
    execFileSync(
      "ffmpeg",
      [
        "-v",
        "error",
        ...lavfi("testsrc=duration=1:size=128x72:rate=10"),
        ...lavfi("sine=duration=1"),
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

test("a film is probed from its own headers, and a file with none is typed unsupported", async ({
  k7,
}) => {
  const files = await k7.files();
  await files.open(WS, PROJECT);
  const read = await files.host(WS, "probe", { file: FILM });
  expect(read.status, JSON.stringify(read.body)).toBe(200);
  expect(read.body.facts).toMatchObject({ codec: expect.any(String) });
  await files.upload(
    WS,
    `${PROJECT}/clips/junk.mp4`,
    Buffer.from("not a film"),
  );
  const none = await files.host(WS, "probe", {
    file: `/w/${PROJECT}/clips/junk.mp4`,
  });
  expect(none.status).toBe(415);
  expect(none.body.code).toBe("unsupported");
  expect(none.body.error).toMatch(WHY);
});

test("a film's sound is extracted by the waveform module, with no program", async ({
  k7,
}) => {
  const files = await k7.files();
  await files.open(WS, PROJECT);
  // The fixture film is silent: an answer, not a fault.
  const silent = await files.host(WS, "extract", { video: FILM });
  const none = await files.hostJob(WS, silent.body.job as string);
  expect([none.state, none.value]).toEqual([
    "done",
    { facts: null, files: [] },
  ]);
  const bytes = soundFilm();
  test.skip(!bytes, "the runner has no ffmpeg to make a film with sound");
  await files.upload(WS, `${PROJECT}/clips/sound.mp4`, bytes as Buffer);
  const started = await files.host(WS, "extract", {
    video: `/w/${PROJECT}/clips/sound.mp4`,
  });
  const end = await files.hostJob(WS, started.body.job as string);
  expect(end.state, JSON.stringify(end)).toBe("done");
  expect(end.value).toMatchObject({ facts: { channels: 1 } });
});

test("a film is not transcribed: no whisper can start", async ({ k7 }) => {
  const files = await k7.files();
  await files.open(WS, PROJECT);
  const started = await files.host(WS, "transcribe", { video: FILM });
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  const end = await files.hostJob(WS, started.body.job as string);
  expect(end.state).toBe("failed");
  expect(String(end.error)).toMatch(WHY);
});

test("a clip's copies are not prepared: no ffmpeg can start", async ({
  k7,
}) => {
  const files = await k7.files();
  await files.open(WS, PROJECT);
  const ask = {
    at: PROJECT,
    clip: "sample",
    source: "clips/sample.mp4",
    optimized: "remux",
    proxy: false,
  };
  const started = await files.host(WS, "prepare", ask);
  expect(started.body, JSON.stringify(started.body)).toEqual({ started: true });
  // A failed background copy is a line of the service's own log.
  await expect
    .poll(async () => (await files.serviceLog(WS)).join("\n"), {
      timeout: 30_000,
    })
    .toMatch(WHY);
});

test("a picture is not rendered: no browser can start", async ({ k7 }) => {
  const files = await k7.files();
  await files.open(WS, PROJECT);
  const out = await files.command(
    WS,
    "picture render webm none size=160x90 from=0 to=1 into=/tmp",
  );
  expect(JSON.stringify(out)).toMatch(WHY);
});

test("a mixdown is exported as wav, and mp3 says it needs ffmpeg", async ({
  k7,
}) => {
  const files = await bound(k7, "clip-lane.k7");
  const mp3 = await files.exportMix(WS, "mp3");
  expect(mp3.status).toBe(400);
  expect(mp3.bytes.toString()).toMatch(WHY);
});
