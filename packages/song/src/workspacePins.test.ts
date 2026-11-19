// workspacePins.test.ts — a workspace's pins read from the nearest `_library-pins.k7x` above a folder.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { LIBRARY_PINS_FILE } from "@studio/song-schema/libraryVersions";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { workspacePins } from "./sharedFolders";

let ws: string;
beforeEach(() => {
  ws = mkdtempSync(join(tmpdir(), "ws-pins-"));
  mkdirSync(join(ws, "song.k7"));
});
afterEach(() => rmSync(ws, { recursive: true, force: true }));

describe("a workspace's pins", () => {
  it("reads the pins file from a folder inside the workspace", () => {
    writeFileSync(join(ws, LIBRARY_PINS_FILE), "k7-core(2)\n");
    expect(workspacePins(join(ws, "song.k7"))).toEqual({ "k7-core": 2 });
  });

  it("answers no pins for a file that does not read", () => {
    writeFileSync(join(ws, LIBRARY_PINS_FILE), "k7-core(");
    expect(workspacePins(ws)).toEqual({});
  });
});
