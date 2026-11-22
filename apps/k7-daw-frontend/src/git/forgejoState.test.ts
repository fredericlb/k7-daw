import { describe, expect, it } from "vitest";
import {
  addAfter,
  canAdd,
  probeOf,
  shortVersion,
  signInAfter,
  sshPort,
  startAdding,
  startingSignIn,
} from "./forgejoState";
import {
  canPickFrom,
  type ForgeAccount,
  type ForgeProbe,
  forgejoServers,
  isSignout,
} from "./forgeState";

const probe: ForgeProbe = {
  id: "git.example",
  url: "https://git.example",
  version: "17.0.2",
  pageSize: 50,
  lfs: true,
  rootUrl: "https://git.example/",
  rootAnswers: true,
  redirects: ["http://127.0.0.1/api/files/git/forge/callback"],
  scopes: ["write:user"],
};

describe("adding a Forgejo server", () => {
  it("goes from the address through the check to the registration", () => {
    let s = addAfter(startAdding, { type: "check" });
    expect(s.phase).toBe("checking");
    s = addAfter(s, { type: "probed", probe });
    expect(s).toEqual({ phase: "register", probe });
    s = addAfter(s, { type: "add" });
    expect(s.phase).toBe("adding");
  });

  it("keeps the check when the add fails, so Add can be pressed again", () => {
    const adding = addAfter({ phase: "register", probe }, { type: "add" });
    const failed = addAfter(adding, { type: "failed", message: "no" });
    expect(failed).toEqual({ phase: "failed", message: "no", probe });
    expect(probeOf(failed)).toBe(probe);
  });

  it("does not add before a check, and a failed check has no probe", () => {
    expect(addAfter(startAdding, { type: "add" })).toBe(startAdding);
    const failed = addAfter(
      { phase: "checking" },
      { type: "failed", message: "x" },
    );
    expect(probeOf(failed)).toBeUndefined();
  });

  it("reads the ssh port: empty is the default, a whole port, or bad", () => {
    expect(sshPort("")).toBeUndefined();
    expect(sshPort(" 2222 ")).toBe(2222);
    expect(sshPort("0")).toBe("bad");
    expect(sshPort("70000")).toBe("bad");
    expect(sshPort("2x")).toBe("bad");
  });

  it("is ready with a client id and no bad port", () => {
    expect(canAdd("", "")).toBe(false);
    expect(canAdd("cid", "")).toBe(true);
    expect(canAdd("cid", "abc")).toBe(false);
  });
});

describe("a server's version", () => {
  it("drops the build metadata after the first plus", () => {
    expect(shortVersion("17.0.2+gitea-1.22.0")).toBe("17.0.2");
    expect(shortVersion("17.0.2")).toBe("17.0.2");
  });
});

describe("a Forgejo sign-in", () => {
  const waiting = signInAfter(startingSignIn, {
    type: "started",
    start: {
      authorizeUrl: "https://git.example/auth",
      redirect: "r",
      expiresIn: 600,
    },
  });

  it("waits on the authorize url and ends on each answer", () => {
    expect(waiting).toEqual({
      phase: "waiting",
      url: "https://git.example/auth",
    });
    const poll = (state: "pending" | "ok" | "denied" | "error" | "expired") =>
      signInAfter(waiting, {
        type: "polled",
        answer: { state, login: "fred", error: "boom" },
      });
    expect(poll("pending")).toBe(waiting);
    expect(poll("ok")).toEqual({
      phase: "done",
      login: "fred",
      keyNote: undefined,
    });
    expect(poll("denied").phase).toBe("denied");
    expect(poll("expired").phase).toBe("expired");
    expect(poll("error")).toEqual({ phase: "failed", message: "boom" });
  });

  it("ignores an answer that arrives out of its phase", () => {
    const answer = { state: "ok" as const, login: "fred" };
    expect(signInAfter(startingSignIn, { type: "polled", answer })).toBe(
      startingSignIn,
    );
  });

  it("starts again on demand", () => {
    expect(signInAfter({ phase: "expired" }, { type: "again" })).toBe(
      startingSignIn,
    );
  });
});

describe("the forge accounts", () => {
  const acc = (o: Partial<ForgeAccount>): ForgeAccount => ({
    id: "x",
    kind: "forgejo",
    name: "x",
    available: true,
    scopes: [],
    ...o,
  });

  it("lists the Forgejo servers only", () => {
    const all = [acc({ kind: "github" }), acc({ id: "y" })];
    expect(forgejoServers(all).map((a) => a.id)).toEqual(["y"]);
  });

  it("picks from GitHub with repo and from a signed-in server", () => {
    expect(
      canPickFrom(acc({ kind: "github", login: "o", scopes: ["repo"] })),
    ).toBe(true);
    expect(canPickFrom(acc({ kind: "github", login: "o", scopes: [] }))).toBe(
      false,
    );
    expect(canPickFrom(acc({ login: "fred" }))).toBe(true);
    expect(canPickFrom(acc({}))).toBe(false);
  });

  it("knows a signout of either forge", () => {
    expect(isSignout("forgejo-signout")).toBe(true);
    expect(isSignout("github-signout")).toBe(true);
    expect(isSignout("scope")).toBe(false);
  });
});
