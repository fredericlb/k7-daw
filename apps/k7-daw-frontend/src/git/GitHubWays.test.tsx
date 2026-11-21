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

describe("GitHub's picker in the connect and clone dialogs", () => {
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
});
