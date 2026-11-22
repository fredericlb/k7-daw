// github-signin.spec.ts — Sign in with GitHub in Settings → Services (one code, `repo` included), then Connect lists the
// repositories, k7 workspaces first, and picking one whose address is a folder connects the workspace.
import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { fakeGithub } from "../../src/fakeGithub.ts";
import { workspaceCard } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";

const WS = "E2E-github";
const SHOWN = { timeout: 30_000 };
const PORT = Number(process.env.E2E_GITHUB_PORT ?? 0);

test("a workspace is connected after Sign in with GitHub in Settings", async ({
  k7,
}) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.skip(!PORT, "the bed points the front at no fake GitHub");
  test.setTimeout(120_000);
  const bare = join(mkdtempSync(join(tmpdir(), "k7-gh-e2e-")), "songs.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const github = await fakeGithub(PORT, [
    { name: "plain", topics: [], sshUrl: "git@github.invalid:octo/plain.git" },
    {
      name: "songs",
      topics: ["k7-workspace"],
      sshUrl: pathToFileURL(bare).href,
    },
  ]);
  const files = await k7.files();
  await files.removeWorkspace(WS);
  await files.createWorkspace(WS);
  const page = await k7.open("/settings?tab=services");
  try {
    // A run before this one may have left the device signed in.
    await page.request.post("/api/files/git/github/signout").catch(() => {});
    await page.reload();
    const section = page.getByRole("region", { name: "GitHub" });
    await section.getByRole("button", { name: "Sign in with GitHub" }).click();
    await expect(section.getByText("ABCD-1234")).toBeVisible(SHOWN);
    expect(github.asked().split(" ")).toContain("repo");
    github.approve();
    await expect(section.getByText("@octo")).toBeVisible(SHOWN);
    await expect(
      section.getByRole("button", { name: "Sign out" }),
    ).toBeVisible();
    expect(github.keys).toHaveLength(1);

    await page.goto("/");
    await workspaceCard(page, WS).waitFor({ timeout: 60_000 });
    await workspaceCard(page, WS)
      .getByRole("button", { name: `What can be done with ${WS}`, exact: true })
      .click();
    await page.getByRole("menuitem", { name: "Connect to git…" }).click();
    const dialog = page.getByRole("dialog", { name: /^Connect .+ to git$/ });
    await expect(dialog.getByText("Signed in as @octo")).toBeVisible(SHOWN);
    await expect(
      dialog.getByRole("button", { name: "Sign in with GitHub" }),
    ).toHaveCount(0);
    const rows = dialog.getByRole("button", { name: /^octo\// });
    await expect(rows).toHaveCount(2, SHOWN);
    await expect(rows.first()).toContainText("octo/songs");

    await rows.first().click();
    await expect(dialog).toBeHidden(SHOWN);
    await expect(page.getByText(`${WS} is connected to`)).toBeVisible(SHOWN);
  } finally {
    await page.request.post("/api/files/git/github/signout").catch(() => {});
    await github.close();
  }
});
