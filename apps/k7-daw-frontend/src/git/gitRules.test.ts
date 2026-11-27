// What the page asks Rust about git, end to end through the module: one call of each kind, whose rules the crate's tests pin.

import { describe, expect, it } from "vitest";
import { commitView } from "./commitState";
import { isWebAddress, sshHost } from "./connectState";
import { nameFromUrl } from "./gitPageTabs";
import { barView, lockedBy, menuView, remoteName } from "./gitState";
import type { GitState } from "./gitTypes";

const conn = (over: Partial<GitState> = {}): GitState => ({
  mode: "connected",
  branch: "main",
  remote: "git@github.com:o/r.git",
  ahead: 1,
  behind: 2,
  changed: 0,
  ...over,
});

describe("the workspace bar, read off the state by Rust", () => {
  it("offers Pull first over a branch that is behind, and locks the editors only for a merge", () => {
    const v = barView(conn());
    expect(v.push).toBe(false);
    expect(v.pushBlocked).toBe("pullFirst");
    expect(v.toPull).toBe(2);
    expect(lockedBy(conn({ merge: 2 }))).toBe(true);
    expect(lockedBy(conn())).toBe(false);
    expect(barView(null).show).toBe(false);
  });

  it("names the remote as a person reads it", () => {
    expect(menuView(conn())).toEqual({
      connected: true,
      remote: "github.com/o/r",
    });
    expect(menuView({ mode: "local" })).toEqual({
      connected: false,
      remote: null,
    });
    expect(remoteName("ssh://git@host:2222/band/set.git")).toBe(
      "host/band/set",
    );
  });
});

describe("a save and an address", () => {
  it("leaves a too-large file out of the commit", () => {
    const big = {
      path: "big.wav",
      change: "added",
      size: 3e9,
      binary: true,
      lfs: false,
      tooLarge: true,
    } as const;
    const v = commitView([{ ...big, path: "a.k7x", tooLarge: false }, big]);
    expect(v.sent.map((e) => e.path)).toEqual(["a.k7x"]);
    expect(v.leftOut.map((e) => e.path)).toEqual(["big.wav"]);
  });

  it("reads an address's host, refuses a web one and names the clone", () => {
    expect(sshHost("ssh://git@h.org:2222/me/r.git")).toEqual({
      host: "h.org",
      port: 2222,
    });
    expect(isWebAddress(" HTTPS://h.org/r.git")).toBe(true);
    expect(nameFromUrl("git@h:me/songs.git")).toBe("songs");
  });
});
