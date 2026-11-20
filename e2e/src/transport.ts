// transport.ts — rolling a project's transport from its bar at the foot, the way a person does.
// Play is pressed ONCE, as soon as it is offered: it reads "loading the project" until the engine holds it (docs/e2e.md).
import { expect, type Locator, type Page } from "@playwright/test";

/** The transport group, where the buttons and the clock are. */
export const transportBar = (page: Page): Locator =>
  page.getByRole("group", { name: "Transport", exact: true });

/** The position readout, a timer. */
export const clockOf = (page: Page): Locator =>
  transportBar(page).getByRole("timer");

/** Presses Play the moment it is enabled and checks the clock leaves where it stood; returns that reading. */
export async function play(page: Page, timeoutMs = 30_000): Promise<string> {
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
  // The clock reads `m:ss·bar:beat`, or `bar:beat` alone on a narrow screen: a beat is the part after the dot.
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
