// lanSurfaces.spec.ts — a second front (B, no engine) opens a surface of the project front A has open, and A cannot
// open the pane B shows. The bed starts B beside A and names it in E2E_PEER_URL; without it the spec skips.
// What a reader gets wrong: the two fronts share this browser's cookies, since a cookie ignores the port.
import type { Page } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";
import { expect, test } from "../../src/target.ts";
import { clockOf, engineUp, transportBar } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

const PEER = process.env.E2E_PEER_URL;
const WORKSPACE = "E2E-lan";
const PROJECT = "lan.k7";

test.skip(!PEER, "E2E_PEER_URL names the second front the bed starts");
test.describe.configure({ mode: "serial" });
test.setTimeout(180_000);

const paneButton = (page: Page, name: string) =>
  page
    .getByRole("group", { name: "Pane", exact: true })
    .getByRole("button", { name });

test("a second device lists, opens and holds the desk, and the main cannot open it", async ({
  k7,
  context,
}) => {
  const files = await k7.files();
  await files.removeWorkspace(WORKSPACE);
  await files.createWorkspace(WORKSPACE);
  // An arrangement with a desk beside its timeline: a clock to roll, and a pane for the other device to hold.
  await files.createProject(WORKSPACE, "mix", "desk.k7");
  const desk = await files.readText(WORKSPACE, "desk.k7/surfaces/desk.k7x");
  await files.createProject(WORKSPACE, "arrangement", PROJECT);
  await files.writeText(WORKSPACE, `${PROJECT}/surfaces/desk.k7x`, desk);

  // 1. A opens the project; B's Surfaces tab lists A's device with the desk row.
  const main = await k7.open(`/@local/${WORKSPACE}/${PROJECT}`);
  const watch = watchPage(main);
  await engineUp(main);
  // The panes are on the right-edge rail, folded under the header's chevron.
  await main
    .getByRole("button", { name: "Show the toolbar", exact: true })
    .click();
  await expect(paneButton(main, "Turntables")).toBeEnabled();
  await context.addCookies(sessionCookies(PEER as string));
  const remote = await context.newPage();
  await remote.goto(`${PEER}/surfaces`);
  const deskRow = remote.getByRole("link", { name: /desk/ });
  await expect(deskRow).toBeVisible({ timeout: 60_000 });

  // The main is on the desk when the remote opens it: the main moves to the Graph, with a toast.
  await paneButton(main, "Turntables").click();
  await expect(paneButton(main, "Turntables")).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // 2. B opens it: no pane switcher, the transport capsule at the bottom, and Play on B moves A's clock.
  await deskRow.click();
  await expect(remote).toHaveURL(
    /\/@lan\/[^/]+\/E2E-lan\/lan\.k7\/@external\/desk$/,
  );
  await expect(transportBar(remote)).toBeVisible({ timeout: 60_000 });
  await expect(
    remote.getByRole("group", { name: "Pane", exact: true }),
  ).toHaveCount(0);
  await expect(remote.getByRole("link", { name: "Home" })).toBeVisible();

  // 4. The main was on the desk: it lands on the Graph and says so.
  await expect(paneButton(main, "Graph")).toHaveAttribute(
    "aria-pressed",
    "true",
    {
      timeout: 15_000,
    },
  );
  await expect(main.getByText(/opened on/)).toBeVisible();

  // 3. The desk is blocked on A, said by its tooltip, and pressing it says so again.
  const blocked = paneButton(main, "Turntables");
  await expect(blocked).toHaveAttribute("aria-disabled", "true");
  await expect(blocked).toHaveAttribute("title", /^In use on /);

  const before = await clockOf(main).innerText();
  await transportBar(remote)
    .getByRole("button", { name: "Play", exact: true })
    .click();
  await expect(clockOf(main)).not.toHaveText(before, { timeout: 20_000 });
  await transportBar(remote)
    .getByRole("button", { name: "Stop", exact: true })
    .click();

  // 5. B goes Home: A's desk is free again within a few seconds.
  await remote.getByRole("link", { name: "Home" }).click();
  await expect(remote).toHaveURL(/\/surfaces$/);
  await expect(blocked).not.toHaveAttribute("aria-disabled", "true", {
    timeout: 10_000,
  });
  watch.assertClean();
});
