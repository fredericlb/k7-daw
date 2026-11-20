// edit.replay.test.ts — the native replays' calls (5,086 vectors, the goldens they reach) re-run through the module over a temp folder.
// A reader would get wrong the paths: the plan's /tmp/k7r-* and checkout paths are moved under a fresh folder, and moved back never.

import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { K7WasmError } from "./abi.ts";
import { editCallEnv, editReady, setIdSource } from "./edit.ts";
import {
  type Backend,
  loadPlan,
  native,
  planDir,
  REPO,
  replay,
  textOf,
} from "./replayPlan.ts";

let base: string;
beforeAll(async () => {
  base = realpathSync(mkdtempSync(join(tmpdir(), "k7-replay-")));
  expect(await editReady()).toBe(true);
});
afterAll(() => rmSync(base, { recursive: true, force: true }));

function walk(dir: string, out: Record<string, string>): void {
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) walk(p, out);
    else out[p] = readFileSync(p, "utf8");
  }
}

const disk: Backend = {
  foldsNames: process.platform === "darwin",
  map: (s) =>
    s.replaceAll("/tmp/k7r-", `${base}/k7r-`).replaceAll(REPO, `${base}/repo`),
  prepare(files, dirs) {
    for (const n of readdirSync(base))
      rmSync(join(base, n), { recursive: true, force: true });
    for (const d of dirs) mkdirSync(d, { recursive: true });
    for (const [p, text] of Object.entries(files)) {
      mkdirSync(dirname(p), { recursive: true });
      writeFileSync(p, text);
    }
  },
  files() {
    const out: Record<string, string> = {};
    walk(base, out);
    return out;
  },
};

describe.skipIf(!planDir())(
  "the native replays, through edit.call over a temp folder",
  () => {
    it("answers and changes what the native crate did, number for number", () => {
      const plan = loadPlan();
      const t = replay(plan, disk);
      expect(t.mismatches.slice(0, 20)).toEqual([]);
      const n = native(plan);
      expect({
        execs: t.execs,
        ret: t.ret,
        thrown: t.thrown,
        unsupported: t.unsupported,
      }).toEqual(n);
      expect(t.execs).toBeGreaterThan(4000);
    }, 600_000);

    it("answers the next call after a trap, on a new instance", () => {
      const plan = loadPlan();
      const x = plan.execs.find((e) => e.ids.length > 0 && "ret" in e.out);
      expect(x).toBeDefined();
      if (!x) return;
      const run = () => {
        const m = JSON.parse(disk.map(JSON.stringify(x))) as typeof x;
        const files: Record<string, string> = {};
        for (const [p, h] of Object.entries(m.files))
          files[p] = disk.map(textOf(plan, h));
        disk.prepare(files, m.dirs);
        return m;
      };
      let m = run();
      setIdSource(() => {
        throw new WebAssembly.RuntimeError("unreachable");
      });
      try {
        editCallEnv(m.env, m.name, m.args);
        expect.unreachable("the trap should have failed the call");
      } catch (e) {
        expect(e).toBeInstanceOf(K7WasmError);
        expect((e as K7WasmError).code).toBe("panic");
      } finally {
        setIdSource(undefined);
      }
      m = run();
      setIdSource(
        () => m.ids.shift() ?? "00000000-0000-4000-8000-000000000001",
      );
      expect(editCallEnv(m.env, m.name, m.args)).toEqual(m.out.ret);
      setIdSource(undefined);
    });
  },
);
