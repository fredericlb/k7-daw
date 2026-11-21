// nav.ts — a header control by its name, wherever the width put it: in the header, or in the phone's overflow menu.
// The panes are `group "Pane"` buttons and the panels `toolbar "Panels"` buttons; a phone holds both as buttons of `dialog "Views and panels"`.
import type { Locator, Page } from "@playwright/test";

// The shell root says its layout in `data-layout` (phone, pad, desk); a page without one (home, settings) falls back to this query.
const MOBILE_QUERY = "(max-width: 820px), (max-height: 520px)";

/** True on a phone layout. */
export async function folded(page: Page): Promise<boolean> {
  return page.evaluate((q) => {
    const doc = globalThis as unknown as {
      document: {
        querySelector(
          s: string,
        ): { getAttribute(n: string): string | null } | null;
      };
      matchMedia(query: string): { matches: boolean };
    };
    const layout = doc.document
      .querySelector("[data-layout]")
      ?.getAttribute("data-layout");
    return layout ? layout === "phone" : doc.matchMedia(q).matches;
  }, MOBILE_QUERY);
}

/** Opens the phone's overflow menu, so what it holds can be found. */
async function openNav(page: Page): Promise<void> {
  if (!(await folded(page))) return;
  const menu = page.getByRole("button", {
    name: "Views and panels",
    exact: true,
  });
  if ((await menu.getAttribute("aria-expanded")) !== "true") await menu.click();
}

/** The tempo readout `<n> bpm`: a button of the transport group. */
export async function tempoReadout(page: Page, n: number): Promise<Locator> {
  return page
    .getByRole("group", { name: "Transport", exact: true })
    .getByRole("button", { name: `${n} bpm`, exact: true });
}

/** A header tool named `name`: a button of the header, or the overflow menu's item once it is open. */
export async function navControl(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("button", exact)
    .or(
      page
        .getByRole("dialog", { name: "Views and panels" })
        .getByRole("button", exact),
    )
    .first();
}

/** A pane of the switcher: `group "Pane"` → `button`, in the header or in the phone's dialog. */
export async function pane(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("group", { name: "Pane", exact: true })
    .getByRole("button", exact)
    .or(
      page
        .getByRole("dialog", { name: "Views and panels" })
        .getByRole("button", exact),
    )
    .first();
}

/** A right panel's button: `toolbar "Panels"` → `button`, or a tile of the phone's dialog. */
export async function panel(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("toolbar", { name: "Panels", exact: true })
    .getByRole("button", exact)
    .or(
      page
        .getByRole("dialog", { name: "Views and panels" })
        .getByRole("button", exact),
    )
    .first();
}
