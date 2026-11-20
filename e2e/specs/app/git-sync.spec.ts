// git-sync.spec.ts — a workspace connected to a bare repository, committed, pushed, cloned into a second workspace,
// edited on both sides and merged. The remote is a folder this spec makes with `git`, so only the web target reaches it.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "../../src/target.ts";

const A = "E2E-git-a";
const B = "E2E-git-b";
const SHOWN = { timeout: 30_000 };
const SLOW = { timeout: 120_000 };

const git = (dir: string, ...args: string[]): string =>
  execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" });

/** Opens a workspace's tree from the home page, where its git bar is. */
async function openTree(page: Page, ws: string): Promise<void> {
  await page.getByRole("link", { name: "Projects", exact: true }).click();
  const search = page.getByRole("searchbox", {
    name: "Search a workspace or a project",
  });
  await expect(async () => {
    await search.fill(ws);
    await page
      .locator(`tr[data-workspace="${ws}"]`)
      .first()
      .getByRole("button", { name: new RegExp(`^${ws} \\d+ projects?$`) })
      .click({ timeout: 5_000 });
  }).toPass({ timeout: 60_000 });
}

/** Commits through the bar's dialog, signing as a test author when the device has none. */
async function commit(page: Page, message: string): Promise<void> {
  const bar = page.getByRole("region", { name: "Git" });
  await bar.getByRole("button", { name: "Commit", exact: true }).click();
  const dialog = page.getByRole("dialog");
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
  await dialog.getByRole("button", { name: "Commit", exact: true }).click();
  await expect(dialog).toBeHidden(SHOWN);
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
      const search = page.getByRole("searchbox", {
        name: "Search a workspace or a project",
      });
      await expect(async () => {
        await search.fill(A);
        await page
          .locator(`tr[data-workspace="${A}"]`)
          .first()
          .getByRole("button", {
            name: `What can be done with ${A}`,
            exact: true,
          })
          .click({ timeout: 5_000 });
      }).toPass({ timeout: 60_000 });
      await page.getByRole("menuitem", { name: "Connect to git…" }).click();
      const dialog = page.getByRole("dialog");
      await dialog.getByLabel("Repository address").fill(url);
      await dialog
        .getByRole("button", { name: "Connect", exact: true })
        .click();
      await expect(dialog).toBeHidden(SHOWN);
    });

    await test.step("commit and push", async () => {
      await openTree(page, A);
      const bar = page.getByRole("region", { name: "Git" });
      await expect(bar).toContainText("remote.git", SHOWN);
      await commit(page, "first take");
      await bar.getByRole("button", { name: "Push", exact: true }).click();
      await expect
        .poll(() => git(bare, "log", "--format=%s", "main").trim(), SHOWN)
        .toContain("first take");
    });

    await test.step("clone it into a second workspace", async () => {
      await page.getByRole("link", { name: "Projects", exact: true }).click();
      await page.getByRole("button", { name: "Clone from git" }).click();
      const dialog = page.getByRole("dialog");
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
      await openTree(page, B);
      await commit(page, "B's line");
      const barB = page.getByRole("region", { name: "Git" });
      await barB.getByRole("button", { name: "Push", exact: true }).click();
      await expect
        .poll(() => git(bare, "log", "--format=%s", "main").trim(), SHOWN)
        .toContain("B's line");

      await files.writeText(A, "notes.txt", "one\nA says\nthree\n");
      await openTree(page, A);
      await commit(page, "A's line");
      const barA = page.getByRole("region", { name: "Git" });
      await expect(page.getByTestId("git-pull-first")).toBeVisible(SLOW);
    });

    await test.step("pull, choose mine, finish the merge", async () => {
      const bar = page.getByRole("region", { name: "Git" });
      await bar.getByRole("button", { name: "Pull", exact: true }).click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("heading", { name: "Merge" })).toBeVisible(
        SHOWN,
      );
      await dialog.getByRole("button", { name: "Keep mine" }).first().click();
      await dialog.getByRole("button", { name: "Finish merge" }).click();
      await expect(dialog).toBeHidden(SHOWN);
      expect(await files.readText(A, "notes.txt")).toContain("A says");
      await expect(
        bar.getByRole("button", { name: /Resolve \d+ conflicts?/ }),
      ).toBeHidden(SHOWN);
    });
  } finally {
    for (const w of [A, B])
      await files.removeWorkspace(w).catch(() => undefined);
    rmSync(dir, { recursive: true, force: true });
  }
});
