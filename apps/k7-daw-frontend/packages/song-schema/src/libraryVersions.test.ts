import { describe, expect, it } from "vitest";
import {
  isRunnable,
  LIBRARY_LEVEL,
  parseLibraryManifest,
  pinnedVersion,
  stringifyLibraryManifest,
  versionOfDir,
} from "./libraryVersions";

describe("libraryVersions", () => {
  it("knows a version folder from any other", () => {
    expect(versionOfDir("1")).toBe(1);
    expect(versionOfDir("12")).toBe(12);
    expect(versionOfDir("draft")).toBe("draft");
    expect(versionOfDir("01")).toBeNull();
    expect(versionOfDir("modules")).toBeNull();
  });

  it("resolves a library the pins do not name to version 1", () => {
    expect(pinnedVersion({ "k7-core": 7, drums: "draft" }, "bass")).toBe(1);
    expect(pinnedVersion({ "k7-core": 7 }, "k7-core")).toBe(7);
  });

  it("round-trips a manifest and says whether this k7 runs it", () => {
    const m = { name: "k7-core", version: 3, requires: LIBRARY_LEVEL + 1 };
    expect(parseLibraryManifest(stringifyLibraryManifest(m))).toEqual(m);
    expect(isRunnable(m)).toBe(false);
    expect(isRunnable({ ...m, requires: LIBRARY_LEVEL })).toBe(true);
    expect(parseLibraryManifest('name("k7-core")\n')).toBeNull();
    expect(parseLibraryManifest('name("k7-core")\nversion(2)\n')).toEqual({
      name: "k7-core",
      version: 2,
      requires: 1,
    });
  });

  it("keeps a manifest's notes across lines", () => {
    const m = { name: "x", version: 1, requires: 1, notes: "one\ntwo" };
    expect(parseLibraryManifest(stringifyLibraryManifest(m))).toEqual(m);
  });
});
