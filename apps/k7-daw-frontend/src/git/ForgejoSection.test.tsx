import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ProbeFacts } from "./AddForgejoServer";
import { ForgejoRows } from "./ForgejoSection";
import type { ForgeAccount, ForgeProbe } from "./forgeState";
import type { GitTransport } from "./gitApi";

const t: GitTransport = { base: "http://127.0.0.1:1", headers: () => ({}) };

const server = (o: Partial<ForgeAccount> = {}): ForgeAccount => ({
  id: "git.example",
  kind: "forgejo",
  name: "Forgejo",
  url: "https://git.example",
  available: true,
  scopes: [],
  ...o,
});

const html = (accounts: ForgeAccount[]) =>
  renderToStaticMarkup(
    <ForgejoRows t={t} accounts={accounts} refresh={async () => {}} />,
  );

describe("Settings → Services, Forgejo", () => {
  it("offers Add a Forgejo server with no server yet", () => {
    const out = html([]);
    expect(out).toContain("Add a Forgejo server");
    expect(out).not.toContain("Sign in");
  });

  it("shows a server not signed in with Sign in and Remove", () => {
    const out = html([server()]);
    expect(out).toContain('aria-label="git.example"');
    expect(out).toContain("Not signed in");
    expect(out).toContain(">Sign in<");
    expect(out).toContain(">Remove<");
  });

  it("shows who is signed in, without a Sign in button", () => {
    const out = html([server({ login: "fred" })]);
    expect(out).toContain("Signed in as @fred");
    expect(out).not.toContain(">Sign in<");
    expect(out).toContain(">Remove<");
  });

  it("leaves GitHub to its own section", () => {
    const out = html([
      server({ id: "github.com", kind: "github", name: "GitHub" }),
    ]);
    expect(out).not.toContain('aria-label="GitHub"');
  });
});

describe("what a check read from a server", () => {
  const probe: ForgeProbe = {
    id: "git.example",
    url: "https://git.example",
    version: "17.0.2",
    pageSize: 50,
    lfs: true,
    rootUrl: "https://git.example/",
    rootAnswers: true,
    redirects: [],
    scopes: [],
  };
  const facts = (o: Partial<ForgeProbe>) =>
    renderToStaticMarkup(<ProbeFacts probe={{ ...probe, ...o }} />);

  it("says the version, LFS on and the web address answering", () => {
    const out = facts({});
    expect(out).toContain("Forgejo 17.0.2 answered.");
    expect(out).toContain("Large files (LFS) are on.");
    expect(out).toContain("The server&#x27;s web address answers.");
  });

  it("warns that audio files will not sync when LFS is off", () => {
    const out = facts({ lfs: false });
    expect(out).toContain("audio files will not sync");
    expect(out).toContain('role="alert"');
  });

  it("warns when the web address does not answer", () => {
    expect(facts({ rootAnswers: false })).toContain(
      "does not answer from here",
    );
  });
});
