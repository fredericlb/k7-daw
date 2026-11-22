import { describe, expect, it } from "vitest";
import { sourceNote } from "./identitySource";

describe("the source line under a prefilled commit author", () => {
  it("names each source, and the system account by the platform", () => {
    expect(sourceNote("github")).toBe("from GitHub");
    expect(sourceNote("forgejo")).toBe("from Forgejo");
    expect(sourceNote("git-config")).toBe("from your git config");
    expect(sourceNote("system", true)).toBe("from this Mac's account");
    expect(sourceNote("system", false)).toBe("from this computer's account");
  });

  it("says nothing for a typed field", () => {
    expect(sourceNote(null)).toBeUndefined();
    expect(sourceNote(undefined)).toBeUndefined();
  });
});
