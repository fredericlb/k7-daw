// build.test.ts — the checked-in module is one of the right shape, and the source hash follows the bytes it covers.
// A reader would get wrong that it proves freshness: `npm run lint:generated` compares the checked-in hashes to the sources.

import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  CRATES,
  INCLUDED,
  pathDeps,
  rustSourceHash,
} from "../scripts/rustSources.mjs";
import { call, wasmReady } from "./abi.ts";
import { SOURCES, WASM } from "./k7Wasm.generated.ts";
import { SONG_SOURCES, SONG_WASM } from "./k7WasmSong.generated.ts";

const EXPORTS = [
  "k7_call",
  "k7_curve_playhead",
  "k7_fs_buf",
  "k7_input",
  "k7_output_len",
  "k7_output_ptr",
  "k7_scalars_ptr",
];

describe("the WebAssembly module", () => {
  it("builds the verbs' module, with the compiler, from the same sources", () => {
    expect(SONG_SOURCES).toBe(SOURCES);
    expect(SONG_WASM.length).toBeGreaterThan(WASM.length * 5);
  });

  it("is built from the crates it hashes, every path dependency of the facade among them", () => {
    expect(pathDeps().length).toBeGreaterThan(0);
    for (const dep of pathDeps()) expect(CRATES).toContain(dep);
  });

  it("holds the compiler and its reader among the crates, and their lock", () => {
    expect(CRATES).toEqual(
      expect.arrayContaining(["packages/song/rust", "packages/k7x/rust"]),
    );
  });

  it("changes its hash when one byte of the compiler's source changes", () => {
    const root = join(import.meta.dirname, "..", "..", "..");
    const copy = mkdtempSync(join(tmpdir(), "k7-wasm-sources-"));
    try {
      for (const crate of CRATES)
        for (const part of ["Cargo.toml", "src"])
          cpSync(join(root, crate, part), join(copy, crate, part), {
            recursive: true,
          });
      for (const file of [
        ...INCLUDED,
        "packages/k7-wasm/rust/Cargo.lock",
        "packages/song/rust/Cargo.lock",
      ]) {
        mkdirSync(dirname(join(copy, file)), { recursive: true });
        cpSync(join(root, file), join(copy, file));
      }
      expect(rustSourceHash(copy)).toBe(rustSourceHash());
      const lib = join(copy, "packages/song/rust/src/lib.rs");
      writeFileSync(lib, `${readFileSync(lib, "utf8")} `);
      expect(rustSourceHash(copy)).not.toBe(rustSourceHash());
    } finally {
      rmSync(copy, { recursive: true, force: true });
    }
  });

  it("changes its hash when the table the compiler includes changes", () => {
    const root = join(import.meta.dirname, "..", "..", "..");
    const copy = mkdtempSync(join(tmpdir(), "k7-wasm-included-"));
    try {
      for (const crate of CRATES)
        for (const part of ["Cargo.toml", "src"])
          cpSync(join(root, crate, part), join(copy, crate, part), {
            recursive: true,
          });
      for (const file of [
        ...INCLUDED,
        "packages/k7-wasm/rust/Cargo.lock",
        "packages/song/rust/Cargo.lock",
      ]) {
        mkdirSync(dirname(join(copy, file)), { recursive: true });
        cpSync(join(root, file), join(copy, file));
      }
      expect(rustSourceHash(copy)).toBe(rustSourceHash());
      const table = join(copy, INCLUDED[0]);
      writeFileSync(table, `${readFileSync(table, "utf8")} `);
      expect(rustSourceHash(copy)).not.toBe(rustSourceHash());
    } finally {
      rmSync(copy, { recursive: true, force: true });
    }
  });

  it("answers the version it was built from", async () => {
    expect(await wasmReady()).toBe(true);
    expect(call<{ sources: string }>({ op: "version" })).toEqual({
      sources: SOURCES,
    });
  });

  it("exports the ABI and nothing else of ours", () => {
    const bin = Uint8Array.from(atob(WASM), (c) => c.charCodeAt(0));
    const names = WebAssembly.Module.exports(new WebAssembly.Module(bin)).map(
      (e) => e.name,
    );
    expect(names.filter((n) => n.startsWith("k7_")).sort()).toEqual(EXPORTS);
    expect(names).toContain("memory");
  });

  it("instantiates in node", async () => {
    expect(await wasmReady()).toBe(true);
  });
});
