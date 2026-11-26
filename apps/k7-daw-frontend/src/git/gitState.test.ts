import { describe, expect, it } from "vitest";
import { barView, lockedBy } from "./gitState";
import type { GitState } from "./gitTypes";

const conn = (over: Partial<GitState> = {}): GitState => ({
  mode: "connected",
  branch: "main",
  remote: "https://h/r.git",
  ahead: 0,
  behind: 0,
  changed: 0,
  ...over,
});

describe("the workspace bar", () => {
  it("draws nothing without a repository", () => {
    expect(barView(null).show).toBe(false);
    expect(barView({ mode: "none" }).show).toBe(false);
  });

  it("carries the last auto-save of a connected workspace only", () => {
    const autosave = {
      branch: "k7/autosave/ada",
      at: 1_700_000_000_000,
      unsent: true,
    };
    expect(barView(conn()).autosave).toBeNull();
    expect(barView(conn({ autosave })).autosave).toEqual({
      at: 1_700_000_000_000,
      unsent: true,
    });
    expect(
      barView({ mode: "local", branch: "main", autosave }).autosave,
    ).toBeNull();
  });

  it("offers Commit over changes only", () => {
    expect(barView(conn()).commit).toBe(false);
    expect(barView(conn({ changed: 2 })).commit).toBe(true);
  });

  it("refuses a push over a branch that is behind: pull first", () => {
    const v = barView(conn({ ahead: 1, behind: 2 }));
    expect(v.push).toBe(false);
    expect(v.pushBlocked).toBe("pullFirst");
    expect(barView(conn({ ahead: 1 })).push).toBe(true);
  });

  it("refuses a pull over uncommitted changes: commit first", () => {
    const v = barView(conn({ behind: 1, changed: 3 }));
    expect(v.pull).toBe(false);
    expect(v.pullBlocked).toBe("commitFirst");
    expect(barView(conn({ behind: 1 })).pull).toBe(true);
  });

  it("offers nothing to push or pull on a local repository", () => {
    const v = barView({ mode: "local", branch: "main", changed: 1 });
    expect(v.remote).toBeNull();
    expect(v.commit).toBe(true);
    expect(v.push || v.pull).toBe(false);
  });

  it("offers only the merge while one waits, and locks the editors", () => {
    const s = conn({ changed: 4, behind: 1, ahead: 1, merge: 2 });
    const v = barView(s);
    expect(v.merge).toBe(2);
    expect(v.commit || v.push || v.pull).toBe(false);
    expect(lockedBy(s)).toBe(true);
    expect(lockedBy(conn())).toBe(false);
  });

  it("says a failed fetch", () => {
    const v = barView(
      conn({
        lastFetch: {
          at: 1_760_000_000_000,
          error: { code: "unreachable", error: "down" },
        },
      }),
    );
    expect(v.fetchError).toBe("down");
  });
});

describe("an empty remote", () => {
  const fetched = { at: 1_760_000_000_000, error: null };

  it("says nothing was pushed, and offers nothing, on a clean clone", () => {
    const v = barView(conn({ remoteEmpty: true, lastFetch: fetched }));
    expect(v.remoteEmpty).toBe(true);
    expect(v.commit || v.push || v.pull).toBe(false);
  });

  it("keeps Commit and Push on the device that has the projects", () => {
    const v = barView(
      conn({ remoteEmpty: true, changed: 3, ahead: 0, lastFetch: fetched }),
    );
    expect(v.commit).toBe(true);
    const w = barView(
      conn({ remoteEmpty: true, ahead: 2, lastFetch: fetched }),
    );
    expect(w.push).toBe(true);
  });
});

describe("a remote with no branch yet", () => {
  it("offers the first push, as the route counts the commits of HEAD", () => {
    const v = barView(conn({ ahead: 3, behind: 0 }));
    expect(v.push).toBe(true);
    expect(v.toPush).toBe(3);
    expect(v.pull).toBe(false);
  });
});
