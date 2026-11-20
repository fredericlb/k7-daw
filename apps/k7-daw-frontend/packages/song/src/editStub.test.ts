// editStub.test.ts — a stubbed export reaches the crate with the process's shared folders, and an answer's tagged values come back.
// A reader would get wrong that the crate reads no environment: an overlay the test sets must travel in the call.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { editEnv, stubBatch } from "./editStub";
import { injectRoots } from "./injectedRoots";
import { sharedRoots } from "./moduleFork";
import { formatAudioDevice } from "./preferences";
import { setLibraryOverlays } from "./sharedFolders";

afterEach(() => {
  setLibraryOverlays([]);
  injectRoots(null);
});

describe("the stubs' door", () => {
  it("carries an admin's overlay in the environment, and the crate's roots name it", () => {
    const root = mkdtempSync(join(tmpdir(), "k7 stub é-"));
    const lib = join(root, "draft");
    mkdirSync(join(lib, "modules"), { recursive: true });
    const project = join(root, "ws", "p");
    mkdirSync(project, { recursive: true });
    writeFileSync(join(project, "project.k7x"), 'project({ name: "p" })\n');
    setLibraryOverlays([{ name: "mine", root: lib }]);
    expect(editEnv(project).overlays).toEqual([["mine", lib]]);
    const roots = sharedRoots(project).map((r) => r.root);
    expect(roots).toContain(join(lib, "modules"));
  });

  it("answers an undefined argument as the TypeScript saw it", () => {
    expect(formatAudioDevice(undefined)).toBe("(system default)");
    expect(formatAudioDevice({ name: "A", type: "ASIO" })).toBe("A (ASIO)");
  });

  it("runs a batch in order and stops at the first throw, leaving the rest unwritten", () => {
    const root = mkdtempSync(join(tmpdir(), "k7 batch é-"));
    const [a, b, c] = ["a", "b", "c"].map((n) => join(root, `${n}.k7x`));
    writeFileSync(join(root, "plain"), "x");
    const set = (f: string) => [
      "song/yaml/edit.setFileValue",
      f,
      ["tempo"],
      120,
    ];
    expect(() =>
      stubBatch([set(a), set(join(root, "plain", "no.k7x")), set(b)] as never),
    ).toThrow();
    expect(readFileSync(a, "utf8")).toContain("tempo");
    expect(existsSync(b)).toBe(false);
    const rets = stubBatch([set(b), set(c)] as never);
    expect(rets).toEqual([undefined, undefined]);
    expect(existsSync(c)).toBe(true);
  });

  it("names the roots a host handed in, never the folder next to the code, which in the Worker is a URL", () => {
    const roots = {
      librariesHome: "/libs",
      accountHome: "/acct",
      libraries: [],
      installed: {},
    };
    injectRoots({ ...roots, meta: "/virtual/meta" });
    expect(editEnv().metaRoot).toBe("/virtual/meta");
    injectRoots({ ...roots, meta: null });
    expect(editEnv().metaRoot).toBe("");
  });
});
