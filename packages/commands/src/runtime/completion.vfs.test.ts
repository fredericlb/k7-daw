// completion.vfs.test.ts — a completion over the capacity project: the compile finds its library folders one round at a time, and a render it stats is absent.
// What a reader gets wrong: the project has no `_modules` folder at all, so every library folder the compile lists is a round of its own.

import { mkdirSync, writeFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { provideCommandsYaml } from "../commandSpec.ts";
import { installPluginVerbs } from "./plugins.ts";
import { commandsYamlText } from "./realFs.ts";
import { createRuntime, type Runtime } from "./runtime.ts";
import { setVfs } from "./shims/current.ts";
import { capacityFiles } from "./testing/capacity.ts";
import { fakeServer } from "./testing/fakeServer.ts";
import { Vfs } from "./vfs.ts";

const LANES = 8;
let rt: Runtime;

beforeEach(async () => {
  provideCommandsYaml(commandsYamlText());
  const disk = new Vfs(true);
  setVfs(disk);
  for (const [rel, text] of capacityFiles(LANES)) {
    const p = `/ws/cap.k7/${rel}`;
    mkdirSync(p.slice(0, p.lastIndexOf("/")), { recursive: true });
    writeFileSync(p, text);
  }
  rt = createRuntime({ remote: fakeServer(disk, "/ws", "cap.k7") });
  await rt.open("cap.k7");
});
afterEach(() => rt.close());

describe("completion over a project that holds no library folders", () => {
  it("lists every lane as a track", async () => {
    const tracks = await rt.complete("track rename ");
    expect(tracks).toHaveLength(LANES);
    expect(tracks).toContain("track rename l0");
  });

  it("answers again from what it learned", async () => {
    const first = await rt.complete("track rename l");
    expect(await rt.complete("track rename l")).toEqual(first);
  });
});

describe("a plugin's verb", () => {
  it("is listed and completes in the runtime that installed the plugins", async () => {
    installPluginVerbs();
    const listed = rt.catalogue().find((c) => c.command === "track transcribe");
    expect(listed?.args.map((a) => a.name)).toEqual(["track_in", "track_out"]);
    expect(await rt.complete("track transcri")).toContain("track transcribe");
    expect(await rt.complete("track transcribe ")).toEqual([]);
  });
});
