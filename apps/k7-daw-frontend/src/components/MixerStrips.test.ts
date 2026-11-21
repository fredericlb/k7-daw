import { describe, expect, it } from "vitest";
import { panText } from "./MixerStrips";

describe("panText", () => {
  it("reads the middle as C, and each side with its distance", () => {
    expect(panText(0.5)).toBe("C");
    expect(panText(0.38)).toBe("L24");
    expect(panText(0.58)).toBe("R16");
    expect(panText(0)).toBe("L100");
    expect(panText(1)).toBe("R100");
  });
});
