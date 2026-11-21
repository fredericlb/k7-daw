// frozen-chains.spec.ts — the Library page's Frozen chains section: a library's cold chain duplicated into the account's and deleted, and the bench.
// The bench is a project the page opens, saved nowhere, and in no list of workspaces (apps/k7-daw-frontend/docs/frozenChains.md).
import type { Locator, Page } from "@playwright/test";
import { homeTab } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

/** The library chain duplicated: k7-core's instrumental, a cold (offline) separation the bench cannot play. */
const SOURCE = "@k7-core/instrumental";
const MINE = "e2e-cold";

/** A chain's row, found by the ref it prints. */
const row = (page: Page, ref: string): Locator =>
  page
    .locator("section")
    .getByText(ref, { exact: true })
    .locator("xpath=ancestor::div[.//button][1]");

async function openPage(page: Page): Promise<void> {
  await expect(page.getByRole("heading", { name: "Your chains" })).toBeVisible({
    timeout: 30_000,
  });
}

/** Deletes the account's chain through its row's Delete and the confirmation. */
async function deleteMine(page: Page): Promise<void> {
  await row(page, `@account/${MINE}`)
    .getByRole("button", { name: "Delete", exact: true })
    .click();
  const confirm = page.getByRole("dialog", {
    name: `Delete the chain ${MINE}?`,
  });
  await confirm.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(row(page, `@account/${MINE}`)).toHaveCount(0, {
    timeout: 15_000,
  });
}

test("a cold chain is duplicated into the account's and deleted, and New chain opens the bench", async ({
  k7,
}) => {
  test.setTimeout(150_000);
  let page = await k7.open("/libraries");
  await openPage(page);
  // A failed run may have left the copy: start without it.
  if ((await row(page, `@account/${MINE}`).count()) > 0) await deleteMine(page);
  const watch = watchPage(page);

  await test.step("duplicate the library's cold chain under a name of the account's", async () => {
    await expect(
      page.getByRole("heading", { name: "Library cold chains" }),
    ).toBeVisible();
    await row(page, SOURCE)
      .getByRole("button", { name: "Duplicate into your chains" })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Duplicate into your chains",
    });
    await dialog.getByRole("textbox").fill(MINE);
    await dialog
      .getByRole("button", { name: "Duplicate into your chains" })
      .click();
    await expect(dialog).toBeHidden();
    const mine = row(page, `@account/${MINE}`);
    await expect(mine).toBeVisible({ timeout: 15_000 });
    // A cold chain cannot run live: its Edit, and its slot button where a machine offers one, say why, disabled.
    const refused = mine.getByRole("button", { name: /^Not live: / });
    await expect(refused.first()).toBeDisabled();
    for (const b of await refused.all()) await expect(b).toBeDisabled();
  });

  await test.step("New chain opens the bare bench, which saves as a new chain", async () => {
    await page.getByRole("button", { name: "New chain" }).click();
    page = await k7.landed(page, /\/@banc\//);
    await engineUp(page, 60_000);
    await expect(
      page.getByRole("button", { name: "Save as one of your chains" }),
    ).toBeVisible();
  });

  await test.step("the bench is in no list of workspaces", async () => {
    page = await k7.open("/");
    await expect(homeTab(page, "Projects")).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(
      page.getByRole("button", { name: "New workspace or clone from git" }),
    ).toBeVisible();
    await expect(
      page.getByRole("region", { name: /^Workspace Banc/ }),
    ).toHaveCount(0);
  });

  await test.step("delete it", async () => {
    page = await k7.open("/libraries");
    await openPage(page);
    await deleteMine(page);
  });
  watch.assertClean();
});
