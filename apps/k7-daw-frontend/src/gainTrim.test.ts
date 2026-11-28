// The TypeScript half of gainTrim is the call; the sign and the clamp are Rust's (song reads_ports_misc.rs).

import { MASTER_MAX_DB, MASTER_MIN_DB } from "@studio/k7-wasm/consts";
import { describe, expect, it } from "vitest";
import { fixedMaster, TARGET_DB, trimFor } from "./gainTrim";

describe("gainTrim", () => {
  it("reaches the module with the target and the field the page holds", () => {
    expect(trimFor(0)).toBe(TARGET_DB);
    expect(trimFor(3)).toBe(-9);
    expect(fixedMaster(3, 6)).toBe(-9);
    expect(fixedMaster(MASTER_MIN_DB + 2, 0)).toBe(MASTER_MIN_DB);
    expect(fixedMaster(MASTER_MAX_DB, 0)).toBeLessThan(MASTER_MAX_DB);
  });
});
