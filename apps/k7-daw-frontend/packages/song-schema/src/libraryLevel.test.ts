// The supervisor reports the library level its machine runs (`LIBRARY_LEVEL` in its registry part), and the editor
// reads it against LIBRARY_LEVEL: the two are one number written twice, so this keeps them equal
// without building the engine. See ../../../../../docs/library-versions-plan.md.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { LIBRARY_LEVEL } from "./libraryVersions";

const supervisor = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../k7-daw-backend/packages/k7-supervisor/src/parts/registry/state.rs",
);

describe("LIBRARY_LEVEL", () => {
  it("is the level the supervisor reports", () => {
    const m = /const LIBRARY_LEVEL: i64 = (\d+);/.exec(
      readFileSync(supervisor, "utf8"),
    );
    expect(Number(m?.[1])).toBe(LIBRARY_LEVEL);
  });
});
