// git-sync.spec.ts — a workspace connected to a bare repository from its Sync settings, saved, cloned into a second
// workspace, edited on both sides and merged. The remote is a folder this spec makes with `git`, so only the web target reaches it.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type { Locator, Page } from "@playwright/test";
import { chooseAnotherHost, gitPage, newMenu } from "../../src/home.ts";
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

/** The device signs its saves: a test author when it has none (Save asks for one in Settings, not on the spot). */
async function ensureAuthor(page: Page): Promise<void> {
  const res = await page.request.get("/api/files/git/identity");
  const id = res.ok()
    ? ((await res.json()) as { name?: string; email?: string })
    : {};
  if (id.name?.trim() && id.email?.trim()) return;
  await page.request.post("/api/files/git/identity", {
    data: { name: "E2E Tester", email: "e2e@example.org" },
  });
}

/** Save from the card: one press commits everything and pushes, and a toast says how it went. */
async function saveFromCard(page: Page, ws: string): Promise<void> {
  await (await card(page, ws))
    .getByRole("button", { name: /^Save, \d+ changes?$/ })
    .click();
}

/** How many commits the bare repository's main holds. */
const commits = (bare: string): number =>
  Number(git(bare, "rev-list", "--count", "main").trim());

test("a workspace is connected, pushed, cloned and merged through a bare repository", async ({
  k7,
}) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.setTimeout(300_000);
  const dir = mkdtempSync(join(tmpdir(), "k7-git-e2e-"));
  const bare = join(dir, "remote.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const url = pathToFileURL(bare).href;
  const files = await k7.files();
  for (const w of [A, B]) await files.removeWorkspace(w);
  await files.createWorkspace(A);
  await files.writeText(A, "notes.txt", "one\ntwo\nthree\n");
  try {
    const page = await k7.open("/");
    await ensureAuthor(page);

    await test.step("connect the workspace from its Sync settings", async () => {
      await (await card(page, A))
        .getByRole("button", {
          name: `What can be done with ${A}`,
          exact: true,
        })
        .click();
      await page.getByRole("menuitem", { name: "Sync settings…" }).click();
      const sync = gitPage(page, `Sync settings for ${A}`);
      await chooseAnotherHost(sync);
      await sync.getByLabel("Repository address").fill(url);
      await sync.getByRole("button", { name: "Connect", exact: true }).click();
      await expect(sync).toBeHidden(SHOWN);
    });

    await test.step("save: commit and push in one press", async () => {
      await saveFromCard(page, A);
      // No dialog: the toast says each step, and ends on Saved.
      await expect(page.getByText("Saved", { exact: true })).toBeVisible(SHOWN);
      await expect
        .poll(() => git(bare, "log", "--format=%s", "main").trim(), SHOWN)
        .toMatch(/^Saved on \d\d\/\d\d\/\d{4} \d\d:\d\d$/m);
    });

    await test.step("clone it into a second workspace", async () => {
      await newMenu(page, "Clone from git");
      const clone = gitPage(page, "Clone a repository");
      await chooseAnotherHost(clone);
      await clone.getByLabel("Repository address").fill(url);
      // The name follows the address until it is typed.
      await clone.getByLabel("Name of the workspace").fill(B);
      await clone.getByRole("button", { name: "Clone", exact: true }).click();
      await expect(clone).toBeHidden(SLOW);
      await expect
        .poll(async () => (await files.projects()).map((w) => w.name), SHOWN)
        .toContain(B);
      expect(await files.readText(B, "notes.txt")).toBe("one\ntwo\nthree\n");
    });

    await test.step("both sides change the same line; the second to save is told to pull first", async () => {
      await files.writeText(B, "notes.txt", "one\nB says\nthree\n");
      // A card reads its git state once, so a write made outside the page shows on a fresh load.
      await page.reload();
      const before = commits(bare);
      await saveFromCard(page, B);
      await expect.poll(() => commits(bare), SHOWN).toBe(before + 1);

      await files.writeText(A, "notes.txt", "one\nA says\nthree\n");
      await page.reload();
      await saveFromCard(page, A);
      // The save stays on this device; the toast offers the Pull, which the card offers too.
      await expect(
        page.getByText("Saved on this device, not sent"),
      ).toBeVisible(SLOW);
    });

    await test.step("pull, choose mine, finish the merge", async () => {
      const ws = await card(page, A);
      await ws.getByRole("button", { name: /^Update/ }).click();
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
