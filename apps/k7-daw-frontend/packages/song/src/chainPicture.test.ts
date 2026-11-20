// PICTURE in a chain project (docs/chainFiles.md): a picture lane is a `track` item heading a
// chain of its own, a shape starts the chain of the lane it draws on, and a fade's STRETCH is a
// subchain of its lane's chain. The segments read all three back as a graph.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { connectNodes } from "@studio/song-schema/yaml/graphFile";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { editChainGraph } from "./chainGraphEdit";
import { readChainProject, readLayout, toSegments } from "./chainProject";
import { loadGraph, validate } from "./graph";
import { wsData } from "./yaml/workspaceDoc";

let root: string;
let dir: string;
const write = (rel: string, text: string) => {
  const abs = join(dir, rel);
  mkdirSync(join(abs, ".."), { recursive: true });
  writeFileSync(abs, text);
};
const module = (rel: string, text: string) => {
  const abs = join(root, "_modules", rel);
  mkdirSync(abs, { recursive: true });
  writeFileSync(join(abs, "module.k7x"), text);
};

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "chain-picture-"));
  dir = join(root, "film.k7");
  for (const s of ["glow", "iris"]) {
    module(`shaders/${s}`, 'type("shader")\nshader("p")\n');
    mkdirSync(join(root, "_modules", "shaders", s, "shaders", "p"), {
      recursive: true,
    });
    for (const f of ["vertex.glsl", "fragment.glsl"])
      writeFileSync(
        join(root, "_modules", "shaders", s, "shaders", "p", f),
        "",
      );
  }
  module(
    "shapes/box",
    'type("shape")\nshape("square")\nparams({ w: 10, h: 10 })\n',
  );
  write(
    "project.k7x",
    'project({ name: "film", tempo: 120, viewport: [640, 360, 30] })\ntrackOrder(["boxes", "main"])\n',
  );
  write("clips/a.k7x", 'source("a.mp4")\n');
  write("clips/a.mp4", "");
  write(
    "tracks/main.k7x",
    'extends("@meta/picture-track")\nevents([\n  ref("a").start(0).length(4),\n  ref("a").start(3).length(4).fadeIn({ over: 1, chain: "iris" }),\n])\n',
  );
  write("tracks/boxes.k7x", 'extends("@meta/picture-track")\nevents([])\n');
  // The lane heads its chain, the stretch is a subchain of it, and the shape starts the other lane's.
  write(
    "chains/main.k7x",
    'start(true)\nsteps([id("e236ac58-39aa-4710-8afd-dd3e145f5514").track("main"), to([chain("glow")])])\nsubchains([id("iris").steps([id("3663c123-f41a-40d0-9a42-27b538d4faf8").extends("@workspace/shaders/iris")])])\n',
  );
  write(
    "chains/glow.k7x",
    'steps([id("078a9886-bfac-42a4-9a04-1bf1dfdb7378").extends("@workspace/shaders/glow"), to([chain("RENDER")])])\n',
  );
  write(
    "chains/boxes.k7x",
    'start(true)\nsteps([\n  id("c3abff25-19a8-49b0-9dd2-d7661d118d3e").extends("@workspace/shapes/box"),\n  id("a2584b21-cbe4-479a-9e6b-4b041d6b002e").track("boxes"),\n])\n',
  );
  write(
    "chains/picture.k7x",
    'start("RENDER")\nsteps([to([chain("VIDEO_OUT")])])\n',
  );
});
afterEach(() => rmSync(root, { recursive: true, force: true }));

const chainData = (name: string) => wsData(join(dir, "chains", `${name}.k7x`));

describe("a chain project with picture", () => {
  it("puts a picture lane at the head of a `start: true` chain of its own", () => {
    const main = readChainProject(dir).chains.main;
    expect(main.start).toBe(true);
    expect(main.steps[0]).toMatchObject({ track: "main" });
  });

  it("makes the stretch a SUBCHAIN of the lane's chain, with no way out of its own", () => {
    const main = readChainProject(dir).chains.main;
    expect(main.subchains).toEqual([
      {
        id: "iris",
        steps: [
          expect.objectContaining({ extends: "@workspace/shaders/iris" }),
        ],
      },
    ]);
  });

  it("starts a SHAPE's chain, the lane it draws on following it", () => {
    const boxes = readChainProject(dir).chains.boxes;
    expect(boxes.start).toBe(true);
    expect(boxes.steps[0]).toMatchObject({ extends: "@workspace/shapes/box" });
    expect(boxes.steps[1]).toMatchObject({ track: "boxes" });
  });

  it("reads back as segments, the stretch landing on its lane", () => {
    const after = loadGraph(dir);
    expect(validate(after)).toEqual([]);
    const iris = readChainProject(dir).chains.main.subchains?.[0].steps[0] as {
      id: string;
    };
    expect(toSegments(readChainProject(dir)).graph.modules?.[iris.id]).toEqual([
      "main",
    ]);
  });
});

describe("a graph edit on a chain project with picture", () => {
  it("keeps the stretch where it was, under its id", () => {
    const glowId = Object.values(readChainProject(dir).nodes).find(
      (n) =>
        (n.value as { extends?: string }).extends === "@workspace/shaders/glow",
    )?.name as string;
    // An unrelated edit: the whole graph is rewritten, the stretch included.
    editChainGraph(dir, (t) => connectNodes(t, glowId, "OUT"));
    expect(
      (chainData("main").subchains as { id: string }[]).map((s) => s.id),
    ).toEqual(["iris"]);
    expect(readChainProject(dir).chains.main.subchains?.[0].id).toBe("iris");
    expect(readLayout(dir)).toEqual({});
  });
});
