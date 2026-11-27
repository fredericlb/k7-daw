import { describe, expect, it } from "vitest";
import { formatSize, identityReady } from "./commitState";

describe("who signs a save", () => {
  it("needs a name and an email to sign", () => {
    expect(identityReady(null)).toBe(false);
    expect(identityReady({ name: "Ana", email: " " })).toBe(false);
    expect(identityReady({ name: "Ana", email: "a@x.org" })).toBe(true);
  });
});

describe("a size", () => {
  it("reads in decimal units", () => {
    expect(formatSize(800)).toBe("800 B");
    expect(formatSize(1_500_000)).toBe("1.5 MB");
    expect(formatSize(250_000_000)).toBe("250 MB");
  });
});
