import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ForgeWays } from "./ForgeWays";
import type { ForgeAccount } from "./forgeState";
import type { GitTransport } from "./gitApi";

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

const html = (accounts: ForgeAccount[], createName?: string) =>
  renderToStaticMarkup(
    <ForgeWays
      t={t}
      accounts={accounts}
      refresh={async () => {}}
      createName={createName}
      disabled={false}
      onPick={() => {}}
      onOther={() => {}}
    />,
  );

describe("the forges' pickers in the connect and clone dialogs", () => {
  it("goes straight to the picker once signed in, with Another host still there", () => {
    const out = html([github], "songs");
    expect(out).toContain("Signed in as @octo");
    expect(out).toContain("Filter repositories");
    expect(out).toContain("New repository");
    expect(out).toContain("Another host…");
    expect(out).not.toContain("Sign in with GitHub");
  });

  it("offers no New repository where a repository cannot be made (Clone)", () => {
    const out = html([github]);
    expect(out).not.toContain("New repository");
  });

  it("shows one picker per signed-in account, a server's under its own name", () => {
    const out = html([github, forgejo], "songs");
    expect(out).toContain("Signed in as @octo");
    expect(out).toContain("Signed in as @fred on git.example");
    expect(out.match(/aria-label="Filter repositories"/g)).toHaveLength(2);
    expect(out.match(/Another host…/g)).toHaveLength(1);
  });
});
