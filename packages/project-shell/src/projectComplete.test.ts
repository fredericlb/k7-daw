// What `projectApi.complete` hands back, whichever shape the host answered in: bare lines and items come out as one list of items.

import { beforeEach, describe, expect, it } from "vitest";
import { configureFileApi } from "./fileApi.ts";
import { projectApi } from "./project.ts";
import { setVerbsHost, type VerbCompletion } from "./verbsHost.ts";

let asked: string[] = [];
let reply: (string | VerbCompletion)[] = [];

beforeEach(() => {
  asked = [];
  configureFileApi({
    base: "http://x",
    userEmail: "a@b.c",
    workspace: "w",
    engineToken: "t",
    engineId: "e1",
  });
  setVerbsHost({
    catalogue: async () => [],
    complete: async (line) => {
      asked.push(line);
      return reply;
    },
    run: async () => ({ output: [], refused: false }),
    videoDoc: async () => ({ error: "none" }),
    videoErrors: async () => ({ errors: [], diagnostics: [] }),
  });
});

describe("projectApi.complete", () => {
  it("asks the host for the line", async () => {
    reply = [];
    await projectApi.complete("node bypass ");
    expect(asked).toEqual(["node bypass "]);
  });

  it("turns a bare line into an item with no label", async () => {
    reply = ["node bypass a", { line: "node bypass b", label: "B" }];
    expect(await projectApi.complete("node bypass ")).toEqual([
      { line: "node bypass a" },
      { line: "node bypass b", label: "B" },
    ]);
  });
});
