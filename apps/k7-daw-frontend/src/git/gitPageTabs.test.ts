import { describe, expect, it } from "vitest";
import type { ForgeAccount } from "./forgeState";
import { pageHref, takeClonePrefill } from "./gitPages";
import { firstTab } from "./gitPageTabs";

const gh = (over: Partial<ForgeAccount> = {}): ForgeAccount => ({
  id: "github.com",
  kind: "github",
  name: "GitHub",
  available: true,
  login: "octo",
  scopes: ["repo"],
  ...over,
});
const fj = (over: Partial<ForgeAccount> = {}): ForgeAccount => ({
  id: "git.example.com",
  kind: "forgejo",
  name: "Forgejo",
  available: true,
  login: "fred",
  scopes: [],
  ...over,
});

describe("the tab a git page opens on", () => {
  it("leads with GitHub when it can list repositories", () => {
    expect(firstTab([gh(), fj()], "")).toBe("github");
  });

  it("leads with Forgejo when GitHub cannot list (not signed in, or no repo)", () => {
    expect(firstTab([gh({ login: undefined }), fj()], "")).toBe("forgejo");
    expect(firstTab([gh({ scopes: [] }), fj()], "")).toBe("forgejo");
  });

  it("opens on the ssh form with no account, or with an address to show", () => {
    expect(firstTab([], "")).toBe("other");
    expect(firstTab([fj({ login: undefined })], "")).toBe("other");
    expect(firstTab([gh()], "git@host:me/songs.git")).toBe("other");
  });
});

describe("the git pages' addresses", () => {
  const home = "http://127.0.0.1/?ws=songs";
  it("adds the page to the home's address and keeps its other parameters", () => {
    const sync = new URL(pageHref(home, { sync: "My songs" }));
    expect(sync.searchParams.get("sync")).toBe("My songs");
    expect(sync.searchParams.get("ws")).toBe("songs");
    expect(
      new URL(pageHref(home, { clone: true })).searchParams.has("clone"),
    ).toBe(true);
  });

  it("leaves a page by dropping only its own parameter", () => {
    const back = new URL(pageHref(`${home}&sync=x&clone=`, null));
    expect(back.searchParams.has("sync")).toBe(false);
    expect(back.searchParams.has("clone")).toBe(false);
    expect(back.searchParams.get("ws")).toBe("songs");
  });

  it("gives a clone's prefill once", () => {
    expect(takeClonePrefill()).toBeNull();
  });
});
