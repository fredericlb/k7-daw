// What `projectApi.command` hands the verbs' host — the options, not the wire.
//
// `want` is the field that decides whether the project's graph is built for nobody to draw, and the way it
// goes wrong is a call site that forgets it. So this pins the three shapes: no `want` at all (which must
// stay the default), the empty list (a caller saying no), and a folder other than the song.

import { beforeEach, describe, expect, it } from "vitest";
import { configureFileApi } from "./fileApi.ts";
import { projectApi } from "./project.ts";
import { setVerbsHost, type VerbRunOptions } from "./verbsHost.ts";

let runs: { line: string; options: VerbRunOptions }[] = [];
let answer: Record<string, unknown> = { output: [], refused: false };

beforeEach(() => {
  runs = [];
  answer = { output: [], refused: false };
  configureFileApi({
    base: "http://x",
    userEmail: "a@b.c",
    workspace: "w",
    engineToken: "t",
    engineId: "e1",
  });
  setVerbsHost({
    catalogue: async () => [],
    videoDoc: async () => ({ error: "none" }),
    videoErrors: async () => ({ errors: [], diagnostics: [] }),
    complete: async () => [],
    run: async (line, options) => {
      runs.push({ line, options });
      return answer as never;
    },
  });
});

describe("projectApi.command", () => {
  it("asks for the graph when the caller says nothing of `want`", async () => {
    await projectApi.command("track mute gtr");
    expect(runs).toEqual([
      { line: "track mute gtr", options: { want: ["graph"] } },
    ]);
  });

  it("carries an EMPTY want, which is a caller refusing the graph", async () => {
    await projectApi.command("track mute gtr", { want: [] });
    expect(runs[0]?.options).toEqual({ want: [] });
  });

  it("carries `at`, `want` and `journal` together", async () => {
    await projectApi.command("graph pin a", {
      at: "_library/x",
      want: [],
      journal: "skip",
    });
    expect(runs[0]?.options).toEqual({
      at: "_library/x",
      want: [],
      journal: "skip",
    });
  });

  it("answers the route's shape: refused only when true, the token and the changed files", async () => {
    answer = {
      output: ["no"],
      refused: true,
      rev: "b",
      revBefore: "a",
      changed: [{ path: "x", kind: "edited" }],
    };
    expect(await projectApi.command("x")).toEqual({
      output: ["no"],
      refused: true,
      rev: "b",
      revBefore: "a",
      changed: [{ path: "x", kind: "edited" }],
    });
    answer = { output: [], refused: false };
    expect(await projectApi.command("x")).toEqual({ output: [] });
  });
});

describe("projectApi.videoDoc", () => {
  const video = (error?: string) =>
    setVerbsHost({
      catalogue: async () => [],
      complete: async () => [],
      run: async () => ({ output: [], refused: false }),
      videoDoc: async () =>
        error ? { error } : { doc: { name: "p" } as never },
      videoErrors: async () => ({ errors: ["w"], diagnostics: [] }),
    });

  it("answers the host's picture, and its warnings", async () => {
    video();
    expect(await projectApi.videoDoc()).toEqual({ name: "p" });
    expect(await projectApi.videoErrors()).toEqual({
      errors: ["w"],
      diagnostics: [],
    });
  });

  it("throws the compile's sentence as a 400, which the viewport shows instead of keeping its picture", async () => {
    video("project.viewport is required");
    await expect(projectApi.videoDoc()).rejects.toMatchObject({
      message: "project.viewport is required",
      status: 400,
    });
  });
});
