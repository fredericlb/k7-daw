// github-signin.spec.ts — Connect through "Sign in with GitHub": the code is shown, the fake GitHub approves, the picker lists
// the k7 workspace first, and picking a repository whose address is a folder connects the workspace.
import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fakeGithub } from "../../src/fakeGithub.ts";
import { workspaceCard } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";

const WS = "E2E-github";
const SHOWN = { timeout: 30_000 };
const PORT = Number(process.env.E2E_GITHUB_PORT ?? 0);

test("a workspace is connected through Sign in with GitHub", async ({ k7 }) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.skip(!PORT, "the bed points the front at no fake GitHub");
  test.setTimeout(120_000);
  const bare = join(mkdtempSync(join(tmpdir(), "k7-gh-e2e-")), "songs.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const github = await fakeGithub(PORT, [
    { name: "plain", topics: [], sshUrl: "git@github.invalid:octo/plain.git" },
    { name: "songs", topics: ["k7-workspace"], sshUrl: `file://${bare}` },
  ]);
  const files = await k7.files();
  await files.removeWorkspace(WS);
  await files.createWorkspace(WS);
  try {
    const page = await k7.open("/");
    // A run before this one may have left the device signed in.
    await page.request.post("/api/files/git/github/signout").catch(() => {});
    await workspaceCard(page, WS).waitFor({ timeout: 60_000 });
    await workspaceCard(page, WS)
      .getByRole("button", { name: `What can be done with ${WS}`, exact: true })
      .click();
    await page.getByRole("menuitem", { name: "Connect to git…" }).click();
    const dialog = page.getByRole("dialog", { name: /^Connect .+ to git$/ });

    await dialog.getByRole("button", { name: "Sign in with GitHub" }).click();
    await expect(dialog.getByText("ABCD-1234")).toBeVisible(SHOWN);
    github.approve();

    await expect(dialog.getByText("Signed in as @octo")).toBeVisible(SHOWN);
    const rows = dialog.getByRole("button", { name: /^octo\// });
    await expect(rows).toHaveCount(2, SHOWN);
    await expect(rows.first()).toContainText("octo/songs");
    expect(github.keys).toHaveLength(1);

    await rows.first().click();
    await expect(dialog).toBeHidden(SHOWN);
    await expect(page.getByText(`${WS} is connected to`)).toBeVisible(SHOWN);
  } finally {
    await github.close();
  }
});
