// routes.spec.ts — the home's side pages and the two hosts no other spec opens: each draws its own content, not just no error.
// `/external`, `/settings`, `/settings/engine`, `/libraries`, `/about`, `/tasks`, and the docs host.
import type { Page } from "@playwright/test";
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { homeTab } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

/** Moves the router to `path` without a reload, as a link of the router does. */
async function routeTo(page: Page, path: string): Promise<void> {
  await page.evaluate((to) => {
    const win = globalThis as unknown as {
      history: { pushState(s: unknown, t: string, u: string): void };
      dispatchEvent(e: unknown): void;
      PopStateEvent: new (type: string) => unknown;
    };
    win.history.pushState({}, "", to);
    win.dispatchEvent(new win.PopStateEvent("popstate"));
  }, path);
}

test("/external lists the surfaces of the project the engine holds", {
  tag: "@tier4",
}, async ({ k7 }) => {
  // The menu lists what the engine already holds, so a project is opened on it first.
  const project = await k7.open(`/@local/${FIXTURES_WORKSPACE}/clip-lane.k7`);
  await engineUp(project);
  const page = await k7.open("/external");
  const watch = watchPage(page);
  await expect(
    page.getByRole("heading", {
      name: `${FIXTURES_WORKSPACE} / clip-lane.k7`,
    }),
  ).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("button", { name: "Refresh" })).toBeVisible();
  const timeline = page.getByRole("link", { name: /timeline/i });
  await expect(timeline).toHaveAttribute(
    "href",
    new RegExp(
      `/@local/${FIXTURES_WORKSPACE}/clip-lane\\.k7/@external/timeline$`,
    ),
  );
  watch.assertClean();
});

test("/settings draws every section with its controls, a tab at a time", async ({
  k7,
}) => {
  const page = await k7.open("/settings");
  const watch = watchPage(page);
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  const sections = async (names: string[]) => {
    for (const name of names)
      await expect(
        page.getByRole("heading", { name, exact: true }),
      ).toBeVisible();
  };
  // General opens first: the language, the AI key, the separation chains and About (apps/k7-daw-frontend/docs/settings.md).
  await expect(page.getByRole("tablist", { name: "Settings" })).toBeVisible();
  await sections(["Language", "AI"]);
  await expect(page.getByRole("region", { name: "About" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Français" })).toBeEnabled();
  await expect(
    page.getByRole("textbox", { name: "New API key" }),
  ).toBeVisible();
  // Devices: this device's paired devices (apps/k7-daw-frontend/docs/settings.md).
  await page.getByRole("tab", { name: "Devices" }).click();
  await sections(["Paired devices"]);
  await expect(page.getByRole("link", { name: /^Engine/ })).toBeVisible();
  await expect(page).toHaveURL(/[?&]tab=devices/);
  // A link straight to a tab opens on it.
  await page.reload();
  await expect(page.getByRole("tab", { name: "Devices" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  watch.assertClean();
});

test("/settings/engine opens as a route change, off the tray", {
  tag: "@tier4",
}, async ({ k7 }) => {
  const page = await k7.open("/settings");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  const watch = watchPage(page);
  // Reached the way App settings reaches it: a route change, not a load.
  await routeTo(page, "/settings/engine");
  await expect(page.getByRole("heading", { name: "Engine" })).toBeVisible();
  watch.assertClean();
});

test("/settings/engine opens when loaded by its address", {
  tag: "@tier4",
}, async ({ k7 }) => {
  const page = await k7.open("/settings/engine");
  await expect(page.getByRole("heading", { name: "Engine" })).toBeVisible({
    timeout: 10_000,
  });
});

test("/libraries lists the bundled libraries, read-only", async ({ k7 }) => {
  const page = await k7.open("/libraries");
  const watch = watchPage(page);
  await expect(
    page.getByRole("heading", { name: "Library", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("@k7-core", { exact: true }).first(),
  ).toBeVisible();
  await expect(page.getByTestId("libraries-readonly-note")).toBeVisible();
  await expect(
    page.getByRole("button", { name: /^(add|remove|publish draft)$/i }),
  ).toHaveCount(0);
  watch.assertClean();
});

test("About, inside App settings, shows the licence and filters the third-party notices", async ({
  k7,
}) => {
  // The old address lands on App settings, where About lives.
  const page = await k7.open("/about");
  const watch = watchPage(page);
  await expect(page).toHaveURL(/\/settings/);
  await expect(page.getByRole("region", { name: "About" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Licence" })).toBeVisible();
  await page.getByRole("button", { name: "Read it" }).click();
  await expect(
    page.getByText(/GNU GENERAL PUBLIC LICENSE/).first(),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Third-party notices" }),
  ).toBeVisible();
  const count = page.getByTestId("notices-count");
  const all = Number((await count.innerText()).split(" ")[2]);
  expect(all).toBeGreaterThan(10);
  await page
    .getByRole("textbox", { name: "Search by name, licence or version" })
    .fill("codemirror");
  await expect
    .poll(async () => Number((await count.innerText()).split(" ")[0]))
    .toBeLessThan(all);
  const notice = page.getByRole("button", { name: /^@codemirror\/state / });
  await notice.click();
  await expect(
    page.getByText(/Permission is hereby granted/).first(),
  ).toBeVisible();
  watch.assertClean();
});

test("/tasks lists the workspaces' TODOS.md, done ones on demand", async ({
  k7,
}) => {
  const page = await k7.open("/tasks");
  const watch = watchPage(page);
  await expect(page.getByRole("heading", { name: "Tasks" })).toBeVisible();
  const tasks = page.getByRole("region", { name: /^Tasks of / });
  await expect(tasks.getByText("E2E open task")).toBeVisible();
  await expect(tasks.getByText("E2E done task")).toBeHidden();
  await page.getByRole("switch", { name: "Show done" }).click();
  await expect(tasks.getByText("E2E done task")).toBeVisible();
  watch.assertClean();
});

test("/docs on the app's origin is no page, and says the way back", async ({
  k7,
}) => {
  const page = await k7.open("/docs");
  const watch = watchPage(page);
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: "That project isn't in the workspace",
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Back to Projects" }).click();
  await expect(homeTab(page, "Projects")).toBeVisible();
  watch.assertClean();
});
