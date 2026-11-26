// nav.ts — a control by its name, wherever the width put it: among the floating controls, or on the right-edge rail.
// The panes are `group "Pane"` and the panels `group "Panels"` buttons of the rail, folded under its chevron on every layout.
import type { Locator, Page } from "@playwright/test";

// The shell root says its layout in `data-layout` (phone, pad, desk); a page without one (home, settings) falls back to this query.
const MOBILE_QUERY = "(max-width: 700px), (max-height: 520px)";

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

/** Unfolds the right-edge rail, folded by default under its chevron: every layout keeps its panes and panels there. */
async function openNav(page: Page): Promise<void> {
  const show = page.getByRole("button", {
    name: "Show the toolbar",
    exact: true,
  });
  if (await show.isVisible()) await show.click();
}

/** The tempo readout: the metronome button "Metronome, <n> bpm — …", or a plain `<n> bpm` where there is no metronome, in the transport group. */
export async function tempoReadout(page: Page, n: number): Promise<Locator> {
  return page
    .getByRole("group", { name: "Transport", exact: true })
    .getByRole("button", { name: new RegExp(`^(Metronome, )?${n} bpm\\b`) });
}

/** A control named `name`: a button of the floating controls, or of the rail once it is unfolded. */
export async function navControl(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  return page.getByRole("button", { name, exact: true }).first();
}

/** A pane of the switcher: `group "Pane"` → `button`, on the right-edge rail. */
export async function pane(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  return page
    .getByRole("group", { name: "Pane", exact: true })
    .getByRole("button", { name, exact: true });
}

/** A panel's button: the right-edge rail (`group "Panels"`, a phone's Mixer too), or the desk's dock group (the Mixer). */
export async function panel(page: Page, name: string): Promise<Locator> {
  await openNav(page);
  const exact = { name, exact: true };
  return page
    .getByRole("group", { name: "Panels", exact: true })
    .getByRole("button", exact)
    .or(
      page
        .getByRole("group", { name: "Editor dock", exact: true })
        .getByRole("button", exact),
    )
    .first();
}
