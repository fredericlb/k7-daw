import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GitHubRows } from "./GitHubSection";
import type { GitTransport } from "./gitApi";
import type { GithubStatus } from "./githubState";

const t: GitTransport = { base: "http://127.0.0.1:1", headers: () => ({}) };

const html = (status: GithubStatus) =>
  renderToStaticMarkup(
    <GitHubRows t={t} status={status} refresh={async () => {}} />,
  );

describe("Settings → Services, GitHub", () => {
  it("offers Sign in with GitHub when nobody is signed in", () => {
    const out = html({ available: true, scopes: [] });
    expect(out).toContain("Sign in with GitHub");
    expect(out).toContain("Not signed in");
    expect(out).not.toContain("Sign out");
  });

  it("shows the login, Manage keys and Sign out once signed in with repo", () => {
    const out = html({
      available: true,
      login: "octo",
      scopes: ["read:user", "write:public_key", "repo"],
    });
    expect(out).toContain("@octo");
    expect(out).toContain("Manage keys on GitHub");
    expect(out).toContain("Sign out");
    expect(out).not.toContain("Allow repositories");
  });

  it("offers Allow repositories to a sign-in GitHub granted without repo", () => {
    const out = html({
      available: true,
      login: "octo",
      scopes: ["read:user", "write:public_key"],
    });
    expect(out).toContain("Allow repositories");
    expect(out).toContain("one more permission");
  });

  it("says the build has no GitHub sign-in where it is off", () => {
    expect(html({ available: false, scopes: [] })).toContain(
      "no GitHub sign-in",
    );
  });
});
