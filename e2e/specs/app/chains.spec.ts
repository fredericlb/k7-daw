// chains.spec.ts — the Chains view edits the graph: a module added on a cable is wired in, a preset switches a node's base.
// Each edit is read back from disk and taken back by `project undo`; chains-edit.k7 is put back before every test.
import type { Locator, Page } from "@playwright/test";
import { closeCommandBar, runCommand } from "../../src/commandBar.ts";
import { expectFile, readFile, restoreProject } from "../../src/disk.ts";
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { folded, pane } from "../../src/nav.ts";
import { expect, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

const WS = FIXTURES_WORKSPACE;
const PROJECT = "chains-edit.k7";
const GRAPH = `${PROJECT}/graph.k7x`;
const SYNTH_NODE = `${PROJECT}/nodes/pad.k7x`;
const SYNTH_MODULE = "@k7-core/instruments/synth/surge-xt";
const EFFECT = "tremolo";

type Point = { x: number; y: number };

test.beforeEach(async ({ k7 }) => {
  await restoreProject(await k7.files(), WS, PROJECT);
});

async function openChains(k7: {
  open(path: string): Promise<Page>;
}): Promise<Page> {
  const page = await k7.open(`/@local/${WS}/${PROJECT}`);
  await engineUp(page);
  await (await pane(page, "Graph")).click();
  return page;
}

/** The graph canvas: a region named "Graph". */
const canvas = (page: Page): Locator =>
  page.getByRole("region", { name: "Graph", exact: true });

/** A card of the canvas, named "node (module)" or just its name: a button while zoomed out, a group once its panel is drawn. */
const card = (page: Page, name: string): Locator =>
  canvas(page)
    .getByRole("button", { name, exact: true })
    .or(canvas(page).getByRole("group", { name, exact: true }));

async function centre(of: Locator): Promise<Point> {
  const box = await of.boundingBox();
  if (!box) throw new Error("the card is not drawn");
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** The `+` on the cable from `from` to `to`, a button hidden until it is hovered or focused. */
const cablePlus = (page: Page, from: string, to: string): Locator =>
  page.getByRole("button", {
    name: `Add a module between ${from} and ${to}`,
    exact: true,
  });

/** Picks EFFECT on the MIX → OUT cable and waits for it on disk; returns the new node's id and graph.k7x as it was. */
async function pickOnCable(
  page: Page,
): Promise<{ node: string; before: string }> {
  const mix = card(page, "MIX");
  const out = card(page, "OUT");
  await expect(mix).toBeVisible();
  await expect(out).toBeVisible();
  // The view pins every card it laid out (`graph pin`, journaled as nothing): read once it has, so undo's text compares with it.
  const before = await expectFile(page, WS, GRAPH, (t) =>
    t.includes("positions("),
  );

  // Enter on the focused `+` is what a keyboard does, and needs no pointer on a cable drawn in SVG.
  const plus = cablePlus(page, "MIX", "OUT");
  const picker = page.getByRole("dialog", { name: "Modules", exact: true });
  const search = picker.getByRole("combobox", { name: "Search modules" });
  // Before the module catalogue lands the `+` pre-writes the command instead (apps/k7-daw-frontend/src/shell/useGraphActions.ts): put it away and press again.
  await expect(async () => {
    await closeCommandBar(page);
    await plus.focus();
    await plus.press("Enter");
    await expect(search).toBeVisible({ timeout: 2_000 });
  }).toPass({ timeout: 30_000 });
  await search.fill(EFFECT);
  await picker
    .getByRole("option", { name: EFFECT, exact: true })
    .first()
    .click();

  // The new node takes MIX's place before OUT: MIX feeds it, it feeds OUT.
  const graph = await expectFile(page, WS, GRAPH, (t) =>
    new RegExp(`MIX: \\["([\\w-]*${EFFECT}[\\w-]*)"\\]`).test(t),
  );
  const node = (graph.match(/MIX: \["([^"]+)"\]/) as RegExpMatchArray)[1];
  expect(graph).toMatch(new RegExp(`${node}: \\["OUT"\\]`));
  await expectFile(page, WS, `${PROJECT}/nodes/${node}.k7x`, (t) =>
    t.includes(`/audio/${EFFECT}`),
  );
  await expect(
    card(page, `${node} (${EFFECT})`).or(card(page, EFFECT)).first(),
  ).toBeVisible();
  return { node, before };
}

test("the bar's right-end capsule follows the pane: Graph views on the Graph, none on the Source", async ({
  k7,
}) => {
  const page = await openChains(k7);
  // A phone folds the capsule into the overflow menu, which the nav helpers cover.
  test.skip(await folded(page), "the capsule is a desktop bar's");
  const views = page.getByRole("group", { name: "Graph views", exact: true });
  await expect(views.getByRole("button", { name: "Zoom fit" })).toBeVisible();
  await expect(
    views.getByRole("button", { name: "Auto layout" }),
  ).toBeVisible();
  const stack = views.getByRole("button", { name: "Stack" });
  await expect(stack).toHaveAttribute("aria-pressed", "false");
  await stack.click();
  await expect(stack).toHaveAttribute("aria-pressed", "true");
  await stack.click();
  await expect(stack).toHaveAttribute("aria-pressed", "false");
  await (await pane(page, "Source")).click();
  await expect(views).toHaveCount(0);
  await expect(
    page.getByRole("group", { name: "Lane views", exact: true }),
  ).toHaveCount(0);
});

test("a library effect picked on the MIX → OUT cable is created and wired between them", async ({
  k7,
}) => {
  const page = await openChains(k7);
  const watch = watchPage(page);
  const { before } = await pickOnCable(page);
  expect(before).toMatch(/MIX: \["OUT"\]/);
  watch.assertClean();
});

test("one project undo takes a whole pick back: the node and its splice", async ({
  k7,
}) => {
  const page = await openChains(k7);
  const watch = watchPage(page);
  const { node, before } = await pickOnCable(page);
  await runCommand(page, "project undo");
  const undone = await expectFile(page, WS, GRAPH, (t) => !t.includes(node));
  expect(undone).toBe(before);
  watch.assertClean();
});

/** Zooms the canvas on `target` until its panel is drawn, which is when its preset dropdown exists. */
async function zoomOnto(page: Page, target: Locator, until: Locator) {
  const at = await centre(target);
  const box = await target.boundingBox();
  // The top of the card, where its header and the dropdown are, stays under the pointer as the camera zooms.
  await page.mouse.move(at.x, (box?.y ?? at.y) + 20);
  // One notch at a time, each followed by a short wait for the panel the camera's new zoom draws.
  await expect(async () => {
    if (await until.isVisible()) return;
    await page.mouse.wheel(0, -200);
    await expect(until).toBeVisible({ timeout: 250 });
  }).toPass({ timeout: 15_000, intervals: [0] });
}

test("a factory preset from the node's dropdown switches the Surge node's base, and undo restores it", async ({
  k7,
}) => {
  const page = await openChains(k7);
  const watch = watchPage(page);
  const synth = card(page, "pad (surge-xt)");
  await expect(synth).toBeVisible();
  const before = await readFile(page, WS, SYNTH_NODE);
  expect(before).not.toContain("base");

  const dropdown = canvas(page).getByRole("button", { name: /^preset(: |$)/ });
  await zoomOnto(page, synth, dropdown);
  await dropdown.click();
  const shelf = page.getByRole("dialog", { name: "Presets", exact: true });
  await expect(
    shelf.getByRole("combobox", { name: "Search presets" }),
  ).toBeVisible();
  await shelf
    .getByRole("option", { name: "Basses - Bass 2", exact: true })
    .click();

  const after = await expectFile(page, WS, SYNTH_NODE, (t) =>
    /Basses\/Bass 2/.test(t),
  );
  expect(after).toContain(`extends("${SYNTH_MODULE}")`);

  // The engine is sent the patch the base names, not the library default.
  const files = await k7.files();
  const doc = (await files.engineDoc(WS)) as {
    tracks: { plugins: { surgePatch?: string }[] }[];
  };
  const patches = doc.tracks
    .flatMap((t) => t.plugins)
    .flatMap((p) => (p.surgePatch ? [p.surgePatch] : []));
  expect(patches).toHaveLength(1);
  expect(patches[0]).toMatch(/Basses[\\/]Bass 2\.fxp$/);

  await runCommand(page, "project undo");
  await expectFile(page, WS, SYNTH_NODE, (t) => t === before);
  watch.assertClean();
});
