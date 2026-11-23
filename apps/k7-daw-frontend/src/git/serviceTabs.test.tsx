import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { ForgeAccount } from "./forgeState";
import type { GitTransport } from "./gitApi";
import { serviceTabs } from "./serviceTabs";

const t: GitTransport = { base: "http://127.0.0.1:1", headers: () => ({}) };

const github: ForgeAccount = {
  id: "github.com",
  kind: "github",
  name: "GitHub",
  available: true,
  login: "octo",
  scopes: ["repo"],
};
const forgejo: ForgeAccount = {
  id: "git.example",
  kind: "forgejo",
  name: "Forgejo",
  url: "https://git.example",
  available: true,
  login: "fred",
  scopes: [],
};

/** Each tab's label, whether it says "not set up", and its content as markup. */
const tabs = (accounts: ForgeAccount[], createName?: string) =>
  serviceTabs({
    t,
    accounts,
    refresh: async () => {},
    createName,
    disabled: false,
    onPick: () => {},
    other: <p>the ssh form</p>,
  }).map((x) => ({
    key: x.key,
    notSetUp: !!x.notSetUp,
    html: renderToStaticMarkup(<MemoryRouter>{x.content}</MemoryRouter>),
  }));

describe("the services' tabs of the git pages", () => {
  it("are GitHub, Forgejo and Other host, in that order", () => {
    expect(tabs([]).map((x) => x.key)).toEqual(["github", "forgejo", "other"]);
    expect(tabs([]).find((x) => x.key === "other")?.html).toContain(
      "the ssh form",
    );
  });

  it("goes straight to GitHub's picker once signed in, never to a sign-in", () => {
    const gh = tabs([github], "songs")[0];
    expect(gh.notSetUp).toBe(false);
    expect(gh.html).toContain("Signed in as @octo");
    expect(gh.html).toContain("Filter repositories");
    expect(gh.html).toContain("New repository");
    expect(gh.html).not.toContain("Sign in with GitHub");
  });

  it("sends GitHub's sign-in to Settings → Services", () => {
    const gh = tabs([{ ...github, login: undefined, scopes: [] }])[0];
    expect(gh.notSetUp).toBe(true);
    expect(gh.html).toContain("Settings → Services");
    expect(gh.html).not.toContain("Filter repositories");
  });

  it("offers no New repository where a repository cannot be made (Clone)", () => {
    expect(tabs([github])[0].html).not.toContain("New repository");
  });

  it("shows a server's picker under its own name, and the servers' section while none is signed in", () => {
    expect(tabs([forgejo], "songs")[1].html).toContain(
      "Signed in as @fred on git.example",
    );
    const none = tabs([{ ...forgejo, login: undefined }])[1];
    expect(none.notSetUp).toBe(true);
    expect(none.html).toContain("Forgejo servers");
  });
});
