// The two `live` archetypes: a rig you play through. Three things they have to get right and none
// shows on a green build — no surface (bar the frame `live-video` cannot do without), no track and
// no lane of either kind, and the input and the lens still arrive. See docs/liveProject.md.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, sep } from "node:path";
import { RECORDER_PLUGIN } from "@studio/engine-protocol/build";
import { OUTPUT_RECORDER_ID } from "@studio/song-schema/moduleTypes";
import { byCodePoint } from "@studio/song-schema/textOrder";
import { describe, expect, it } from "vitest";
import { carriesCamera, loadGraph, validate } from "./graph";
import { MIXER_PLUGIN, trackMixer } from "./project";
import { writeProjectFiles } from "./scaffold";
import {
  LIVE_CAMERA,
  LIVE_FADER,
  LIVE_INPUT,
  liveProjectFiles,
  liveVideoProjectFiles,
  SHARED_STRIP,
  scaffoldLive,
  scaffoldLiveVideo,
} from "./scaffoldLive";
import {
  hasPicture,
  isVideoProject,
  projectSurfaces,
  surfaceIssues,
} from "./surfaces";
import { librariesFixture, setDoc, writeDoc } from "./testSupport";
import { wsData } from "./yaml/workspaceDoc";

/** Every file under `dir`, project-relative and slash-separated. */
function tree(dir: string): string[] {
  const out: string[] = [];
  const walk = (at: string) => {
    for (const e of readdirSync(at, { withFileTypes: true }).sort((a, b) =>
      byCodePoint(a.name, b.name),
    )) {
      const p = join(at, e.name);
      out.push(relative(dir, p).split(sep).join("/"));
      if (e.isDirectory()) walk(p);
    }
  };
  walk(dir);
  return out;
}

/**
 * A workspace holding the rig and the library it borrows from — the shared strip stubbed by the
 * plugin it is, so what is pinned is the scaffold's ref resolving rather than the account's file.
 */
const made = (write: (dir: string, name: string) => unknown): string => {
  const ws = mkdtempSync(join(tmpdir(), "live-"));
  const lib = join(librariesFixture(ws), "modules", "audio", "utility");
  for (const [name, plugin] of [["mixer", MIXER_PLUGIN]] as const)
    writeDoc(join(lib, name, "module.k7x"), {
      plugin: { type: "internal", name: plugin },
    });
  const dir = join(ws, "rig.k7");
  mkdirSync(dir);
  write(dir, "rig");
  return dir;
};

/** Run `f` against a fresh project of that shape and clean up whatever it did. */
const onShape = (
  write: (dir: string, name: string) => unknown,
  f: (dir: string) => void,
): void => {
  const dir = made(write);
  try {
    f(dir);
  } finally {
    // The workspace, not just the project: the library beside it is part of what was made.
    rmSync(join(dir, ".."), { recursive: true, force: true });
  }
};

/** …the audio rig as its FILE MAP writes it — a graph, the form the bench keeps — which most of this file is about. */
const on = (f: (dir: string) => void): void =>
  onShape((d, n) => scaffoldLive(d, n, "graph"), f);

/** …and the same rig with a lens, its file map written as it is. */
const onVideo = (f: (dir: string) => void): void =>
  onShape((d, n) => writeProjectFiles(d, liveVideoProjectFiles(n)), f);

/** `graph.k7x`'s `modules` map. */
const graphModules = (dir: string): Record<string, string[]> =>
  wsData(join(dir, "graph.k7x")).modules as Record<string, string[]>;

/** The keys of that map, in file order. */
const graphKeys = (dir: string): string[] => Object.keys(graphModules(dir));

/** Give the input node `channel`, as the project's own picker writes it. */
const pickChannel = (dir: string, channel: number[]): void =>
  setDoc(join(dir, "nodes", `${LIVE_INPUT}.k7x`), ["channel"], channel);

describe("what it writes", () => {
  it("declares NO surface at all — which is the whole of what makes it this shape", () =>
    on((dir) => {
      expect(existsSync(join(dir, "surfaces"))).toBe(false);
      expect(projectSurfaces(dir).surfaces).toEqual([]);
      // …and says nothing about it: a bare rig has no arrangement to draw and no picture to
      // composite, so neither missing-surface warning fires. That is what "no surface" has to mean.
      expect(surfaceIssues(dir)).toEqual([]);
    }));

  it("holds no material and no folders for it", () =>
    on((dir) => {
      const t = tree(dir);
      for (const gone of ["clips", "sequences", "video_tracks", "surfaces"])
        expect(t).not.toContain(gone);
    }));

  it("is the file map it says it is, and nothing else", () =>
    on((dir) => {
      expect(
        tree(dir)
          .filter((p) => p.endsWith(".k7x"))
          .sort(),
      ).toEqual(Object.keys(liveProjectFiles("rig")).sort());
    }));
});

describe("the graph it writes", () => {
  it("is INPUT -> FADER -> MIX -> REC -> OUT", () =>
    on((dir) => {
      expect(graphKeys(dir)).toEqual([LIVE_INPUT, LIVE_FADER, "MIX"]);
      const modules = graphModules(dir);
      // The fader lands on the SUM, not on the interface: the two compile the same, and only
      // this one draws where the signal goes. See docs/liveProject.md.
      expect(modules[LIVE_FADER]).toEqual(["MIX"]);
      // …and the master run goes straight out: what the bar records is what reaches OUT.
      expect(modules.MIX).toEqual(["OUT"]);
    }));

  it("authors NO track file — its nodes are the input and the fader", () =>
    on((dir) => {
      expect(
        tree(dir)
          .filter((f) => f.startsWith("nodes/"))
          .sort(),
      ).toEqual([`nodes/${LIVE_INPUT}.k7x`, `nodes/${LIVE_FADER}.k7x`].sort());
    }));

  it("borrows the SHARED strip, which is what carries the face", () =>
    on((dir) => {
      const node = wsData(join(dir, "nodes", `${LIVE_FADER}.k7x`));
      expect(node.extends).toBe(SHARED_STRIP);
    }));
});

describe("and it PLAYS", () => {
  it("says exactly one thing, and it is the CHOICE only you can make", () =>
    on((dir) => {
      // No default input: which socket you are plugged into is a fact about the rig, so the
      // scaffold leaves `channel` out and the project asks for it. Nothing ELSE warns —
      // a fresh project complaining about its own wiring would be the scaffold's fault.
      expect(validate(loadGraph(dir))).toEqual([
        "input 'IN': no channel — it feeds nothing",
      ]);
    }));

  it("says nothing at all once a channel is chosen", () =>
    on((dir) => {
      pickChannel(dir, [1]);
      expect(validate(loadGraph(dir))).toEqual([]);
    }));
});

// The SAME rig with a lens on it: `live-video` adds a camera and the frame it fills, and adds
// nothing else — no timeline, no video lane, no clip. See docs/liveProject.md.
describe("the live-video archetype", () => {
  it("is the audio rig plus exactly two files", () => {
    const audio = Object.keys(liveProjectFiles("rig"));
    const both = Object.keys(liveVideoProjectFiles("rig"));
    expect(both.filter((f) => !audio.includes(f)).sort()).toEqual([
      `nodes/${LIVE_CAMERA}.k7x`,
      "surfaces/viewport.k7x",
    ]);
  });

  it("still authors NO lane, of either kind", () =>
    onVideo((dir) => {
      const t = tree(dir);
      for (const gone of ["video_tracks", "clips", "sequences"])
        expect(t).not.toContain(gone);
      expect(t.filter((f) => f.startsWith("nodes/")).sort()).toEqual(
        [
          `nodes/${LIVE_INPUT}.k7x`,
          `nodes/${LIVE_FADER}.k7x`,
          `nodes/${LIVE_CAMERA}.k7x`,
        ].sort(),
      );
    }));

  it("wires the lens to RENDER, as the fader lands on the sum", () =>
    onVideo((dir) => {
      expect(graphKeys(dir)).toEqual([
        LIVE_INPUT,
        LIVE_FADER,
        "MIX",
        LIVE_CAMERA,
        "RENDER",
      ]);
    }));

  it("declares the viewport, the ONE surface this shape has", () =>
    onVideo((dir) => {
      expect(projectSurfaces(dir).surfaces.map((s) => s.surface.type)).toEqual([
        "viewport",
      ]);
      // …and nothing is owed: no lanes to draw, so no timeline is missed.
      expect(surfaceIssues(dir)).toEqual([]);
    }));

  it("ANSWERS that it has a picture, though it holds no video lane", () =>
    onVideo((dir) => {
      // The gate the file service reads before it compiles one. `video_tracks/` is not the
      // question any more — a lens needs no lane (docs/camera.md).
      expect(hasPicture(dir)).toBe(true);
      expect(isVideoProject(dir)).toBe(false);
    }));

  it("says the one thing the audio rig says, and nothing about the lens", () =>
    onVideo((dir) => {
      // A camera with no lens named is not a complaint: "" is the machine's own (docs/camera.md).
      expect(validate(loadGraph(dir))).toEqual([
        "input 'IN': no channel — it feeds nothing",
      ]);
    }));
});

// A new rig is born as CHAIN files, the main layout: the same rig, its nodes named by id. What has to
// survive the conversion is what it plays and what it says — docs/liveProject.md, docs/chainFiles.md.
describe("born as chain files", () => {
  it("holds chain files, and neither the graph nor a node file", () =>
    onShape(scaffoldLive, (dir) => {
      const t = tree(dir);
      expect(t).toContain("chains");
      expect(t).not.toContain("graph.k7x");
      expect(t).not.toContain("nodes");
    }));

  it("says the one thing a fresh rig says: pick a channel", () =>
    onShape(scaffoldLive, (dir) => {
      expect(validate(loadGraph(dir))).toEqual([
        expect.stringMatching(/^input '.+': no channel — it feeds nothing$/),
      ]);
    }));
});
