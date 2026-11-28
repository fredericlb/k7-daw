// touch.spec.ts — on a touch screen the timeline moves under two fingers only: a one-finger swipe scrolls nothing, two fingers scroll.
// Touches are the browser's own (CDP Input.dispatchTouchEvent), so `touch-action` decides as it does on a tablet (apps/k7-daw-frontend/docs/timeline.md).
import type { Page } from "@playwright/test";
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { expect, TARGET, test } from "../../src/target.ts";

const WS = FIXTURES_WORKSPACE;
const PROJECT = "edit-undo.k7";

test.use({ hasTouch: true });
test.skip(TARGET !== "web", "the phone suites touch with real fingers");

type Pt = { x: number; y: number };

/** The timeline's scroller: the one container with lanes in it that reads `touch-action: none`. */
async function scroller(page: Page): Promise<{ left: number; box: DOMRect }> {
  return page.evaluate(() => {
    const el = [...document.querySelectorAll<HTMLElement>("div")].find(
      (d) =>
        d.style.touchAction === "none" && d.querySelector("[data-sel-row]"),
    );
    if (!el) throw new Error("no timeline scroller");
    return { left: el.scrollLeft, box: el.getBoundingClientRect().toJSON() };
  });
}

/** A swipe of `fingers` side by side, `dx` pixels to the left, in ten steps. */
async function swipe(page: Page, at: Pt, fingers: number, dx: number) {
  const cdp = await page.context().newCDPSession(page);
  const points = (t: number) =>
    Array.from({ length: fingers }, (_, i) => ({
      x: at.x - dx * t,
      y: at.y + i * 40,
      id: i,
    }));
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: points(0),
  });
  for (let s = 1; s <= 10; s++)
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchMove",
      touchPoints: points(s / 10),
    });
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchEnd",
    touchPoints: [],
  });
  await cdp.detach();
}

test("one finger scrolls no lane; two fingers scroll them", async ({ k7 }) => {
  const page = await k7.open(`/@local/${WS}/${PROJECT}`);
  await expect(
    page.getByRole("button", { name: "mute melody", exact: true }),
  ).toBeVisible({ timeout: 30_000 });
  // Zoomed in, so the lanes are wider than the pane and have somewhere to scroll.
  const { box } = await scroller(page);
  const mid = { x: box.x + box.width * 0.6, y: box.y + 40 };
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.down("Control");
  for (let i = 0; i < 6; i++) await page.mouse.wheel(0, -100);
  await page.keyboard.up("Control");
  await expect.poll(async () => (await scroller(page)).left).toBeGreaterThan(0);
  const before = (await scroller(page)).left;

  await swipe(page, mid, 1, 200);
  await page.waitForTimeout(300);
  expect((await scroller(page)).left).toBe(before);

  await swipe(page, mid, 2, 200);
  await expect
    .poll(async () => (await scroller(page)).left)
    .toBeGreaterThan(before + 150);
});
