// edit.test.ts — the crate's edits through the module over a real folder: the imports reach `node:fs`, the throws are the TypeScript's.
// A reader would get wrong that this file runs over the disk: edit.vfs.test.ts runs the same module over the Vfs.

import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { editBatch, editBytes, editCall, editReady } from "./edit.ts";

let dir: string;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), "k7-edit-"));
  mkdirSync(join(dir, "song.k7"));
  writeFileSync(join(dir, "song.k7", "project.k7x"), "tempo(120)\n");
  writeFileSync(join(dir, "song.k7", "graph.k7x"), "events([])\n");
  expect(await editReady()).toBe(true);
});
afterAll(() => rmSync(dir, { recursive: true, force: true }));

const SET = "song/yaml/edit.setFileValue";

describe("edit.call over the disk", () => {
  it("sets a value in a file the module read through the imports", () => {
    const file = join(dir, "song.k7", "project.k7x");
    editCall(SET, file, ["tempo"], 90);
    expect(readFileSync(file, "utf8")).toBe("tempo(90)\n");
  });

  it("edits two files in one batch, each read and written through the buffer", () => {
    const a = join(dir, "song.k7", "project.k7x");
    const b = join(dir, "song.k7", "graph.k7x");
    const r = editBatch([
      { fn: SET, args: [a, ["name"], "x"] },
      { fn: SET, args: [b, ["events"], ["B"]] },
    ]);
    expect(r.thrown).toBeUndefined();
    expect(r.rets).toHaveLength(2);
    expect(readFileSync(a, "utf8")).toContain("x");
    expect(readFileSync(b, "utf8")).toContain("B");
  });

  it("throws the message of the function that threw, as an Error", () => {
    expect(() => editCall("song/nowhere.nothing")).toThrow(
      "song/nowhere.nothing: no such edit function",
    );
  });

  it("writes the bytes of a binary argument", () => {
    const file = join(dir, "song.k7", "blob.bin");
    editCall(
      "song/yaml/workspaceFile.wsWrite",
      file,
      editBytes(new Uint8Array([0, 1, 2, 250, 255])),
    );
    expect([...readFileSync(file)]).toEqual([0, 1, 2, 250, 255]);
  });
});
