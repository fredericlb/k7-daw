// transport.ts — rolling a project's transport from its bar at the foot, the way a person does.
// Play is pressed ONCE, as soon as it is offered: it reads "loading the project" until the engine holds it (docs/e2e.md).
import { expect, type Locator, type Page } from "@playwright/test";
import { folded } from "./nav.ts";

/** The transport group, where the buttons and the clock are. */
export const transportBar = (page: Page): Locator =>
  page.getByRole("group", { name: "Transport", exact: true });

/** The position readout, a timer. */
export const clockOf = (page: Page): Locator =>
  transportBar(page).getByRole("timer");

/** On a phone a panel is a full-screen sheet over the transport (docs/shell.md): closed first, as a person would. */
export async function uncover(page: Page): Promise<void> {
  if (!(await folded(page))) return;
  for (const sheet of await page
    .locator('[role="dialog"][data-slot="panel-frame"]')
    .all())
    if (await sheet.isVisible())
      await sheet
        .getByRole("button", { name: /^Close/ })
        .first()
        .click({ timeout: 5_000 })
        .catch(() => {});
}

/** Presses Play the moment it is enabled and checks the clock leaves where it stood; returns that reading. */
export async function play(page: Page, timeoutMs = 30_000): Promise<string> {
  await uncover(page);
  const clock = clockOf(page);
  const button = transportBar(page).getByRole("button", {
    name: "Play",
    exact: true,
  });
  await expect(button).toBeEnabled({ timeout: timeoutMs });
  const before = await clock.innerText();
  await button.click();
  // A picture project on a phone has taken more than 5 s to move its clock on the first play.
  await expect(clock).not.toHaveText(before, { timeout: 15_000 });
  return before;
}

/** Waits until the clock has read `beats` more beats than it did: the engine is rendering the project, not just started. */
export async function rolls(page: Page, beats: number): Promise<void> {
  const seen = new Set<string>();
  // The clock reads one timecode, `bar:beat` on a song: each new reading is a beat gone by.
  const beatsSeen = async () => {
    const text = await clockOf(page).innerText();
    seen.add(text.split("·").at(-1) ?? text);
    return seen.size;
  };
  await expect
    .poll(beatsSeen, { timeout: 30_000, intervals: [50] })
    .toBeGreaterThan(beats);
}

/** The engine's light: the header's `engine-status`, whose `data-state` is up, down or none. */
export const engineLight = (page: Page): Locator =>
  page.getByTestId("engine-status").first();

/** Waits until the page says its engine is connected. */
export async function engineUp(page: Page, timeoutMs = 30_000): Promise<void> {
  await expect(engineLight(page)).toHaveAttribute("data-state", "up", {
    timeout: timeoutMs,
  });
}
