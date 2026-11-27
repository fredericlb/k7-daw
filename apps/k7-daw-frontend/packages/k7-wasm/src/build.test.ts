// build.test.ts — the checked-in module is one of the right shape, and the source hash follows the bytes it covers.
// A reader would get wrong that it proves freshness: `npm run lint:generated` compares the checked-in hashes to the sources.

import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
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
import { BYTES, SOURCES } from "./k7Wasm.generated.ts";

const EXPORTS = [
  "k7_call",
  "k7_cells_len",
  "k7_cells_ptr",
  "k7_curve_playhead",
  "k7_input",
  "k7_output_len",
  "k7_output_ptr",
  "k7_round_beat",
  "k7_scalars_ptr",
];

const MODULE = new URL("./k7Wasm.wasm", import.meta.url);

describe("the WebAssembly module", () => {
  it("is the file the hashes were stamped for", () => {
    expect(statSync(MODULE).size).toBe(BYTES);
  });

  it("imports nothing from its host", () => {
    const mod = new WebAssembly.Module(readFileSync(MODULE));
    expect(WebAssembly.Module.imports(mod)).toEqual([]);
  });

  it("is built from the crates it hashes, every path dependency of the facade among them", () => {
    expect(pathDeps().length).toBeGreaterThan(0);
    for (const dep of pathDeps()) expect(CRATES).toContain(dep);
  });

  it("holds the compiler and its reader among the crates, and their lock", () => {
    expect(CRATES).toEqual(
      expect.arrayContaining(["apps/k7-daw-wasm/song", "apps/k7-daw-wasm/k7x"]),
    );
  });

  it("changes its hash when one byte of the compiler's source changes", () => {
    const root = join(import.meta.dirname, "..", "..", "..", "..", "..");
    const copy = mkdtempSync(join(tmpdir(), "k7-wasm-sources-"));
    try {
      for (const crate of CRATES)
        for (const part of ["Cargo.toml", "src"])
          cpSync(join(root, crate, part), join(copy, crate, part), {
            recursive: true,
          });
      for (const file of [
        ...INCLUDED,
        "apps/k7-daw-wasm/k7-wasm/Cargo.lock",
        "apps/k7-daw-wasm/song/Cargo.lock",
      ]) {
        mkdirSync(dirname(join(copy, file)), { recursive: true });
        cpSync(join(root, file), join(copy, file));
      }
      expect(rustSourceHash(copy)).toBe(rustSourceHash());
      const lib = join(copy, "apps/k7-daw-wasm/song/src/lib.rs");
      writeFileSync(lib, `${readFileSync(lib, "utf8")} `);
      expect(rustSourceHash(copy)).not.toBe(rustSourceHash());
    } finally {
      rmSync(copy, { recursive: true, force: true });
    }
  });

  it("changes its hash when the table the compiler includes changes", () => {
    const root = join(import.meta.dirname, "..", "..", "..", "..", "..");
    const copy = mkdtempSync(join(tmpdir(), "k7-wasm-included-"));
    try {
      for (const crate of CRATES)
        for (const part of ["Cargo.toml", "src"])
          cpSync(join(root, crate, part), join(copy, crate, part), {
            recursive: true,
          });
      for (const file of [
        ...INCLUDED,
        "apps/k7-daw-wasm/k7-wasm/Cargo.lock",
        "apps/k7-daw-wasm/song/Cargo.lock",
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
    const bin = readFileSync(MODULE);
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
