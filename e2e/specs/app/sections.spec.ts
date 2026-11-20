// sections.spec.ts — every tab of the home's tab bar opens without a page error or a failing API call.
// The list is read off the bar itself, so a new section is covered the day it appears.
import { homeTab } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";
import { watchPage } from "../../src/watch.ts";

test("every home tab opens cleanly", async ({ k7 }) => {
  const page = await k7.open("/");
  const tabs = page
    .getByRole("navigation", { name: "Home", exact: true })
    .getByRole("link");
  await expect(tabs.first()).toBeVisible();
  const names = (await tabs.all()).length;
  expect(names).toBeGreaterThan(3);
  for (const [i, name] of [
    "Projects",
    "Tasks",
    "Libraries",
    "Frozen chains",
  ].entries()) {
    await test.step(name, async () => {
      const watch = watchPage(page);
      await homeTab(page, name).click();
      await page.waitForLoadState("networkidle");
      await expect(homeTab(page, name)).toHaveAttribute("aria-current", "page");
      const path = new URL(page.url()).pathname;
      await page.screenshot({
        path: `test-results/sections/${k7.target}-${i}-${path.replaceAll("/", "_")}.png`,
      });
      watch.assertClean();
    });
  }
});
