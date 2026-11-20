// compile.vfs.test.ts — the WebAssembly compile over a bundle made from a project: drafts laid over it, a read outside it named.

import { join } from "node:path";
import { writeDoc, writeLane } from "@studio/song/testSupport";
import { beforeAll, describe, expect, it } from "vitest";
import { type Loaded, loadProject } from "./bundle.ts";
import {
  CompileNeedsFiles,
  type Compiler,
  viewOf,
  wasmCompiler,
} from "./compiler.ts";
import { setVfs } from "./shims/current.ts";
import { fakeServer } from "./testing/fakeServer.ts";
import { Vfs } from "./vfs.ts";

const root = "/ws";
const dir = join(root, "song.k7");
let compiler: Compiler;
let loaded: Loaded;

beforeAll(async () => {
  const disk = new Vfs(true);
  setVfs(disk);
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  writeDoc(join(dir, "graph.k7x"), {
    modules: { gtr: ["@workspace/audio/amp"], "@workspace/audio/amp": ["OUT"] },
  });
  writeLane(dir, "gtr", "track", { events: [] });
  writeDoc(join(root, "_modules", "audio", "amp", "module.k7x"), {
    plugin: { type: "internal", name: "Chorus" },
  });
  loaded = await loadProject(fakeServer(disk, root, "song.k7"), "song.k7");
  compiler = await wasmCompiler();
});

describe("the WebAssembly compile", () => {
  it("compiles the bundle's project and sees a draft laid over it", () => {
    const h = compiler.open(loaded.songBundle);
    try {
      const doc = viewOf<{ doc: { tracks: { name: string }[] } }>(
        compiler.compile(h, ["engine"]),
        "engine",
      ).doc;
      expect(doc.tracks.map((t) => t.name)).toEqual(["gtr"]);
      compiler.update(h, [
        { path: join(dir, "tracks", "pad.k7x"), text: "events: []\n" },
      ]);
      expect(compiler.compile(h, ["errors"]).views.errors).toBeDefined();
    } finally {
      compiler.close(h);
    }
  });

  it("names the file a read reached outside the bundle", () => {
    const bundle = {
      ...loaded.songBundle,
      files: { ...(loaded.songBundle.files as object) },
    };
    delete (bundle.files as Record<string, unknown>)["/w/song.k7/project.k7x"];
    const h = compiler.open(bundle);
    try {
      expect(() => compiler.compile(h, ["engine"])).toThrow(CompileNeedsFiles);
    } finally {
      compiler.close(h);
    }
  });
});
