// workspace-ui.spec.ts — a workspace and its projects made, renamed, copied and deleted through the home page's own dialogs.
// Each act is checked twice: on the screen, and in the file service's listing the page itself reads.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Locator, Page } from "@playwright/test";
import { gzipSync, strToU8, zipSync } from "fflate";
import { FileService } from "../../src/files.ts";
import { FIXTURES_DIR } from "../../src/fixtures.ts";
import { newMenu, projectLink, workspaceCard } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";
import { watchPage } from "../../src/watch.ts";

const WS = "E2E-ui";
const TARGET_WS = "E2E-ui-copy";
const DROP_WS = "E2E-ui-drop";
// The list re-reads after an act, and the relay may answer a beat behind the service.
const SHOWN = { timeout: 15_000 };
/** A module of the source workspace that the copied project extends, which the copy must bring along. */
const MODULE = "_modules/audio/compressor/module.k7x";

/** The account's workspaces and their project folders, as GET /workspaces/projects answers them. */
async function listing(page: Page): Promise<Record<string, string[]>> {
  const workspaces = await (await FileService.ofPage(page)).projects();
  return Object.fromEntries(
    workspaces.map((w) => [w.name, w.projects.map((p) => p.path).sort()]),
  );
}

/** Polls the listing until `ws` holds exactly `projects`; undefined means the workspace is absent. */
async function expectListed(
  page: Page,
  ws: string,
  projects: string[] | undefined,
): Promise<void> {
  await expect
    .poll(async () => (await listing(page))[ws], {
      message: `${ws} should list ${projects}`,
      timeout: 15_000,
    })
    .toEqual(projects);
}

/** A workspace's card on the home page: its header and its projects. */
function workspaceRow(page: Page, ws: string): Locator {
  return workspaceCard(page, ws);
}

/** The project's ⋮ menu inside `row`, opened on the item named `act`. */
async function projectAct(
  page: Page,
  row: Locator,
  label: string,
  act: string,
): Promise<Locator> {
  await row
    .getByRole("button", {
      name: `What can be done with ${label}`,
      exact: true,
    })
    .click();
  await page.getByRole("menuitem", { name: act, exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Makes a workspace through the "New workspace" dialog. */
async function newWorkspace(page: Page, ws: string): Promise<void> {
  await newMenu(page, "New workspace");
  const dialog = page.getByRole("dialog", { name: "New workspace" });
  await dialog.getByRole("textbox").fill(ws);
  await dialog.getByRole("button", { name: "Create", exact: true }).click();
  await expect(dialog).toBeHidden({ timeout: 30_000 });
  // The card appears with the next read of the list.
  await expect(workspaceRow(page, ws)).toBeVisible();
}

test("a workspace and its project are made, renamed, copied and deleted from the home page", async ({
  k7,
}) => {
  test.setTimeout(180_000);
  const files = await k7.files();
  // Recreated by the UI below, so a rerun starts from nothing.
  await files.removeWorkspace(WS);
  await files.removeWorkspace(TARGET_WS);

  const page = await k7.open("/");
  const watch = watchPage(page);
  await expect(
    page.getByRole("heading", { name: "Projects", level: 1 }),
  ).toBeVisible();

  await test.step("create two workspaces", async () => {
    await newWorkspace(page, WS);
    await newWorkspace(page, TARGET_WS);
    await expectListed(page, WS, []);
    await expectListed(page, TARGET_WS, []);
  });

  const row = workspaceRow(page, WS);

  await test.step("create a DJ set project in the archetype dialog", async () => {
    await row.getByRole("button", { name: `New project in ${WS}` }).click();
    const dialog = page.getByRole("dialog", { name: "New project" });
    await dialog.getByText("DJ set", { exact: true }).click();
    await expect(dialog.getByRole("radio", { name: /^DJ set/ })).toBeChecked();
    await dialog.getByRole("textbox").fill("first");
    await expect(dialog.getByText(`${WS}/first.k7`)).toBeVisible();
    await dialog.getByRole("button", { name: "Create", exact: true }).click();
    // The new project opens on its own; back on Home it is in its card.
    await expect(page).toHaveURL(/first\.k7/, { timeout: 30_000 });
    await page.goto("/");
    await expectListed(page, WS, ["first.k7"]);
    await expect(projectLink(page, WS, "first")).toBeVisible(SHOWN);
    await expect(row.getByText("DJ set", { exact: true })).toBeVisible();
  });

  await test.step("rename it", async () => {
    const dialog = await projectAct(page, row, "first", "Rename");
    await dialog
      .getByRole("textbox", { name: "New name", exact: true })
      .fill("renamed");
    await dialog.getByRole("button", { name: "Rename", exact: true }).click();
    await expect(dialog).toBeHidden({ timeout: 30_000 });
    await expectListed(page, WS, ["renamed.k7"]);
    await expect(projectLink(page, WS, "renamed")).toBeVisible(SHOWN);
    await expect(projectLink(page, WS, "first")).toHaveCount(0);
  });

  await test.step("copy it into the other workspace, with the @workspace module it extends", async () => {
    await files.writeText(
      WS,
      MODULE,
      readFileSync(join(FIXTURES_DIR, MODULE), "utf8"),
    );
    await files.writeText(
      WS,
      "renamed.k7/nodes/glue.k7x",
      'extends("@workspace/audio/compressor")\n',
    );
    const dialog = await projectAct(page, row, "renamed", "Copy");
    await dialog.getByRole("button", { name: TARGET_WS, exact: true }).click();
    await expect(dialog.getByText(`${TARGET_WS} / renamed.k7`)).toBeVisible();
    await dialog.getByRole("button", { name: "Copy", exact: true }).click();
    await expect(dialog).toBeHidden({ timeout: 60_000 });
    await expectListed(page, TARGET_WS, ["renamed.k7"]);
    await expect(projectLink(page, TARGET_WS, "renamed")).toBeVisible(SHOWN);
    await expectListed(page, WS, ["renamed.k7"]);
    // Without its module the copy would open on "extends unknown module" (apps/k7-daw-backend/docs/file-workspaces.md).
    expect(await files.readText(TARGET_WS, MODULE)).toBe(
      await files.readText(WS, MODULE),
    );
  });

  await test.step("delete the copy", async () => {
    const target = workspaceRow(page, TARGET_WS);
    const dialog = await projectAct(page, target, "renamed", "Delete");
    await expect(
      dialog.getByRole("heading", { name: "Delete “renamed”?" }),
    ).toBeVisible();
    await dialog.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(dialog).toBeHidden({ timeout: 30_000 });
    await expectListed(page, TARGET_WS, []);
    await expect(projectLink(page, TARGET_WS, "renamed")).toHaveCount(0, SHOWN);
    // The original is untouched by deleting its copy.
    await expectListed(page, WS, ["renamed.k7"]);
  });

  watch.assertClean();
  await files.removeWorkspace(WS);
  await files.removeWorkspace(TARGET_WS);
});

/** A set with one audio clip whose sample sits in the zip beside it: the folder Live's Collect All and Save leaves. */
const SET = `<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12402" Creator="Ableton Live 12.0"><LiveSet>
<MainTrack><DeviceChain><Mixer><Tempo><Manual Value="110" /></Tempo><TimeSignature><Manual Value="201" /></TimeSignature></Mixer></DeviceChain></MainTrack>
<Tracks><AudioTrack Id="1"><Name><EffectiveName Value="audio" /></Name><DeviceChain><Mixer><Speaker><Manual Value="true" /></Speaker></Mixer>
<MainSequencer><Sample><ArrangerAutomation><Events><AudioClip Id="0" Time="0"><CurrentStart Value="0" /><CurrentEnd Value="8" />
<Loop><LoopStart Value="0" /><LoopEnd Value="8" /><StartRelative Value="0" /><LoopOn Value="false" /></Loop><Name Value="take" />
<SampleRef><FileRef><RelativePath Value="Samples/take.wav" /><Path Value="C:/x/take.wav" /><Name Value="take.wav" /></FileRef><DefaultSampleRate Value="44100" /></SampleRef>
<IsWarped Value="true" /><WarpMarkers></WarpMarkers></AudioClip></Events></ArrangerAutomation></Sample></MainSequencer></DeviceChain></AudioTrack></Tracks></LiveSet></Ableton>`;

test("an Ableton project zipped and dropped on a workspace row is translated by the page into a project with its sample", async ({
  k7,
}) => {
  test.setTimeout(180_000);
  const files = await k7.files();
  await files.removeWorkspace(DROP_WS);
  const page = await k7.open("/");
  const watch = watchPage(page);
  await expect(
    page.getByRole("heading", { name: "Projects", level: 1 }),
  ).toBeVisible();
  await newWorkspace(page, DROP_WS);
  await expectListed(page, DROP_WS, []);

  const wav = Buffer.alloc(44 + 4800, 1);
  wav.write("RIFF", 0);
  wav.write("WAVEfmt ", 8);
  const zip = zipSync({
    "Song/Song.als": gzipSync(strToU8(SET)),
    "Song/Samples/take.wav": new Uint8Array(wav),
  });
  await test.step("drop the zip on the workspace's card", async () => {
    const data = await page.evaluateHandle(
      ({ name, bytes }) => {
        const d = new DataTransfer();
        d.items.add(new File([new Uint8Array(bytes)], name));
        return d;
      },
      { name: "Song.zip", bytes: [...zip] },
    );
    const row = workspaceRow(page, DROP_WS);
    for (const type of ["dragenter", "dragover", "drop"])
      await row.dispatchEvent(type, { dataTransfer: data });
  });

  await test.step("name it and read the log", async () => {
    const dialog = page.getByRole("dialog", { name: "Import this project" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("textbox").fill("dropped");
    await dialog.getByRole("button", { name: "Import", exact: true }).click();
    await expect(dialog.getByText("wrote dropped.k7")).toBeVisible({
      timeout: 60_000,
    });
  });

  await test.step("the project and its sample are in the workspace", async () => {
    await expectListed(page, DROP_WS, ["dropped.k7"]);
    expect(await files.readText(DROP_WS, "dropped.k7/project.k7x")).toContain(
      "tempo",
    );
    const tree = JSON.stringify(await files.tree(DROP_WS));
    expect(tree).toMatch(/clips[\s\S]*\.wav/);
  });

  watch.assertClean();
  await files.removeWorkspace(DROP_WS);
});
