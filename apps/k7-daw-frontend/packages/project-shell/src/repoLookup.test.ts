// A device holds the bundled seed alone: its account lists `k7-core`, so the gate has a library and opens a project.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { configureFileApi, resetFileApi } from "./fileApi.ts";
import { accountLibraries } from "./repoLookup.ts";

const answer = (body: unknown): Response =>
  ({
    ok: true,
    status: 200,
    headers: new Headers(),
    text: () => Promise.resolve(JSON.stringify(body)),
    json: () => Promise.resolve(body),
  }) as unknown as Response;

beforeEach(() => {
  configureFileApi({
    base: "http://x",
    userEmail: "a@b.c",
    workspace: "w",
    engineToken: "t",
    engineId: "e1",
  });
});
afterEach(() => {
  resetFileApi();
  vi.unstubAllGlobals();
});

describe("a device with only the bundled seed", () => {
  it("lists k7-core, so the gate finds a library and does not stop on none", async () => {
    vi.stubGlobal("fetch", () =>
      Promise.resolve(answer({ libraries: [{ name: "k7-core", version: 4 }] })),
    );
    const libraries = await accountLibraries();
    expect(libraries.map((l) => l.name)).toEqual(["k7-core"]);
  });

  it("is an empty list, never a throw, when the reply names none", async () => {
    vi.stubGlobal("fetch", () => Promise.resolve(answer({})));
    expect(await accountLibraries()).toEqual([]);
  });
});
