import { describe, expect, it } from "vitest";
import {
  arrange,
  canPick,
  GITHUB_SCOPES,
  type GithubRepo,
  hasRepoScope,
  pushedAgo,
  signedIn,
  signInAfter,
  startingSignIn,
} from "./githubState";

const code = {
  userCode: "ABCD-1234",
  verificationUri: "https://github.com/login/device",
  interval: 5,
  expiresIn: 900,
};
const waiting = signInAfter(startingSignIn, { type: "started", code });

describe("the device-flow screen", () => {
  it("shows the code once started and keeps polling through pending and slow_down", () => {
    expect(waiting).toMatchObject({
      phase: "waiting",
      code: "ABCD-1234",
      interval: 5,
    });
    const pending = signInAfter(waiting, {
      type: "polled",
      answer: { state: "pending" },
    });
    expect(pending).toEqual(waiting);
    const slow = signInAfter(waiting, {
      type: "polled",
      answer: { state: "slow_down", interval: 10 },
    });
    expect(slow).toMatchObject({ phase: "waiting", interval: 10 });
  });

  it("ends on expired, denied, a failure, or the login, and Try again starts over", () => {
    const end = (state: "expired" | "denied") =>
      signInAfter(waiting, { type: "polled", answer: { state } }).phase;
    expect([end("expired"), end("denied")]).toEqual(["expired", "denied"]);
    expect(
      signInAfter(waiting, { type: "failed", message: "offline" }),
    ).toEqual({
      phase: "failed",
      message: "offline",
    });
    expect(
      signInAfter(waiting, {
        type: "polled",
        answer: { state: "ok", login: "octo", keyNote: "n" },
      }),
    ).toEqual({ phase: "done", login: "octo", keyNote: "n" });
    expect(signInAfter({ phase: "denied" }, { type: "again" })).toEqual(
      startingSignIn,
    );
  });

  it("ignores an answer that arrives after the screen left waiting", () => {
    const denied = signInAfter(waiting, {
      type: "polled",
      answer: { state: "denied" },
    });
    expect(
      signInAfter(denied, {
        type: "polled",
        answer: { state: "ok", login: "x" },
      }),
    ).toBe(denied);
  });
});

describe("the status", () => {
  it("is signed in only where the build offers GitHub and a login exists, and asks repo apart", () => {
    expect(signedIn({ available: true, login: "o", scopes: [] })).toBe(true);
    expect(signedIn({ available: false, login: "o", scopes: [] })).toBe(false);
    expect(signedIn(null)).toBe(false);
    expect(
      hasRepoScope({ available: true, scopes: ["read:user", "repo"] }),
    ).toBe(true);
    expect(hasRepoScope({ available: true, scopes: ["read:user"] })).toBe(
      false,
    );
  });

  it("lets Connect show GitHub's picker only to a person signed in with repo", () => {
    const granted = ["read:user", "write:public_key", "repo"];
    expect(canPick({ available: true, login: "o", scopes: granted })).toBe(
      true,
    );
    // What real GitHub grants a sign-in that did not ask for repo.
    expect(
      canPick({
        available: true,
        login: "o",
        scopes: ["read:user", "write:public_key"],
      }),
    ).toBe(false);
    expect(canPick({ available: true, scopes: [] })).toBe(false);
    expect(GITHUB_SCOPES).toEqual(granted);
  });
});

const repo = (fullName: string, pushedAt: string, k7 = false): GithubRepo => ({
  fullName,
  sshUrl: `git@github.com:${fullName}.git`,
  private: true,
  pushedAt,
  k7,
});

describe("the picker's list", () => {
  const all = [
    repo("o/old", "2026-01-01T00:00:00Z"),
    repo("o/song", "2026-03-01T00:00:00Z", true),
    repo("o/new", "2026-05-01T00:00:00Z"),
    repo("o/band", "2026-02-01T00:00:00Z", true),
  ];

  it("puts k7 workspaces under their own heading, each list most recently pushed first", () => {
    const { k7, others } = arrange(all, "");
    expect(k7.map((r) => r.fullName)).toEqual(["o/song", "o/band"]);
    expect(others.map((r) => r.fullName)).toEqual(["o/new", "o/old"]);
  });

  it("filters on the full name without case, across both lists", () => {
    const { k7, others } = arrange(all, " SO");
    expect(k7.map((r) => r.fullName)).toEqual(["o/song"]);
    expect(others).toEqual([]);
  });
});

describe("how long ago", () => {
  const now = Date.parse("2026-06-01T12:00:00Z");
  const ago = (iso: string) => pushedAgo(iso, now);
  it("takes the largest whole unit", () => {
    expect(ago("2026-06-01T11:59:50Z")).toEqual({ unit: "now", n: 0 });
    expect(ago("2026-06-01T11:15:00Z")).toEqual({ unit: "minutes", n: 45 });
    expect(ago("2026-06-01T07:00:00Z")).toEqual({ unit: "hours", n: 5 });
    expect(ago("2026-05-29T12:00:00Z")).toEqual({ unit: "days", n: 3 });
    expect(ago("2026-03-01T12:00:00Z")).toEqual({ unit: "months", n: 3 });
    expect(ago("2024-06-01T12:00:00Z")).toEqual({ unit: "years", n: 2 });
  });
});
