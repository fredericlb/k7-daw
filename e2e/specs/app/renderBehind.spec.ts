// renderBehind.spec.ts — an offline render started in front goes on when the app is sent behind (ios.md, Busy.swift).
// The one thing a reader would get wrong: it needs the simulator's Settings app to cover k7, so it runs on the ios target alone.
import { execFileSync } from "node:child_process";
import { FIXTURES_WORKSPACE } from "../../src/fixtures.ts";
import { IOS_BUNDLE, simctl, simulator } from "../../src/ios.ts";
import { expect, TARGET, test } from "../../src/target.ts";
import { engineUp } from "../../src/transport.ts";

const PROJECT = "clip-lane.k7";

test("a mixdown export started in front finishes with the app behind", async ({
  k7,
}) => {
  test.skip(
    TARGET !== "ios",
    "the simulator's Settings is what sends k7 behind",
  );
  test.setTimeout(240_000);
  const page = await k7.open(`/@local/${FIXTURES_WORKSPACE}/${PROJECT}`);
  await engineUp(page, 60_000);
  const files = await k7.files();
  const [engine] = (await (await files.request.get("/api/engines")).json()) as {
    id: string;
  }[];
  files.bindEngine(engine?.id ?? "");
  await files.open(FIXTURES_WORKSPACE, PROJECT);
  const lines = async () => await files.logs(FIXTURES_WORKSPACE);
  const started = files.exportMix(FIXTURES_WORKSPACE, "wav");
  execFileSync("xcrun", [
    "simctl",
    "launch",
    simulator(),
    "com.apple.Preferences",
  ]);
  try {
    const results = [await started];
    // Several in a row, so the app is behind for most of the work.
    for (let i = 0; i < 5; i++)
      results.push(await files.exportMix(FIXTURES_WORKSPACE, "wav"));
    for (const r of results) {
      expect(r.status, r.bytes.toString("utf8", 0, 200)).toBe(200);
      expect(r.bytes.toString("latin1", 0, 4)).toBe("RIFF");
    }
    // Between "a render began" and "a render ended" the device never closes; once idle behind another it does.
    let rendering = false;
    let closedWhileRendering = 0;
    let closedIdle = 0;
    // Only this launch's lines: the log keeps an earlier run's.
    const all = await lines();
    const from = all
      .map((l) => l.includes("the app started the engine"))
      .lastIndexOf(true);
    for (const line of all.slice(Math.max(0, from))) {
      if (line.includes("busy: a render began")) rendering = true;
      else if (line.includes("busy: a render ended")) rendering = false;
      else if (line.includes("audio device closed behind another app")) {
        if (rendering) closedWhileRendering++;
        else closedIdle++;
      }
    }
    expect(closedWhileRendering).toBe(0);
    expect(closedIdle, "the app was never behind another").toBeGreaterThan(0);
  } finally {
    // Back in front, or the worker's next spec plays on an engine whose device closed behind Settings.
    simctl("launch", simulator(), IOS_BUNDLE);
  }
});
