// features.spec.ts — each fixture project opens in the browser and DRAWS its feature; playing is transport.spec's and the golden render's.
// One entry per fixture in e2e/fixtures; the table is what a reader checks against fixtures/README.md.
import type { Page } from "@playwright/test";
import { runCommand } from "../../src/commandBar.ts";
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { folded, pane, tempoReadout } from "../../src/nav.ts";
import { openPanel } from "../../src/panels.ts";
import { expect, type K7, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

type Check = (page: Page) => Promise<void>;

const button = (page: Page, name: string) =>
  page.getByRole("button", { name, exact: true });
const lane =
  (name: string): Check =>
  async (page) => {
    await expect(button(page, `mute ${name}`)).toBeVisible();
  };

/** What each fixture must show once open; `bug` for a known failure. */
const FEATURES: Record<string, { check: Check; bug?: string }> = {
  "clip-lane.k7": { check: lane("drums") },
  "clip-loop.k7": { check: lane("drums") },
  "clip-fades.k7": { check: lane("drums") },
  "clip-warp.k7": { check: lane("drums") },
  "note-lane.k7": { check: lane("melody") },
  "strudel-lane.k7": { check: lane("pad") },
  "group-lane.k7": {
    check: async (page) => {
      await lane("kicks")(page);
      await lane("snares")(page);
      await expect(
        page.getByRole("group", { name: "drums", exact: true }),
      ).toBeVisible();
    },
  },
  "curves.k7": { check: lane("melody") },
  "rack.k7": { check: lane("beat") },
  "ports.k7": {
    check: async (page) => {
      await lane("deckA")(page);
      await lane("deckB")(page);
    },
  },
  "doc-note.k7": {
    check: async (page) => {
      // Notes show by default, a card named for the note; `ui notes` hides them.
      await (await pane(page, "Graph")).click();
      const note = page
        .getByRole("region", { name: "Graph", exact: true })
        .getByText("about-drums", { exact: true });
      await expect(note).toBeVisible();
      await runCommand(page, "ui notes");
      await expect(note).toBeHidden();
    },
  },
  "mute-solo.k7": {
    check: async (page) => {
      await expect(button(page, "solo kicks")).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      await expect(button(page, "mute muted")).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      await expect(button(page, "solo snares")).toHaveAttribute(
        "aria-pressed",
        "false",
      );
    },
  },
  "mixer-strip.k7": {
    check: async (page) => expect(await openPanel(page, "Mixer")).toBeVisible(),
  },
  "master-chain.k7": { check: lane("drums") },
  "lane-panel.k7": {
    check: async (page) => {
      await lane("drums")(page);
      // A double-click on the lane's name opens its chain in the dock (apps/k7-daw-frontend/docs/chainsGraph.md).
      await page.getByTitle(/^drums( —|$)/).dblclick();
      // The panel's sixth knob: the dock draws each module's own panel.
      await expect(
        page.getByRole("slider", { name: "SC gain", exact: true }),
      ).toBeVisible();
    },
  },
  "mastering.k7": {
    check: async (page) => {
      await openPanel(page, "Mixer");
      await expect(
        page.getByRole("group", { name: "Limiter", exact: true }),
      ).toBeVisible();
    },
  },
  "library-module.k7": { check: lane("drums") },
  "video-lane.k7": {
    check: async (page) =>
      expect(page.getByRole("dialog", { name: "Picture" })).toBeVisible(),
  },
  "markers-settings.k7": {
    check: async (page) => {
      // The metronome wears the bpm, and its bubble holds the meter.
      const metro = await tempoReadout(page, 96);
      await expect(metro).toHaveAttribute("aria-pressed", "true");
      await metro.click();
      await expect(
        page.getByRole("spinbutton", { name: "beats per bar" }),
      ).toHaveValue("3");
      await expect(
        page.getByRole("combobox", { name: "note value" }),
      ).toHaveText("4");
      await page.keyboard.press("Escape");
    },
  },
  "chains-only.k7": {
    check: async (page) => {
      await expect(await pane(page, "Graph")).toBeVisible();
      await expect(await pane(page, "Arrangement")).toHaveCount(0);
    },
  },
};

const BROKEN = ["broken-unresolved.k7", "broken-sequence.k7"];

async function openProject(k7: K7, project: string): Promise<Page> {
  const page = await k7.open(`/@local/${FIXTURES_WORKSPACE}/${project}`);
  await engineUp(page);
  return page;
}

for (const [project, { check, bug }] of Object.entries(FEATURES)) {
  test(`${project} draws its feature`, async ({ k7 }) => {
    test.fail(!!bug, bug);
    const page = await openProject(k7, project);
    const watch = watchPage(page);
    await check(page);
    watch.assertClean();
  });
}

for (const project of BROKEN) {
  test(`${project} says why it does not load`, async ({ k7 }) => {
    const page = await openProject(k7, project);
    const watch = watchPage(page);
    await expect(page.getByText(/timeline unavailable/)).toBeVisible();
    // The count is the badge on the rail's chevron (and on Logs, once the rail is open).
    await expect(page.getByTestId("problem-count").first()).toHaveText(/^\d+$/);
    watch.assertClean();
  });
}
