// edit.replay.vfs.test.ts — the same plan over the Worker's Vfs: the imports reach the shims, so the browser's path is the one proven.
// A reader would get wrong that nothing touches the disk here: the plan is read through the real fs, the edits go through the shims.

import { beforeAll, describe, expect, it } from "vitest";
import { setVfs } from "../../commands/src/runtime/shims/current.ts";
import { Vfs } from "../../commands/src/runtime/vfs.ts";
import { editReady } from "./edit.ts";
import {
  type Backend,
  loadPlan,
  native,
  planDir,
  replay,
} from "./replayPlan.ts";

beforeAll(async () => {
  expect(await editReady()).toBe(true);
});

let vfs: Vfs;
const backend: Backend = {
  map: (s) => s,
  prepare(files, dirs) {
    vfs = new Vfs(true);
    setVfs(vfs);
    for (const d of dirs) vfs.mkdir(d, true);
    for (const [p, text] of Object.entries(files)) {
      vfs.mkdir(p.slice(0, p.lastIndexOf("/")) || "/", true);
      vfs.load(p, { text, len: text.length });
    }
  },
  files() {
    const out: Record<string, string> = {};
    for (const [p, rec] of vfs.files)
      if (rec.text !== undefined) out[p] = rec.text;
    return out;
  },
};

describe.skipIf(!planDir())(
  "the native replays, through edit.call over the Vfs",
  () => {
    it("answers and changes what the native crate did, number for number", () => {
      const plan = loadPlan();
      const t = replay(plan, backend);
      expect(t.mismatches.slice(0, 20)).toEqual([]);
      const n = native(plan);
      expect({
        execs: t.execs,
        ret: t.ret,
        thrown: t.thrown,
        unsupported: t.unsupported,
      }).toEqual(n);
    }, 600_000);
  },
);
