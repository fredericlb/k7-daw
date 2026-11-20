import { mkdirSync, mkdtempSync, rmSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { cachedForModules, shedModuleReads } from "./moduleCache";

let root: string;
afterEach(() => rmSync(root, { recursive: true, force: true }));

/** A project folder whose module folder is old enough to key on. */
function project(name: string): string {
  const dir = join(root, name);
  mkdirSync(join(dir, "nodes"), { recursive: true });
  const old = new Date(Date.now() - 60_000);
  for (const p of [join(dir, "nodes"), dir]) utimesSync(p, old, old);
  return dir;
}

it("sheds every project's module reads but the one read last", () => {
  root = mkdtempSync(join(tmpdir(), "module-cache-"));
  const [a, b] = [project("a.k7"), project("b.k7")];
  let builds = 0;
  const read = (dir: string) => cachedForModules(dir, "vocab", () => ++builds);
  read(a);
  read(b);
  expect([read(a), read(b), builds]).toEqual([1, 2, 2]);
  shedModuleReads();
  // b was read last and stays; a is built again.
  expect([read(b), read(a), builds]).toEqual([2, 3, 3]);
});
