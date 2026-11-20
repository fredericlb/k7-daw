// A library's SOUND files: listed as `@<library>/<path>`, resolved back to the file, and carried by a
// module's `ir` param all the way into the compiled document the engine is handed (docs/libraryFiles.md).

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { libraryFile, libraryFiles } from "./libraryFiles";
import { loadModuleRefs, loadModules } from "./loader";
import { resolveModule } from "./moduleResolve";
import { librariesFixture, writeDoc, writeNode } from "./testSupport";

let root: string;
let lib: string;
const touch = (path: string) => {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, "");
};

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "td-libfiles-"));
  lib = librariesFixture(root, "k7-core");
  touch(join(lib, "irs", "fender deluxe", "big.wav"));
  touch(join(lib, "irs", "fender deluxe", "notes.txt"));
  touch(join(lib, "samples", "kicks", "808.wav"));
});
afterEach(() => {
  rmSync(root, { recursive: true, force: true });
  delete process.env.STUDIO_HOME;
});

/** A project `name` whose lane `lane` runs into `node` and on to OUT, `node` holding `value`. */
function project(
  name: string,
  lane: string,
  node: string,
  value: Record<string, unknown>,
): string {
  const p = join(root, name);
  writeDoc(join(p, "project.k7x"), { project: { name: name.split(".")[0] } });
  writeNode(p, lane, { extends: "@meta/track", events: [] });
  writeNode(p, node, value);
  writeDoc(join(p, "graph.k7x"), {
    modules: { [lane]: [node], [node]: ["OUT"] },
  });
  return p;
}

const AMP = { plugin: { type: "internal", name: "Neural Amp Modeler" } };
const writeShared = (path: string, value: Record<string, unknown>) =>
  writeDoc(join(root, "_modules", path, "module.k7x"), value);

describe("libraryFiles", () => {
  it("lists a family's SOUND files as @<library>/<path>, and nothing else", () => {
    expect(libraryFiles("irs")).toEqual(["@k7-core/fender deluxe/big.wav"]);
    expect(libraryFiles("samples")).toEqual(["@k7-core/kicks/808.wav"]);
  });

  it("resolves a ref back to the file, in that family's folder only", () => {
    expect(libraryFile("irs", "@k7-core/fender deluxe/big.wav")).toBe(
      join(lib, "irs", "fender deluxe", "big.wav"),
    );
    // The same path under the OTHER family is not this file.
    expect(libraryFile("samples", "@k7-core/fender deluxe/big.wav")).toBe(
      join(lib, "samples", "fender deluxe", "big.wav"),
    );
  });

  it("refuses a ref that climbs out of the folder, or names no library", () => {
    expect(libraryFile("irs", "@k7-core/../modules/x.k7x")).toBeUndefined();
    expect(libraryFile("irs", "@nope/big.wav")).toBeUndefined();
    expect(libraryFile("irs", "fender deluxe/big.wav")).toBeUndefined();
  });
});

describe("a module's `ir` param", () => {
  const song = () => {
    writeShared("audio/amp", AMP);
    return project("song.k7", "gtr", "amp", {
      extends: "@workspace/audio/amp",
      params: { ir: "@k7-core/fender deluxe/big.wav" },
    });
  };

  it("resolves to the library's file, keeping the name it was written with", () => {
    const p = song();
    const amp = resolveModule(loadModules(p).amp, loadModuleRefs(p));
    expect(amp.ir).toBe(join(lib, "irs", "fender deluxe", "big.wav"));
    expect(amp.assets?.ir).toBe("@k7-core/fender deluxe/big.wav");
  });
});

describe("a module's `model` param, from a library's models/", () => {
  const song = (model: string) => {
    touch(join(root, "_modules", "audio", "amp", "assets", "own.nam"));
    writeShared("audio/amp", AMP);
    return project("song.k7", "gtr", "amp", {
      extends: "@workspace/audio/amp",
      params: { model },
    });
  };

  beforeEach(() => {
    touch(join(lib, "models", "proteus", "ts9.json"));
    touch(join(lib, "models", "proteus", "README.md"));
    touch(join(lib, "models", "amp.nam"));
  });

  it("lists a library's models — .nam and .json, nothing else", () => {
    expect(libraryFiles("models")).toEqual([
      "@k7-core/amp.nam",
      "@k7-core/proteus/ts9.json",
    ]);
  });
});

const kit = (sample: string, more: Record<string, unknown> = {}) => ({
  extends: "@workspace/instruments/drums",
  type: "rack",
  rack: { symbols: { BD: { note: 36, sample } } },
  ...more,
});

describe("a PROJECT's own samples, as @project/…", () => {
  it("lists them before the libraries', and resolves them in that project only", () => {
    const p = join(root, "demo.k7");
    touch(join(p, "samples", "kick.wav"));
    expect(libraryFiles("samples", p)).toEqual([
      "@project/kick.wav",
      "@k7-core/kicks/808.wav",
    ]);
    expect(libraryFile("samples", "@project/kick.wav", p)).toBe(
      join(p, "samples", "kick.wav"),
    );
    // With no project to be read for, `@project/` names nothing rather than guessing one.
    expect(libraryFile("samples", "@project/kick.wav")).toBeUndefined();
  });
});
