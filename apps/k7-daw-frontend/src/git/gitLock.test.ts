import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { refusedWrite, setWriteGate } from "@studio/project-shell/fileApi";
import { afterEach, describe, expect, it, vi } from "vitest";
import { GitError } from "./gitApi";
import { afterRead, installGates, mergeGate } from "./gitLock";
import type { GitState } from "./gitTypes";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

afterEach(() => setWriteGate(null));

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

describe("the merge's gate", () => {
  const write = { kind: "file", method: "PUT", path: "/file" } as never;
  const read = { kind: "file", method: "GET", path: "/file" } as never;

  it("refuses a write and lets a read pass", () => {
    expect(mergeGate(write)).toBe("git.lockedByMerge");
    expect(mergeGate(read)).toBeNull();
  });

  it("installs the gate while a merge waits and takes it off", () => {
    const off = installGates(true);
    expect(refusedWrite({ kind: "command", line: "track new x" })).toBe(
      "git.lockedByMerge",
    );
    off();
    expect(refusedWrite({ kind: "command", line: "track new x" })).toBeNull();
  });

  it("installs nothing when nothing locks", () => {
    installGates(false);
    expect(refusedWrite({ kind: "command", line: "track new x" })).toBeNull();
  });

  it("is installed by App's own effect", () => {
    const app = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), "..", "App.tsx"),
      "utf8",
    );
    expect(app).toContain("installGates(merging)");
  });
});
