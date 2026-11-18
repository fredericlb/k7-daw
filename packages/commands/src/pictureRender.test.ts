// `picture render` over the video demo against a fake host: the words reach the renderer as the browser export
// reads them beside the scene the verb built, the work runs in the background once, and a host with no renderer says where to go.

import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { sceneFrom } from "@studio/song-video/sceneFrom";
import { loadVideoProject } from "@studio/song-video/videoProject";
import { webVideoProjectOf } from "@studio/song-video/webVideo";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { runCommand } from "./commandSpec";

const DEMO = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "demos",
  "video-demo.k7",
);
let song: string;

const logs: string[] = [];
const refusals: string[] = [];
const asked: unknown[] = [];
let queued: (() => Promise<string>)[] = [];
const host = (opts: { render?: boolean; background?: boolean } = {}) =>
  ({
    songPath: song,
    project: () => ({ tracks: [], groups: [] }),
    baseModules: [],
    log: (l: string[]) => logs.push(...l),
    refuse: (l: string[]) => refusals.push(...l),
    reload: () => {},
    ...(opts.render === false
      ? {}
      : {
          renderPicture: async (o: unknown) => {
            asked.push(o);
            return "exports/song.mp4";
          },
        }),
    ...(opts.background === false
      ? {}
      : {
          background: (_tag: string, work: () => Promise<string>) => {
            if (queued.length) return false;
            queued.push(work);
            return true;
          },
        }),
  }) as never;

beforeEach(() => {
  song = join(mkdtempSync(join(tmpdir(), "td-prender-")), "video-demo.k7");
  cpSync(DEMO, song, { recursive: true });
  logs.length = 0;
  refusals.length = 0;
  asked.length = 0;
  queued = [];
});
afterEach(() => rmSync(dirname(song), { recursive: true, force: true }));

/** What the renderer was asked, less the scene (checked on its own). */
const lastAsk = () => {
  const { scene: _scene, ...rest } = asked.at(-1) as Record<string, unknown>;
  return rest;
};

describe("picture render", () => {
  it("hands the export's own settings to the renderer, in the background", async () => {
    await runCommand(
      "picture render mp4 none size=720p quality=max from=0:10 to=40",
      host(),
      "cli",
    );
    expect(refusals).toEqual([]);
    expect(logs[0]).toMatch(/in the background \(mp4, silent\)/);
    expect(await queued[0]?.()).toBe("picture rendered: exports/song.mp4");
    expect(lastAsk()).toEqual({
      format: "mp4",
      audio: false,
      settings: { size: { height: 720 }, quality: "max", from: 10, to: 40 },
    });
  });

  it("defaults to a webm with the mixdown, and waits when the host has no background", async () => {
    await runCommand("picture render", host({ background: false }), "cli");
    expect(lastAsk()).toMatchObject({ format: "webm", settings: {} });
    // The scene is the browser export's own: the viewport, the lanes, and every media URL a path under media/.
    const scene = (
      asked.at(-1) as { scene: { viewport: number[]; fps: number } }
    ).scene;
    expect(scene.viewport[0]).toBeGreaterThan(0);
    expect(scene.fps).toBeGreaterThan(0);
    expect(JSON.stringify(scene)).toMatch(/"media\/[^"]+"/);
    // Lanes, timelines and fades are the browser export's own construction, not just present.
    const expected = sceneFrom(
      webVideoProjectOf(loadVideoProject(song), (abs) => abs),
      null,
      "export",
      (p: string) => `media/${encodeURIComponent(p)}`,
    );
    expect(scene).toEqual(expected);
    expect(Object.keys(scene).length).toBeGreaterThan(3);
    expect(logs).toEqual(["picture rendered: exports/song.mp4"]);
  });

  it("refuses a second render while one runs, a setting it cannot read, and a host with no renderer", async () => {
    const h = host();
    await runCommand("picture render", h, "cli");
    await runCommand("picture render", h, "cli");
    await runCommand("picture render size=huge", h, "cli");
    await runCommand("picture render", host({ render: false }), "cli");
    expect(refusals[0]).toMatch(/already running/);
    expect(refusals[1]).toMatch(/^usage: picture render/);
    expect(refusals[2]).toMatch(/project export video/);
  });

  it("refuses a project that draws no picture before asking the host", async () => {
    song = join(song, "..", "nopic.k7");
    mkdirSync(song, { recursive: true });
    writeFileSync(
      join(song, "project.k7x"),
      "project:\n  name: t\n  tempo: 120\n",
    );
    await runCommand("picture render", host(), "cli");
    expect(asked).toEqual([]);
    expect(refusals.at(-1)).toMatch(/draws no picture/);
  });
});
