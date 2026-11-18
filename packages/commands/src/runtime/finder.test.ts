// finder.test.ts — the BUILT Worker holds no server-only module: the finder's list must be empty, its builtins pinned.
// What a reader gets wrong: `leftover` is the song package's debt to 3c and 3d, not 3f's; M10 empties it.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime, serverInputs } from "../../scripts/bundleRuntime.mjs";

let out: string;
let left: string[] = [];
let leftover: string[] = [];

beforeAll(async () => {
  out = mkdtempSync(join(tmpdir(), "rt-finder-"));
  const built = await bundleRuntime({ target: "worker", outdir: out });
  left = built.serverLeft;
  leftover = built.leftover;
}, 120_000);
afterAll(() => rmSync(out, { recursive: true, force: true }));

describe("the Worker's server-only modules", () => {
  it("holds none of the modules the verbs reached the server through", () => {
    expect(left).toEqual([]);
  });

  // song's chainProject, autoFreeze and parseStore hash with node:crypto, which the Worker never calls.
  it("leaves external only the song package's own `node:crypto`", () => {
    expect(leftover).toEqual(["node:crypto"]);
  });

  it("names a server module by its package, whatever folder the build ran from", () => {
    const metafile = {
      inputs: {
        "../ffmpeg/src/ffmpeg.ts": {},
        "../audio-passes/src/resolveChain.ts": {},
        "../take-render/src/render.ts": {},
        "../stem-separation/src/separate.ts": {},
        "src/runtime/shims/childProcess.ts": {},
      },
    };
    expect(serverInputs(metafile)).toEqual([
      "../ffmpeg/src/ffmpeg.ts",
      "../stem-separation/src/separate.ts",
      "../take-render/src/render.ts",
      "src/runtime/shims/childProcess.ts",
    ]);
  });
});
