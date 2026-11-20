// nav.ts — a header control by its name, wherever the width put it: in the header, or in k7's Menu on a phone.
// On a phone the panes become menu items and the header's tools move into the same menu (apps/k7-daw-frontend/src/components/MobileChrome.tsx).
import type { Locator, Page } from "@playwright/test";

// useIsMobile's query (apps/k7-daw-frontend/src/useIsMobile.ts); a desktop header has a Menu button too, so the button alone says nothing.
const MOBILE_QUERY = "(max-width: 820px), (max-height: 520px)";

/** True on a phone layout: the shell root says it in `data-layout`; a page without one (home, settings) falls back to the media query. */
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

/** Opens the Menu when the header has folded, so what it holds can be found. */
async function openNav(page: Page): Promise<void> {
  if (!(await folded(page))) return;
  const menu = page
    .getByRole("button", { name: "Views and panels", exact: true })
    .or(page.getByRole("button", { name: "Menu", exact: true }))
    .first();
  if ((await menu.getAttribute("aria-expanded")) !== "true") await menu.click();
}

/** The tempo readout `<n> bpm`: a button of the transport, or on today's phone the one in the Info popover. */
export async function tempoReadout(page: Page, n: number): Promise<Locator> {
  const name = { name: `${n} bpm`, exact: true };
  const direct = page.getByRole("button", name);
  const info = page.getByRole("button", { name: "Project and engine" });
  if (!(await folded(page)) || (await info.count()) === 0) return direct;
  if ((await info.getAttribute("aria-expanded")) !== "true") await info.click();
  return page
    .getByRole("dialog", { name: "Project and engine" })
    .getByRole("button", name);
}

/** A pane or header tool named `name`: a header button, or the Menu's item or button once it is open. */
export async function navControl(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  // A tool can be both: a phone's Commands is the Menu's item and the footer's button, the same act.
  return page
    .getByRole("button", exact)
    .or(page.getByRole("menu").getByRole("menuitem", exact))
    .first();
}

// The restyle renames the panes; each keeps its old name until then, so one helper serves both.
const OLD_PANE: Record<string, string> = {
  Arrangement: "Timeline",
  Graph: "Chains",
  Source: "Editor",
};

/** A pane of the switcher: `group "Pane"` → `button`, a phone's `menuitemradio`, or the header button it is today. */
export async function pane(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("group", { name: "Pane", exact: true })
    .getByRole("button", exact)
    .or(page.getByRole("menuitemradio", exact))
    .or(await navControl(page, OLD_PANE[name] ?? name))
    .first();
}

/** A right panel's button: `toolbar "Panels"` → `button`, a phone's `menuitem`, or the header button it is today. */
export async function panel(
  page: Page,
  name: string,
  oldName: string = name,
): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("toolbar", { name: "Panels", exact: true })
    .getByRole("button", exact)
    .or(page.getByRole("menuitem", exact))
    .or(await navControl(page, oldName))
    .first();
}
