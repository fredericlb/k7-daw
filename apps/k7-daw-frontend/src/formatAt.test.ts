// formatAt: minutes and seconds, the hour only when there is one, never negative.

import { describe, expect, it } from "vitest";
import { formatAt } from "./formatAt";

describe("formatAt", () => {
  it("writes minutes and seconds, and the hour only when there is one", () => {
    expect(formatAt(0)).toBe("0:00");
    expect(formatAt(83.7)).toBe("1:23");
    expect(formatAt(3723)).toBe("1:02:03");
  });

  it("floors and clamps", () => {
    expect(formatAt(119.9)).toBe("1:59");
    expect(formatAt(-2)).toBe("0:00");
  });
});
