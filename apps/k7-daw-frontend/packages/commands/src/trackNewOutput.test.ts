// `track new`'s output names each file it wrote once, with `/` on every platform: the chain file written for the
// lane is also the one its strip goes into. See docs/verbs.md.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { scaffoldLive } from "@studio/song/scaffoldLive";
import { afterEach, beforeEach, expect, it } from "vitest";
import { runCommand } from "./commandSpec";
import { testHost } from "./testHost";

let root: string;
let dir: string;
beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "track-new-"));
  dir = join(root, "live.k7");
  scaffoldLive(dir, "live");
});
afterEach(() => rmSync(root, { recursive: true, force: true }));

it("names the lane's file and its chain once each, spelled with /", async () => {
  const h = testHost({ songPath: dir });
  await runCommand("track new pad", h, "cli");
  expect(h.lines.refuse).toEqual([]);
  expect(h.lines.log[0]).toBe("+ track pad (tracks/pad.k7x, chains/pad.k7x)");
});
