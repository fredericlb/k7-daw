import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { RENDER_AUDIO_LEVEL } from "@studio/song-schema/formatLevel";
import { GRAPH_FILE } from "@studio/song-schema/yaml/graphSchema";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import { scaffold } from "./scaffold";
import { wsData } from "./yaml/workspaceDoc";

// The only project k7 writes from scratch, so "does it load and play" is the whole contract.

// A workspace, since `scaffold()` now also seeds the WORKSPACE's shared synth module beside it.
let ws: string;
let dir: string;
beforeEach(() => {
  ws = mkdtempSync(join(tmpdir(), "td-scaffold-"));
  dir = join(ws, "song.k7");
});
afterEach(() => rmSync(ws, { recursive: true, force: true }));

/** Every file under `dir`, project-relative, forward-slashed. */
const tree = (): string[] =>
  (readdirSync(dir, { recursive: true }) as unknown as string[])
    .map((f) => String(f).split("\\").join("/"))
    .sort();

describe("a scaffolded project", () => {
  it("is born a CHAIN project, the main layout: chain files and lanes, no graph and no node files", () => {
    const written = scaffold(dir, "demo");
    expect(tree().filter((f) => f.endsWith(".cue"))).toEqual([]);
    expect(tree()).toContain("project.k7x");
    expect(
      tree().filter((f) => f.startsWith("chains/")).length,
    ).toBeGreaterThan(0);
    expect(tree()).toContain("tracks/melody.k7x");
    expect(tree()).not.toContain(GRAPH_FILE);
    expect(tree().filter((f) => f.startsWith("nodes/"))).toEqual([]);
    // No workspace file is YAML any more.
    expect(tree().filter((f) => /\.ya?ml$/.test(f))).toEqual([]);
    // What it says it wrote is what is there.
    expect([...written].sort()).toEqual(tree().filter((f) => f.includes(".")));
  });
});

// The risk the pair carries is DRIFT, so `video` is pinned as `arrangement` plus picture, entry for entry.
describe("arrangement vs video", () => {
  it("gives an arrangement no picture at all — not a frame, not a shot, not the sample", () => {
    scaffold(dir, "demo");
    const files = tree();
    expect(files).toContain("surfaces/timeline.k7x");
    expect(files).not.toContain("surfaces/viewport.k7x");
    expect(files.filter((f) => f.startsWith("video_tracks/"))).toEqual([]);
    expect(files.filter((f) => f.startsWith("clips/"))).toEqual([]);
    // `loadVideoProject` REQUIRES a viewport, so the line without a lane fails on a missing file.
    expect(
      (wsData(join(dir, "project.k7x")).project as Record<string, unknown>)
        .viewport,
    ).toBeUndefined();
  });

  it("gives video the arrangement plus picture, sharing every other file", () => {
    scaffold(dir, "demo", "video");
    const files = tree();
    for (const f of [
      "surfaces/timeline.k7x",
      "surfaces/viewport.k7x",
      "tracks/main.k7x", // a chain project, its lanes in tracks/
      "clips/sample.k7x",
      "clips/sample.mp4", // COPIED, not referenced where the package sits
    ])
      expect(files).toContain(f);
    expect(
      (wsData(join(dir, "project.k7x")).project as Record<string, unknown>)
        .viewport,
    ).toEqual([1280, 720, 30]);
  });

  it("keeps the shared half identical, byte for byte", () => {
    // Catches drift: restating an entry instead of ADDING to the song's files fails on that entry.
    const other = join(ws, "audio.k7");
    scaffold(other, "demo");
    scaffold(dir, "demo", "video");
    const read = (root: string, rel: string) =>
      readFileSync(join(root, rel), "utf8");
    const files = (root: string): string[] =>
      (readdirSync(root, { recursive: true }) as unknown as string[])
        .map((f) => String(f).split("\\").join("/"))
        .filter((f) => /\.(k7x|strudel)$/.test(f) && !f.startsWith("chains/"));
    // Both are checked below instead: the header gains the picture, and the master run feeds RENDER.
    const merged = new Set(["project.k7x"]);
    for (const rel of files(other)) {
      if (merged.has(rel)) continue;
      expect(read(dir, rel)).toBe(read(other, rel));
    }
    // …and the header differs by exactly the picture fields, nothing else.
    const a = wsData(join(other, "project.k7x"));
    const v = wsData(join(dir, "project.k7x"));
    expect(v).toEqual({
      ...a,
      project: {
        ...(a.project as object),
        viewport: [1280, 720, 30],
      },
      format: RENDER_AUDIO_LEVEL,
    });
  });
});
