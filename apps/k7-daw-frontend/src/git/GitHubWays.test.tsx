import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GitHubWays } from "./GitHubWays";
import type { GitTransport } from "./gitApi";
import type { GithubStatus } from "./githubState";

const t: GitTransport = { base: "http://127.0.0.1:1", headers: () => ({}) };

const html = (status: GithubStatus, createName?: string) =>
  renderToStaticMarkup(
    <GitHubWays
      t={t}
      status={status}
      refresh={async () => {}}
      createName={createName}
      disabled={false}
      onPick={() => {}}
      onOther={() => {}}
    />,
  );

describe("the two ways in the connect and clone dialogs", () => {
  it("offers Sign in with GitHub and Another host when nobody is signed in", () => {
    const out = html({ available: true, scopes: [] });
    expect(out).toContain("Sign in with GitHub");
    expect(out).toContain("Another host…");
  });

  it("goes straight to the picker once signed in, with Another host still there", () => {
    const out = html(
      { available: true, login: "octo", scopes: ["repo"] },
      "songs",
    );
    expect(out).toContain("Signed in as @octo");
    expect(out).toContain("Filter repositories");
    expect(out).toContain("New repository");
    expect(out).toContain("Another host…");
    expect(out).not.toContain("Sign in with GitHub");
  });

  it("offers no New repository where a repository cannot be made (Clone)", () => {
    const out = html({ available: true, login: "octo", scopes: ["repo"] });
    expect(out).not.toContain("New repository");
  });

  it("asks for the repo permission, saying why, when the picker opens without it", () => {
    const out = html(
      { available: true, login: "octo", scopes: ["read:user"] },
      "songs",
    );
    expect(out).toContain("one more permission");
    expect(out).not.toContain("Filter repositories");
  });
});
