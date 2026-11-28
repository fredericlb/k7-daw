// The wiring of the LIBRARIES rows: the collator order every list is sent in, and Rust's rows and path rules back.
// The rows' shape and the path rules are tested in song-edit (apps/k7-daw-wasm/song-edit/src/library_tree/tests.rs).

import type { FileNode, ModuleLibrary } from "@studio/web-protocol";
import { describe, expect, it } from "vitest";
import {
  fileFolderOf,
  isChainFile,
  isLibraryPath,
  libraryModuleRef,
  libraryOfPath,
  libraryTree,
  readOnlyPaths,
  rootOf,
} from "./libraryTree";

const dir = (name: string, path: string, children: FileNode[]): FileNode => ({
  name,
  path,
  dir: true,
  children,
});
const file = (name: string, path: string): FileNode => ({
  name,
  path,
  dir: false,
  kind: "text",
});

const workspace = (nodes: FileNode[]): ModuleLibrary => ({
  ns: "workspace",
  path: "_modules",
  exists: true,
  nodes,
});
const core = (nodes: FileNode[] = []): ModuleLibrary => ({
  ns: "k7-core",
  path: "_libraries/k7-core/modules",
  exists: true,
  nodes,
});

describe("libraryTree", () => {
  // The service answers in code points, capitals first; a person reads `drive` before `Phaser`, folders above files.
  it("sorts every level's rows for people, folders first, before Rust shapes them", () => {
    const at = "_modules/audio";
    const level = workspace([
      dir("audio", at, [
        file("README.k7x", `${at}/README.k7x`),
        dir("Phaser", `${at}/Phaser`, []),
        dir("drive", `${at}/drive`, []),
      ]),
    ]);
    const audioRow = libraryTree([level])[0].children?.[0];
    expect(audioRow?.children?.map((n) => n.name)).toEqual([
      "drive",
      "Phaser",
      "README.k7x",
    ]);
  });

  it("sorts a library's chains by their path's segments, and a folder chain's nodes", () => {
    const rows = libraryTree(
      [core()],
      [
        { ref: "@k7-core/process/normalize", path: "p/n", nodes: ["b", "a"] },
        { ref: "@k7-core/live/vocals", path: "p/v.k7x" },
      ],
    )[0].children?.[1].children;
    expect(rows?.map((c) => c.name)).toEqual([
      "live/vocals",
      "process/normalize",
    ]);
    expect(rows?.[1].children?.map((n) => n.name)).toEqual(["a", "b"]);
    expect(isChainFile("p/v.k7x")).toBe(true);
  });

  it("answers the path rules and the roots through the facade", () => {
    const engine: ModuleLibrary = {
      ns: "engine",
      path: "",
      exists: true,
      readOnly: true,
      nodes: [dir("audio", "@engine/audio", [file("nam", "@engine/nam")])],
    };
    const levels = [engine, workspace([]), core()];
    expect(readOnlyPaths(levels)).toEqual(
      new Set(["@engine", "@engine/audio", "@engine/nam"]),
    );
    expect(rootOf(levels, "_modules/audio")?.ns).toBe("workspace");
    expect(rootOf(levels, "@engine/audio")).toBeNull();
    const module = "_libraries/k7-core/modules/audio/reverb/module.k7x";
    expect(libraryModuleRef(module)).toBe("@k7-core/audio/reverb");
    expect(libraryOfPath(module)).toBe("k7-core");
    expect(isLibraryPath(module)).toBe(true);
    expect(fileFolderOf("_libraries/k7-core/irs/fender")).toBe("irs");
  });
});
