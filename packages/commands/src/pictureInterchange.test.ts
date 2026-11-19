// `picture interchange` over the video demo: the cut lands at the fixed path the shell downloads,
// OTIO carries the lanes with media and none of shapes, an EDL the one lane named.

import {
  cpSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { runCommand } from "./commandSpec";

const here = dirname(fileURLToPath(import.meta.url));
const DEMO = join(here, "..", "..", "..", "demos", "video-demo.k7");

let song: string;
const logs: string[] = [];
const refusals: string[] = [];
const host = (extra: Record<string, unknown> = {}) =>
  ({
    songPath: song,
    project: () => ({ tracks: [], groups: [] }),
    baseModules: [],
    log: (l: string[]) => logs.push(...l),
    refuse: (l: string[]) => refusals.push(...l),
    reload: () => {},
    ...extra,
  }) as never;

beforeEach(() => {
  logs.length = 0;
  refusals.length = 0;
  song = join(mkdtempSync(join(tmpdir(), "td-pinter-")), "video-demo.k7");
  cpSync(DEMO, song, { recursive: true });
});
afterEach(() => rmSync(dirname(song), { recursive: true, force: true }));

describe("picture interchange", () => {
  it("writes every lane with media as an OTIO track, media named from exports/", async () => {
    await runCommand("picture interchange otio", host(), "cli");
    expect(refusals).toEqual([]);
    expect(logs[0]).toBe("+ exports/cut.otio");
    const t = JSON.parse(
      readFileSync(join(song, "exports", "cut.otio"), "utf8"),
    );
    expect(t.OTIO_SCHEMA).toBe("Timeline.1");
    const names = t.tracks.children.map((c: { name: string }) => c.name);
    expect(names).toEqual(["main", "pip"]);
    const clip = t.tracks.children[0].children[0];
    expect(clip.media_reference.target_url).toBe("../clips/counter.mp4");
  });

  it("writes one lane as an EDL, and refuses a lane that is not there", async () => {
    await runCommand("picture interchange edl main", host(), "cli");
    const edl = readFileSync(join(song, "exports", "cut.edl"), "utf8");
    expect(edl).toMatch(/^TITLE: /);
    expect(edl).toContain("* FROM CLIP NAME: counter.mp4");
    await runCommand("picture interchange edl nope", host(), "cli");
    expect(refusals.join("\n")).toMatch(/no picture lane 'nope'/);
  });

  it("asks the host's probe for an open-ended last shot's length, and says what it leaves out without one", async () => {
    const lane = join(song, "tracks", "main.k7x");
    writeFileSync(
      lane,
      readFileSync(lane, "utf8").replace(".length(5)", ".length(0)"),
    );
    const probed: string[] = [];
    const probe = async (f: string) => {
      probed.push(f);
      return { seconds: 9 };
    };
    await runCommand("picture interchange edl main", host({ probe }), "cli");
    expect(probed.map((f) => f.split("/").at(-1))).toEqual(["bars.mp4"]);
    expect(logs.join("\n")).not.toMatch(/left out/);
    logs.length = 0;
    await runCommand(
      "picture interchange edl main",
      host({ probe: undefined }),
      "cli",
    );
    expect(logs.join("\n")).toMatch(/left out/);
  });
});
