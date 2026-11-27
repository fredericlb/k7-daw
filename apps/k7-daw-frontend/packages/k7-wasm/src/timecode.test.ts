// timecode.test.ts — the timecode facade reaches the module: a read, a write, a rounding, and a refusal as its own error code.
// The rules (every field, tick and signature case) are song-schema's: src/tests/timecode_tests.rs.

import { describe, expect, it } from "vitest";
import { K7WasmError } from "./abi.ts";
import {
  beatsToDuration,
  beatsToPosition,
  parseDuration,
  parsePosition,
  roundBeat,
  TIMECODE,
} from "./timecode.ts";

describe("the timecode facade", () => {
  it("reads and writes positions and durations through the module", () => {
    expect(parsePosition("2:1:1:1", 4, 4)).toBe(4);
    expect(parseDuration("0:1", 4, 4)).toBe(1);
    expect(beatsToPosition(0.375, 4, 4)).toBe("1:1:1:2.120");
    expect(beatsToDuration(1, 4, 4)).toBe("0:1");
    expect(roundBeat(0.1 + 0.2)).toBe(0.3);
    expect(TIMECODE.test("1:1.120")).toBe(true);
  });

  it("throws a bad timecode as the module's own code", () => {
    try {
      parseDuration("a:b", 4, 4);
      expect.unreachable();
    } catch (e) {
      expect(e).toBeInstanceOf(K7WasmError);
      expect((e as K7WasmError).code).toBe("bad_timecode");
    }
  });
});
