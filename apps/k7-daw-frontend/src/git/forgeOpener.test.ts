import { afterEach, describe, expect, it, vi } from "vitest";
import { signInWayOf } from "./forgeOpener";
import { git } from "./gitApi";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

const channel = { postMessage() {}, addEventListener() {} };

afterEach(() => vi.unstubAllGlobals());

describe("how a Forgejo sign-in leaves k7 and comes back", () => {
  it("keeps the tab and the loopback in a plain browser", () => {
    expect(signInWayOf({})).toEqual({ open: "link" });
  });

  it("keeps the tab and the loopback in the desktop app, whose host gives a new tab to the system browser", () => {
    expect(signInWayOf({ k7Desktop: {} })).toEqual({ open: "link" });
  });

  it("opens iOS's sheet and comes back through the app's scheme", () => {
    expect(signInWayOf({ k7Ios: channel })).toEqual({
      redirect: "app",
      open: "sheet",
    });
  });

  it("hands Android's tab to the browser and comes back through the app's scheme", () => {
    expect(signInWayOf({ k7Android: channel })).toEqual({
      redirect: "app",
      open: "link",
    });
  });

  it("asks the front for the app's redirect only when the host returns through it", async () => {
    const f = vi.fn().mockImplementation(
      async () =>
        new Response(JSON.stringify({ authorizeUrl: "u" }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
    );
    vi.stubGlobal("fetch", f);
    const t = { base: "/api/files", headers: () => ({}) };
    await git.forgeStart(t, "git.example", "app");
    await git.forgeStart(t, "git.example");
    expect(JSON.parse(f.mock.calls[0][1].body)).toEqual({ redirect: "app" });
    expect(JSON.parse(f.mock.calls[1][1].body)).toEqual({});
  });
});
