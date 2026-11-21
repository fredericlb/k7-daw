// home.ts — the Home pages' controls by role and name: the tab bar, the New menu, a workspace's card and its project links.
// What a reader gets wrong: a project opens through a click on its row or its link, and the app cancels the navigation.
import type { Locator, Page } from "@playwright/test";

/** A tab of the floating bar: a link of `navigation "Home"` (Projects, Tasks, Library, Settings). */
export const homeTab = (page: Page, name: string): Locator =>
  page
    .getByRole("navigation", { name: "Home", exact: true })
    .getByRole("link", {
      name,
      exact: true,
    });

/** The top bar's New menu, opened on `item` ("New workspace" or "Clone from git"). */
export async function newMenu(page: Page, item: string): Promise<void> {
  await page
    .getByRole("button", { name: "New workspace or clone from git" })
    .click();
  await page.getByRole("menuitem", { name: item, exact: true }).click();
}

/** A workspace's card, which also carries the drop handlers. */
export const workspaceCard = (page: Page, ws: string): Locator =>
  page.getByRole("region", { name: `Workspace ${ws}`, exact: true });

/** A project's link inside its workspace's card, by the project's name without its suffix. */
export const projectLink = (page: Page, ws: string, project: string): Locator =>
  workspaceCard(page, ws).getByRole("link", {
    name: `Open ${project}`,
    exact: true,
  });

/** In a Connect or Clone dialog that opens on GitHub, takes the plain way; a dialog without GitHub is already on it. */
export async function chooseAnotherHost(dialog: Locator): Promise<void> {
  const other = dialog.getByRole("button", { name: "Another host…" });
  const shown = await other
    .waitFor({ state: "visible", timeout: 5_000 })
    .then(() => true)
    .catch(() => false);
  if (shown) await other.click();
}
