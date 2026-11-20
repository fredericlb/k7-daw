// `picture prepare`'s codec choice, no ffmpeg: a probe made up answers HEVC, and the page's user agent says what plays.
// What a reader gets wrong: the Worker's `process.platform` is always "linux", so only the user agent tells Android.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { runCommand } from "./commandSpec";
import { testHost } from "./testHost";

let song: string;
const logs: string[] = [];

const HEVC = {
  codec: "hevc",
  width: 160,
  height: 120,
  pixFmt: "yuv420p",
  fps: 30,
  seconds: 1,
  container: "mov,mp4,m4a,3gp,3g2,mj2",
};

const host = () =>
  testHost({
    songPath: song,
    log: (l: string[]) => logs.push(...l),
    probe: async () => HEVC,
  });

beforeEach(() => {
  logs.length = 0;
  song = mkdtempSync(join(tmpdir(), "td-pmedia-"));
  mkdirSync(join(song, "clips"), { recursive: true });
  mkdirSync(join(song, "nodes"), { recursive: true });
  writeFileSync(
    join(song, "nodes", "main.k7x"),
    'extends("@meta/picture-track")\nevents([ref("pro").start(0).length(1)])\n',
  );
  writeFileSync(join(song, "clips", "pro.k7x"), 'source("clips/pro.mp4")\n');
  writeFileSync(join(song, "clips", "pro.mp4"), "");
});
afterEach(() => {
  vi.unstubAllGlobals();
  rmSync(song, { recursive: true, force: true });
});

describe("picture prepare's codec set", () => {
  it("plays HEVC as it is on an Android WebView", async () => {
    vi.stubGlobal("navigator", {
      userAgent:
        "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/120",
    });
    await runCommand("picture prepare main", host(), "cli");
    expect(logs).toEqual(["pro: ready as it is"]);
  });

  it("does not play HEVC as it is on a desktop browser", async () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/120",
    });
    await runCommand("picture prepare main", host(), "cli");
    expect(logs[0]).toMatch(/hevc/i);
  });
});
