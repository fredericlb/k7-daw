// git-sync.spec.ts — a workspace connected to a bare repository, committed, pushed, cloned into a second workspace,
// edited on both sides and merged. The remote is a folder this spec makes with `git`, so only the web target reaches it.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "../../src/target.ts";

const A = "E2E-git-a";
const B = "E2E-git-b";
const SHOWN = { timeout: 30_000 };
const SLOW = { timeout: 120_000 };

const git = (dir: string, ...args: string[]): string =>
  execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" });

/** A workspace's card on the home page, found once the list has it. */
async function card(page: Page, ws: string): Promise<Locator> {
  const region = page.getByRole("region", { name: `Workspace ${ws}` });
  await expect(region).toBeVisible({ timeout: 60_000 });
  return region;
}

/** Commit & Push from the card, signing as a test author when the device has none; resolves with the open dialog. */
async function commitAndPush(
  page: Page,
  ws: string,
  message: string,
): Promise<Locator> {
  await (await card(page, ws))
    .getByRole("button", { name: /^Commit & Push, \d+ changes?$/ })
    .click();
  const dialog = page.getByRole("dialog", { name: "Commit & Push" });
  const name = dialog.getByLabel("Name", { exact: true });
  // The author question comes once the device's identity is read: isVisible would not wait for it.
  const asked = await name
    .waitFor({ state: "visible", timeout: 5_000 })
    .then(() => true)
    .catch(() => false);
  if (asked) {
    await name.fill("E2E Tester");
    await dialog.getByLabel("Email", { exact: true }).fill("e2e@example.org");
  }
  await dialog.getByLabel("Message", { exact: true }).fill(message);
  await dialog
    .getByRole("button", { name: "Commit & Push", exact: true })
    .click();
  return dialog;
}

test("a workspace is connected, pushed, cloned and merged through a bare repository", async ({
  k7,
}) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.setTimeout(300_000);
  const dir = mkdtempSync(join(tmpdir(), "k7-git-e2e-"));
  const bare = join(dir, "remote.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const url = `file://${bare}`;
  const files = await k7.files();
  for (const w of [A, B]) await files.removeWorkspace(w);
  await files.createWorkspace(A);
  await files.writeText(A, "notes.txt", "one\ntwo\nthree\n");
  try {
    const page = await k7.open("/");

    await test.step("connect the workspace from its menu", async () => {
      await (await card(page, A))
        .getByRole("button", {
          name: `What can be done with ${A}`,
          exact: true,
        })
        .click();
      await page.getByRole("menuitem", { name: "Connect to git…" }).click();
      const dialog = page.getByRole("dialog", { name: /^Connect .+ to git$/ });
      await dialog.getByLabel("Repository address").fill(url);
      await dialog
        .getByRole("button", { name: "Connect", exact: true })
        .click();
      await expect(dialog).toBeHidden(SHOWN);
    });

    await test.step("commit and push in one act", async () => {
      const dialog = await commitAndPush(page, A, "first take");
      // The dialog closes by itself when the push ends, and a toast says what went.
      await expect(dialog).toBeHidden(SHOWN);
      await expect(page.getByText("Committed and pushed")).toBeVisible(SHOWN);
      await expect
        .poll(() => git(bare, "log", "--format=%s", "main").trim(), SHOWN)
        .toContain("first take");
    });

    await test.step("clone it into a second workspace", async () => {
      await page
        .getByRole("button", { name: "New workspace or clone from git" })
        .click();
      await page.getByRole("menuitem", { name: "Clone from git" }).click();
      const dialog = page.getByRole("dialog", { name: "Clone a repository" });
      await dialog.getByLabel("Name of the workspace").fill(B);
      await dialog.getByLabel("Repository address").fill(url);
      await dialog.getByRole("button", { name: "Clone", exact: true }).click();
      await expect(dialog).toBeHidden(SLOW);
      await expect
        .poll(async () => (await files.projects()).map((w) => w.name), SHOWN)
        .toContain(B);
      expect(await files.readText(B, "notes.txt")).toBe("one\ntwo\nthree\n");
    });

    await test.step("both sides change the same line; the second to push is told to pull first", async () => {
      await files.writeText(B, "notes.txt", "one\nB says\nthree\n");
      // A card reads its git state once, so a write made outside the page shows on a fresh load.
      await page.reload();
      const dialogB = await commitAndPush(page, B, "B's line");
      await expect(dialogB).toBeHidden(SHOWN);
      await expect
        .poll(() => git(bare, "log", "--format=%s", "main").trim(), SHOWN)
        .toContain("B's line");

      await files.writeText(A, "notes.txt", "one\nA says\nthree\n");
      await page.reload();
      const dialogA = await commitAndPush(page, A, "A's line");
      await expect(dialogA.getByTestId("git-pull-first")).toBeVisible(SLOW);
      await dialogA
        .getByRole("button", { name: "Close", exact: true })
        .first()
        .click();
    });

    await test.step("pull, choose mine, finish the merge", async () => {
      const ws = await card(page, A);
      await ws.getByRole("button", { name: /^Pull/ }).click();
      // The Conflict screen is a full-screen modal dialog.
      const dialog = page.getByRole("dialog", { name: "Merge" });
      await expect(dialog).toBeVisible(SLOW);
      await dialog
        .getByRole("listbox", { name: "Conflicts" })
        .getByRole("option")
        .first()
        .click();
      await dialog
        .getByRole("radiogroup", { name: "Choice" })
        .getByRole("radio", { name: "Mine" })
        .click();
      await dialog.getByRole("button", { name: "Finish merge" }).click();
      await expect(dialog).toBeHidden(SHOWN);
      expect(await files.readText(A, "notes.txt")).toContain("A says");
      await expect(
        ws.getByRole("button", { name: /Resolve \d+ conflicts?/ }),
      ).toBeHidden(SHOWN);
    });
  } finally {
    for (const w of [A, B])
      await files.removeWorkspace(w).catch(() => undefined);
    rmSync(dir, { recursive: true, force: true });
  }
});
