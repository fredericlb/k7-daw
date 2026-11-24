import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BarBody } from "./GitBar";
import { announceGitMoved, onGitMoved } from "./gitBus";
import { gitMenuEntries, menuView, remoteName } from "./gitState";
import type { GitState } from "./gitTypes";

const connected: GitState = {
  mode: "connected",
  branch: "main",
  remote: "git@github.com:fredericlb/mahrave_part_2.git",
  ahead: 1,
  behind: 2,
  changed: 93,
};

describe("a remote as a person reads it", () => {
  it("keeps the host and the repository, never the user or the suffix", () => {
    expect(remoteName("git@github.com:fredericlb/mahrave_part_2.git")).toBe(
      "github.com/fredericlb/mahrave_part_2",
    );
    expect(remoteName("ssh://git@host.example:2222/o/r.git")).toBe(
      "host.example/o/r",
    );
  });
});

describe("the workspace menu's git lines", () => {
  it("offers Sync settings on a workspace with no remote", () => {
    expect(gitMenuEntries(menuView({ mode: "none" }))).toEqual(["sync"]);
    expect(gitMenuEntries(menuView(null))).toEqual(["sync"]);
  });

  it("says the remote and offers Sync settings once connected", () => {
    const v = menuView(connected);
    expect(v).toEqual({
      connected: true,
      remote: "github.com/fredericlb/mahrave_part_2",
    });
    expect(gitMenuEntries(v)).toEqual(["connected", "sync"]);
  });

  it("never offers the demos a remote, and still says one they already have", () => {
    expect(gitMenuEntries(menuView({ mode: "none" }), true)).toEqual([]);
    expect(gitMenuEntries(menuView(connected), true)).toEqual(["connected"]);
  });
});

describe("the home row's badge", () => {
  const row = (s: GitState | null) =>
    renderToStaticMarkup(
      <BarBody
        row
        state={s}
        onAct={() => {}}
        onCommit={() => {}}
        onMerge={() => {}}
      />,
    );

  it("draws nothing for a workspace outside git", () => {
    expect(row({ mode: "none" })).toBe("");
  });

  it("shows the branch, the counts and the actions in the bar's wording", () => {
    const out = row(connected);
    expect(out).toContain("main");
    expect(out).toContain("2 to update");
    expect(out).toContain("1 to push");
    expect(out).toContain("93 files changed");
    expect(out).toContain(">Save<");
  });
});

describe("the git bus", () => {
  it("wakes the listeners of one workspace only, until they stop", () => {
    let a = 0;
    let b = 0;
    const stop = onGitMoved("A", () => a++);
    const other = onGitMoved("B", () => b++);
    announceGitMoved("A");
    stop();
    announceGitMoved("A");
    other();
    announceGitMoved("B");
    expect([a, b]).toEqual([1, 0]);
  });
});
