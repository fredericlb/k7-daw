// git-autosave.spec.ts — an open project's edit reaches the person's technical branch on its own after 20 quiet seconds,
// the rail's Save says when, and Save squashes it onto main and restarts the branch from there.
// The remote is a folder this spec makes with `git`, so only the web target reaches it.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { panel } from "../../src/nav.ts";
import { expect, test } from "../../src/target.ts";

const WS = "E2E-autosave";
const EMAIL = "e2e@example.org";
const BRANCH = "k7/autosave/e2e-at-example.org";
const SLOW = { timeout: 90_000 };

const git = (dir: string, ...args: string[]): string =>
  execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" });

/** What the bare repository holds at `rev`, or "" when the ref or the file is not there. */
const shown = (bare: string, rev: string): string => {
  try {
    return git(bare, "show", rev);
  } catch {
    return "";
  }
};

/** The commit `ref` names in the bare repository, or "" when it does not exist. */
const tip = (bare: string, ref: string): string => {
  try {
    return git(bare, "rev-parse", "--verify", "--quiet", ref).trim();
  } catch {
    return "";
  }
};

test("an edit is auto-saved to the person's branch and a Save squashes it onto main", async ({
  k7,
}) => {
  test.skip(k7.target !== "web", "the remote is a folder on this machine");
  test.setTimeout(300_000);
  const dir = mkdtempSync(join(tmpdir(), "k7-autosave-e2e-"));
  const bare = join(dir, "remote.git");
  execFileSync("git", ["init", "--bare", "-b", "main", bare]);
  const files = await k7.files();
  await files.removeWorkspace(WS);
  await files.createWorkspace(WS);
  const { project } = await files.createProject(WS, "live", "live.k7");
  await files.writeText(WS, "notes.txt", "one\n");
  try {
    const home = await k7.open("/");
    const api = (route: string, data: object) =>
      home.request.post(`/api/files/git/${route}`, { data });
    await api("identity", { name: "E2E Tester", email: EMAIL });
    expect(
      (
        await api("connect", { workspace: WS, url: pathToFileURL(bare).href })
      ).ok(),
    ).toBe(true);
    expect(
      (await api("commit", { workspace: WS, message: "first" })).ok(),
    ).toBe(true);
    expect((await api("push", { workspace: WS })).ok()).toBe(true);
    const saves = Number(git(bare, "rev-list", "--count", "main").trim());

    const page = await k7.open(`/@local/${WS}/${project}`);
    await test.step("an edit reaches the technical branch without a Save", async () => {
      await files.writeText(WS, "notes.txt", "one\ntwo\n");
      await expect
        .poll(() => shown(bare, `${BRANCH}:notes.txt`), SLOW)
        .toBe("one\ntwo\n");
      expect(Number(git(bare, "rev-list", "--count", "main").trim())).toBe(
        saves,
      );
    });

    await test.step("Save, in the Export panel the rail opens first, says when the last auto-save was", async () => {
      await (await panel(page, "Export")).click();
      await expect(page.getByTestId("git-autosaved")).toHaveText(
        /^Auto-saved at \d{1,2}:\d\d/,
        SLOW,
      );
    });

    await test.step("Save is one commit on main, and the branch restarts from it", async () => {
      await page.getByRole("button", { name: /^Save \d+ changes?$/ }).click();
      await expect(page.getByText("Saved", { exact: true })).toBeVisible(SLOW);
      await expect
        .poll(
          () => Number(git(bare, "rev-list", "--count", "main").trim()),
          SLOW,
        )
        .toBe(saves + 1);
      expect(shown(bare, "main:notes.txt")).toBe("one\ntwo\n");
      await expect.poll(() => tip(bare, BRANCH), SLOW).toBe(tip(bare, "main"));
    });
  } finally {
    await files.removeWorkspace(WS).catch(() => undefined);
    rmSync(dir, { recursive: true, force: true });
  }
});
