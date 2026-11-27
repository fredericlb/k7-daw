import { describe, expect, it } from "vitest";
import {
  GITHUB_SCOPES,
  hasRepoScope,
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

  it("asks one code for the person, the key and repo", () => {
    const granted = ["read:user", "write:public_key", "repo"];
    expect(GITHUB_SCOPES).toEqual(granted);
  });
});
