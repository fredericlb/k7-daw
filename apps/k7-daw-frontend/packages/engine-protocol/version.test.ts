// The wire's version is one number written three times — here, in the engine server's version.rs and in
// the supervisor's wire.rs — so this keeps them equal without building either.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  ENGINE_PROTOCOL,
  engineOutdated,
  engineProtocolOf,
  MIN_ENGINE_PROTOCOL,
} from "./version.ts";

const apps = join(dirname(fileURLToPath(import.meta.url)), "../../..");

describe("ENGINE_PROTOCOL", () => {
  for (const [file, re] of [
    [
      "k7-daw-backend/packages/k7-runtime-server/src/wire/version.rs",
      /pub const ENGINE_PROTOCOL: u32 = (\d+);/,
    ],
    [
      "k7-daw-backend/packages/k7-supervisor/src/wire.rs",
      /pub const ENGINE_PROTOCOL: i64 = (\d+);/,
    ],
  ] as const)
    it(`is the version ${file} states`, () => {
      const m = re.exec(readFileSync(join(apps, file), "utf8"));
      expect(Number(m?.[1])).toBe(ENGINE_PROTOCOL);
    });

  it("is never below the minimum it serves", () => {
    expect(ENGINE_PROTOCOL).toBeGreaterThanOrEqual(MIN_ENGINE_PROTOCOL);
  });
});

describe("engineProtocolOf", () => {
  it("reads a number or a header's digits", () => {
    expect(engineProtocolOf(3)).toBe(3);
    expect(engineProtocolOf("2")).toBe(2);
  });

  it("reads anything else as 1, the version before the field", () => {
    for (const v of [undefined, null, "", "x", 0, -2, 1.5, "1.5", [2]])
      expect(engineProtocolOf(v)).toBe(1);
  });
});

describe("engineOutdated", () => {
  it("compares against the minimum given", () => {
    expect(engineOutdated(undefined, 2)).toBe(true);
    expect(engineOutdated("2", 2)).toBe(false);
    expect(engineOutdated(1)).toBe(false);
  });
});
