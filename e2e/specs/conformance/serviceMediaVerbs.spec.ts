// serviceMediaVerbs.spec.ts — the verbs that reach the file service's media: a picture lane's preparation, its cut
// written for another editor and read back, its sound put on an audio lane, whisper's transcript, a playlist's stems
// refused, and a picture rendered on the server. A film with sound is made by the runner's ffmpeg, else those skip.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { putTree, REPO } from "../../src/projectTree.ts";
import { onDisk, onDiskBytes, Service } from "../../src/service.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 300_000 });

/** A three-second film with a 440 Hz tone, mp4 with AAC, made by the runner's ffmpeg; null when it has none. */
function filmWithSound(): Buffer | null {
  const dir = mkdtempSync(join(tmpdir(), "k7-film-"));
  try {
    const out = join(dir, "talk.mp4");
    const r = spawnSync(
      process.env.FFMPEG_BIN || "ffmpeg",
      [
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x240:rate=25:duration=3",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=3",
        "-c:v",
        "mpeg4",
        "-c:a",
        "aac",
        "-shortest",
        out,
      ],
      { stdio: "ignore" },
    );
    return r.status === 0 ? readFileSync(out) : null;
  } catch {
    return null;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const film = filmWithSound();
const verb = (line: string) => svc.verb(ws, line, { want: [] });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  await putTree(
    svc,
    ws,
    join(REPO, "e2e/fixtures/video-lane.k7"),
    "video-lane.k7",
  );
  expect((await svc.open(ws, "video-lane.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

test("a lane's footage a browser plays is left as it is", async () => {
  const r = await verb("picture prepare main");
  expect([r.status, r.body.output, r.body.changed]).toEqual([
    200,
    ["sample: ready as it is"],
    [],
  ]);
});

// A 160x90 one-second film: a full-size one never ends in the Mac's software GL.
test("the picture is rendered on the server into exports/, in the background", async () => {
  const r = await verb("picture render webm none size=160x90 from=0 to=1");
  // The Worker waits for the render the server runs, and says where it landed.
  expect(r.body.output).toEqual(["picture rendered: exports/video-lane.webm"]);
  test.skip(!filesRoot, NO_FILES_ROOT);
  let bytes: Buffer | null = null;
  let seen = -1;
  // Done once two looks two seconds apart find the same non-empty file.
  await expect
    .poll(
      () => {
        try {
          bytes = onDiskBytes(ws, "video-lane.k7/exports/video-lane.webm");
        } catch {
          return false;
        }
        const same = bytes.length > 0 && bytes.length === seen;
        seen = bytes.length;
        return same;
      },
      { timeout: 240_000, intervals: [2000] },
    )
    .toBe(true);
  // A WebM is EBML: its first four bytes say so.
  expect((bytes as unknown as Buffer).subarray(0, 4).toString("hex")).toBe(
    "1a45dfa3",
  );
  // A run asked to keep the media leaves a copy, which a comparison of two implementations' frames reads.
  if (process.env.CONFORMANCE_MEDIA_COPY)
    writeFileSync(
      join(process.env.CONFORMANCE_MEDIA_COPY, "render.webm"),
      bytes as unknown as Buffer,
    );
});

test("a silent shot's sound is said missing, and whisper is told it has nothing to hear", async () => {
  const sound = await verb("picture sound main dialog");
  expect([sound.body.output, sound.body.changed]).toEqual([
    [
      "sample: clips/sample.mp4 has no sound — the shot stays silent",
      "nothing on main has a sound",
    ],
    [],
  ]);
  const heard = await verb("track transcribe main subs");
  expect(heard.status).toBe(200);
  expect(heard.body.output[0]).toBe(
    "whisper is listening to clips/sample.mp4 — this can take a few minutes",
  );
  expect(heard.body.output[1]).toMatch(
    /^error: (ffmpeg could not read sample\.mp4|ffmpeg is not installed)/,
  );
});

test("a film's sound is extracted beside it as a float WAV on an audio lane", async () => {
  test.skip(!film, "the runner has no ffmpeg to make a film with sound");
  expect(
    (await svc.upload(ws, "video-lane.k7/clips", "talk.mp4", film as Buffer))
      .status,
  ).toBe(200);
  expect(
    (
      await svc.put(
        "/file",
        {
          path: "video-lane.k7/clips/sample.k7x",
          content: 'source("clips/talk.mp4")\n',
        },
        ws,
      )
    ).status,
  ).toBe(200);
  const r = await verb("picture sound main dialog");
  expect(r.status).toBe(200);
  expect(r.body.refused).toBeUndefined();
  const wav = (r.body.changed as { path: string }[])
    .map((c) => c.path)
    .find((p) => p.endsWith(".wav"));
  expect(wav, JSON.stringify(r.body)).toBeTruthy();
  test.skip(!filesRoot, NO_FILES_ROOT);
  const bytes = onDiskBytes(ws, `video-lane.k7/${wav}`);
  // RIFF, a 16-byte fmt of format 3 (IEEE float), 32 bits, then data: extractSound's header.
  expect([
    bytes.toString("ascii", 0, 4),
    bytes.readUInt16LE(20),
    bytes.readUInt16LE(34),
    bytes.toString("ascii", 36, 40),
  ]).toEqual(["RIFF", 3, 32, "data"]);
  expect(bytes.readUInt32LE(24)).toBe(48000);
  expect(bytes.readUInt32LE(40)).toBe(bytes.length - 44);
  if (process.env.CONFORMANCE_MEDIA_COPY)
    writeFileSync(join(process.env.CONFORMANCE_MEDIA_COPY, "sound.wav"), bytes);
  // The lane playing it names the extracted file, wherever the verb wrote that lane.
  const named = (r.body.changed as { path: string }[])
    .map((c) => c.path)
    .filter((p) => p.endsWith(".k7x"))
    .map((p) => onDisk(ws, `video-lane.k7/${p}`));
  const file = (wav as string).split("/").pop() as string;
  expect(named.some((t) => t.includes(file))).toBe(true);
});

test("whisper hears a film with sound into subtitles on a picture lane", async () => {
  test.skip(!film, "the runner has no ffmpeg to make a film with sound");
  const r = await verb("track transcribe main subs");
  expect(r.status).toBe(200);
  const said = r.body.output as string[];
  expect(said[0]).toBe(
    "whisper is listening to clips/talk.mp4 — this can take a few minutes",
  );
  // transcribeAudio's own words for a server with neither CLI, or whisper-cli without its model.
  test.skip(
    said.some((l) =>
      /^error: (no whisper on this machine|whisper-cli failed: whisper-cli needs a model)/.test(
        l,
      ),
    ),
    "no whisper, or no whisper model, on the server",
  );
  expect(
    said.some((l) => l.startsWith("error:")),
    said.join("\n"),
  ).toBe(false);
});

test("a playlist's stems need a playlist and a separating chain", async () => {
  expect((await verb("playlist stems")).body).toMatchObject({
    output: [
      "usage: playlist stems <playlist> [chain]",
      "       (e.g. playlist stems ete-2026 @k7-core/stems)",
    ],
    refused: true,
    changed: [],
  });
  expect((await verb("playlist stems nope")).body).toMatchObject({
    output: [
      "no separating chain: name one, or set this machine's with `system stems <chaîne>`",
    ],
    refused: true,
    changed: [],
  });
});

test("the cut is written for another editor, otio every lane, edl one, and read back into new lanes", async () => {
  const otio = await verb("picture interchange otio");
  expect([otio.body.output, otio.body.changed]).toEqual([
    ["+ exports/cut.otio"],
    [{ path: "exports/cut.otio", kind: "added" }],
  ]);
  const timeline = JSON.parse(
    await svc.text(ws, "video-lane.k7/exports/cut.otio"),
  );
  expect(timeline.OTIO_SCHEMA).toMatch(/^Timeline\./);
  const edl = await verb("picture interchange edl main");
  expect(edl.body.output).toEqual(["+ exports/cut.edl"]);
  expect(await svc.text(ws, "video-lane.k7/exports/cut.edl")).toMatch(
    /^TITLE: /,
  );
  const missing = await verb("picture import exports/none.otio");
  expect([missing.body.output, missing.body.refused]).toEqual([
    [
      "no exports/none.otio in this project — put the file in it, then name it from the project folder",
    ],
    true,
  ]);
  const back = await verb("picture import exports/cut.otio conf");
  expect(back.status).toBe(200);
  expect(back.body.refused).toBeUndefined();
  expect(back.body.output.length).toBeGreaterThan(0);
  const added = (back.body.changed as { path: string; kind: string }[])
    .filter((c) => c.kind === "added")
    .map((c) => c.path);
  expect(added.some((p) => p.startsWith("nodes/conf"))).toBe(true);
});
