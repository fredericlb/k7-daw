// panels.ts — the shell's panels: open one from its button and get the panel itself, the `complementary`
// of a Pad or Desk or the `dialog` sheet of a phone, both named by the panel's title (apps/k7-daw-frontend/src/shell/panels.tsx).
// The Events and the Mixer are the bottom dock's occupants: one dock, named by what it holds, its Close button "Close editor".
import { expect, type Locator, type Page } from "@playwright/test";
import { folded, panel } from "./nav.ts";

/** The panel named `title`, open or not: where its body is found. */
export async function panelBody(page: Page, title: string): Promise<Locator> {
  const name = { name: title, exact: true };
  return (await folded(page))
    ? page.getByRole("dialog", name)
    : page.getByRole("complementary", name);
}

/** Opens the panel whose toolbar button is `title` and waits for it; one already open is left as it is. */
export async function openPanel(page: Page, title: string): Promise<Locator> {
  const body = await panelBody(page, title);
  if (!(await body.isVisible())) await (await panel(page, title)).click();
  await expect(body).toBeVisible();
  return body;
}

/** Closes the panel named `title` with its Close button: a right panel's "Close panel", or the dock's "Close editor". */
export async function closePanel(page: Page, title: string): Promise<void> {
  const body = await panelBody(page, title);
  await body.getByRole("button", { name: /^Close (panel|editor)$/ }).click();
  await expect(body).toBeHidden();
}
