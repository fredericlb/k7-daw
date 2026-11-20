// nodeDeleteGuard.test.ts — `node delete` refuses to leave a module short of a mandatory input, before any write.
// The module is the WORKSPACE's (`_modules/`), so the fixture holds over the Vfs shims too, with no repo file read.

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeDoc, writeLane, writeNode } from "@studio/song/testSupport";
import { wsText } from "@studio/song/yaml/workspaceDoc";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { runCommand } from "./commandSpec";

let ws: string;
let dir: string;
const logs: string[] = [];
const host = () =>
  ({
    songPath: dir,
    project: () => ({ tracks: [], groups: [] }),
    baseModules: [],
    log: (l: string[]) => logs.push(...l),
    refuse: (l: string[]) => logs.push(...l),
    reload: () => {},
  }) as never;

beforeEach(() => {
  ws = mkdtempSync(join(tmpdir(), "td-guard-"));
  dir = join(ws, "set.k7");
  writeDoc(join(dir, "project.k7x"), { project: { name: "set", tempo: 120 } });
  // A set's deck in small: the lane feeds a platter whose one input is mandatory.
  writeDoc(join(ws, "_modules", "audio", "plat", "module.k7x"), {
    plugin: { type: "internal", name: "Chorus" },
    inputs: [{ name: "in", type: "audio" }],
  });
  writeDoc(join(dir, "graph.k7x"), {
    modules: { deck: ["platter"], platter: ["OUT"] },
  });
  writeLane(dir, "deck", "track", { events: [] });
  writeNode(dir, "platter", { extends: "@workspace/audio/plat" });
  logs.length = 0;
});
afterEach(() => rmSync(ws, { recursive: true, force: true }));

describe("node delete on a node nothing feeds", () => {
  it("refuses when it is a module's only feed, and changes nothing", async () => {
    const before = wsText(join(dir, "graph.k7x"));
    await runCommand("node delete deck", host(), "cli");
    expect(logs.join(" ")).toMatch(/deck is the only signal reaching platter/);
    expect(wsText(join(dir, "graph.k7x"))).toBe(before);
    expect(existsSync(join(dir, "nodes", "deck.k7x"))).toBe(true);
  });

  it("deletes it when another source still feeds what it fed", async () => {
    writeDoc(join(dir, "graph.k7x"), {
      modules: { deck: ["platter"], spare: ["platter"], platter: ["OUT"] },
    });
    writeLane(dir, "spare", "track", { events: [] });
    await runCommand("node delete deck", host(), "cli");
    expect(existsSync(join(dir, "nodes", "deck.k7x"))).toBe(false);
  });

  it("deletes a node something feeds, since the splice rejoins the run", async () => {
    writeNode(dir, "trim", { extends: "@workspace/audio/plat" });
    writeDoc(join(dir, "graph.k7x"), {
      modules: { deck: ["trim"], trim: ["platter"], platter: ["OUT"] },
    });
    await runCommand("node delete trim", host(), "cli");
    expect(existsSync(join(dir, "nodes", "trim.k7x"))).toBe(false);
  });
});
