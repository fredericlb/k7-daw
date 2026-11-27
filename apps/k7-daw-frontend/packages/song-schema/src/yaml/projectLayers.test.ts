// `layers` in project.k7x: an empty list, which most songs carry, is read by nothing; naming a lane
// means picture lanes their open did not migrate, refused with what replaced it (docs/formatLevel.md).

import { validateFile } from "@studio/k7-wasm/validate";
import { describe, expect, it } from "vitest";

const root = (layers: string) =>
  validateFile(
    "project.k7x",
    `project({ name: "s", tempo: 120, layers: ${layers} })\n`,
  );

describe("project.layers", () => {
  it("is accepted empty, and read by nothing", () => {
    expect(root("[]").ok).toBe(true);
  });

  it("is refused naming a lane, saying what draws the picture now", () => {
    const r = root('["main"]');
    expect(r.ok).toBe(false);
    expect(r.issues[0]?.message).toMatch(/trackOrder/);
  });
});
