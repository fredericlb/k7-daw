import { describe, expect, it } from "vitest";
import {
  commitView,
  formatSize,
  identityReady,
  identityToSave,
  stepRows,
} from "./commitState";
import type { StatusEntry } from "./gitTypes";

const row = (over: Partial<StatusEntry> = {}): StatusEntry => ({
  path: "a.k7x",
  change: "modified",
  size: 10,
  binary: false,
  lfs: false,
  ...over,
});

describe("the commit dialog", () => {
  it("refuses with nothing changed", () => {
    expect(commitView([], "m").block).toBe("noChanges");
  });

  it("refuses an empty message", () => {
    expect(commitView([row()], "  ").block).toBe("noMessage");
    expect(commitView([row()], "a take").block).toBeNull();
  });

  it("leaves a too-large file out, names it, and does not block on it", () => {
    const big = row({ path: "big.wav", size: 3e9, tooLarge: true });
    const v = commitView([row(), big], "m");
    expect(v.block).toBeNull();
    expect(v.leftOut.map((e) => e.path)).toEqual(["big.wav"]);
  });

  it("has nothing to commit when only too-large files remain", () => {
    const big = row({ path: "big.wav", size: 3e9, tooLarge: true });
    expect(commitView([big], "m").block).toBe("noChanges");
  });

  it("counts the large files that go through LFS", () => {
    expect(
      commitView([row({ lfs: true }), row(), row({ lfs: true })], "m").lfs,
    ).toBe(2);
  });

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

describe("the identity of a commit", () => {
  const acct = { name: "ana", email: "ana@x.org" };
  it("is signed with what the route prefilled, and stored only once edited", () => {
    expect(identityReady(acct)).toBe(true);
    expect(identityToSave(acct, { ...acct })).toBeNull();
    expect(identityToSave(acct, { ...acct, name: "Ana B" })).toEqual({
      name: "Ana B",
      email: "ana@x.org",
    });
    expect(identityToSave(null, acct)).toEqual(acct);
  });
});

describe("the step list of Commit & Push", () => {
  const both = { commit: true, push: true };
  const states = (rows: ReturnType<typeof stepRows>) =>
    rows.map((r) => r.state);

  it("runs commit then push in order", () => {
    expect(states(stepRows({ kind: "running", step: "commit" }, both))).toEqual(
      ["active", "pending"],
    );
    expect(states(stepRows({ kind: "running", step: "push" }, both))).toEqual([
      "done",
      "active",
    ]);
    expect(states(stepRows({ kind: "done" }, both))).toEqual(["done", "done"]);
  });

  it("marks the push as a warning when the remote is ahead", () => {
    expect(states(stepRows({ kind: "pullFirst" }, both))).toEqual([
      "done",
      "warn",
    ]);
  });

  it("keeps the sentence of a failed step", () => {
    const rows = stepRows({ kind: "failed", step: "push", error: "no" }, both);
    expect(rows[1]).toEqual({ id: "push", state: "error", detail: "no" });
  });

  it("drops the commit row for a push alone and the push row without a remote", () => {
    expect(
      stepRows({ kind: "ready" }, { commit: false, push: true }).map(
        (r) => r.id,
      ),
    ).toEqual(["push"]);
    expect(
      stepRows({ kind: "ready" }, { commit: true, push: false }).map(
        (r) => r.id,
      ),
    ).toEqual(["commit"]);
  });
});
