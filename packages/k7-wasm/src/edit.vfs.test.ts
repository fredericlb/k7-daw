// edit.vfs.test.ts — the module's edits over the Worker's Vfs: an unknown folder is `needs_files`, never an empty read.
// A reader would get wrong that the Vfs is the only state: the module keeps no copy of a file between calls.

import { beforeAll, describe, expect, it } from "vitest";
import { setVfs } from "../../commands/src/runtime/shims/current.ts";
import { Vfs } from "../../commands/src/runtime/vfs.ts";
import { K7WasmError } from "./abi.ts";
import { editBatch, editCall, editReady } from "./edit.ts";

const SET = "song/yaml/edit.setFileValue";
let vfs: Vfs;

beforeAll(async () => {
  expect(await editReady()).toBe(true);
});

function fresh(): void {
  vfs = new Vfs(false);
  setVfs(vfs);
  vfs.load("/ws/song.k7/project.k7x", { text: "tempo(120)\n", len: 11 });
  vfs.loadList("/ws/song.k7", [{ name: "project.k7x", kind: "file" }]);
}

describe("edit.call over the Vfs", () => {
  it("writes into the Vfs, which stages it", () => {
    fresh();
    editCall(SET, "/ws/song.k7/project.k7x", ["tempo"], 90);
    expect(vfs.textOf("/ws/song.k7/project.k7x")).toBe("tempo(90)\n");
    expect(vfs.touched()).toContain("/ws/song.k7/project.k7x");
  });

  it("asks for a file the Vfs does not know yet, naming it", () => {
    fresh();
    let caught: unknown;
    try {
      editCall(SET, "/ws/other.k7/project.k7x", ["tempo"], 90);
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(K7WasmError);
    expect((caught as K7WasmError).code).toBe("needs_files");
    expect((caught as K7WasmError).files.length).toBeGreaterThan(0);
  });

  it("stops a batch at the first throw and answers the calls before it", () => {
    fresh();
    const r = editBatch([
      { fn: SET, args: ["/ws/song.k7/project.k7x", ["tempo"], 80] },
      { fn: "song/nowhere.nothing", args: [] },
      { fn: SET, args: ["/ws/song.k7/project.k7x", ["tempo"], 70] },
    ]);
    expect(r.rets).toHaveLength(1);
    expect(r.thrown).toContain("no such edit function");
    expect(vfs.textOf("/ws/song.k7/project.k7x")).toContain("80");
  });
});
