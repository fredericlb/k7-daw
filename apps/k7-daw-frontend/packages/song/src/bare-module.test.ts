import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ModuleSchema } from "@studio/song-schema/yaml/moduleSchema";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  insertChainStepAfter,
  replaceChainStep,
  writeNode,
} from "./testSupport";

// A module needn't extend anything, so the schema, the ref-check and the build loop must all allow it.

const here = dirname(fileURLToPath(import.meta.url)); // apps/k7-daw-frontend/packages/song/src
const DEMOS = join(here, "..", "..", "..", "..", "..", "demos");
const SONG = join(DEMOS, "synth-demo.k7");

// Found by the PREFIX "synth", since the song names a NODE there, and a miss throws loudly.

describe("bare (extends-less) modules", () => {
  let ws: string;
  let dir: string;
  beforeEach(() => {
    ws = mkdtempSync(join(tmpdir(), "td-bare-"));
    dir = join(ws, "synth-demo.k7");
    cpSync(SONG, dir, { recursive: true });
    // The Examples workspace's own shared library, beside every demo project.
    cpSync(join(DEMOS, "_modules"), join(ws, "_modules"), { recursive: true });
  });
  afterEach(() => rmSync(ws, { recursive: true, force: true }));

  it("makes extends optional, and has no `name` field at all", () => {
    // A bare module has to VALIDATE, and `name` must not exist since the FOLDER names it.
    expect(ModuleSchema.safeParse({}).success).toBe(true);
    expect(
      ModuleSchema.safeParse({ extends: "instruments/fourosc" }).success,
    ).toBe(true);
    expect(ModuleSchema.safeParse({ name: "kit" }).success).toBe(false);
  });
});
