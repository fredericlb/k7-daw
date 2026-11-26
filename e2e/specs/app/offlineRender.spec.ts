// offlineRender.spec.ts — an offline render (a freeze, a mixdown export) is made by the engine, on a phone too.
// The one thing a reader would get wrong: on a phone no program starts, so the supervisor asks the running engine (apps/k7-daw-backend/packages/k7-supervisor/docs/supervisor.md).
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { expect, type K7, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";

const PROJECT = "clip-lane.k7";

test.describe.configure({ mode: "serial", timeout: 240_000 });

/** The files service bound to the phone's own engine, as the page binds it once an engine is picked. */
async function bound(k7: K7) {
  // The page picks the engine on load, which claims it for the account: only a claimed engine is bound to its files.
  const page = await k7.open(`/@local/${FIXTURES_WORKSPACE}/${PROJECT}`);
  await engineUp(page, 60_000);
  const files = await k7.files();
  // The page's claim lands a moment after its engine is up, and a render named before it binds nothing.
  let id = "";
  await expect
    .poll(
      async () => {
        const res = await files.request.get("/api/engines");
        const [engine] = (await res.json()) as {
          id: string;
          assigned?: boolean;
        }[];
        id = engine?.id ?? "";
        return engine?.assigned === true;
      },
      { timeout: 30_000 },
    )
    .toBe(true);
  files.bindEngine(id);
  return files;
}

/** A line run until the engine's machine is on the jobs path, which comes up a moment after the app does. */
async function whenMachine(
  files: Awaited<ReturnType<typeof bound>>,
  line: string,
) {
  let body = await files.command(FIXTURES_WORKSPACE, line);
  for (
    let i = 0;
    i < 20 && JSON.stringify(body).includes("no engine machine");
    i++
  ) {
    await new Promise((r) => setTimeout(r, 2000));
    body = await files.command(FIXTURES_WORKSPACE, line);
  }
  return body;
}

/** Every file path of a workspace tree, flattened. */
function paths(nodes: { path: string; children?: unknown[] }[]): string[] {
  return nodes.flatMap((n) => [
    n.path,
    ...paths((n.children ?? []) as typeof nodes),
  ]);
}

test("a mixdown export renders the whole project to a wav", async ({ k7 }) => {
  const files = await bound(k7);
  await files.open(FIXTURES_WORKSPACE, PROJECT);
  const { status, bytes } = await files.exportMix(FIXTURES_WORKSPACE, "wav");
  expect(status, bytes.toString("utf8", 0, 200)).toBe(200);
  expect(bytes.toString("latin1", 0, 4)).toBe("RIFF");
  expect(bytes.length).toBeGreaterThan(10_000);
});

test("a tracks export zips each track's audio, notes and page", async ({
  k7,
}) => {
  const files = await bound(k7);
  await files.open(FIXTURES_WORKSPACE, PROJECT);
  const { status, bytes } = await files.exportTracks(FIXTURES_WORKSPACE);
  expect(status, bytes.toString("utf8", 0, 200)).toBe(200);
  expect(bytes.toString("latin1", 0, 2)).toBe("PK");
  // Entry names sit in clear in the local headers, and the audio is stored.
  const text = bytes.toString("latin1");
  expect(text).toContain("README.md");
  expect(text).toMatch(/01-[^\0]*\.md/);
  expect(text).toMatch(/01-[^\0]*\.wav/);
});

test("a node freeze renders its lane into renders/", async ({ k7 }) => {
  const files = await bound(k7);
  await files.open(FIXTURES_WORKSPACE, PROJECT);
  const body = await whenMachine(files, "node freeze drums");
  const said = JSON.stringify(body);
  expect(body.refused, said).toBeFalsy();
  expect(said).not.toMatch(/cannot start a program|not served here/);
  expect(said).toMatch(/drums: frozen to renders\/drums\.wav/);
  await expect
    .poll(async () =>
      paths(await files.tree(FIXTURES_WORKSPACE)).some((p) =>
        p.endsWith("renders/drums.wav"),
      ),
    )
    .toBe(true);
  // A second freeze takes the lane back, so the project is whole for the next spec.
  const thawed = await files.command(FIXTURES_WORKSPACE, "node freeze drums");
  expect(JSON.stringify(thawed)).toMatch(/drums: unfrozen/);
});
