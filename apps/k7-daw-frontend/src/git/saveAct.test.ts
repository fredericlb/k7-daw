import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GitState, StatusEntry } from "./gitTypes";

const shown: { title?: string; message: string; variant?: string }[] = [];
vi.mock("@studio/project-shell/toast", () => ({
  showToast: (t: { title?: string; message: string; variant?: string }) => {
    shown.push(t);
    return shown.length;
  },
  updateToast: () => {},
  dismissToast: () => {},
}));

const calls: string[] = [];
let entries: StatusEntry[] = [];
let state: GitState = { mode: "none" };
let identity = { name: "Ana", email: "ana@x.org" };
let pushFails: unknown = null;
vi.mock("./gitApi", async (actual) => {
  const real = await actual<typeof import("./gitApi")>();
  return {
    ...real,
    git: {
      status: async () => ({ entries }),
      state: async () => state,
      identity: async () => identity,
      commit: async (
        _t: unknown,
        _w: string,
        message: string,
        exclude: string[],
      ) => {
        calls.push(`commit ${message} -${exclude.join(",")}`);
        return { ok: true, id: "c1" };
      },
      push: async () => {
        calls.push("push");
        if (pushFails) throw pushFails;
        return { ok: true };
      },
    },
  };
});

const { save, saveMessage } = await import("./saveAct");
const { GitError } = await import("./gitApi");

const t = { base: "/api/files", headers: () => ({}) };
const row = (over: Partial<StatusEntry> = {}): StatusEntry => ({
  path: "a.k7x",
  change: "modified",
  size: 10,
  binary: false,
  lfs: false,
  ...over,
});
const connected: GitState = {
  mode: "connected",
  branch: "main",
  remote: "git@git.example.org:me/songs.git",
  ahead: 0,
  behind: 0,
  changed: 1,
};
const run = () => save(t, "songs", { onPull: () => calls.push("pull") });

beforeEach(() => {
  shown.length = 0;
  calls.length = 0;
  entries = [row()];
  state = connected;
  identity = { name: "Ana", email: "ana@x.org" };
  pushFails = null;
});

describe("Save", () => {
  it("records the moment as its message", () => {
    expect(saveMessage(new Date(2026, 10, 23, 0, 14))).toBe(
      "Saved on 23/11/2026 00:14",
    );
  });

  it("commits everything, then pushes, and ends on Saved", async () => {
    await run();
    expect(calls[0]).toMatch(/^commit Saved on .+ -$/);
    expect(calls[1]).toBe("push");
    expect(shown.at(-1)?.title).toBe("Saved");
    expect(shown.at(-1)?.message).toContain("git.example.org/me/songs · main");
  });

  it("leaves a too-large file out and says so", async () => {
    entries = [row(), row({ path: "big.wav", size: 3e9, tooLarge: true })];
    await run();
    expect(calls[0]).toMatch(/-big\.wav$/);
    expect(shown.some((s) => s.message.includes("big.wav"))).toBe(true);
  });

  it("only commits a workspace with no remote", async () => {
    state = { mode: "local", branch: "main", ahead: 0, behind: 0, changed: 1 };
    await run();
    expect(calls).toHaveLength(1);
    expect(shown.at(-1)?.title).toBe("Saved");
  });

  it("only pushes when nothing changed and saves wait", async () => {
    entries = [];
    state = { ...connected, ahead: 2, changed: 0 };
    await run();
    expect(calls).toEqual(["push"]);
  });

  it("says there is no author and commits nothing", async () => {
    identity = { name: "", email: "" };
    await run();
    expect(calls).toEqual([]);
    expect(shown.at(-1)?.title).toBe("Not saved: no author yet");
  });

  it("keeps the save on this device when the remote moved on, and offers the Pull", async () => {
    pushFails = new GitError("behind", 409, "behind", {});
    await run();
    const last = shown.at(-1) as {
      title?: string;
      action?: { onClick: () => void };
    };
    expect(last.title).toBe("Saved on this device, not sent");
    last.action?.onClick();
    expect(calls.at(-1)).toBe("pull");
  });

  it("says a failed push with a Retry", async () => {
    pushFails = new Error("timed out");
    await run();
    expect(shown.at(-1)).toMatchObject({
      title: "Saved on this device, not sent",
      message: "timed out",
      variant: "error",
    });
  });
});
