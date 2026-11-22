// forgejo-signin.spec.ts — Add a Forgejo server in Settings → Services (address, check, nothing registered), sign in
// through the server's page in a second browser tab, then Connect lists its repositories and picking a folder connects.
import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { fakeForgejo } from "../../src/fakeForgejo.ts";
import { workspaceCard } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";

const WS = "E2E-forgejo";
const SHOWN = { timeout: 30_000 };
const PORT = Number(process.env.E2E_FORGEJO_PORT ?? 0);

test("a workspace is connected after adding and signing in to a Forgejo server", async ({
  k7,
}) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.skip(!PORT, "the bed has no fake Forgejo port");
  test.setTimeout(120_000);
  const bare = join(mkdtempSync(join(tmpdir(), "k7-fj-e2e-")), "songs.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const forgejo = await fakeForgejo(PORT, [
    { name: "plain", topics: [], sshUrl: "git@forgejo.invalid:fred/plain.git" },
    {
      name: "songs",
      topics: ["k7-workspace"],
      sshUrl: pathToFileURL(bare).href,
    },
  ]);
  const id = `127.0.0.1:${PORT}`;
  const files = await k7.files();
  await files.removeWorkspace(WS);
  await files.createWorkspace(WS);
  const page = await k7.open("/settings?tab=services");
  const forget = () =>
    page.request
      .post(`/api/files/git/forges/${encodeURIComponent(id)}/remove`)
      .catch(() => {});
  try {
    // A run before this one may have left the server added.
    await forget();
    await page.reload();
    const adding = page.getByRole("region", { name: "Forgejo servers" });
    await adding.getByRole("button", { name: "Add a Forgejo server" }).click();
    await adding.getByLabel("Server address").fill(forgejo.url);
    await adding.getByRole("button", { name: "Check" }).click();
    await expect(adding.getByText("Forgejo 17.0.2 answered.")).toBeVisible(
      SHOWN,
    );
    await expect(adding.getByText("Large files (LFS) are on.")).toBeVisible();
    // No client id: the server's built-in client signs in, so nothing is registered on it.
    await adding.getByRole("button", { name: "Add server" }).click();

    const server = page.getByRole("region", { name: id });
    await expect(server.getByText("Not signed in")).toBeVisible(SHOWN);
    await server.getByRole("button", { name: "Sign in" }).click();
    // The sign-in link opens a second tab, which the fake sends on to the front's callback page.
    const open = server.getByRole("link", { name: /^Open .* to sign in$/ });
    const [tab] = await Promise.all([
      page.context().waitForEvent("page"),
      open.click({ timeout: 30_000 }),
    ]);
    await tab.waitForLoadState();
    await expect(tab.getByText(/you can close this tab/i)).toBeVisible(SHOWN);
    await tab.close();
    // The built-in client comes back to the front's root, which sends the browser on to the callback.
    expect(forgejo.redirects()[0]).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/$/);
    await expect(server.getByText("Signed in as @fred")).toBeVisible(SHOWN);
    expect(forgejo.keys).toHaveLength(1);

    await page.goto("/");
    await workspaceCard(page, WS).waitFor({ timeout: 60_000 });
    await workspaceCard(page, WS)
      .getByRole("button", { name: `What can be done with ${WS}`, exact: true })
      .click();
    await page.getByRole("menuitem", { name: "Connect to git…" }).click();
    const dialog = page.getByRole("dialog", { name: /^Connect .+ to git$/ });
    await expect(dialog.getByText(`Signed in as @fred on ${id}`)).toBeVisible(
      SHOWN,
    );
    const rows = dialog.getByRole("button", { name: /^fred\// });
    await expect(rows).toHaveCount(2, SHOWN);
    await expect(rows.first()).toContainText("fred/songs");

    await rows.first().click();
    await expect(dialog).toBeHidden(SHOWN);
    await expect(page.getByText(`${WS} is connected to`)).toBeVisible(SHOWN);
  } finally {
    await forget();
    await forgejo.close();
  }
});
