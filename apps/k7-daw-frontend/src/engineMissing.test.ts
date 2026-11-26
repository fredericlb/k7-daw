import { describe, expect, it } from "vitest";
import { missingKey, missingOf } from "./engineMissing";

// What each wording means is song-schema's (engine/warnings.rs); this pins the page's key.
describe("missingOf", () => {
  it("reads a refused file, a missing one and an absent plugin by Rust's rules", () => {
    expect(missingOf("clip source not found/invalid: /a/b/take.wav")).toEqual({
      what: "clip",
      name: "take.wav",
      why: "not-found",
    });
    expect(missingOf("audio device not found: Focusrite")).toBeNull();
  });

  it("keys the same file named twice as one", () => {
    const a = missingOf("clip source not found/invalid: /a/take.wav");
    const b = missingOf("clip source not found/invalid: /b/take.wav");
    expect(a && b && missingKey(a) === missingKey(b)).toBe(true);
  });
});
