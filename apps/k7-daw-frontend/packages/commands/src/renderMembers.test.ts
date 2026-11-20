// `selection consolidate` and `node freeze` over the host's `render` and `seconds`: no engine, the members are the fakes.
// What is pinned is that a verb asks the HOST for the render and the length, never a file it reads itself.

import {
  cpSync,
  existsSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { runCommand } from "./commandSpec";
import { compileDisk, compileReady } from "./runtime/testing/compileDisk";
import { testHost } from "./testHost";

beforeAll(compileReady);

const SONG = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
  "..",
  "demos",
  "node-demo.k7",
);

let root: string;
let song: string;
const asked: { out: string; tracks: number }[] = [];

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "td-render-members-"));
  song = join(root, "ws", "song");
  cpSync(SONG, song, { recursive: true });
  asked.length = 0;
});
afterEach(() => rmSync(root, { recursive: true, force: true }));

/** A render that writes a placeholder mix and remembers what it was asked. */
const fakeRender = async (
  doc: { tracks: unknown[] },
  out: string,
): Promise<{ mix: string; stems: string[] }> => {
  asked.push({ out, tracks: doc.tracks.length });
  writeFileSync(join(out, "mix.wav"), "RIFF");
  return { mix: "mix.wav", stems: [] };
};

describe("node freeze", () => {
  it("renders through the host's render, into the file the module is then frozen to", async () => {
    const h = testHost({
      songPath: song,
      project: () => compileDisk(song),
      render: fakeRender as never,
    });
    await runCommand("node freeze drums", h, "cli");
    expect(h.lines.refuse).toEqual([]);
    expect(asked).toHaveLength(1);
    const said = h.lines.log.join("\n");
    expect(said).toMatch(/drums: frozen to /);
    const rel = /frozen to (\S+)/.exec(said)?.[1] as string;
    expect(existsSync(join(song, rel))).toBe(true);
  });

  it("says there is no engine when the host has no render", async () => {
    const h = testHost({
      songPath: song,
      project: () => compileDisk(song),
      render: undefined,
    });
    await runCommand("node freeze drums", h, "cli");
    expect(h.lines.refuse.join(" ")).toMatch(/no engine to render with/);
  });
});

describe("selection consolidate", () => {
  const sound = (seconds: (f: string) => number | null) =>
    testHost({
      songPath: song,
      project: () => compileDisk(song),
      render: fakeRender as never,
      seconds,
    });

  it("reports the length the host's seconds states for the render", async () => {
    const track = compileDisk(song).tracks.find((t) =>
      t.clips.some((c) => c.type === "audio"),
    );
    expect(track).toBeDefined();
    const at = "1:1";
    const h = sound(() => 7.25);
    await runCommand(`selection consolidate ${track?.id} ${at} 9:1`, h, "cli");
    expect(h.lines.refuse).toEqual([]);
    expect(asked).toHaveLength(1);
    expect(h.lines.log.join("\n")).toMatch(/7\.25s rendered/);
  });

  it("refuses with no engine, naming what consolidating audio means", async () => {
    const track = compileDisk(song).tracks.find((t) =>
      t.clips.some((c) => c.type === "audio"),
    );
    const h = testHost({
      songPath: song,
      project: () => compileDisk(song),
      render: undefined,
    });
    await runCommand(`selection consolidate ${track?.id} 1:1 9:1`, h, "cli");
    expect(h.lines.refuse.join(" ")).toMatch(/no engine to render with/);
  });
});
