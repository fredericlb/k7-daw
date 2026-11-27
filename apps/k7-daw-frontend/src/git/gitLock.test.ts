import { describe, expect, it, vi } from "vitest";
import { GitError } from "./gitApi";
import { afterRead } from "./gitLock";
import type { GitState } from "./gitTypes";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

const merging = { mode: "connected", merge: 1 } as GitState;

describe("the merge's lock fails closed", () => {
  it("keeps the last state over a failed read", () => {
    expect(afterRead(merging, { error: new Error("down") })).toBe(merging);
    expect(afterRead(merging, { error: new GitError("x", 500, "", {}) })).toBe(
      merging,
    );
  });

  it("clears only over a missing route", () => {
    expect(
      afterRead(merging, { error: new GitError("x", 501, "", {}) }),
    ).toBeNull();
    expect(
      afterRead(merging, { error: new GitError("x", 404, "", {}) }),
    ).toBeNull();
  });

  it("takes a good answer as it comes", () => {
    const free = { mode: "connected" } as GitState;
    expect(afterRead(merging, { state: free })).toBe(free);
  });
});
