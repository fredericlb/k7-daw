import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCommand } from "@studio/commands/commandSpec";
import { installServerSide } from "@studio/k7-sdk/install";
import { pictureLaneFile, pictureLaneIds } from "@studio/song/pictureLanes";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { wsData, wsText } from "@studio/song/yaml/workspaceDoc";
import { listVideoPlacements } from "@studio/song-schema/yaml/videoTrackFile";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { transcriptionPlugin } from "./index";

// The whole walk, on a real folder: a picture lane holding a shot, the clip naming a placeholder video, and a host whose whisper answers a fixture SRT.
// What is pinned is the file flow: which lane is listened to, where the text lands, and that a rerun replaces rather than refuses.

let song: string;
/** What the host's whisper answers: one cue. */
const SRT = [
  "1",
  "00:00:00,000 --> 00:00:00,900",
  "hello from the fake whisper",
  "",
].join("\n");
const logs: string[] = [];
const refusals: string[] = [];

const host = () =>
  ({
    songPath: song,
    project: () => ({ tracks: [], groups: [] }),
    baseModules: [],
    log: (l: string[]) => logs.push(...l),
    refuse: (l: string[]) => refusals.push(...l),
    reload: () => {},
    transcribe: async () => SRT,
  }) as never;

// The video is a placeholder: the fake host's whisper listens to nothing.
const stage = (): void => {
  writeLane(song, "cam", "picture-track", {
    events: [{ ref: "intro", start: 2, length: 0, from: 0.5 }],
  });
  writeDoc(join(song, "clips", "intro.k7x"), { source: "clips/intro.mp4" });
  writeFileSync(join(song, "clips", "intro.mp4"), "");
};

// The plugin's verb exists only once its plugin is installed — the same act the
// file-service boot performs, so the tests exercise the real path rather than a static map.
installServerSide(transcriptionPlugin);

beforeEach(() => {
  logs.length = 0;
  refusals.length = 0;
  song = mkdtempSync(join(tmpdir(), "td-transcribe-"));
});
afterEach(() => {
  rmSync(song, { recursive: true, force: true });
});

describe("track transcribe", () => {
  it("refuses a video a clip names outside the project", async () => {
    writeLane(song, "cam", "picture-track", {
      events: [{ ref: "intro", start: 0, length: 0 }],
    });
    writeDoc(join(song, "clips", "intro.k7x"), {
      source: "../../elsewhere/intro.mp4",
    });
    await runCommand("track transcribe cam lyrics", host(), "cli");
    expect(refusals.join("\n")).toMatch(/outside the project/);
  });

  it("writes subtitles placed on a new picture lane where the shot is", async () => {
    stage();
    await runCommand("track transcribe cam lyrics", host(), "cli");
    expect(refusals).toEqual([]);
    const srt = readFileSync(join(song, "clips", "lyrics.srt"), "utf8");
    expect(srt).toContain("hello from the fake whisper");
    expect(srt).toContain("00:00:00,000 --> 00:00:00,900");
    expect(wsData(join(song, "clips", "lyrics.k7x")).source).toBe(
      "clips/lyrics.srt",
    );
    expect(pictureLaneIds(song)).toContain("lyrics");
    const lane = pictureLaneFile(song, "lyrics");
    expect(listVideoPlacements(wsText(String(lane?.path)))).toMatchObject([
      { ref: "lyrics", start: 2, length: 0, sourceFrom: 0.5 },
    ]);
  });

  it("replaces its own placement on a rerun, keeping the lane's others", async () => {
    stage();
    await runCommand("track transcribe cam lyrics", host(), "cli");
    const lane = String(pictureLaneFile(song, "lyrics")?.path);
    writeLane(song, "lyrics", "picture-track", {
      events: [
        { ref: "lyrics", start: 9, length: 1 },
        { ref: "credits", start: 20, length: 3 },
      ],
    });
    await runCommand("track transcribe cam lyrics", host(), "cli");
    expect(refusals).toEqual([]);
    expect(listVideoPlacements(wsText(lane)).map((p) => p.ref)).toEqual([
      "credits",
      "lyrics",
    ]);
    expect(logs.join(" ")).toMatch(/already existed/);
  });

  it("refuses a sound lane, or a clip of that name that is no subtitle file", async () => {
    stage();
    writeLane(song, "notes", "track", { events: [] });
    await runCommand("track transcribe cam notes", host(), "cli");
    expect(refusals.join(" ")).toMatch(/'notes' is a sound lane/);
    writeDoc(join(song, "clips", "intro2.k7x"), { source: "clips/intro.mp4" });
    await runCommand("track transcribe cam intro2", host(), "cli");
    expect(refusals.join(" ")).toMatch(/not a subtitle file/);
    expect(existsSync(join(song, "clips", "intro2.srt"))).toBe(false);
  });

  it("refuses a lane that is not there, or one placing nothing", async () => {
    await runCommand("track transcribe nope lyrics", host(), "cli");
    expect(refusals.join(" ")).toMatch(/no picture lane "nope"/);
    // An ACTIVATION: no ref, no video.
    writeLane(song, "cam", "picture-track", {
      events: [{ start: 0, length: 4 }],
    });
    await runCommand("track transcribe cam lyrics", host(), "cli");
    expect(refusals.join(" ")).toMatch(/places no shot/);
  });

  it("refuses a clip with no video on disk before whisper runs", async () => {
    writeLane(song, "cam", "picture-track", {
      events: [{ ref: "intro", start: 0, length: 0 }],
    });
    writeDoc(join(song, "clips", "intro.k7x"), { source: "clips/intro.mp4" });
    await runCommand("track transcribe cam lyrics", host(), "cli");
    expect(refusals.join(" ")).toMatch(/not on this machine/);
    expect(existsSync(join(song, "nodes", "lyrics.k7x"))).toBe(false);
  });
});
